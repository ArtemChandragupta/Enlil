use std::time::Duration;
use eframe::egui;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::fs::OpenOptions;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
extern crate umya_spreadsheet;

const IP_NOZ: &str = "127.0.0.27";
const IP_CON: &str = "127.0.0.28";
const IP_203: &str = "127.0.0.203";
const IP_204: &str = "127.0.0.204";
const SERVER_PORT: u16 = 9000;
const LOG_FILE: &str = "nflow_out.txt";

// Структура для хранения данных
#[derive(Default)]
struct ServerData {
    resp_con: VecDeque<String>,
    resp_noz: VecDeque<String>,
    resp_203: VecDeque<String>,
    resp_204: VecDeque<String>,
    metrics: VecDeque<String>,
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
        Box::new(|_cc| Ok(Box::new(MonitoringApp { 
            shared_data: shared_data.clone() 
        }))),
    );
}

// Поток сбора данных
fn data_collection_thread(shared_data: Arc<Mutex<ServerData>>) {
    let log_file = Arc::new(Mutex::new(
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(LOG_FILE)
            .expect("Failed to open log file"),
    ));
    
    let mut book = umya_spreadsheet::new_file();
    let headstrf = "Time\tFlow, kg/s\tDeltaP, Pa\tP, Pa\tTemp, K\tTemp2, K\n";
    
    {
        let mut log = log_file.lock().unwrap();
        writeln!(log, "{}", headstrf).expect("Failed to write to log file");
    }

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
            // let plist_203 = parse_response(&resp_203);
            let plist_204 = parse_response(&resp_204);
            let blist = ["1,1".to_string(), "2,1".to_string(), "3,1".to_string()];

            // Расчеты
            let delp1i = plist_204[8] - plist_204[9];
            let p1ci = plist_204[8] + blist[1].replace(',', ".").parse::<f64>().unwrap_or(0.0) * 100.0;
            let t1ci = resp_noz.parse::<f64>().unwrap_or(0.0) + 273.15;
            let t2i = resp_con.parse::<f64>().unwrap_or(0.0) + 273.15;
            let mflow = calc_g(t1ci, delp1i, p1ci);

            // Формирование строки для сохранения
            let savestr = format!(
                "{}\t{:.6}\t{:.2}\t{:.2}\t{:.3}\t{:.3}\n",
                timestamp, mflow, 
                resp_noz.parse::<f64>().unwrap_or(0.0),
                resp_con.parse::<f64>().unwrap_or(0.0),
                t1ci, t2i
            );

            // Обновление общих данных
            let mut data = shared_data.lock().unwrap();
            data.resp_noz.push_back(resp_noz);
            data.resp_con.push_back(resp_con);
            data.resp_203.push_back(resp_203);
            data.resp_204.push_back(resp_204);
            data.metrics.push_back(savestr.clone());

            // Ограничение истории
            // if data.resp_noz.len() > 10 { data.resp_noz.pop_front(); }
            // if data.resp_con.len() > 10 { data.resp_con.pop_front(); }
            // if data.resp_203.len() > 10 { data.resp_203.pop_front(); }
            // if data.resp_204.len() > 10 { data.resp_204.pop_front(); }
            // if data.metrics.len() > 10 { data.metrics.pop_front(); }

            // Запись в Excel
            let sheet = book.get_sheet_by_name_mut("Sheet1").unwrap();
            sheet.get_cell_mut("A1").set_value("Monitoring Data");
            let _ = umya_spreadsheet::writer::xlsx::write(&book, "./monitoring_data.xlsx");

            // Запись в лог
            let mut log = log_file.lock().unwrap();
            writeln!(log, "{}", savestr).expect("Log write failed");
        }
    }
}

// Графический интерфейс
impl eframe::App for MonitoringApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_secs(1));
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Real-time Server Monitoring");
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

            // ui.separator();
            // 
            // // Метрики
            // ui.heading("Calculated Metrics");
            // egui::ScrollArea::vertical().show(ui, |ui| {
            //     for metric in &data.metrics {
            //         ui.label(metric);
            //     }
            // });
        });
    }
}

// Остальные функции остаются без изменений
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
