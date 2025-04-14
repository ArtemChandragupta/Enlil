use eframe::egui;
use crate::egui::Color32;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::TcpStream,
    sync::mpsc,
    time::{sleep, Duration},
};

#[derive(Default)]
struct Model {
    data: [Option<f64>; 4],
    statuses: [ConnectionStatus; 4],
    addresses: [String; 4],
}

#[derive(Clone, PartialEq)]
enum ConnectionStatus {
    Connected,
    Disconnected,
    Error,
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        Self::Disconnected
    }
}

enum Msg {
    UpdateData { server_id: usize, value: f64 },
    StatusChange { server_id: usize, status: ConnectionStatus },
}

struct App {
    model: Model,
    rx: mpsc::UnboundedReceiver<Msg>,
}

impl App {
    fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let addresses = [
            "127.0.0.27:9000".to_string(),
            "127.0.0.28:9000".to_string(),
            "127.0.0.29:9000".to_string(),
            // "127.0.0.30:9000".to_string(),
        ];

        for (server_id, addr) in addresses.iter().enumerate() {
            let tx = tx.clone();
            let addr = addr.clone();
            tokio::spawn(async move {
                client_loop(addr, server_id, tx).await;
            });
        }

        Self {
            model: Model {
                addresses,
                ..Default::default()
            },
            rx,
        }
    }

    fn update_model(&mut self, msg: Msg) {
        match msg {
            Msg::UpdateData { server_id, value } => {
                if server_id < 4 {
                    self.model.data[server_id] = Some(value);
                }
            }
            Msg::StatusChange { server_id, status } => {
                if server_id < 4 {
                    self.model.statuses[server_id] = status;
                }
            }
        }
    }
}

async fn client_loop(address: String, server_id: usize, tx: mpsc::UnboundedSender<Msg>) {
    loop {
        match TcpStream::connect(&address).await {
            Ok(stream) => {
                tx.send(Msg::StatusChange {
                    server_id,
                    status: ConnectionStatus::Connected,
                })
                .unwrap();

                let mut reader = BufReader::new(stream);
                let mut line = String::new();

                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break, // Connection closed
                        Ok(_) => {
                            if let Ok(number) = line.trim().parse::<f64>() {
                                let processed = number * 2.0;
                                tx.send(Msg::UpdateData {
                                    server_id,
                                    value: processed,
                                })
                                .unwrap();
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            Err(_) => {
                tx.send(Msg::StatusChange {
                    server_id,
                    status: ConnectionStatus::Error,
                })
                .unwrap();
            }
        }

        tx.send(Msg::StatusChange {
            server_id,
            status: ConnectionStatus::Disconnected,
        })
        .unwrap();

        sleep(Duration::from_secs(1)).await;
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(msg) = self.rx.try_recv() {
            self.update_model(msg);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("TCP Data Monitor");

            egui::Grid::new("data_grid")
                .striped(true)
                .num_columns(3)
                .show(ui, |ui| {
                    ui.strong("Server");
                    ui.strong("Status");
                    ui.strong("Value");
                    ui.end_row();

                    for i in 0..4 {
                        ui.label(&self.model.addresses[i]);
                        
                        let status = &self.model.statuses[i];
                        let (text, color) = match status {
                            ConnectionStatus::Connected => ("✓ Connected", Color32::GREEN),
                            ConnectionStatus::Disconnected => ("✖ Disconnected", Color32::GRAY),
                            ConnectionStatus::Error => ("⚠ Error", Color32::YELLOW),
                        };
                        
                        ui.colored_label(color, text);
                        
                        if let Some(value) = self.model.data[i] {
                            ui.label(format!("{:.2}", value));
                        } else {
                            ui.label("N/A");
                        }
                        ui.end_row();
                    }
                });
        });
    }
}

#[tokio::main]
async fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "TCP Data Client",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}
