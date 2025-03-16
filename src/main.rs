use std::{
    time::{Duration, SystemTime, UNIX_EPOCH},
    io::{Read, Write},
    net::TcpStream,
    sync::{Arc, Mutex},
    thread,
};
use eframe::egui;
extern crate umya_spreadsheet;

const IP_NOZ: &str = "127.0.0.27";
const IP_CON: &str = "127.0.0.28";
const IP_203: &str = "127.0.0.203";
const IP_204: &str = "127.0.0.204";
const SERVER_PORT: u16 = 9000;

// Структура для хранения результатов вычислений
#[derive(Clone, Default)]
struct ComputationResults {
    timestamp:    u64,
    mflow:        f64,
    delp1i:       f64,
    p1ci:         f64,
    t1ci:         f64,
    t2i:          f64,
    pstat:       [f64; 4],
    ppito:       [f64; 4],
    sflow:       [f64; 4],
    sflow_fract:  f64,
    sflow_uneven: f64,
}

// Структура для хранения данных
#[derive(Default)]
struct ServerData {
    computed_results: Vec<ComputationResults>
}

// Основное приложение
struct MonitoringApp {
    shared_data: Arc<Mutex<ServerData>>,
}

fn main() -> eframe::Result {
    // Общие данные для потоков
    let shared_data = Arc::new(Mutex::new(ServerData::default()));
    
    // Запускаем поток сбора данных
    let data_clone = shared_data.clone();
    thread::spawn(move || {
        data_collection_thread(data_clone);
    });

    // Запускаем GUI
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Server Monitoring System",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(MonitoringApp { shared_data: shared_data.clone() }))
        }),
    )
}

// Поток сбора данных
fn data_collection_thread(shared_data: Arc<Mutex<ServerData>>) {
    loop {
        thread::sleep(std::time::Duration::from_secs(1));

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        // Получение данных с серверов
        let resp_noz = fetch_data_from_server(IP_NOZ, SERVER_PORT)
            .unwrap_or_else(|err| {
                println!("NOZ error: {err}");
                "err".to_string()
            });
        
        let resp_con = fetch_data_from_server(IP_CON, SERVER_PORT)
            .unwrap_or_else(|err| {
                println!("CON error: {err}");
                "err".to_string()
            });
        
        let resp_203 = fetch_data_from_server(IP_203, SERVER_PORT)
            .unwrap_or_else(|err| {
                println!("203 error: {err}");
                "err".to_string()
            });
        
        let resp_204 = fetch_data_from_server(IP_204, SERVER_PORT)
            .unwrap_or_else(|err| {
                println!("204 error: {err}");
                "err".to_string()
            });

        // Обработка и сохранение данных
        if resp_noz != "err" && resp_con != "err" && resp_203 != "err" && resp_204 != "err" {
            let plist_203 = parse_response(&resp_203);
            let plist_204 = parse_response(&resp_204);
            let blist     = [1.1, 2.1, 3.1];

            // Расчеты
            let delp1i = plist_204[8] - plist_204[9];
            let p1ci   = plist_204[8] + blist[1] * 100.0;
            let t1ci   = resp_noz.parse::<f64>().unwrap_or(0.0) + 273.15;
            let t2i    = resp_con.parse::<f64>().unwrap_or(0.0) + 273.15;

            let mflow  = calc_g(t1ci, delp1i, p1ci);

            let pstat = [
                plist_204[0] + blist[1] * 100.0,
                plist_204[1] + blist[1] * 100.0,
                plist_204[2] + blist[1] * 100.0,
                plist_204[3] + blist[1] * 100.0,
            ];

            let ppito = [
                pstat[0] + plist_203[11],
                pstat[1] + plist_203[12],
                pstat[2] + plist_203[13],
                pstat[3] + plist_203[14],
            ];

            let sflow = [
                calc_gs(ppito[0], pstat[0], t2i),
                calc_gs(ppito[1], pstat[1], t2i),
                calc_gs(ppito[2], pstat[2], t2i),
                calc_gs(ppito[3], pstat[3], t2i),
            ];

            let sflow_sum    = sflow.iter().sum::<f64>();
            let sflow_ave    = sflow_sum / 4.0;
            let sflow_fract  = sflow_sum / mflow * 100.0;
            let sflow_uneven = 100.0 * (sflow[0].max(sflow[1]).max(sflow[2]).max(sflow[3]) - sflow[0].min(sflow[1]).min(sflow[2]).min(sflow[3])) / sflow_ave;

            let result = ComputationResults {
                timestamp,
                mflow,
                delp1i,
                p1ci,
                t1ci,
                t2i,
                pstat,
                ppito,
                sflow,
                sflow_fract,
                sflow_uneven,
            };

            // Обновление общих данных
            let mut data = shared_data.lock().unwrap();
            data.computed_results.push(result.clone());
        }
    }
}

