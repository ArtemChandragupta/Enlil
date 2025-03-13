use eframe::egui;
use crossbeam_channel::{bounded, Receiver};
use reqwest::blocking;
use serde::Deserialize;
use std::collections::VecDeque;
use std::time::Duration;

#[derive(Debug, Deserialize, Clone)]
struct ServerData {
    num1: i32,
    num2: i32,
}

struct App {
    receivers: [Receiver<ServerData>; 4],
    data:      [VecDeque<ServerData>; 4],
}

impl App {
    fn new(receivers: [Receiver<ServerData>; 4]) -> Self {
        Self {
            receivers,
            data: [
                VecDeque::new(),
                VecDeque::new(),
                VecDeque::new(),
                VecDeque::new(),
            ],
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Обновляем данные для всех серверов
        for i in 0..4 {
            while let Ok(new_data) = self.receivers[i].try_recv() {
                self.data[i].push_front(new_data.clone());
                // Ограничиваем историю последними 100 записями
                if self.data[i].len() > 100 {
                    self.data[i].pop_back();
                }
            }
        }

        // Отображаем интерфейс
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Данные с четырех серверов");
            
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("data_grid")
                    .num_columns(4)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        // Заголовки таблицы
                        ui.strong("Сервер 1");
                        ui.strong("Сервер 2");
                        ui.strong("Сервер 3");
                        ui.strong("Сервер 4");
                        ui.end_row();

                        // Определяем максимальную длину данных
                        let max_len = self.data.iter().map(|d| d.len()).max().unwrap_or(0);

                        // Функция для форматирования данных
                        fn format_data(data: Option<&ServerData>) -> String {
                            match data {
                                Some(d) => format!("{}, {}", d.num1, d.num2),
                                None => "--, --".to_string(),
                            }
                        }

                        // Отображаем данные построчно
                        for row in 0..max_len {
                            for server in 0..4 {
                                ui.label(format_data(self.data[server].get(row)));
                            }
                            ui.end_row();
                        }
                    });
            });
        });

        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

fn main() -> eframe::Result<()> {
    // Создаем каналы для каждого сервера
    let (sender1, receiver1) = bounded(1000);
    let (sender2, receiver2) = bounded(1000);
    let (sender3, receiver3) = bounded(1000);
    let (sender4, receiver4) = bounded(1000);

    // Список URL серверов
    let urls = [
        "http://127.0.0.27:9000",
        "http://127.0.0.28:9000",
        "http://127.0.0.203:9000",
        "http://127.0.0.204:9000",
    ];

    // Запускаем потоки для опроса серверов
    for (i, url) in urls.iter().enumerate() {
        let sender = match i {
            0 => sender1.clone(),
            1 => sender2.clone(),
            2 => sender3.clone(),
            3 => sender4.clone(),
            _ => unreachable!(),
        };
        let url = url.to_string();
        
        std::thread::spawn(move || loop {
            match blocking::get(&url) {
                Ok(response) => {
                    if let Ok(data) = response.json::<ServerData>() {
                        let _ = sender.send(data);
                    }
                }
                Err(e) => eprintln!("Ошибка запроса к серверу {}: {}", url, e),
            }
            std::thread::sleep(Duration::from_secs(1));
        });
    }

    let options = eframe::NativeOptions {
        // initial_window_size: Some(egui::vec2(800.0, 400.0)),
        ..Default::default()
    };

    eframe::run_native(
        "Server Data Viewer",
        options,
        Box::new(|_cc| Ok(Box::new(App::new([receiver1, receiver2, receiver3, receiver4])))),
    )
}
