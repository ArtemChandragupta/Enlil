use eframe::egui;
use crossbeam_channel::{bounded, Receiver};
use reqwest::blocking;
use serde::Deserialize;
use std::collections::VecDeque;
use std::time::Duration;

pub struct TemplateApp {
    label: String,
    value: f32,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            label: "Hello World!".to_owned(),
            value: 2.7,
        }
    }
}

impl TemplateApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Default::default()
    }
}

impl eframe::App for TemplateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) { 

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
                        // for entry in &self.data {
                            // ui.label(entry.num1.to_string());
                            // ui.label(entry.num2.to_string());
                            // ui.end_row();
                        // }
                    });
            }); 
        });

        ctx.request_repaint_after(Duration::from_millis(500));
    }
}
