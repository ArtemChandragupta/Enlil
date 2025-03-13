use std::io::{Read, Write};
use std::net::TcpStream;
use std::fs::OpenOptions;
use std::time::{SystemTime, UNIX_EPOCH};
use std::thread;
use std::sync::{Arc, Mutex};
extern crate umya_spreadsheet;

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

fn main() {
    let log_file = Arc::new(Mutex::new(OpenOptions::new().append(true).create(true).open(LOG_FILE).expect("Failed to open log file")));
    let headstrf = "Time\tFlow, kg/s\tDeltaP, Pa\tP, Pa\tTemp, K\tTemp2, K\n";
    let mut book = umya_spreadsheet::new_file();

    {
        let mut log = log_file.lock().unwrap();
        writeln!(log, "{}", headstrf).expect("Failed to write to log file");
    }

    loop {
        thread::sleep(std::time::Duration::from_secs(1));

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_secs();

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

        if resp_noz == "err" || resp_con == "err" || resp_203 == "err" || resp_204 == "err" {
            eprintln!("Received data is incomplete or invalid.");
            continue;
        }

        let plist_203 = parse_response(&resp_203);
        let plist_204 = parse_response(&resp_204);

        let blist = vec!["1,1".to_string(), "2,1".to_string(), "3,1".to_string()];

        let delp1i = plist_204[8] - plist_204[9];
        let p1ci   = plist_204[8] + blist[1].replace(",", ".").parse::<f64>().unwrap_or(0.0) * 100.0;
        let t1ci   = resp_noz.parse::<f64>().unwrap_or(0.0) + 273.15;
        let t2i    = resp_con.parse::<f64>().unwrap_or(0.0) + 273.15;

        let mflow = calc_g(t1ci, delp1i, p1ci);

        let pstat1 = plist_204[0] +                 blist[1].replace(",", ".").parse::<f64>().unwrap_or(0.0) * 100.0;
        let ppito1 = plist_204[0] + plist_203[11] + blist[1].replace(",", ".").parse::<f64>().unwrap_or(0.0) * 100.0;
        let pstat2 = plist_204[1] +                 blist[1].replace(",", ".").parse::<f64>().unwrap_or(0.0) * 100.0;
        let ppito2 = plist_204[1] + plist_203[12] + blist[1].replace(",", ".").parse::<f64>().unwrap_or(0.0) * 100.0;
        let pstat3 = plist_204[2] +                 blist[1].replace(",", ".").parse::<f64>().unwrap_or(0.0) * 100.0;
        let ppito3 = plist_204[2] + plist_203[13] + blist[1].replace(",", ".").parse::<f64>().unwrap_or(0.0) * 100.0;
        let pstat4 = plist_204[3] +                 blist[1].replace(",", ".").parse::<f64>().unwrap_or(0.0) * 100.0;
        let ppito4 = plist_204[3] + plist_203[14] + blist[1].replace(",", ".").parse::<f64>().unwrap_or(0.0) * 100.0;

        let sflow1 = calc_gs(ppito1, pstat1, t2i);
        let sflow2 = calc_gs(ppito2, pstat2, t2i);
        let sflow3 = calc_gs(ppito3, pstat3, t2i);
        let sflow4 = calc_gs(ppito4, pstat4, t2i);

        let sflow_sum    = sflow1 + sflow2 + sflow3 + sflow4;
        let sflow_ave    = sflow_sum / 4.0;
        let sflow_fract  = sflow_sum / mflow * 100.0;
        let sflow_uneven = 100.0 * (sflow1.max(sflow2).max(sflow3).max(sflow4) - sflow1.min(sflow2).min(sflow3).min(sflow4)) / sflow_ave;

        let savestr = format!("{}\t{:.6}\t{:.2}\t{:.2}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.2}\t{:.2}\n",
                              timestamp, mflow, resp_noz.parse::<f64>().unwrap_or(0.0), resp_con.parse::<f64>().unwrap_or(0.0),
                              sflow1, sflow2, sflow3, sflow4, sflow_fract, sflow_uneven);

        book.get_sheet_by_name_mut("Sheet1").unwrap().get_cell_mut("A1").set_value("TEST1");
        let path = std::path::Path::new("./bbb.xlsx");
        let _ = umya_spreadsheet::writer::xlsx::write(&book, path);

        println!("{}", savestr);
    }
}


