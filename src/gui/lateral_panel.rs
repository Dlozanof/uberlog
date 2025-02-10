use crate::gui::{MyApp, Middledude, MiddleDudeType};
use chrono::NaiveDate;
use egui::{Color32, RichText};

#[derive(Default)]
pub struct LateralPanel {


    pub midledudes: Vec<Middledude>,

    // Old shite
    pub date_filter_from: Option<NaiveDate>,
    pub date_filter_to: Option<NaiveDate>,
}

impl MyApp {
    pub fn lateral_panel_update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        egui::SidePanel::right("side_panel").min_width(300.0)
        .show(ctx, |ui| {

            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Filter list")).size(20.0));
                if ui.button("Refresh").clicked() {
                    self.refresh_filtered_logs();
                }
            });

            let mut removed_filter_idx = None;
            for (idx, dude) in &mut self.lateral_panel.midledudes.iter_mut().enumerate() {

                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Filter {}", idx)).size(15.0));
                    ui.separator();
                    if ui.button("Remove").clicked() {
                        removed_filter_idx = Some(idx);
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Module");
                    ui.add(egui::TextEdit::singleline(&mut dude.module));
                });
                ui.horizontal(|ui| {
                    ui.label("Log level");
                    ui.add(egui::TextEdit::singleline(&mut dude.log_level));
                });
                ui.horizontal(|ui| {
                    ui.label("Message");
                    ui.add(egui::TextEdit::singleline(&mut dude.msg));
                });
                ui.horizontal(|ui| {
                    ui.label("Operation");
                    egui::ComboBox::from_label(format!("Operation for filter {}", idx))
                        .selected_text(match dude.kind {
                            MiddleDudeType::Exclusion => "Exclusion".to_owned(),
                            MiddleDudeType::Inclusion => "Inclusion".to_owned(),
                            MiddleDudeType::Highlighter => "Highlighter".to_owned()
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut dude.kind, MiddleDudeType::Highlighter, "Highlight");
                            ui.selectable_value(&mut dude.kind, MiddleDudeType::Exclusion, "Exclusion");
                            ui.selectable_value(&mut dude.kind, MiddleDudeType::Inclusion, "Inclusion");
                        }
                    );
                });
                match &mut dude.kind {
                    MiddleDudeType::Inclusion => (),
                    MiddleDudeType::Exclusion => (),
                    MiddleDudeType::Highlighter =>
                    {
                        ui.horizontal(|ui| {
                            ui.label("Highlight color");
                            ui.color_edit_button_srgba(&mut dude.color);
                        });
                    }
                }
                ui.separator();
            }
            if let Some(idx) = removed_filter_idx {
                self.lateral_panel.midledudes.remove(idx);
                self.refresh_filtered_logs();
            }

            if ui.button("Add").clicked() {
                self.lateral_panel.midledudes.push( Middledude {
                    kind: MiddleDudeType::Highlighter,
                    log_level: String::new(),
                    module: String::new(),
                    msg: String::new(),
                    color: Color32::GOLD,
                });
            }
        });
    }
}
