use eframe::egui;
use crossbeam_channel::{bounded, Receiver};
use reqwest::blocking;
use serde::Deserialize;
use std::collections::VecDeque;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct ServerData {
    num1: i32,
    num2: i32,
}

struct App {
    receiver: Receiver<ServerData>,
    data: VecDeque<ServerData>, // Изменили на VecDeque
}

impl App {
    fn new(receiver: Receiver<ServerData>) -> Self {
        Self {
            receiver,
            data: VecDeque::new(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Получаем новые данные и добавляем в начало
        while let Ok(new_data) = self.receiver.try_recv() {
            self.data.push_front(new_data); // Добавляем в начало
        }

        // Отображаем интерфейс
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Данные с сервера");
            
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("data_grid")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        // Заголовки таблицы
                        ui.strong("Число 1");
                        ui.strong("Число 2");
                        ui.end_row();

                        // Отображаем данные в порядке очереди (новые сверху)
                        for entry in &self.data {
                            ui.label(entry.num1.to_string());
                            ui.label(entry.num2.to_string());
                            ui.end_row();
                        }
                    });
            });
        });

        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

// Остальная часть кода без изменений...
fn main() -> eframe::Result<()> {
    let (sender, receiver) = bounded(1000);

    std::thread::spawn(move || loop {
        match blocking::get("http://127.0.0.27:8080") {
            Ok(response) => {
                if let Ok(data) = response.json::<ServerData>() {
                    let _ = sender.send(data);
                }
            }
            Err(e) => eprintln!("Ошибка запроса: {}", e),
        }
        std::thread::sleep(Duration::from_secs(1));
    });

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(300.0, 400.0)),
        ..Default::default()
    };

    eframe::run_native(
        "Server Data Viewer",
        options,
        Box::new(|_cc| Box::new(App::new(receiver))),
    )
}
