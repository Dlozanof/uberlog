use core::time;
use std::{fs::read_to_string, io::Write, path::PathBuf, sync::mpsc::{Receiver, Sender}, thread::{self, JoinHandle}};

use elf::{endian::AnyEndian, ElfBytes};
use probe_rs::{probe::{list::Lister, DebugProbeInfo}, rtt::{Rtt, ScanRegion}, Permissions};
use ratatui::style::{self, Modifier, Style};
use tracing::{debug, error, info};
use crate::{configuration::{ApplicationConfiguration, LogBackend, TargetConfiguration}, log_source::{file_source::FileSource, log_source::LogSource}, LogFilter, LogFilterType, LogMessage, LogTimestamp};

/// In charge of providing a place where to store not-fully-processed logs
///
/// Since bytes come in chunks that do not have to contain a full string, partial
/// storage is needed
pub struct RawLogStorageManager {
    storage: Vec<Vec<u8>>
}

impl RawLogStorageManager {
    pub fn new() -> RawLogStorageManager {
        RawLogStorageManager{
            storage: Vec::new()
        }
    }

    pub fn take_bytes(&mut self, id: i32) -> Option<Vec<u8>> {
        let id = id as usize;
        if id >= self.storage.len() {
            error!("Storage for {} does not exist", id);
            return None;
        }

        Some(std::mem::take(&mut self.storage[id]))
    }

    pub fn set_bytes(&mut self, id: i32, bytes: Vec<u8>) {
        let id = id as usize;
        if id >= self.storage.len() {
            error!("Storage for {} does not exist", id);
            return;
        }

        self.storage[id] = bytes;
    }

    pub fn alloc_new_vector(&mut self) -> i32 {
        let ret = self.storage.len();
        self.storage.push(Vec::new());
        ret as i32
    }
}

pub struct Commander {

    /// Connected target information
    pub probes: Vec<TargetMcu>,

    /// Log filtering feature
    filters: Vec<LogFilter>,
    logs_raw: Vec<LogMessage>,

    /// Configuration
    pub target_cfg: TargetConfiguration,
    pub app_cfg: ApplicationConfiguration,

    /// Log streaming information
    pub stream_logs: bool,
    pub stream_logs_file_handle: Option<std::fs::File>,

    // Temporary log storage
    pub log_storage: RawLogStorageManager,

    /// Command input
    pub command_rx: Receiver<Command>,
    /// Command output (provided to thread creates by Commander)
    pub command_tx: Sender<Command>,

    /// Command response output (to send response to other modules)
    pub command_response_tx: Sender<CommandResponse>,

    // Store the rtt_tx channel for cloning purposes, will not use it directly
    pub rtt_tx: Sender<LogMessage>
}

pub enum Command {

    // File
    OpenFile(std::path::PathBuf),
    StreamFile(String),
    StreamLogs(bool, String),

    // Probes
    GetProbes,
    Connect(TargetInformation),
    Disconnect(String),
    Delete(String),
    Reset(String),
    ParseLogBytes(i32, Vec<u8>),

    // Misc
    PrintMessage(String),

    // Filters
    AddFilter(LogFilter),
    ClearFilters,
    GetFilters,

    // Logs
    ClearLogs,
    FindLog(String),
}


pub enum CommandResponse {
    /// Misc
    TextMessage{message: String},

    /// Probes
    ProbeInformation{probes: Vec<TargetInformation>},
    ProbeConnected(bool),

    /// Filters
    UpdateFilterList(Vec<LogFilter>),
    UpdateLogs(Vec<LogMessage>),

    /// Log search
    UpdateSearchLog(String),
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

/// Read only information of a Target
/// 
/// `Commander` holds some non-shareable attributes of the targets, but others like
/// the target name/type must be exchanged with the UI thread to properly display the
/// information. This class purpose is exactly that.
#[derive(Clone)]
pub struct TargetInformation {
    pub probe_name: String,
    pub probe_serial: String,
    pub mcu: String,
    pub backend: LogBackendInformation,
    pub is_connected: bool,
}

impl TargetInformation {
    fn from(mcu: &TargetMcu) -> TargetInformation {
        TargetInformation {
            probe_name: format!("{} ({})", mcu.name, mcu.probe_info.identifier),
            probe_serial: mcu.probe_info.serial_number.clone().unwrap_or("Unknown".to_string()),
            mcu: mcu.mcu.clone(),
            backend: mcu.backend.clone(),
            is_connected: mcu.is_connected,
        }
    }
}



/// This class holds the whole state of a target MCU
/// 
/// Entrypoint to operating with an MCU from other parts of the code, one per
/// target connected to the system.
pub struct TargetMcu {
    /// Name of the target, coming from a configuration file
    pub name: String,
    /// State of the debug probe attached to the target
    pub probe_info: DebugProbeInfo,
    /// MCU name, must be compatible with probe-rs
    pub mcu: String,
    /// Store connection status
    pub is_connected: bool,

