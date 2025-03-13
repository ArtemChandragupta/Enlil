use eframe::egui;
use rand::{distributions::Alphanumeric, Rng};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

struct App {
    rows: VecDeque<[String; 5]>,
    last_update: Instant,
}

impl Default for App {
    fn default() -> Self {
        Self {
            rows: VecDeque::new(),
            last_update: Instant::now(),
        }
    }
}

impl App {
    fn generate_random_row(&mut self) {
        let mut rng = rand::thread_rng();

        let col1 = rng.gen_range(0..100000).to_string();
        let col2: String = (0..7).map(|_| rng.sample(Alphanumeric) as char).collect();
        let col3 = format!("{:.4}", rng.gen::<f64>());
        let col4 = if rng.gen_bool(0.5) { "Yes" } else { "No" }.to_string();
        let col5 = chrono::Local::now().format("%H:%M:%S").to_string();

        self.rows.push_front([col1, col2, col3, col4, col5]);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_secs(1));

        if self.last_update.elapsed() > Duration::from_secs(1) {
            self.generate_random_row();
            self.last_update = Instant::now();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                
                ui.heading("Real-time Data Table");
                
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            egui::Grid::new("data_grid")
                                .num_columns(5)
                                .spacing([40.0, 4.0])
                                .striped(true)
                                .min_col_width(100.0)
                                .show(ui, |ui| {
                                    // Стилизация заголовков
                                    ui.style_mut().override_text_style = Some(egui::TextStyle::Heading);
                                    
                                    for &header in &["ID", "Code", "Value", "Status", "Time"] {
                                        ui.label(
                                            egui::RichText::new(header)
                                                .strong()
                                                .color(egui::Color32::from_rgb(25, 118, 210))
                                        );
                                    }
                                    ui.end_row();

                                    // Стиль для данных
                                    ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                                    
                                    for row in &self.rows {
                                        for (i, cell) in row.iter().enumerate() {
                                            let mut label = egui::Label::new(cell)
                                                .wrap();
                                            
                                            if i == 2 {
                                                label = label.layout_in_ui(egui::Layout::right_to_left(egui::Align::Center));
                                            }
                                            
                                            ui.add(
                                                label.frame(
                                                    egui::Frame::none()
                                                        .fill(egui::Color32::from_gray(10))
                                                        .rounding(4.0)
                                                )
                                            );
                                        }
                                        ui.end_row();
                                    }
                                });
                        });
                    });
                
                ui.add_space(20.0);
            });
        });
    }
}

fn main() {
    let options = eframe::NativeOptions {
        vsync: false,  // Отключаем VSYNC для более предсказуемых обновлений
        ..Default::default()
    };
    
    let _ = eframe::run_native(
        "Real-time Table App",
        options,
        Box::new(|_cc| Ok(Box::new(App::default()))),
    );
}
