use std::{
    time::{Duration, SystemTime, UNIX_EPOCH},
    sync::{Arc, Mutex},
};
use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use tokio::{
    net::TcpStream,
    time,
    io::{AsyncWriteExt,AsyncReadExt},
    time::Instant
};

// Основное состояние приложения
struct State {
    shared_data: Arc<Mutex<ServerData>>,
}

// Структура для хранения данных
#[derive(Default)]
struct ServerData {
    computed_results: Vec<ComputationResults>,
    servers:          Vec<ServerInfo>,
}

// Структура для хранения результатов вычислений
#[derive(Clone, Default)]
struct ComputationResults {
    timestamp: u64,
    flow: Vec<f64>
}

#[derive(Clone)]
struct ServerInfo {
    name:    String,
    address: String,
    online:  bool,
}


#[tokio::main]
async fn main() -> eframe::Result {
    // Инициализация стандартных серверов
    let servers = vec![
        ServerInfo {
            name: "m1".to_string(),
            address: "127.0.0.27:9000".to_string(),
            online: false,
        },
        ServerInfo {
            name: "m2".to_string(),
            address: "127.0.0.28:9000".to_string(),
            online: false,
        },
        ServerInfo {
            name: "m3".to_string(),
            address: "127.0.0.29:9000".to_string(),
            online: false,
        },
    ];

    // Общие данные для потоков
    let shared_data = Arc::new(Mutex::new(ServerData {
        computed_results: Vec::new(),
        servers,
    }));
    
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

        // Получаем копию списка серверов
        let servers = {
            let data = shared_data.lock().unwrap();
            data.servers.clone()
        };

        // Параллельное получение данных со всех серверов 
        let responses = futures::future::join_all(
            servers.iter().map(|server| fetch_data_async(&server.address))
        ).await;

        // Обновляем статусы
        {
            let mut data = shared_data.lock().unwrap();
            for (i, resp) in responses.iter().enumerate() {
                data.servers[i].online = resp.is_ok();
            }
        }

        // Собираем данные
        let flow: Vec<f64> = responses
            .into_iter()
            .map(|resp| resp
                .map(|s| s.parse().unwrap_or(0.0))
                .unwrap_or(0.0)
            )
            .collect();

        let result = ComputationResults {
            timestamp,
            flow,
        };

        let mut data = shared_data.lock().unwrap();
        data.computed_results.push(result);

    }
}

// Графический интерфейс
impl eframe::App for State {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_secs(1)); // Обновление каждую секунду
        
        egui::SidePanel::right("right_panel")
            .resizable(false)
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Настройки");
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.heading("Серверы");
                        let mut data = self.shared_data.lock().unwrap();
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (index,server) in data.servers.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label("Имя:");
                                    ui.text_edit_singleline(&mut server.name);
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Адрес:");
                                    ui.text_edit_singleline(&mut server.address);
                                });

                                let status = if server.online {
                                    "✅ Online"
                                } else {
                                    "❌ Offline"
                                };

                                ui.horizontal(|ui|{
                                    ui.label(status);
                                });
                                ui.add_space(10.0);
                            }
                        });

                        if ui.button("+").clicked() {
                            let len = data.servers.len() + 1;
                            data.servers.push(
                                ServerInfo {
                                    name: format!("m{}", len),
                                    address: "127.0.0.1:9000".to_string(),
                                    online: false,
                                },
                            )
                        }
                    });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let icon = egui::include_image!("../assets/logo_big.svg");
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
                .set_margin_fraction(egui::Vec2::new(0.0, 0.0))
                .x_axis_label("time")
                .y_axis_label("signal")
                .show(ui, |plot_ui| {
                    let computed_results = &data.computed_results;
                    let start_index = computed_results.len().saturating_sub(20);
                    let last_20 = &computed_results[start_index..];

                    // Генерация линий для всех данных
                    for (i, server) in data.servers.iter().enumerate() {
                        let points: PlotPoints = last_20
                            .iter()
                            .map(|r| [r.timestamp as f64, r.flow.get(i).copied().unwrap_or(0.0)])
                            .collect();
                        
                        plot_ui.line(Line::new(points).name(&server.name));
                    }
                });
        }); 
    }
}

async fn fetch_data_async(address: &str) -> Result<String, std::io::Error> {
    let mut stream = TcpStream::connect(address).await?;
    stream.write_all(b"rffff0").await?;

    let mut response = Vec::with_capacity(128);
    let mut buf = [0u8; 1024];
    
    let start = Instant::now();
    loop {
        let read = stream.read(&mut buf).await?;
        if read == 0 { break; }
        response.extend_from_slice(&buf[..read]);
        
        if start.elapsed() > Duration::from_secs(3) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut, 
                "Response timeout"
            ));
        }
    }
    
    Ok(String::from_utf8_lossy(&response).into())
}