    /// Details about the log backend used by the target
    pub backend: LogBackendInformation,

    pub log_thread: Option<JoinHandle<()>>,

    pub log_thread_control_tx: Option<Sender<bool>>,
}




impl Commander {
    /// Create a new Commander
    /// 
    /// Intended to be used in the beginning of the aplication, to create the single commander that
    /// will handle all the connected probes and related targets
    pub fn new(command_tx: Sender<Command>, command_rx: Receiver<Command>, command_response_tx: Sender<CommandResponse>, rtt_tx: Sender<LogMessage>, cfg: TargetConfiguration, app_cfg: &ApplicationConfiguration) -> Commander {
        let mut ret = Commander {
            probes: Vec::new(),
            filters: Vec::new(),
            logs_raw: Vec::new(),
            target_cfg: cfg,
            app_cfg: app_cfg.clone(),
            command_rx,
            command_tx,
            command_response_tx,
            rtt_tx,
            stream_logs: false,
            stream_logs_file_handle: None,
            log_storage: RawLogStorageManager::new(),
        };
        let _ = ret.init();
        ret
    }

    /// Open log file
    /// 
    /// Open a file with text data and parse it
    fn cmd_open_file(&mut self, _path: PathBuf) -> Result<(), String> {

        //if let Ok(content) = read_to_string(path) {
        //    for line in content.lines() {
        //        // Store it
        //        self.logs_raw.push(line.to_string());
        //        
        //        // Apply filters
        //        if let Some(log_message) = self.apply_filters(line.to_string()) {
        //            let _ = self.rtt_tx.send(log_message);
        //        }
        //    }
        //}

        Ok(())
    }

    /// Stream file 
    fn cmd_stream_file(&mut self, path: String) -> Result<(), String> {

        let id = self.log_storage.alloc_new_vector();
        let mut log_source = FileSource::new(id, path.clone(), self.command_tx.clone());
        log_source.connect();

        Ok(())
    }

    /// Configure log streaming
    /// 
    /// Receive a status update and a path where to stream
    fn cmd_log_stream(&mut self, streaming: bool, path: String) -> Result<(), String> {
        if streaming {
            // Make sure we were not streaming already
            if self.stream_logs {
                error!("Already streaming!");
                return Err("Already streaming".to_string());
            }

            // Otherwise open file
            if let Ok(mut p) = std::fs::File::create(&path) {
                for log in &self.logs_raw {
                    let _ = p.write_all(log.message.as_bytes());
                }
                self.stream_logs_file_handle = Some(p);
                self.stream_logs = true;
                let _ = self.command_response_tx.send(CommandResponse::TextMessage { message: format!("Saved/streaming data into <{}>", path) });
            }
        } else {
            // Make sure we were not -not- streaming already
            if !self.stream_logs {
                error!("Nothing to do, really");
                return Err("Nothing to do, really".to_string());
            }

            // Otherwise close file
            self.stream_logs_file_handle = None;
            self.stream_logs = false;

            let _ = self.command_response_tx.send(CommandResponse::TextMessage { message: "Streaming stopped".to_string() });
        }
        
        Ok(())
    }

