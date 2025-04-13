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

// // const IP_NOZ: &str = "192.168.0.27";
// const IP_CON: &str = "192.168.0.28";
// const IP_203: &str = "192.168.0.203";
// const IP_204: &str = "192.168.0.204";
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
            Ok(Box::new(MonitoringApp { shared_data: shared_data.clone() }))
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
        let (resp_noz, resp_con, resp_203, resp_204) = tokio::join!(
            fetch_data_async(IP_NOZ, SERVER_PORT),
            fetch_data_async(IP_CON, SERVER_PORT),
            fetch_data_async(IP_203, SERVER_PORT),
            fetch_data_async(IP_204, SERVER_PORT),
        );

        // Обработка ошибок
        let resp_noz = resp_noz.unwrap_or_else(|err| {
            println!("NOZ error: {err}");
            "err".to_string()
        });
        
        let resp_con = resp_con.unwrap_or_else(|err| {
            println!("CON error: {err}");
            "err".to_string()
        });
        
        let resp_203 = resp_203.unwrap_or_else(|err| {
            println!("203 error: {err}");
            "err".to_string()
        });
        
        let resp_204 = resp_204.unwrap_or_else(|err| {
            println!("204 error: {err}");
            "err".to_string()
        });

        // Обработка данных (без изменений)
        if resp_noz != "err" && resp_con != "err" && resp_203 != "err" && resp_204 != "err" {
            let plist_203 = parse_response(&resp_203);
            let plist_204 = parse_response(&resp_204);
            let blist     = [1.1, 2.1, 3.1];

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

            Plot::new("combined_plot")
                .legend(Legend::default().position(egui_plot::Corner::RightTop))
                .show(ui, |plot_ui| {
                    let mflow_points: PlotPoints = data.computed_results
                        .iter()
                        .map(|r| [r.timestamp as f64, r.mflow])
                        .collect();
                    plot_ui.line(Line::new(mflow_points).name("Mass Flow (kg/s)"));

                    let t1ci_points: PlotPoints = data.computed_results
                        .iter()
                        .map(|r| [r.timestamp as f64, r.t1ci])
                        .collect();
                    plot_ui.line(Line::new(t1ci_points).name("Nozzle T, C"));

                    let t2i_points: PlotPoints = data.computed_results
                        .iter()
                        .map(|r| [r.timestamp as f64, r.t2i])
                        .collect();
                    plot_ui.line(Line::new(t2i_points).name("Conus T, C"));

                    let sflow_points: PlotPoints = data.computed_results
                        .iter()
                        .map(|r| [r.timestamp as f64, r.sflow[0]])
                        .collect();
                    plot_ui.line(Line::new(sflow_points).name("G1, kg/s"));

                    let sflow_points: PlotPoints = data.computed_results
                        .iter()
                        .map(|r| [r.timestamp as f64, r.sflow[1]])
                        .collect();
                    plot_ui.line(Line::new(sflow_points).name("G2, kg/s"));

                    let sflow_points: PlotPoints = data.computed_results
                        .iter()
                        .map(|r| [r.timestamp as f64, r.sflow[2]])
                        .collect();
                    plot_ui.line(Line::new(sflow_points).name("G3, kg/s"));

                    let sflow_points: PlotPoints = data.computed_results
                        .iter()
                        .map(|r| [r.timestamp as f64, r.sflow[3]])
                        .collect();
                    plot_ui.line(Line::new(sflow_points).name("G4, kg/s"));

                    let sflow_fract_points: PlotPoints = data.computed_results
                        .iter()
                        .map(|r| [r.timestamp as f64, r.sflow_fract])
                        .collect();
                    plot_ui.line(Line::new(sflow_fract_points).name("G Fraction (%)"));

                    let sflow_uneven_points: PlotPoints = data.computed_results
                        .iter()
                        .map(|r| [r.timestamp as f64, r.sflow_uneven])
                        .collect();
                    plot_ui.line(Line::new(sflow_uneven_points).name("G uneven (%)"));
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
