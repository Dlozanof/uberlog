use std::{
    fmt,
    io::Write,
    sync::mpsc::{Receiver, Sender},
};

use crate::{
    LogFilter, LogMessage, LogTimestamp,
    configuration::{ApplicationConfiguration, LogBackend, TargetConfiguration},
    log_source::{LogSource, LogSourceTrait, RttSource, UartSource},
};
use elf::{ElfBytes, endian::AnyEndian};
use probe_rs::probe::{DebugProbeInfo, list::Lister};
use probe_rs::flashing;
use ratatui::style::{Modifier, Style};
use tracing::{debug, error, info, warn};

mod file_io;
mod source_handler;
mod user_commands;
mod filter_handler;
pub use user_commands::{find_log, stream_file, stream_start, stream_stop};
pub use filter_handler::add_filter;

pub struct Commander {
    /// Connected target information
    log_sources: Vec<LogSource>,

    /// New sources IDs will just increase this number
    log_source_id: u32,

    /// Log filtering feature
    filters: Vec<LogFilter>,

    /// All received log messages
    log_messages: Vec<LogMessage>,

    /// Target configuration (from .gadget.yaml)
    pub target_cfg: Option<TargetConfiguration>,

    /// Application configuration (from ~/.config/uberlog/config.yaml)
    pub app_cfg: ApplicationConfiguration,

    /// Log streaming information
    pub stream_logs: bool,
    pub stream_logs_file_handle: Option<std::fs::File>,

    /// Command input
    pub command_rx: Receiver<Command>,

    /// Command output (provided to threads created by Commander)
    pub command_tx: Sender<Command>,

    /// Command response output (to send response to the TUI)
    pub command_response_tx: Sender<UiCommand>,

    /// Channel TX for sending parsed log messages
    pub log_message_tx: Sender<LogMessage>,
}

pub enum Command {
    // File
    StreamFile(String),
    StreamStdin,
    StreamLogs(bool, String),

    // LogSources
    ConnectLogSource(u32),
    DisconnectLogSource(u32),

    // Probes
    RefreshProbeInfo,
    Reset(u32),
    Reflash(u32),

    // Misc
    PrintMessage(String),

    // Filters
    AddFilter(LogFilter),
    ClearFilters,
    GetFilters,

    // Logs
    ParseLogBytes(u32, Vec<u8>),
    ClearLogs,
    FindLog(String),
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Command::ClearLogs => "ClearLogs",
            Command::GetFilters => "GetFilters",
            Command::ParseLogBytes(_, _) => "ParseLogBytes",
            Command::ClearFilters => "ClearFilters",
            Command::Reset(_) => "Reset",
            Command::Reflash(_) => "Reflash",
            Command::AddFilter(_) => "AddFilter",
            Command::PrintMessage(_) => "PrintMessage",
            Command::FindLog(_) => "FindLog",
            Command::RefreshProbeInfo => "RefreshProbeInfo",
            Command::StreamLogs(_, _) => "StreamLogs",
            Command::StreamFile(_) => "StreamFile",
            Command::StreamStdin => "StreamStdin",
            Command::ConnectLogSource(_) => "ConnectLogSource",
            Command::DisconnectLogSource(_) => "DisconnectLogSource",
        };
        write!(f, "{}", text)
    }
}

pub enum UiCommand {
    /// Misc
    TextMessage {
        message: String,
    },

    /// Sources
    AddNewSource(u32 /* ID */, String /* Text to display */),
    RemoveSource(u32 /* ID */),
    SetConnectionSource(u32 /* ID */, bool /* Is connected */),

    /// Filters
    UpdateFilterList(Vec<LogFilter>),
    UpdateLogs(Vec<LogMessage>),

    /// Log search
    UpdateSearchLog(String),
}

impl fmt::Display for UiCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            UiCommand::TextMessage { message: _ } => "TextMessage",
            UiCommand::AddNewSource(_, _) => "AddNewSource",
            UiCommand::SetConnectionSource(_, _) => "SetConnectionSource",
            UiCommand::UpdateFilterList(_) => "UpdateFilterList",
            UiCommand::UpdateLogs(_) => "UpdateLogs",
            UiCommand::UpdateSearchLog(_) => "UpdateSearchLog",
            UiCommand::RemoveSource(_) => "RemoveSource",
        };
        write!(f, "{}", text)
    }
}