    /// Process incoming commands
    /// 
    /// Core of this module, this function is designed in a way that a thread is to be calling it periodically
    /// forever. It will block waiting for commands and then process them as required.
    pub fn process(&mut self) -> Result<(), String> {
        if let Ok(response) = self.command_rx.recv() {
            debug!("Processing {}", command_to_string(&response));
            match response {
                Command::GetProbes => {
                    return self.cmd_get_probes();
                }
                Command::Disconnect(probe_serial) => {
                    return self.cmd_disconnect(probe_serial);
                },
                Command::Delete(probe_serial) => {
                    return self.cmd_delete(probe_serial);
                },
                Command::Connect(probe_details) => {
                    return self.cmd_connect(probe_details);
                }
                Command::Reset(probe_serial) => {
                    return self.cmd_reset(probe_serial);
                },
                Command::StreamLogs(streaming, path) => {
                    return self.cmd_log_stream(streaming, path);
                }
                Command::ParseLogBytes(id, bytes) => {
                    return self.cmd_parse_bytes(id, bytes);
                }
                Command::OpenFile(path) => {
                    return self.cmd_open_file(path);
                }
                Command::StreamFile(path) => {
                    return self.cmd_stream_file(path);
                }
                Command::PrintMessage(msg) => {
                    let _ = self.command_response_tx.send(CommandResponse::TextMessage { message: msg });
                }
                Command::AddFilter(filter) => {
                    return self.add_filter(filter);
                }
                Command::ClearFilters => {
                    return self.clear_filters();
                }
                Command::GetFilters => {
                    let _ = self.command_response_tx.send(CommandResponse::UpdateFilterList(self.filters.clone()));
                }
                Command::ClearLogs => {
                    return self.clear_logs();
                }
                Command::FindLog(log) => {
                    return self.update_log_search(log);
                }
            }
        }
        else {
            error!("Channel broke, stop further processing");
            return Err(String::from("channel broken"));
        }
        Ok(())
    }

    /// Change the log being searched for
    fn update_log_search(&self, log: String) -> Result<(), String> {
        let _ = self.command_response_tx.send(CommandResponse::UpdateSearchLog(log));
        Ok(())
    }

    /// Clear logs
    /// 
    /// Remove all stored logs and request a clear also to the UI
    fn clear_logs(&mut self) -> Result<(), String> {
        self.logs_raw.clear();
        let _ = self.command_response_tx.send(CommandResponse::UpdateLogs(Vec::new()));
        Ok(())
    }

    /// Clear filters
    /// 
    /// Clear the available filters, and reprocess the log messages
    fn clear_filters(&mut self) -> Result<(), String> {
        
        // Clear filters
        self.filters.clear();

        // Empty current log list
        let filtered_messages: Vec<LogMessage> = self.logs_raw
            .iter()
            .map(|msg| self.apply_filters(msg.timestamp, msg.source_id, msg.message.to_string()))
            .map(|msg| msg.unwrap())
            .collect();

        let _ = self.command_response_tx.send(CommandResponse::UpdateLogs(filtered_messages));
        Ok(())
    }

    /// Add a new filter
    /// 
    /// Not only store the new filter, but also regenerate the filtered log list and send it to the
    /// application so it can update the log view
    fn add_filter(&mut self, filter: LogFilter) -> Result<(), String> {
        
        // Add new filter
        self.filters.push(filter.clone());

        // Empty current log list
        let filtered_messages: Vec<LogMessage> = self.logs_raw
            .iter()
            .map(|msg| self.apply_filters(msg.timestamp, msg.source_id, msg.message.to_string()))
            .filter(|msg| msg.is_some())
            .map(|msg| msg.unwrap())
            .collect();

        let _ = self.command_response_tx.send(CommandResponse::UpdateLogs(filtered_messages));
        let _ = self.command_response_tx.send(CommandResponse::TextMessage { message: format!("Added: {:?}", filter) });

        Ok(())
    }

    fn cmd_parse_bytes(&mut self, id: i32, bytes: Vec<u8>) -> Result<(), String> {
        debug!("Received <-- {:?} -->", bytes);

        // Get current bytes
        let mut log_bytes = match self.log_storage.take_bytes(id) {
            Some(bytes) => bytes,
            None => {
                error!("Wrong storage ID: {}", id);
                return Err("Wrong storage ID".to_string());
            }
        };

        log_bytes.extend(bytes);

        // Remove ansi colors...
        log_bytes = strip_ansi_escapes::strip(&log_bytes);

        // ... and remove strange characters
        log_bytes.retain(|c| c.is_ascii());

        // The remaining part should be utf8, so transform it
        let s = match std::str::from_utf8(&log_bytes) {
            Ok(v) => v,
            Err(e) => {
                error!("Invalid UTF-8 sequence: {}", e);
                return Err("Invalid characters".to_string());
            }
        };

        // Get timestamp
        let ts = LogTimestamp::now();

        let mut count = 0;
        // Iterate over every line
        for line in s.split_inclusive('\n') {
            if !line.contains('\n') {
                break;
            }
            count = count + line.len();

            debug!("Processing line <-- {} -->", line);

            // Store it
            self.logs_raw.push(LogMessage {
                timestamp: ts,
                source_id: id,
                message: line.to_string(),
                style: Style::default().add_modifier(Modifier::DIM),
            });

            // If we are streaming logs to a file, add the line to it
            if let Some(handle) =  &mut self.stream_logs_file_handle {
                if !self.stream_logs {
                    error!("Handle not null even though streaming is disabled!!");
                }

                let _ = handle.write_all(line.as_bytes());
            }

            // Apply filters
            if let Some(log_message) = self.apply_filters(ts, id, line.to_string()) {
                let _ = self.rtt_tx.send(log_message);
            }
        }

        // Let's try to be as ineficient as possible
        let (_, b) = log_bytes.split_at(count);
        self.log_storage.set_bytes(id, Vec::from(b));

        Ok(())
    }