// Графический интерфейс
impl eframe::App for MonitoringApp {
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
                        let data = self.shared_data.lock().unwrap();
                        save_to_excel(&data.computed_results);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });

            ui.separator();

            let data = self.shared_data.lock().unwrap();

            // Таблица с сырыми данными
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("server_data_grid")
                    .num_columns(10)
                    .spacing([20.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.heading("Time");
                        ui.heading("Flow, kg/s");
                        ui.heading("nzT, °C");
                        ui.heading("cT, °C");
                        ui.heading("G1, kg/s");
                        ui.heading("G2, kg/s");
                        ui.heading("G3, kg/s");
                        ui.heading("G4, kg/s");
                        ui.heading("Gfrac, %");
                        ui.heading("Guneven, %");
                        ui.end_row();

                        for result in data.computed_results.iter().rev().take(50) {
                            ui.label(result.timestamp.to_string());
                            ui.label(format!("{:.2}",  result.mflow));
                            ui.label(format!("{:.1}",  result.t1ci - 273.15)); // Конвертация K -> °C
                            ui.label(format!("{:.1}",  result.t2i - 273.15));
                            ui.label(format!("{:.2}",  result.sflow[0]));
                            ui.label(format!("{:.2}",  result.sflow[1]));
                            ui.label(format!("{:.2}",  result.sflow[2]));
                            ui.label(format!("{:.2}",  result.sflow[3]));
                            ui.label(format!("{:.1}", result.sflow_fract));
                            ui.label(format!("{:.1}", result.sflow_uneven));
                            ui.end_row();
                        }
                    });
            });
        });
    }
}

fn fetch_data_from_server(ip: &str, port: u16) -> Result<String, std::io::Error> {
    let mut stream = TcpStream::connect((ip, port))?;
    stream.write_all(b"rffff0")?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    Ok(String::from_utf8_lossy(&response).to_string())
}

