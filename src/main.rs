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
    start_time:       Option<u64>,
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
            start_time: None,
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

        let responses = fetch_all_servers(&shared_data).await;
        update_server_statuses(&shared_data, &responses);

        if *is_collecting.lock().unwrap() {
            let timestamp = current_timestamp();
            let flow = parse_responses(&responses);
            save_computation_result(shared_data.clone(), ComputationResults { timestamp, flow });
        }
    }
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

    // Устанавливаем время начала при первом сохранении
    if data.start_time.is_none() {
        data.start_time = Some(result.timestamp);
    }
    
    // Вычисляем относительное время
    let relative_timestamp = result.timestamp - data.start_time.unwrap();
    let new_result = ComputationResults {
        timestamp: relative_timestamp,
        flow: result.flow,
    };

    data.computed_results.push(new_result);
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
        ctx.request_repaint_after(Duration::from_secs(1));
        
        egui::SidePanel::right("right_panel")
            .resizable(false)
            .default_width(200.0)
            .show(ctx, |ui| {
                render_side_panel(ui, self);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            render_main_content(ui, self);
        });
    }
}

// Боковая панель
fn render_side_panel(ui: &mut egui::Ui, state: &mut State) {
    ui.vertical_centered(|ui| ui.heading("Настройки"));
    ui.separator();
    
    render_plot_settings(ui, state);
    render_collection_control(ui, state);
    render_server_list(ui, state);
}

fn render_plot_settings(ui: &mut egui::Ui, state: &mut State) {
    ui.heading("Настройки графика");
    ui.horizontal(|ui| {
        ui.label("Точек на графике:");
        ui.add(egui::DragValue::new(&mut state.points_to_show).range(2..=500));
    });
}

fn render_collection_control(ui: &mut egui::Ui, state: &mut State) {
    ui.separator();
    ui.heading("Управление сбором");
    
    let is_collecting = *state.is_collecting.lock().unwrap();
    let button_text = if is_collecting { "⏹ Остановить сбор" } else { "▶ Начать сбор" };
    
    if ui.button(button_text).clicked() {
        toggle_collection_state(state, is_collecting);
    }
}

fn toggle_collection_state(state: &mut State, current_state: bool) {
    let mut is_collecting = state.is_collecting.lock().unwrap();
    *is_collecting = !current_state;

    let mut data = state.shared_data.lock().unwrap();
    if !*is_collecting {
        // Очищаем данные при остановке
        data.computed_results.clear();
        data.start_time = None;
    } else {
        // Сбрасываем время начала при новом сборе
        data.start_time = None;
    }
}

fn render_server_list(ui: &mut egui::Ui, state: &mut State) {
    ui.separator();
    ui.vertical(|ui| {
        let is_collecting = *state.is_collecting.lock().unwrap();
        let mut data = state.shared_data.lock().unwrap();
        let mut to_remove = Vec::new();

        render_server_list_header(ui, &mut data, is_collecting);
        render_servers(ui, &mut data, is_collecting, &mut to_remove);
        remove_selected_servers(&mut data, to_remove);
    });
}

fn render_server_list_header(ui: &mut egui::Ui, data: &mut ServerData, is_collecting: bool) {
    ui.horizontal(|ui| {
        ui.heading("Серверы");
        if !is_collecting && ui.button("+ добавить").clicked() {
            add_new_server(data);
        }
    });
}

fn add_new_server(data: &mut ServerData) {
    let len = data.servers.len() + 1;
    data.servers.push(ServerInfo {
        name: format!("m{}", len),
        address: "127.0.0.1:9000".to_string(),
        online: false,
    });
}

fn render_servers(
    ui: &mut egui::Ui,
    data: &mut ServerData,
    is_collecting: bool,
    to_remove: &mut Vec<usize>,
) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (index, server) in data.servers.iter_mut().enumerate() {
            ui.add_space(10.0);
            render_server_entry(ui, server, is_collecting, index, to_remove);
        }
    });
}

fn render_server_entry(
    ui: &mut egui::Ui,
    server: &mut ServerInfo,
    is_collecting: bool,
    index: usize,
    to_remove: &mut Vec<usize>,
) {
    ui.group(|ui| {
        render_server_fields(ui, server, is_collecting);
        render_server_status(ui, server, is_collecting, index, to_remove);
    });
}

fn render_server_fields(ui: &mut egui::Ui, server: &mut ServerInfo, is_collecting: bool) {
    ui.horizontal(|ui| {
        ui.label("Имя:");
        ui.add_enabled(!is_collecting, egui::TextEdit::singleline(&mut server.name));
    });
    ui.horizontal(|ui| {
        ui.label("Адрес:");
        ui.add_enabled(!is_collecting, egui::TextEdit::singleline(&mut server.address));
    });
}

fn render_server_status(
    ui: &mut egui::Ui,
    server: &ServerInfo,
    is_collecting: bool,
    index: usize,
    to_remove: &mut Vec<usize>,
) {
    ui.horizontal(|ui| {
        ui.label(if server.online { "✅ Online" } else { "❌ Offline" });
        if !is_collecting && ui.button("-").clicked() {
            to_remove.push(index);
        }
    });
}

fn remove_selected_servers(data: &mut ServerData, to_remove: Vec<usize>) {
    for &index in to_remove.iter().rev() {
        data.servers.remove(index);
    }
}

// Главная панель
fn render_main_content(ui: &mut egui::Ui, state: &mut State) {
    render_header(ui);
    ui.separator();
    render_plot(ui, state);
}

fn render_header(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let icon = egui::include_image!("../assets/logo_big.svg");
        ui.add(egui::Image::new(icon).fit_to_exact_size(egui::Vec2::new(64.0, 64.0)));
        ui.vertical(|ui| {
            ui.heading("Real-time Server Monitoring");
            egui::widgets::global_theme_preference_buttons(ui);
            if ui.button("Save to excel and quit").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    });
}

// График
fn render_plot(ui: &mut egui::Ui, state: &mut State) {
    let data = state.shared_data.lock().unwrap();
    let plot_lines = prepare_plot_lines(&data, state.points_to_show);

    Plot::new("combined_plot")
        .legend(Legend::default().position(egui_plot::Corner::RightTop))
        .allow_zoom(false).allow_scroll(false).allow_drag(false)
        .set_margin_fraction(egui::Vec2::new(0.0, 0.0))
        .x_axis_label("time")
        .y_axis_label("signal")
        .x_axis_formatter(|value, _| format_seconds(&value))
        .show(ui, |plot_ui| {
            for (line, server) in plot_lines.into_iter().zip(data.servers.iter()) {
                plot_ui.line(line.name(&server.name));
            }
        });
}

// Добавим функцию для форматирования секунд
fn format_seconds(mark: &egui_plot::GridMark) -> String {
    let total = mark.value as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn prepare_plot_lines(data: &ServerData, points_to_show: usize) -> Vec<Line> {
    let computed_results = &data.computed_results;
    let start_index = computed_results.len().saturating_sub(points_to_show);
    
    (0..data.servers.len()).map(|i| {
        let points: PlotPoints = computed_results[start_index..]
            .iter()
            .map(|r| [r.timestamp as f64, r.flow.get(i).copied().unwrap_or(0.0)])
            .collect();
        Line::new(points)
    }).collect()
}
