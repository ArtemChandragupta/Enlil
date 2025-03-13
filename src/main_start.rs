use eframe::egui;
use std::io::{Read, Write};
use std::fs::OpenOptions;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::thread;

// ... [остальные импорты из вашего кода] ...

#[derive(Default)]
struct ServerResponses {
    resp_con: VecDeque<String>,
    resp_noz: VecDeque<String>,
    resp_203: VecDeque<String>,
    resp_204: VecDeque<String>,
}

struct MyApp {
    shared_data: Arc<Mutex<ServerResponses>>,
}

const IP_NOZ: &str = "127.0.0.27";
const IP_CON: &str = "127.0.0.28";
const IP_203: &str = "127.0.0.203";
const IP_204: &str = "127.0.0.204";
const SERVER_PORT: u16 = 9000;
const LOG_FILE: &str = "nflow_out.txt";

fn fetch_data_from_server(ip: &str, port: u16) -> Result<String, std::io::Error> {
    let mut stream = TcpStream::connect((ip, port))?;
    stream.write_all(b"rffff0")?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    Ok(String::from_utf8_lossy(&response).to_string())
}

fn main() {
    let shared_data = Arc::new(Mutex::new(ServerResponses::default()));
    
    // Запускаем поток для сбора данных
    let data_clone = shared_data.clone();
    thread::spawn(move || {
        // let log_file = Arc::new(Mutex::new(
            // OpenOptions::new()
                // .append(true)
                // .create(true)
                // .open(LOG_FILE)
                // .expect("Failed to open log file"),
        // ));
        
        // ... [ваш код инициализации log файла и excel] ...

        loop {
            thread::sleep(std::time::Duration::from_secs(1));

            // Получаем данные с серверов
            let resp_noz = fetch_data_from_server(IP_NOZ, SERVER_PORT).unwrap_or_else(|err| {
                println!("Problem getting data from nozzile: {err}");
                "err".to_string()
            });
            
            let resp_con = fetch_data_from_server(IP_CON, SERVER_PORT).unwrap_or_else(|err| {
                println!("Problem getting data from conus: {err}");
                "err".to_string()
            });
            
            let resp_203 = fetch_data_from_server(IP_203, SERVER_PORT).unwrap_or_else(|err| {
                println!("Problem getting data from 203: {err}");
                "err".to_string()
            });
            
            let resp_204 = fetch_data_from_server(IP_204, SERVER_PORT).unwrap_or_else(|err| {
                println!("Problem getting data from 204: {err}");
                "err".to_string()
            });

            // Обновляем общие данные
            let mut data = data_clone.lock().unwrap();
            data.resp_noz.push_back(resp_noz);
            data.resp_con.push_back(resp_con);
            data.resp_203.push_back(resp_203);
            data.resp_204.push_back(resp_204);

            // Поддерживаем максимальную историю в 10 записей
            if data.resp_noz.len() > 10 { data.resp_noz.pop_front(); }
            if data.resp_con.len() > 10 { data.resp_con.pop_front(); }
            if data.resp_203.len() > 10 { data.resp_203.pop_front(); }
            if data.resp_204.len() > 10 { data.resp_204.pop_front(); }

            // ... [остальная логика вашего кода] ...
        }
    });

    // Запускаем GUI
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Server Monitor",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp { 
            shared_data: shared_data.clone() 
        }))),
    );
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Server Responses Monitor");
            ui.separator();
            
            let data = self.shared_data.lock().unwrap();
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("response_grid")
                    .num_columns(4)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        // Заголовки
                        ui.heading("NOZ");
                        ui.heading("CON");
                        ui.heading("203");
                        ui.heading("204");
                        ui.end_row();

                        // Определяем максимальное количество строк
                        let max_rows = data.resp_noz.len()
                            .max(data.resp_con.len())
                            .max(data.resp_203.len())
                            .max(data.resp_204.len());

                        // Отображаем данные
                        for i in 0..max_rows {
                            ui.label(&*data.resp_noz.get(i).unwrap_or(&"N/A".to_string()));
                            ui.label(&*data.resp_con.get(i).unwrap_or(&"N/A".to_string()));
                            ui.label(&*data.resp_203.get(i).unwrap_or(&"N/A".to_string()));
                            ui.label(&*data.resp_204.get(i).unwrap_or(&"N/A".to_string()));
                            ui.end_row();
                        }
                    });
            });
        });
    }
}

// ... [остальные функции из вашего кода остаются без изменений] ...