    /// Reset target
    /// 
    /// Issue a reset on the MCU connected to the indicated probe
    fn cmd_reset(&mut self, probe_serial: String) -> Result<(), String> {
        let probe_info = self.probes.iter_mut().find(|target_mcu| *target_mcu.probe_info.serial_number.as_ref().unwrap() == probe_serial).ok_or("Unable to find probe")?;

        if probe_info.log_thread_control_tx.is_some() {
            return Err(String::from("Target is connected, can't reset"));
        }

        let mut probe = match probe_info.probe_info.open() {
            Err(e) => return Err(format!("{}", e)),
            Ok(val) => val,
        };
        let _ = probe.target_reset();
        Ok(())
    }

    /// Connect to a logging backend
    /// 
    /// Connects to the UART device or RTT channel and starts streaming data
    fn cmd_connect(&mut self, probe_details: TargetInformation) -> Result<(), String> {

        let probe_info: &mut TargetMcu = self.probes.iter_mut().find(|target_mcu| *target_mcu.probe_info.serial_number.as_ref().unwrap() == probe_details.probe_serial).ok_or("Unable to find probe")?;
        
        info!("Connecting to {}", probe_info.probe_info.clone().serial_number.expect("no serial number!"));
        if probe_info.is_connected {
            info!("Probe is already connected");
            return Ok(());
        }
    
        match &probe_info.backend {
            LogBackendInformation::Rtt(_) => return self.cmd_connect_rtt(probe_details),
            LogBackendInformation::Uart(_,_) => return self.cmd_connect_uart(probe_details)
        }
    }