/// Read-only information for log backend
///
/// This enum used in TargetInformation to provide details about the kind of logging backend
/// the target uses.
#[derive(Clone)]
pub enum LogBackendInformation {
    Rtt(u64),
    Uart(String, u32),
}

/// This class holds the whole state of a target MCU
///
/// Entrypoint to operating with an MCU from other parts of the code, one per
/// target connected to the system.
#[derive(Clone)]
pub struct TargetMcu {
    /// Name of the target, coming from a configuration file
    pub name: String,

    /// State of the debug probe attached to the target
    pub probe_info: DebugProbeInfo,

    /// MCU name, must be compatible with probe-rs
    pub mcu: String,

    /// Details about the log backend used by the target
    pub backend: LogBackendInformation,
}

impl Commander {
    /// Create a new Commander
    ///
    /// Intended to be used in the beginning of the aplication, to create the single commander that
    /// will handle all the connected probes and related targets
    pub fn new(
        command_tx: Sender<Command>,
        command_rx: Receiver<Command>,
        command_response_tx: Sender<UiCommand>,
        rtt_tx: Sender<LogMessage>,
        cfg: Option<TargetConfiguration>,
        app_cfg: &ApplicationConfiguration,
    ) -> Commander {
        let mut ret = Commander {
            log_sources: Vec::new(),
            log_source_id: 0,
            filters: Vec::new(),
            log_messages: Vec::new(),
            target_cfg: cfg,
            app_cfg: app_cfg.clone(),
            command_rx,
            command_tx,
            command_response_tx,
            log_message_tx: rtt_tx,
            stream_logs: false,
            stream_logs_file_handle: None,
        };
        let _ = ret.cmd_refresh_probe_info();
        ret
    }

    /// Process incoming commands
    ///
    /// Core of this module, this function is designed in a way that a thread is to be calling it periodically
    /// forever. It will block waiting for commands and then process them as required.
    pub fn process(&mut self) -> Result<(), String> {
        if let Ok(response) = self.command_rx.recv() {
            debug!("Processing {}", &response);
            match response {
                Command::RefreshProbeInfo => {
                    return self.cmd_refresh_probe_info();
                }
                Command::Reset(source_id) => {
                    return self.reset_log_source(source_id);
                }
                Command::Reflash(source_id) => {
                    return self.reflash_log_source(source_id);
                }
                Command::StreamLogs(streaming, path) => {
                    return self.cmd_log_stream(streaming, path);
                }
                Command::ParseLogBytes(id, bytes) => {
                    return self.cmd_parse_bytes(id, bytes);
                }
                Command::StreamFile(path) => {
                    return self.cmd_stream_file(path);
                }
                Command::StreamStdin => {
                    return self.cmd_stream_stdin();
                }
                Command::PrintMessage(msg) => {
                    let _ = self
                        .command_response_tx
                        .send(UiCommand::TextMessage { message: msg });
                }
                Command::AddFilter(filter) => {
                    return self.add_filter(filter);
                }
                Command::ClearFilters => {
                    return self.clear_filters();
                }
                Command::GetFilters => {
                    let _ = self
                        .command_response_tx
                        .send(UiCommand::UpdateFilterList(self.filters.clone()));
                }
                Command::ClearLogs => {
                    return self.clear_logs();
                }
                Command::FindLog(log) => {
                    return self.update_log_search(log);
                }
                Command::ConnectLogSource(id) => {
                    return self.connect_log_source(id);
                }
                Command::DisconnectLogSource(id) => {
                    return self.disconnect_log_source(id);
                }
            }
        } else {
            error!("Channel broke, stop further processing");
            return Err(String::from("channel broken"));
        }
        Ok(())
    }

    /// Change the log being searched for
    fn update_log_search(&self, log: String) -> Result<(), String> {
        let _ = self
            .command_response_tx
            .send(UiCommand::UpdateSearchLog(log));
        Ok(())
    }

