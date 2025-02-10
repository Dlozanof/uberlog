use egui::Color32;
use probe_rs::{probe::{list::Lister, DebugProbeInfo}, rtt::Rtt, Permissions};
use tracing::info;

use crate::{commander::{Command, CommandResponse}, gui::top_bar::TopBar};
use crate::gui::lateral_panel::LateralPanel;
use std::{collections::BinaryHeap, time::Duration};
use std::sync::mpsc::{Receiver, Sender};

#[derive(Clone)]
pub struct LogMessage {
    pub message: String,
    pub color: Color32,
}

#[derive(PartialEq)]
pub enum MiddleDudeType {
    Exclusion,
    Inclusion,
    Highlighter,
}

pub struct Middledude {
    pub kind: MiddleDudeType,
    pub log_level: String,
    pub module: String,
    pub msg: String,
    pub color: Color32,
}

pub struct TargetMcu {
    pub mcu: String,
    pub rtt_address: String,
    pub probe: Option<DebugProbeInfo>,
}

pub struct MyApp {
    // Top bar
    pub top_bar: TopBar,

    // Lateral panel
    pub lateral_panel: LateralPanel,

    // -- Functional stuff
    pub command_tx: Sender<Command>,
    pub command_response_rx: Receiver<CommandResponse>,
    pub rtt_data_rx: Receiver<String>,
    pub is_streaming: bool,

    // Currently selected message
    pub curr_log_idx: usize,

    // Temporary storage
    pub logs: Vec<LogMessage>,
    pub filtered_logs: Vec<LogMessage>,
}

impl MyApp {

    /// Clear al log messages
    /// 
    /// Useful for restarting an analysis, just clears `logs` and `filtered_logs` vectors
    pub fn clear_logs(&mut self) {
        self.logs.clear();
        self.filtered_logs.clear();
    }

    //pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
    pub fn new(command_tx: Sender<Command>, command_response_rx: Receiver<CommandResponse>, rtt_data_rx: Receiver<String>) -> Self {
        MyApp {
            top_bar: TopBar {
                targets: Vec::new(),
                out_path: String::new(),
                streaming: false,
                search_log: String::new(),
                open_file_dialog: None,
                opened_file: None,
            },
            lateral_panel: Default::default(),
            command_tx,
            command_response_rx,
            rtt_data_rx,
            is_streaming: false,
            curr_log_idx: 0,
            logs: Vec::new(),
            filtered_logs: Vec::new(),
        }
    }

    pub fn search_log(&mut self, search_string: &String) -> bool {
        for (idx, log) in self.filtered_logs.iter().enumerate() {
            if log.message.contains(search_string) {
                self.curr_log_idx = idx;
                return true;
            }
        }
        false
    }

    pub fn apply_filters(&self, log: LogMessage) -> Option<LogMessage> {

        let mut log = Some(log);

        for current_filter in &self.lateral_panel.midledudes {

            if log.is_none() {
                return log;
            }

            match current_filter.kind {
                MiddleDudeType::Inclusion => {
                    let tmp_log = log.clone().unwrap();

                    let matches_level = tmp_log.message.contains(&current_filter.log_level) && !current_filter.log_level.is_empty();
                    let matches_module = tmp_log.message.contains(&current_filter.module) && !current_filter.module.is_empty();
                    let matches_msg = tmp_log.message.contains(&current_filter.msg) && !current_filter.msg.is_empty();
                    let retain_it = matches_module || matches_level || matches_msg;
                    if (retain_it) {
                        continue;
                    } else {
                        log = None;
                    }
                },
                MiddleDudeType::Exclusion => {
                    let tmp_log = log.clone().unwrap();

                    let matches_level = tmp_log.message.contains(&current_filter.log_level) && !current_filter.log_level.is_empty();
                    let matches_module = tmp_log.message.contains(&current_filter.module) && !current_filter.module.is_empty();
                    let matches_msg = tmp_log.message.contains(&current_filter.msg) && !current_filter.msg.is_empty();
                    let retain_it = !(matches_module || matches_level || matches_msg);

                    if (retain_it) {
                        continue;
                    } else {
                        log = None;
                    }
                },
                MiddleDudeType::Highlighter => {
                    let tmp_log = log.clone().unwrap();

                    let matches_level = tmp_log.message.contains(&current_filter.log_level) && !current_filter.log_level.is_empty();
                    let matches_module = tmp_log.message.contains(&current_filter.module) && !current_filter.module.is_empty();
                    let matches_msg = tmp_log.message.contains(&current_filter.msg) && !current_filter.msg.is_empty();
                    if matches_level || matches_module || matches_msg {
                        log = Some(LogMessage {
                            color: current_filter.color,
                            message: log.unwrap().message,
                        });
                    }
                }
            }
        }
        log
    }

    // TODO: Probably I should be able to use `apply_filters` in a loop here
    pub fn refresh_filtered_logs(&mut self) {
        self.filtered_logs = self.logs.clone();

        for current_filter in &self.lateral_panel.midledudes {
            match current_filter.kind {
                MiddleDudeType::Inclusion => {
                    self.filtered_logs.retain(|log| {
                        let matches_level = log.message.contains(&current_filter.log_level) && !current_filter.log_level.is_empty();
                        let matches_module = log.message.contains(&current_filter.module) && !current_filter.module.is_empty();
                        let matches_msg = log.message.contains(&current_filter.msg) && !current_filter.msg.is_empty();
                        let retain_it = matches_module || matches_level || matches_msg;

                        retain_it
                    });
                },
                MiddleDudeType::Exclusion => {
                    self.filtered_logs.retain(|log| {
                        let matches_level = log.message.contains(&current_filter.log_level) && !current_filter.log_level.is_empty();
                        let matches_module = log.message.contains(&current_filter.module) && !current_filter.module.is_empty();
                        let matches_msg = log.message.contains(&current_filter.msg) && !current_filter.msg.is_empty();
                        let retain_it = matches_module || matches_level || matches_msg;

                        !retain_it
                    });
                },
                MiddleDudeType::Highlighter => {
                    self.filtered_logs.iter_mut().for_each(|log| {
                        let matches_level = log.message.contains(&current_filter.log_level) && !current_filter.log_level.is_empty();
                        let matches_module = log.message.contains(&current_filter.module) && !current_filter.module.is_empty();
                        let matches_msg = log.message.contains(&current_filter.msg) && !current_filter.msg.is_empty();
                        if matches_level || matches_module || matches_msg {
                            log.color = current_filter.color;
                        }
                    });
                }
            }
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Process messages from Commander thread
        if let Ok(response) = self.command_response_rx.try_recv() {
            match response {
                CommandResponse::ProbeInformation { probes } => {
                    self.top_bar.targets = probes;
                },
                CommandResponse::TextMessage { message } => {
                    info!("{}", message);
                },
            }
        }


        self.top_bar_update(ctx, frame);
        self.lateral_panel_update(ctx, frame);
        self.central_panel_update(ctx, frame);
    }
}