    /// Connect to an UART backend
    /// 
    /// Specific function for connecting to an RTT backend
    fn cmd_connect_uart(&mut self, probe_details: TargetInformation) -> Result<(), String> {
    
        let probe_info: &mut TargetMcu = self.probes.iter_mut().find(|target_mcu| *target_mcu.probe_info.serial_number.as_ref().unwrap() == probe_details.probe_serial).ok_or("Unable to find probe")?;

        let (dev_path, baud) = match &probe_info.backend {
            LogBackendInformation::Uart(path, baud) => (path.to_owned(), baud.to_owned()),
            LogBackendInformation::Rtt(_) => return Err("Something really wrong happened".to_string())
        };

        if probe_info.log_thread_control_tx.is_none() {
            let (log_thread_control_tx, rtt_stream_control_rx) = std::sync::mpsc::channel();
            probe_info.log_thread_control_tx = Some(log_thread_control_tx);

            let tx = self.command_tx.clone();
            let command_response_tx = self.command_response_tx.clone();
            probe_info.is_connected = true;

            // Copy serial to the thread, since it uses to request a Delete of the probe in case
            // of hard error
            let serial_id = probe_info.probe_info.clone().serial_number.expect("No serial");
            let commander_tx = self.command_tx.clone();

            // Actual thread
            let handle = std::thread::spawn(move || {

                info!("Thread started");

                info!("Opening serial port {} with baud rate {}", dev_path, baud);
                let mut port = serialport::new(dev_path, baud)
                    .timeout(std::time::Duration::from_secs(1))
                    .open().expect("Failed to open port");
                info!("Serial port opened");
                let _ = command_response_tx.send(CommandResponse::TextMessage { message: "Connected".to_string() });
                let _ = command_response_tx.send(CommandResponse::ProbeConnected(true));


                loop {
                    // Check no message was received
                    if let Ok(response) = rtt_stream_control_rx.try_recv() {
                        if !response {
                            info!("Stop streaming thread");
                            break;
                        }
                    }

                    // Read as much data as available
                    let mut buf: [u8; 200] = [0; 200];
                    let count = match port.read(buf.as_mut_slice()) {
                        Err(e) => {
                            // Timeout is expected since we are polling
                            if e.kind() == std::io::ErrorKind::TimedOut {
                                continue;
                            }
                            
                            // Broken pipe means the cable was disconnected and an infinite loop
                            // happens, manage it
                            if e.kind() == std::io::ErrorKind::BrokenPipe {
                                error!("Serial port connection error");
                                let _ = commander_tx.send(Command::Disconnect(serial_id.clone()));
                                let _ = commander_tx.send(Command::Delete(serial_id.clone()));
                                let _ = commander_tx.send(Command::PrintMessage( String::from("Serial port connection error") ));
                                break;
                            }

                            // Otherwise report it
                            error!("Port read error: {}", e);
                            let _ = command_response_tx.send(CommandResponse::TextMessage { message: format!("Error reading port {}", e) });
                            
                            continue;
                        },
                        Ok(count) => count,
                    };

                    // If there is data, clean and send it
                    if count > 0 {

                        debug!("Read {} bytes", count);
                        // Take the part with data
                        let (buf, _) = buf.split_at(count);

                        // Send the message
                        debug!("Sending: <-- {:?} -->", buf);
                        match tx.send(Command::ParseLogBytes(0, Vec::from(buf))) {
                            Ok(_) => (),
                            Err(e) => {
                                error!("Send error: {}", e);
                                let _ = command_response_tx.send(CommandResponse::TextMessage { message: "Internal error!!".to_string() });
                                continue;
                            }
                        }
                    }
                    thread::sleep(time::Duration::from_millis(10));
                }
            });
            probe_info.log_thread = Some(handle);
        }
        Ok(())
    }

    /// Connect to an RTT backend
    /// 
    /// Specific function for connecting to an RTT backend
    fn cmd_connect_rtt(&mut self, probe_details: TargetInformation) -> Result<(), String> {

        let probe_info: &mut TargetMcu = self.probes.iter_mut().find(|target_mcu| *target_mcu.probe_info.serial_number.as_ref().unwrap() == probe_details.probe_serial).ok_or("Unable to find probe")?;

        info!("Opening probe...");
        let probe = match probe_info.probe_info.open() {
            Err(e) => return Err(format!("{}", e)),
            Ok(val) => val,
        };

        info!("Session...");
        let mut session = match probe.attach(probe_info.mcu.clone(), Permissions::default()) {
            Err(e) => return Err(format!("{}", e)),
            Ok(val) => val,
        };

        let rtt_address = match probe_info.backend {
            LogBackendInformation::Rtt(addr) => addr,
            LogBackendInformation::Uart(_,_) => return Err("Something really wrong happened".to_string())
        };
        
        if probe_info.log_thread_control_tx.is_none() {
            let (rtt_stream_control_tx, rtt_stream_control_rx) = std::sync::mpsc::channel();
            probe_info.log_thread_control_tx = Some(rtt_stream_control_tx);

            let tx = self.command_tx.clone();
            info!("Converting {}", rtt_address);

            let command_response_tx = self.command_response_tx.clone();
            let handle = std::thread::spawn(move || {

                info!("Thread started");

                let mut core = session.core(0).expect("OOPS");
                info!("Core open");
                // Attach to RTT
                //let mut rtt = match Rtt::attach(&mut core, &memory_map) {
                let mut rtt = match Rtt::attach_region(&mut core,&ScanRegion::Exact(rtt_address)) {
                    Ok(val) => val,
                    Err(e) => {
                        error!("Attach region error: {}", e);
                        return;
                    }
                };
                info!("Region attached");
                info!("There are {} channels", rtt.up_channels().len());

                let _ = command_response_tx.send(CommandResponse::TextMessage { message: "Connected".to_string() });
                let _ = command_response_tx.send(CommandResponse::ProbeConnected(true));
                let input = &mut rtt.up_channels()[0];
                {
                    loop {
                        // Check no message was received
                        if let Ok(response) = rtt_stream_control_rx.try_recv() {
                            if !response {
                                info!("Stop streaming thread");
                                break;
                            }
                        }
                        // Read as much data as available
                        let mut buf: [u8; 200] = [0; 200];
                        let count = match input.read(&mut core, &mut buf) {
                            Ok(val) => val,
                            Err(e) => {
                                error!("RTT read error: {}", e);
                                continue;
                            }
                        };
                        
                        // If there is data, clean and send it
                        if count > 0 {

                            info!("Read {} bytes", count);
                            // Take the part with data
                            let (buf, _) = buf.split_at(count);

                            // Send the message
                            info!("Sending: <-- {:?} -->", buf);
                            match tx.send(Command::ParseLogBytes(0, Vec::from(buf))) {
                                Ok(_) => (),
                                Err(e) => {
                                    error!("Message send error: {}", e);
                                    continue;
                                }
                            }
                        }
                        thread::sleep(time::Duration::from_millis(10));
                    }
                }
            });
            probe_info.log_thread = Some(handle);
        };                    
        Ok(())
    }