    /// Clear logs
    ///
    /// Remove all stored logs and request a clear also to the UI
    fn clear_logs(&mut self) -> Result<(), String> {
        self.log_messages.clear();
        let _ = self
            .command_response_tx
            .send(UiCommand::UpdateLogs(Vec::new()));
        Ok(())
    }


    /// Process received bytes from the different log sources
    /// 
    /// Low level function, it's purpose is to receive raw bytes from all the log sources and
    /// process them into log messages (strings). It also applies all the defined filters and
    /// let's the UI know that a new message has been received.
    fn cmd_parse_bytes(&mut self, id: u32, bytes: Vec<u8>) -> Result<(), String> {
        // Get current bytes
        let idx = match self.get_source_idx(id) {
            Some(idx) => idx,
            None => {
                error!("Source has no RAM state!");
                return Err("Source has no RAM state".to_string());
            }
        };

        let mut log_bytes = match self.log_sources[idx].take_storage() {
            Some(bytes) => bytes,
            None => Vec::new(),
        };

        // Append new bytes
        let bytes_len = bytes.len();
        log_bytes.extend(bytes);

        // Remove zeros
        log_bytes.retain(|&b| b != 0);

        debug!(
            "Received {} bytes. Current storage state:\n{:?}",
            bytes_len, log_bytes
        );

        // Get timestamp
        let ts = LogTimestamp::now();

        let mut count = 0;

        // Split at every newline
        for raw_line in log_bytes.split_inclusive(|&c| c == b'\n') {
            // If current line does not contain '\n' do not process it (is incomplete)
            if !raw_line.contains(&b'\n') {
                break;
            }

            // Update count of used-up bytes
            count = count + raw_line.len();

            debug!("Bytes:\n{:?}", raw_line);

            // Clean it up
            //let raw_line_clean = strip_ansi_escapes::strip(&raw_line);
            //debug!("Bytes clean:\n{:?}", raw_line_clean);

            /*
            let line = match std::str::from_utf8(&raw_line) {
                Ok(v) => v.to_string(),
                Err(e) => {
                    error!("Invalid UTF-8 sequence: {}", e);
                    return Err("Invalid characters".to_string());
                }
            };
            */
            let line = String::from_utf8_lossy(raw_line).to_string();

            debug!("Line: {}", &line);

            // Store it
            self.log_messages.push(LogMessage {
                timestamp: ts,
                source_id: id as i32,
                message: line.clone(),
                style: Style::default().add_modifier(Modifier::DIM),
            });

            // If we are streaming logs to a file, add the line to it
            if let Some(handle) = &mut self.stream_logs_file_handle {
                if !self.stream_logs {
                    error!("Handle not null even though streaming is disabled!!");
                }

                let _ = handle.write_all(line.as_bytes());
            }

            // Apply filters
            if let Some(log_message) = self.apply_filters(ts, id as i32, line.to_string()) {
                let _ = self.log_message_tx.send(log_message);
            }
        }

        // Let's try to be as ineficient as possible
        let (_, b) = log_bytes.split_at(count);
        self.log_sources[idx].set_storage(Vec::from(b));

        Ok(())
    }