fn calc_g(t1c: f64, delp1: f64, p1c: f64) -> f64 {
    const DC:    f64 = 0.105;
    const D:     f64 = 0.346;
    const KA:    f64 = 1.4;
    const R:     f64 = 287.1;
    const ALFAR: f64 = 0.0000167;
    const TIZM:  f64 = 288.15;

    let mut g     = 1.0;
    let m         = (DC / D).powi(2);
    let mu        = 1.76 + (2.43 - 1.76) * (150.0 + 273.15 - t1c) / 150.0;
    let kw        = (1.002 - 0.0318 * m + 0.0907 * m.powi(2)) - (0.0062 - 0.1017 * m + 0.2972 * m.powi(2)) * D / 1000.0;
    let a1        = delp1 / p1c;
    let eps       = ((1.0 - a1).powf(2.0 / KA) * (KA / (KA - 1.0)) * (1.0 - (1.0 - a1).powf((KA - 1.0) / KA)) * (1.0 - m.powi(2)) / (a1 * (1.0 - m.powi(2) * (1.0 - a1).powf(2.0 / KA)))).sqrt();
    let mut re    = 0.0361 * g * 1_000_000.0 / (D * mu);
    let mut alfac = (0.99 - 0.2262 * m.powf(2.05) + (0.000215 - 0.001125 * m.powf(0.5) + 0.00249 * m.powf(2.35)) * (1_000_000.0 / re).powf(1.15)) * kw / (1.0 - m.powi(2)).sqrt();
    let mut fc    = std::f64::consts::PI * (DC.powi(2) + 2.0 * ALFAR * DC.powi(2) * (t1c - TIZM)) / 4.0;
    let mut ga    = alfac * eps * fc * (2.0 * delp1 * p1c / (R * t1c)).sqrt();

    while (ga - g).abs() / g >= 0.0001 {
        g     = ga;
        re    = 0.0361 * g * 1_000_000.0 / (D * mu);
        alfac = (0.99 - 0.2262 * m.powf(2.05) + (0.000215 - 0.001125 * m.powf(0.5) + 0.00249 * m.powf(2.35)) * (1_000_000.0 / re).powf(1.15)) * kw / (1.0 - m.powi(2)).sqrt();
        fc    = std::f64::consts::PI * (DC.powi(2) + 2.0 * ALFAR * DC.powi(2) * (t1c - TIZM)) / 4.0;
        ga    = alfac * eps * fc * (2.0 * delp1 * p1c / (R * t1c)).sqrt();
    }
    g
}

fn calc_gs(ppito: f64, pst: f64, tcone: f64) -> f64 {
    const DS: f64 = 0.068;
    const KA: f64 = 1.4;
    const R: f64  = 287.1;

    let pmed  = (ppito - pst) * (2.0 / 3.0) + pst;
    let dens  = pst / (R * tcone * (pst / pmed).powf((KA - 1.0) / KA));
    let speed = (2.0 * (pmed - pst) / dens).sqrt();
    dens * speed * (DS / 2.0).powi(2) * std::f64::consts::PI
}

fn parse_response(resp: &str) -> Vec<f64> {
    resp.split_whitespace()
        .rev()
        .map(|x| x.parse().unwrap_or(0.0) * 6894.75672)
        .collect()
}

fn save_to_excel(results: &[ComputationResults]) {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.get_sheet_by_name_mut("Sheet1").unwrap();
    
    let headers = [
        "Time", "Flow, kg/s", "DeltaP, Pa", "P,Pa", 
        "t1ci", "t2i", "pstat1", "ppito1", "pstat2", 
        "ppito2", "pstat3", "ppito3", "pstat4", "ppito4",
        "sflow1", "sflow2", "sflow3", "sflow4", 
        "sflow_fract", "sflow_uneven"
    ];
    for (i, header) in headers.iter().enumerate() {
        sheet.get_cell_mut((i as u32 + 1, 1)).set_value(header.to_string());
    }

    // Данные
    for (row_idx, result) in results.iter().enumerate() {
        let row = row_idx as u32 + 2;
        let cols = [
            result.timestamp.to_string(),
            result.mflow.to_string(),
            result.delp1i.to_string(),
            result.p1ci.to_string(),
            result.t1ci.to_string(),
            result.t2i.to_string(),
            result.pstat[0].to_string(),
            result.ppito[0].to_string(),
            result.pstat[1].to_string(),
            result.ppito[1].to_string(),
            result.pstat[2].to_string(),
            result.ppito[2].to_string(),
            result.pstat[3].to_string(),
            result.ppito[3].to_string(),
            result.sflow[0].to_string(),
            result.sflow[1].to_string(),
            result.sflow[2].to_string(),
            result.sflow[3].to_string(),
            result.sflow_fract.to_string(),
            result.sflow_uneven.to_string(),
        ];

        for (col, value) in cols.iter().enumerate() {
            sheet.get_cell_mut((col as u32 + 1, row)).set_value(value);
        }
    } 

    umya_spreadsheet::writer::xlsx::write(&book, "monitoring_data.xlsx").unwrap();
}
