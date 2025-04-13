use std::{
    time::{Duration, SystemTime, UNIX_EPOCH},
    sync::{Arc, Mutex},
};
use eframe::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use tokio::{
    net::TcpStream,
    time,
    io::{AsyncWriteExt, AsyncReadExt}
};

const SERVER_PORT: u16 = 9000;
const REQUEST_COMMAND: &[u8] = b"rffff0";
const MAX_DATA_POINTS: usize = 20;
const FETCH_TIMEOUT: Duration = Duration::from_secs(2);

struct ServerInfo {
    ip: &'static str,
    name: &'static str,
}

const SERVERS: [ServerInfo; 3] = [
    ServerInfo { ip: "127.0.0.27", name: "NOZ" },
    ServerInfo { ip: "127.0.0.28", name: "CON" },
    ServerInfo { ip: "127.0.0.29", name: "203" },
];

#[derive(Clone, Default)]
struct ComputationResults {
    timestamp: u64,
    metrics: [f64; 3],
}

#[derive(Default)]
struct ServerData {
    computed_results: Vec<ComputationResults>
}

struct MonitoringApp {
    shared_data: Arc<Mutex<ServerData>>,
}

#[tokio::main]
async fn main() -> eframe::Result {
    let shared_data = Arc::new(Mutex::new(ServerData::default()));
    
    let data_clone = shared_data.clone();
    tokio::spawn(async move {
        data_collection_task(data_clone).await
    });

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Server Monitoring System",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(MonitoringApp { shared_data }))
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

        let mut metrics = [0.0; 3];
        let futures = SERVERS.iter().enumerate().map(|(i, server)| async move {
            let result = fetch_data_async(server.ip, SERVER_PORT).await;
            (i, server.name, result)
        });

        let results = futures::future::join_all(futures).await;
        
        for (idx, name, result) in results {
            match result {
                Ok(value) => {
                    metrics[idx] = value.parse().unwrap_or_else(|_| {
                        eprintln!("[{}] Invalid numeric value: {}", name, value);
                        0.0
                    });
                }
                Err(e) => {
                    eprintln!("[{}] Connection error: {}", name, e);
                    metrics[idx] = 0.0;
                }
            }
        }

        let result = ComputationResults {
            timestamp,
            metrics,
        };

        let mut data = shared_data.lock().unwrap();
        data.computed_results.push(result);
    }
}

impl eframe::App for MonitoringApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_secs(1));

        let plot_data = {
            let data = self.shared_data.lock().unwrap();
            data.computed_results
                .iter()
                .rev()
                .take(MAX_DATA_POINTS)
                .cloned()
                .collect::<Vec<_>>()
        };

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Ok(icon) = egui::include_image!("../assets/icon.jpg") {
                    ui.image(icon, egui::Vec2::splat(64.0));
                }
                ui.vertical(|ui| {
                    ui.heading("Real-time Server Monitoring");
                    egui::widgets::global_theme_preference_buttons(ui);
                    if ui.button("Save to Excel and quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });

            ui.separator();

            Plot::new("metrics_plot")
                .legend(Legend::default().position(egui_plot::Corner::RightTop))
                .allow_zoom(false)
                .show(ui, |plot_ui| {
                    let names = ["G2, kg/s", "G3, kg/s", "G4, kg/s"];
                    
                    for (idx, name) in names.iter().enumerate() {
                        let points: PlotPoints = plot_data
                            .iter()
                            .map(|r| [r.timestamp as f64, r.metrics[idx]])
                            .collect();
                        
                        plot_ui.line(Line::new(points).name(name));
                    }
                });
        });
    }
}

async fn fetch_data_async(ip: &str, port: u16) -> Result<String, std::io::Error> {
    let mut stream = time::timeout(FETCH_TIMEOUT, TcpStream::connect((ip, port)))
        .await??;
    
    stream.write_all(REQUEST_COMMAND).await?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;
    
    Ok(String::from_utf8_lossy(&response).into_owned())
}
