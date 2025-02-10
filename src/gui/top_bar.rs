use std::path::PathBuf;

use rfd::FileDialog;
use tracing::info;

use crate::gui::{MyApp, TargetMcu};
use crate::commander::{Command, LogBackendInformation, TargetInformation};

pub struct TopBar {
    pub targets: Vec<TargetInformation>,

    // Log streaming
    pub out_path: String,
    pub streaming: bool,

    // Search
    pub search_log: String,

    // Open file
    pub opened_file: Option<PathBuf>,
    pub open_file_dialog: Option<FileDialog>,
}

impl MyApp {
    pub fn top_bar_update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {

            for target in &mut self.top_bar.targets {
                ui.label(format!("{} with serial [{}]", target.probe_name, target.probe_serial));
                ui.horizontal(|ui| {
                    ui.label("Microcontroller");
                    ui.add(egui::TextEdit::singleline(&mut target.mcu).desired_width(150.0));
                    
                    match &target.backend {
                        LogBackendInformation::Rtt(elfpath) => {
                            ui.label(format!("{}", elfpath));
                        }
                        LogBackendInformation::Uart(port, baud) => {
                            ui.label(format!("{} - (baudrate {})", port, baud));
                        }
                    }

                    if ui.button("Connect").clicked() {
                        info!("Connect {}", target.mcu);
                        let _ = self.command_tx.send(Command::ReceiveDrawContext(ctx.clone()));
                        let _ = self.command_tx.send(Command::Connect(target.clone()));
                    }
                    if ui.button("Disconnect").clicked() {
                        info!("Disconnect {}", target.mcu);
                        let _ = self.command_tx.send(Command::Disconnect(target.probe_serial.clone()));
                    }
                    if ui.button("Reset").clicked() {
                        info!("Resetting {}", target.mcu);
                        let _ = self.command_tx.send(Command::Reset(target.probe_serial.clone()));
                    }
                });
            }

            ui.horizontal(|ui| {

                // Record logs section
                ui.label("Record logs to file");
                match self.top_bar.streaming {
                    true => {
                        if ui.button("Stop").clicked() {
                            if !self.top_bar.out_path.is_empty() {
                                self.top_bar.streaming = false;
                                let _ = self.command_tx.send(Command::StreamLogs(false, String::new()));
                            }
                        }
                    },
                    false => {
                        if ui.button("Play").clicked() {
                            if !self.top_bar.out_path.is_empty() {
                                self.top_bar.streaming = true;
                                let _ = self.command_tx.send(Command::StreamLogs(true, self.top_bar.out_path.clone()));
                            }
                        }
                    }
                };
                ui.add(egui::TextEdit::singleline(&mut self.top_bar.out_path).desired_width(150.0));

                // Open log file section
                if (ui.button("Open file")).clicked() {
                    let file = FileDialog::new()
                        .pick_file();
                    if let Some(file) = file {
                        self.clear_logs();
                        self.command_tx.send(Command::OpenFile(file));
                    }
                }

                // Manage screen section
                ui.separator();
                if ui.button("Refresh probe list").clicked() {
                    let _ = self.command_tx.send(Command::GetProbes);
                }
                    if ui.button("Clear screen").clicked() {
                    self.clear_logs();
                }
            });

            // Search bar
            ui.horizontal(|ui| {
                ui.label("Search");
                ui.add(egui::TextEdit::singleline(&mut self.top_bar.search_log).desired_width(150.0));
            });
        });
    }
}