    /// Implementation for Command::GetProbes
    /// 
    /// Reinitialize all the probe/target information and use it to generate a vector of `TargetInformation`, which
    /// is later sent via a mpsc channel to the entity that queried it
    /// Self reveiew: If this was better it would be nasty, currently is just... welp.
    fn cmd_get_probes (&mut self) -> Result<(), String> {
        info!("Get probes");

        // If there are no probes, initialize the module. Otherwise update list
        if self.probes.is_empty() {
            let _ = self.init();
        }
        else {

            // Add new probes
            let lister = Lister::new();
            for probe in lister.list_all() {
                if let Some(target) = self.target_cfg.targets.iter().filter(|t| t.probe_id == *probe.serial_number.as_ref().unwrap()).next() {

                    // Get current list of probe serial ids 
                    let current_serials: Vec<String> = self.probes.iter().map(|t| t.probe_info.serial_number.clone().unwrap()).collect();

                    // If new one is already present, skip further steps
                    if let Some(probe_serial) = &probe.serial_number {
                        if current_serials.contains(&probe_serial) {
                            continue;
                        }
                    }

                    // Otherwise add it
                    self.probes.push(TargetMcu {
                            name: target.name.clone(),
                            mcu: target.processor.clone(),
                            probe_info: probe.clone(),
                            backend: match &target.log_backend {
                                LogBackend::Rtt{elf_path} => LogBackendInformation::Rtt(Commander::rtt_block_from_elf(elf_path)?),
                                LogBackend::Uart{dev, baud}  => LogBackendInformation::Uart(dev.clone(), *baud),
                            },
                            log_thread: None,
                            log_thread_control_tx: None,
                            is_connected: false,
                    });
                }
            }

            // Remove non existin probes
            let available_probes_serials: Vec<String> = lister.list_all().iter().map(|t| t.serial_number.clone().expect("No serial!")).collect();
            let missing_probes: Vec<&TargetMcu> = self.probes.iter().filter(|t| !available_probes_serials.contains(&t.probe_info.serial_number.clone().unwrap())).collect();

            for probe in missing_probes {
                info!("Probe missing, cleaning up: {}", probe.probe_info.serial_number.clone().expect("no serial"));
                let _ = self.command_tx.send(Command::Delete(probe.probe_info.serial_number.clone().expect("No serial!")));
            }
        }

        // Create output
        let return_value = CommandResponse::ProbeInformation{
            probes: self.probes.iter().map(|target| TargetInformation::from(target)).collect()
        };

        // Send the message
        let _ = self.command_response_tx.send(return_value);

        Ok(())
    }

    /// Disconnect the logging backend
    /// 
    /// Depending on the used log backend, either disconnect the RTT channel and kill the thread
    /// or close the /dev/ttyXXX port
    fn cmd_disconnect(&mut self, probe_serial: String) -> Result<(), String> {
        let probe_info = self.probes.iter_mut().find(|target_mcu| *target_mcu.probe_info.serial_number.as_ref().unwrap() == probe_serial).ok_or("Unable to find probe")?;

        info!("Disconnecting {}", probe_info.probe_info.clone().serial_number.expect("no serial number!"));
        if !probe_info.is_connected {
            info!("Probe is not connected");
            return Ok(());
        }

        let handle = probe_info.log_thread.take().ok_or("Streaming was not active, or something really bad is happening (thread leaked)")?;
        let channel = probe_info.log_thread_control_tx.take().ok_or("Thread is running but the SPMC channel is down!")?;
        match channel.send(false) {
            Ok(_) => (),
            Err(_) => {
                error!("Channel closed, benign if happens during handling of unexpected disconnection");
            }
        }

        // Wait for the thread to die, and remove the session
        match handle.join() {
            Ok(_) => (),
            Err(e) => return Err(format!("{:?}", e)),
        }

        info!("Disconnected");
        probe_info.is_connected = false;
        let _ = self.command_response_tx.send(CommandResponse::TextMessage { message: "Disconnected".to_string() });
        let _ = self.command_response_tx.send(CommandResponse::ProbeConnected(false));
        Ok(())
    }

