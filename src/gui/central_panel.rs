
use crate::gui::{LogMessage, MyApp};
use color_eyre::owo_colors::OwoColorize;
use egui::{Color32, FontFamily, FontId, RichText};
use egui_extras::{StripBuilder, Size};
use tracing::{error, info, warn};

impl MyApp {
    pub fn central_panel_update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        egui::CentralPanel::default().show(ctx, |ui| {
            while let Ok(message) = self.rtt_data_rx.try_recv() {
                info!("Received <-- {:?} -->", message);
                
                let log_message = LogMessage {
                    message: message[0 .. message.len() - 1].to_owned(),
                    color: Color32::TRANSPARENT,
                };
                // Store the log message
                self.logs.push(log_message.clone());
                // Apply filters and add it to the filtered logs list too
                if let Some(filtered_log_message) = self.apply_filters(log_message) {
                    self.filtered_logs.push(filtered_log_message);
                }
            }
            let text_style = egui::TextStyle::Body;
            let row_height = ui.text_style_height(&text_style);
            let total_rows = self.filtered_logs.len();
            StripBuilder::new(ui)
                .sizes(Size::remainder(), 1) // for the table
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        egui::ScrollArea::both()
                            .stick_to_bottom(true)
                            .auto_shrink([false, false])
                            .show_rows(ui, row_height, total_rows, |ui, row_range| {
                                egui::Grid::new("some_unique_id")
                                    .striped(false)
                                    .show(ui, |ui| {
                                    ui.label(RichText::new("Message").size(15.0).family(FontFamily::Monospace));
                                    ui.end_row();

                                    for idx in row_range {
                                        let bg_color = self.filtered_logs.get(idx).unwrap().color;

                                        let mut color = Color32::BLACK;
                                        if bg_color == Color32::TRANSPARENT {
                                            color = Color32::WHITE;
                                        }

                                        ui.label(RichText::new(&self.filtered_logs.get(idx).unwrap().message)
                                            .size(15.0)
                                            .family(FontFamily::Monospace)
                                            .background_color(bg_color)
                                            .color(color)
                                        );
                                        ui.end_row();
                                    }
                                });
                        });
                    });
                });
        });
            
    }
}
