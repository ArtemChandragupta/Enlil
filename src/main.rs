use std::time::Duration;
use eframe::egui;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
extern crate umya_spreadsheet;

const IP_NOZ: &str = "127.0.0.27";
const IP_CON: &str = "127.0.0.28";
const IP_203: &str = "127.0.0.203";
const IP_204: &str = "127.0.0.204";
const SERVER_PORT: u16 = 9000;

// Структура для хранения данных
#[derive(Default)]
struct ServerData {
    resp_con: VecDeque<String>,
    resp_noz: VecDeque<String>,
    resp_203: VecDeque<String>,
    resp_204: VecDeque<String>,
}

// Основное приложение
struct MonitoringApp {
    shared_data: Arc<Mutex<ServerData>>,
}

fn main() {
    // Общие данные для потоков
    let shared_data = Arc::new(Mutex::new(ServerData::default()));
    
    // Запускаем поток сбора данных
    let data_clone = shared_data.clone();
    thread::spawn(move || {
        data_collection_thread(data_clone);
    });

    // Запускаем GUI
    let options = eframe::NativeOptions::default();
    let _ = eframe::run_native(
        "Server Monitoring System",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Ok(Box::new(MonitoringApp {
            shared_data: shared_data.clone() 
        }))}),
    );
}

// Поток сбора данных
fn data_collection_thread(shared_data: Arc<Mutex<ServerData>>) {

    let mut book = initialize_excel_file();
    let mut row = 2;

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

            let sflow1 = calc_gs(ppito[0], pstat[0], t2i);
            let sflow2 = calc_gs(ppito[1], pstat[1], t2i);
            let sflow3 = calc_gs(ppito[2], pstat[2], t2i);
            let sflow4 = calc_gs(ppito[3], pstat[3], t2i);

            let sflow_sum    = sflow1 + sflow2 + sflow3 + sflow4;
            let sflow_ave    = sflow_sum / 4.0;
            let sflow_fract  = sflow_sum / mflow * 100.0;
            let sflow_uneven = 100.0 * (sflow1.max(sflow2).max(sflow3).max(sflow4) - sflow1.min(sflow2).min(sflow3).min(sflow4)) / sflow_ave;

            // Обновление общих данных
            let mut data = shared_data.lock().unwrap();
            data.resp_noz.push_front(resp_noz);
            data.resp_con.push_front(resp_con);
            data.resp_203.push_front(resp_203);
            data.resp_204.push_front(resp_204);

            write_data_to_excel(
                &mut book,
                row,
                timestamp,
                mflow,
                delp1i,
                p1ci,
                t1ci,
                t2i,
                pstat,
                ppito,
                sflow1,
                sflow2,
                sflow3,
                sflow4,
                sflow_fract,
                sflow_uneven,
            );
            row += 1;
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
                ui.heading("Real-time Server Monitoring");
            });

            ui.separator();

            let data = self.shared_data.lock().unwrap();

            // Таблица с сырыми данными
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("server_data_grid")
                    .num_columns(4)
                    .spacing([20.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.heading("NOZZLE");
                        ui.heading("CONUS");
                        ui.heading("SERVER 203");
                        ui.heading("SERVER 204");
                        ui.end_row();

                        let max_rows = data.resp_noz.len()
                            .max(data.resp_con.len())
                            .max(data.resp_203.len())
                            .max(data.resp_204.len());

                        for i in 0..max_rows {
                            ui.label(&**data.resp_noz.get(i).unwrap_or(&"N/A".into()));
                            ui.label(&**data.resp_con.get(i).unwrap_or(&"N/A".into()));
                            ui.label(&**data.resp_203.get(i).unwrap_or(&"N/A".into()));
                            ui.label(&**data.resp_204.get(i).unwrap_or(&"N/A".into()));
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

fn initialize_excel_file() -> umya_spreadsheet::Spreadsheet {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.get_sheet_by_name_mut("Sheet1").unwrap();
    
    let headers = [
        "Time", "Flow, kg/s", "DeltaP, Pa", "P,Pa", 
        "t1ci", "t2i", "pstat1", "ppito1", "pstat2", 
        "ppito2", "pstat3", "ppito3", "pstat4", "ppito4",
        "sflow1", "sflow2", "sflow3", "sflow4", 
        "sflow_fract", "sflow_uneven"
    ];

    for (idx, header) in headers.iter().enumerate() {
        sheet.get_cell_mut((idx as u32 + 1, 1)).set_value(header.to_string());
    }

    let _ = umya_spreadsheet::writer::xlsx::write(&book, "./monitoring_data.xlsx");
    book
}

fn write_data_to_excel(
    book: &mut umya_spreadsheet::Spreadsheet,
    row: u32,
    timestamp: u64,
    mflow: f64,
    delp1i: f64,
    p1ci: f64,
    t1ci: f64,
    t2i: f64,
    pstat: [f64; 4],
    ppito: [f64; 4],
    sflow1: f64,
    sflow2: f64,
    sflow3: f64,
    sflow4: f64,
    sflow_fract: f64,
    sflow_uneven: f64,
) {
    let sheet = book.get_sheet_by_name_mut("Sheet1").unwrap();
    
    let data = [
        timestamp.to_string(),
        mflow.to_string(),
        delp1i.to_string(),
        p1ci.to_string(),
        t1ci.to_string(),
        t2i.to_string(),
        pstat[0].to_string(),
        ppito[0].to_string(),
        pstat[1].to_string(),
        ppito[1].to_string(),
        pstat[2].to_string(),
        ppito[2].to_string(),
        pstat[3].to_string(),
        ppito[3].to_string(),
        sflow1.to_string(),
        sflow2.to_string(),
        sflow3.to_string(),
        sflow4.to_string(),
        sflow_fract.to_string(),
        sflow_uneven.to_string(),
    ];

    for (col, value) in data.iter().enumerate() {
        sheet.get_cell_mut((col as u32 + 1, row)).set_value(value.clone());
    }

    let _ = umya_spreadsheet::writer::xlsx::write(book, "./monitoring_data.xlsx");
}
