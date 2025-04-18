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
};

// Основное состояние приложения
struct State {
    shared_data:    Arc<Mutex<ServerData>>,
    points_to_show: usize,
    is_collecting:  Arc<Mutex<bool>>,
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
    let servers       = create_default_servers();
    let shared_data   = Arc::new(Mutex::new(ServerData::new(servers)));
    let is_collecting = Arc::new(Mutex::new(false));
    
    start_data_collection_task(shared_data.clone(), is_collecting.clone());
    run_gui(shared_data, is_collecting).await
}

// Инициализация ===========================================================

fn create_default_servers() -> Vec<ServerInfo> {
    vec![
        ServerInfo::new("m1", "127.0.0.27:9000"),
        ServerInfo::new("m2", "127.0.0.28:9000"),
        ServerInfo::new("m3", "127.0.0.29:9000"),
    ]
}

impl ServerInfo {
    fn new(name: &str, address: &str) -> Self {
        Self {
            name:    name.to_string(),
            address: address.to_string(),
            online:  false,
        }
    }
}

impl ServerData {
    fn new(servers: Vec<ServerInfo>) -> Self {
        Self {
            computed_results: Vec::new(),
            servers,
        }
    }
}

// Логика сбора данных =====================================================

fn start_data_collection_task(
    shared_data:   Arc<Mutex<ServerData>>,
    is_collecting: Arc<Mutex<bool>>,
) {
    tokio::spawn(async move {
        data_collection_loop(shared_data, is_collecting).await
    });
}

async fn data_collection_loop(
    shared_data:   Arc<Mutex<ServerData>>,
    is_collecting: Arc<Mutex<bool>>,
) {
    let mut interval = time::interval(Duration::from_secs(1));
    
    loop {
        interval.tick().await;
        if !should_collect(&is_collecting) { continue }

        let timestamp = current_timestamp();
        let responses = fetch_all_servers(&shared_data).await;
        process_responses(shared_data.clone(), responses, timestamp).await;
    }
}

fn should_collect(is_collecting: &Arc<Mutex<bool>>) -> bool {
    *is_collecting.lock().unwrap()
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

async fn fetch_all_servers(shared_data: &Arc<Mutex<ServerData>>) -> Vec<Result<String, std::io::Error>> {
    let servers = {
        let data = shared_data.lock().unwrap();
        data.servers.clone()
    };
    
    futures::future::join_all(
        servers.iter().map(|server| fetch_data_async(&server.address))
    ).await
}

async fn process_responses(
    shared_data: Arc<Mutex<ServerData>>,
    responses:   Vec<Result<String, std::io::Error>>,
    timestamp:   u64,
) {
    let flow = parse_responses(&responses);
    update_server_statuses(&shared_data, &responses);
    save_computation_result(shared_data, ComputationResults { timestamp, flow });
}

fn parse_responses(responses: &[Result<String, std::io::Error>]) -> Vec<f64> {
    responses
        .iter()
        .map(|resp| resp
            .as_ref()
            .map(|s| s.parse().unwrap_or(0.0))
            .unwrap_or(0.0)
        )
        .collect()
}

fn update_server_statuses(shared_data: &Arc<Mutex<ServerData>>, responses: &[Result<String, std::io::Error>]) {
    let mut data = shared_data.lock().unwrap();
    for (server, resp) in data.servers.iter_mut().zip(responses.iter()) {
        server.online = resp.is_ok();
    }
}

fn save_computation_result(shared_data: Arc<Mutex<ServerData>>, result: ComputationResults) {
    let mut data = shared_data.lock().unwrap();
    data.computed_results.push(result);
}

// GUI ======================================================================

async fn run_gui(
    shared_data:   Arc<Mutex<ServerData>>,
    is_collecting: Arc<Mutex<bool>>
) -> eframe::Result {
    eframe::run_native(
        "Server Monitoring System",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(State {
                shared_data,
                points_to_show: 20,
                is_collecting,
            }))
        }),
    )
}

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
                ui.separator();
                ui.heading("Настройки графика");
                ui.horizontal(|ui| {
                    ui.label("Точек на графике:");
                    ui.add(egui::DragValue::new(&mut self.points_to_show).range(2..=500));
                });

                // Начало/остановка запросов
                ui.separator();
                ui.heading("Управление сбором");
                let button_text = {
                    let is_collecting = self.is_collecting.lock().unwrap();
                    if *is_collecting { "⏹ Остановить сбор" } else { "▶ Начать сбор" }
                };
                if ui.button(button_text).clicked() {
                    let new_state = {
                        let mut is_collecting = self.is_collecting.lock().unwrap();
                        *is_collecting = !*is_collecting;
                        *is_collecting
                    };

                    if !new_state {
                        let mut data = self.shared_data.lock().unwrap();
                        data.computed_results.clear();
                    }
                }

                // Список опрашиваемых серверов
                ui.separator();
                ui.vertical(|ui| {
                    let is_collecting = {
                        let is_collecting = self.is_collecting.lock().unwrap();
                        *is_collecting
                    };

                    let mut data = self.shared_data.lock().unwrap();
                    let mut to_remove = Vec::new();

                    ui.horizontal(|ui| {
                        ui.heading("Серверы");
                        if !is_collecting && ui.button("+ добавить").clicked() {
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

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (index,server) in data.servers.iter_mut().enumerate() {
                            ui.add_space(10.0);
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Имя:");
                                    ui.add_enabled(
                                        !is_collecting,
                                        egui::TextEdit::singleline(&mut server.address)
                                    );
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Адрес:");
                                    ui.add_enabled(
                                        !is_collecting,
                                        egui::TextEdit::singleline(&mut server.address)
                                    );
                                });

                                let status = if server.online {
                                    "✅ Online"
                                } else {
                                    "❌ Offline"
                                };

                                ui.horizontal(|ui| {
                                    ui.label(status);
                                    // Сохраняем индекс при нажатии кнопки
                                    if !is_collecting && ui.button("-").clicked() {
                                        to_remove.push(index);
                                    }
                                });
                            });
                        }

                        for &index in to_remove.iter().rev() {
                            data.servers.remove(index);
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
                    let start_index = computed_results.len().saturating_sub(self.points_to_show);
                    let last_points = &computed_results[start_index..];

                    // Генерация линий для всех данных
                    for (i, server) in data.servers.iter().enumerate() {
                        let points: PlotPoints = last_points
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

    let mut response = Vec::new();
    match tokio::time::timeout(Duration::from_secs(3), stream.read_to_end(&mut response)).await {
        Ok(Ok(_bytes_read)) => Ok(String::from_utf8_lossy(&response).into_owned()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut, 
            "Response timeout"
        )),
    }
}
