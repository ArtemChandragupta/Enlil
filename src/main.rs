use std::{
    time::{Duration, SystemTime, UNIX_EPOCH},
    sync::{Arc, Mutex},
};
use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use tokio::{
    net::TcpStream,
    time,
    io::AsyncWriteExt
};
extern crate umya_spreadsheet;

const IP_1: &str = "127.0.0.27";
const IP_2: &str = "127.0.0.28";
const IP_3: &str = "127.0.0.29";
const SERVER_PORT: u16 = 9000;

// Структура для хранения результатов вычислений
#[derive(Clone, Default)]
struct ComputationResults {
    timestamp: u64,
    m1:        f64,
    m2:        f64,
    m3:        f64,
}

// Структура для хранения данных
#[derive(Default)]
struct ServerData {
    computed_results: Vec<ComputationResults>
}

// Основное приложение
struct State {
    shared_data: Arc<Mutex<ServerData>>,
}

#[tokio::main]
async fn main() -> eframe::Result {
    // Общие данные для потоков
    let shared_data = Arc::new(Mutex::new(ServerData::default()));
    
    // Запускаем поток сбора данных
    let data_clone = shared_data.clone();
    tokio::spawn(async move {
        data_collection_task(data_clone).await
    });

    // Запускаем GUI
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Server Monitoring System",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(State { shared_data }))
        }),
    )
}

async fn data_collection_task(shared_data: Arc<Mutex<ServerData>>) {
    let mut interval = time::interval(Duration::from_secs(1));
    
    loop {
        interval.tick().await;
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        // Параллельное получение данных со всех серверов
        let (resp_1, resp_2, resp_3) = tokio::join!(
            fetch_data_async(IP_1, SERVER_PORT),
            fetch_data_async(IP_2, SERVER_PORT),
            fetch_data_async(IP_3, SERVER_PORT),
        );

        // Обработка ошибок
        let m1 = resp_1.unwrap_or_else(|err| {
            println!("NOZ error: {err}");
            "err".to_string()
        });
        
        let m2 = resp_2.unwrap_or_else(|err| {
            println!("CON error: {err}");
            "err".to_string()
        });
        
        let m3 = resp_3.unwrap_or_else(|err| {
            println!("203 error: {err}");
            "err".to_string()
        });

        let m1 = m1.parse::<f64>().unwrap_or(0.0);
        let m2 = m2.parse::<f64>().unwrap_or(0.0);
        let m3 = m3.parse::<f64>().unwrap_or(0.0);

        let result = ComputationResults {
            timestamp, m1, m2, m3
        };

        let mut data = shared_data.lock().unwrap();
         data.computed_results.push(result.clone());

    }
}

// Графический интерфейс
impl eframe::App for State {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_secs(1)); // Обновление каждую секунду

        egui::CentralPanel::default().show(ctx, |ui| {

            ui.horizontal(|ui| {
                let icon = egui::include_image!("../assets/icon.jpg");
                ui.add(egui::Image::new(icon).fit_to_exact_size(egui::Vec2::new(64.0, 64.0)));
                ui.vertical(|ui| {
                    ui.heading("Real-time Server Monitoring");
                    egui::widgets::global_theme_preference_buttons(ui);
                    if ui.button("Save to excell and quit").clicked() {
                        // let data = self.shared_data.lock().unwrap();
                        // save_to_excel(&data.computed_results);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });

            ui.separator();

            let data = self.shared_data.lock().unwrap();

            Plot::new("combined_plot")
                .legend(Legend::default().position(egui_plot::Corner::RightTop))
                .allow_zoom(false).allow_scroll(false).allow_drag(false)
                .show(ui, |plot_ui| {
                    let computed_results = &data.computed_results;
                    let start_index = computed_results.len().saturating_sub(20);
                    let last_20 = &computed_results[start_index..];

                    let points_m1: PlotPoints = last_20
                        .iter()
                        .map(|r| [r.timestamp as f64, r.m1])
                        .collect();
                    plot_ui.line(Line::new(points_m1).name("G2, kg/s"));

                    let points_m2: PlotPoints = last_20
                        .iter()
                        .map(|r| [r.timestamp as f64, r.m2])
                        .collect();
                    plot_ui.line(Line::new(points_m2).name("G3, kg/s"));

                    let points_m3: PlotPoints = last_20
                        .iter()
                        .map(|r| [r.timestamp as f64, r.m3])
                        .collect();
                    plot_ui.line(Line::new(points_m3).name("G4, kg/s"));
                });
        });
    }
}

async fn fetch_data_async(ip: &str, port: u16) -> Result<String, std::io::Error> {
    let mut stream = TcpStream::connect((ip, port)).await?;
    stream.write_all(b"rffff0").await?;

    let mut response = Vec::new();
    let mut buf = [0u8; 1024];
    
    loop {
        time::timeout(Duration::from_secs(1), stream.readable())
            .await
            .map_err(|_| std::io::Error::new(
                std::io::ErrorKind::TimedOut, 
                "Read timeout"
            ))??;

        match stream.try_read(&mut buf) {
            Ok(0) => break, // Конец потока
            Ok(n) => response.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(e),
        }
    }

    Ok(String::from_utf8_lossy(&response).to_string())
}