    /// Implementation for Command::GetProbes
    ///
    /// Reinitialize all the probe/target information and use it to generate a vector of `TargetInformation`, which
    /// is later sent via a mpsc channel to the entity that queried it
    /// Self reveiew: If this was better it would be nasty, currently is just... welp.
    fn cmd_refresh_probe_info(&mut self) -> Result<(), String> {
        info!("Refresh probe information");

        if self.target_cfg.is_none() {
            let _ = self.command_response_tx.send(UiCommand::TextMessage { message: "No .gadget.yaml file provided".to_string() });
            return Ok(());
        }

        // Add new probes
        let lister = Lister::new();
        for probe in lister.list_all() {
            // More IDs than needed will be generated.
            let id = self.get_new_source_id();

            if let Some(target) = self
                .target_cfg.as_ref().unwrap()
                .targets
                .iter()
                .filter(|t| t.probe_id == *probe.serial_number.as_ref().unwrap())
                .next()
            {
                // Get current list of probe serial ids
                let mut current_serials: Vec<String> = Vec::new();
                for source in &mut self.log_sources {
                    match source {
                        LogSource::RttSource(s) => {
                            current_serials.push(s.get_probe_state().serial_number.clone().unwrap());
                        },
                        LogSource::UartSource(s) => {
                            current_serials.push(s.get_probe_state().serial_number.clone().unwrap());
                        }
                        _ => (),
                    }
                }

                // If new one is already present, skip further steps
                if let Some(probe_serial) = &probe.serial_number {
                    if current_serials.contains(&probe_serial) {
                        continue;
                    }
                }

                let new_target = TargetMcu {
                    name: target.name.clone(),
                    mcu: target.processor.clone(),
                    probe_info: probe.clone(),
                    backend: match &target.log_backend {
                        LogBackend::Rtt { elf_path } => {
                            LogBackendInformation::Rtt(Commander::rtt_block_from_elf(elf_path)?)
                        }
                        LogBackend::Uart { dev, baud } => {
                            LogBackendInformation::Uart(dev.clone(), *baud)
                        }
                    },
                };

                // Also add the log source
                match &target.log_backend {
                    LogBackend::Rtt { elf_path: _ } => {
                        // Create the log source
                        let new_source = RttSource::new(id, new_target, self.command_tx.clone());
                        // Store it
                        self.log_sources.push(LogSource::RttSource(new_source));
                    }
                    LogBackend::Uart { dev: _, baud: _ } => {
                        // Create the log source
                        let new_source = UartSource::new(id, new_target, self.command_tx.clone());
                        // Store it
                        self.log_sources.push(LogSource::UartSource(new_source));
                    }
                }
                // Let UI know of the change
                let _ = self.command_response_tx.send(UiCommand::AddNewSource(
                    id,
                    self.log_sources.last().unwrap().id_string(),
                ));
            }
        }

        // Get current available probe serials
        let available_probes_serials: Vec<String> = lister
            .list_all()
            .iter()
            .map(|t| t.serial_number.clone().expect("No serial!"))
            .collect();

        // And remove the log sources that are not available anymore
        for i in 0..self.log_sources.len() {
            let keep_source = match &mut self.log_sources[i] {
                LogSource::FileSource(_) => true,
                LogSource::StdinSource(_) => true,
                LogSource::RttSource(s) => {
                    available_probes_serials.contains(s.get_probe_state().serial_number.as_ref().unwrap())
                },
                LogSource::UartSource(s) => {
                    available_probes_serials.contains(s.get_probe_state().serial_number.as_ref().unwrap())
                }
            };

            if !keep_source {
                self.log_sources[i].disconnect();

                let _ = self
                    .command_response_tx
                    .send(UiCommand::RemoveSource(self.log_sources[i].id()));
            }
        }


        Ok(())
    }

    /// For RTT targets, parse the elf file and get the RTT address
    /// fn cmd_disconnect(&mut self, probe_serial: String) -> Result<(), String> {
    fn rtt_block_from_elf(path: &String) -> Result<u64, String> {
        let path = std::path::PathBuf::from(path);
        let file_data = std::fs::read(path).unwrap();

        let slice = file_data.as_slice();
        let file = ElfBytes::<AnyEndian>::minimal_parse(slice).unwrap();

        let (symtab, strtab) = file
            .symbol_table()
            .expect("Failed to read symbol table")
            .expect("Failed to find symbol table");

        // Does not seem to be possible to use fancy functions with iterators, so old school

        for symbol in symtab {
            let strtab_idx = symbol.st_name as usize;
            if strtab_idx != 0 {
                match strtab.get(strtab_idx) {
                    Err(_) => (),
                    Ok(symb_name) => {
                        if symb_name == "_SEGGER_RTT" {
                            return Ok(symbol.st_value);
                        }
                    }
                }
            }
        }

        error!("Unable to find _SEGGER_RTT symbol in elf file");
        Err(String::new())
    }

}