    /// Delete a probe
    ///
    /// Used when a probe is not present anymore.
    fn cmd_delete(&mut self, probe_serial: String) -> Result<(), String> {

        // Get index
        let probe_idx = self.probes.iter_mut().position(|target_mcu| *target_mcu.probe_info.serial_number.as_ref().unwrap() == probe_serial).ok_or("Unable to find probe")?;

        // Print information and make sure the probe is disconnected, request a disconnect+delete
        // otherwise
        let probe_info = &self.probes[probe_idx];
        info!("Delete {}", probe_info.probe_info.clone().serial_number.expect("no serial number!"));
        if probe_info.is_connected {
            info!("Probe is still connected, queuing a disconnect and another delete");
            let _ = self.command_tx.send(Command::Disconnect(probe_serial.clone()));
            let _ = self.command_tx.send(Command::Delete(probe_serial));
            return Ok(());
        }

        // Probe is not connected, remove it
        self.probes.remove(probe_idx);

        // Refresh probe list
        let _ = self.command_tx.send(Command::GetProbes);
            
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

    /// Reinitialize the internal probe field
    fn init(&mut self) -> Result<(), String> {
        self.probes.clear();

        // Fill list of probes 
        let lister = Lister::new();
        for probe in lister.list_all() {
            info!("Matching {:?}", probe);

            if let Some(target) = self.target_cfg.targets.iter().filter(|t| t.probe_id == *probe.serial_number.as_ref().unwrap()).next() {
                
            self.probes.push(TargetMcu {
                    name: target.name.clone(),
                    mcu: target.processor.clone(),
                    probe_info: probe.clone(),
                    backend: match &target.log_backend {
                        LogBackend::Rtt{elf_path} => LogBackendInformation::Rtt(Commander::rtt_block_from_elf(elf_path)?),
                        LogBackend::Uart{dev, baud}  => LogBackendInformation::Uart(dev.clone(), *baud),
                    },
                    log_thread: None,
                    log_thread_control_tx: None,
                    is_connected: false,
                });
            }
        }

        self.log_storage.alloc_new_vector(); // Hopefully zero

        Ok(())
    }
    
    /// Apply filters to a log message
    fn apply_filters(&self, timestamp: LogTimestamp, id: i32, log: String) -> Option<LogMessage> {

        let mut log = Some(LogMessage{
            timestamp: timestamp.clone(),
            style: Style::default().add_modifier(Modifier::DIM),
            message: log,
            source_id: id,
        });

        for current_filter in &self.filters {
            if log.is_none() {
                return log;
            }
            match current_filter.kind {
                LogFilterType::Inclusion => {
                    let tmp_log = log.clone().unwrap();
                    let retain_it = tmp_log.message.contains(&current_filter.msg) && !current_filter.msg.is_empty();
                    if retain_it {
                        continue;
                    } else {
                        log = None;
                    }
                },
                LogFilterType::Exclusion => {
                    let tmp_log = log.clone().unwrap();
                    let retain_it = !tmp_log.message.contains(&current_filter.msg) && !current_filter.msg.is_empty();
                    if retain_it {
                        continue;
                    } else {
                        log = None;
                    }
                },
                LogFilterType::Highlighter => {
                    let tmp_log = log.clone().unwrap();
                    let matches_msg = tmp_log.message.contains(&current_filter.msg) && !current_filter.msg.is_empty();
                    if matches_msg {
                        log = Some(LogMessage{
                            timestamp: timestamp.clone(),
                            message: log.unwrap().message,
                            style: current_filter.style,
                            source_id: id,
                        });
                    }
                }
            }
        }
        log
    }
}

/// Utility function to get the enum string
fn command_to_string(cmd: &Command) -> String {
    match cmd {
        Command::ClearLogs => String::from("ClearLogs"),
        Command::GetFilters => String::from("GetFilters"),
        Command::ParseLogBytes(_, _) => String::from("ParseLogBytes"),
        Command::ClearFilters => String::from("ClearFilters"),
        Command::Reset(_) => String::from("Reset"),
        Command::AddFilter(_) => String::from("AddFilter"),
        Command::Disconnect(_) => String::from("Disconnect"),
        Command::PrintMessage(_) => String::from("PrintMessage"),
        Command::Connect(_) => String::from("Connect"),
        Command::FindLog(_) => String::from("FindLog"),
        Command::GetProbes => String::from("GetProbes"),
        Command::OpenFile(_) => String::from("OpenFile"),
        Command::StreamLogs(_, _) => String::from("StreamLogs"),
        Command::Delete(_) => String::from("Delete"),
        Command::StreamFile(_) => String::from("StreamFile"),
    }
}

/// Add filter callback
/// 
/// Add a filter by parsing the `input` field. It has the general form:
/// {h/i/e} (optional)color word
/// 
/// Examples:
///     h red wrn -> add highlight filter (color red) for lines containing "wrn"
///     i tempo -> add inclusion filter for lines containing "tempo"
///     e tempo -> add exclusion filter for lines containing "tempo"
pub fn add_filter(sender: &Sender<Command>, input: Vec<String>) -> Result<(), String> {
    if input.is_empty() {
        return Err(String::from("Filter information missing"));
    }

    if input.len() < 2 {
        return Err(String::from("Wrong arguments. Expected \'/{h,i,e} {color} word\'"));
    }

    let mut idx = 0;

    let kind = match input[idx].chars().next() {
        Some('h') => LogFilterType::Highlighter,
        Some('i') => LogFilterType::Inclusion,
        Some('e') => LogFilterType::Exclusion,
        _ => {
            return Err("Wrong argument".to_owned());
        }
    };
    idx = idx + 1;

    // Inclusion/exclusion do not change color
    let mut color = style::Color::Blue;
    if input.len() == 3 {
        match input[idx].as_str() {
            "red" => color = style::Color::Red,
            "green" => color = style::Color::Green,
            "yellow" => color = style::Color::Yellow,
            "white" => color = style::Color::White,
            "blue" => color = style::Color::Blue,
            "magenta" => color = style::Color::Magenta,
            _ => ()
        }
        idx = idx + 1;
    }

    let filter_style = Style {
        fg: Some(color),
        ..Default::default()
    };

    let _ = sender.send(Command::AddFilter(LogFilter {
        style: filter_style,
        kind,
        msg: input[idx].clone(),
    }));

    Ok(())
}

/// Start streaming into a file
pub fn stream_start(sender: &Sender<Command>, input: Vec<String>) -> Result<(), String> {
    if input.len() != 1 {
        return Err(String::from("Wrong arguments, expected just the path"));
    }
    let _ = sender.send(Command::StreamLogs(true, input[0].clone()));
    Ok(())
}

/// Stop streaming into a file
pub fn stream_stop(sender: &Sender<Command>, input: Vec<String>) -> Result<(), String> {
    if input.len() != 0 {
        return Err(String::from("Too many arguments"));
    }
    let _ = sender.send(Command::StreamLogs(false, String::new()));
    Ok(())
}

/// Stop streaming into a file
pub fn find_log(sender: &Sender<Command>, input: Vec<String>) -> Result<(), String> {
    if input.len() != 1 {
        return Err(String::from("Nothing to search for"));
    }
    let _ = sender.send(Command::FindLog(input[0].clone()));
    Ok(())
}

/// Open a file
pub fn open_file(sender: &Sender<Command>, input: Vec<String>) -> Result<(), String> {
    if input.is_empty() {
        return Err(String::from("path no"));
    }

    if input.len() > 1 {
        return Err(String::from("Too many arguments"));
    }

    let _ = sender.send(Command::OpenFile(PathBuf::from(&input[0])));

    Ok(())
}

/// Stream an input file 
pub fn stream_file(sender: &Sender<Command>, input: Vec<String>) -> Result<(), String> {
    if input.is_empty() {
        return Err(String::from("path no"));
    }

    if input.len() > 1 {
        return Err(String::from("Too many arguments"));
    }

    let _ = sender.send(Command::StreamFile(input[0].clone()));

    Ok(())
}



