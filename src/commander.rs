use core::time;
use std::{fs::read_to_string, io::Write, mem, path::PathBuf, sync::{mpsc::{Receiver, Sender}, Arc}, thread::{self, JoinHandle}};

use color_eyre::section;
use elf::{endian::AnyEndian, ElfBytes};
use parking_lot::FairMutex;
use probe_rs::{probe::{self, list::Lister, DebugProbeInfo, Probe}, rtt::{Rtt, ScanRegion}, Permissions, Session, Target};
use tracing::{error, info};
use crate::configuration::{Configuration, LogBackend};

pub enum Command {
    GetProbes,
    Connect(TargetInformation),
    Disconnect(String),
    Reset(String),
    ReceiveDrawContext(egui::Context),
    StreamLogs(bool, String),
    ParseLogBytes(Vec<u8>),
    OpenFile(std::path::PathBuf)
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
}

impl TargetInformation {
    fn from(mcu: &TargetMcu) -> TargetInformation {
        TargetInformation {
            probe_name: format!("{} ({})", mcu.name, mcu.probe_info.identifier),
            probe_serial: mcu.probe_info.serial_number.clone().unwrap_or("Unknown".to_string()),
            mcu: mcu.mcu.clone(),
            backend: mcu.backend.clone(),
        }
    }
}


pub enum CommandResponse {
    TextMessage{message: String},
    ProbeInformation{probes: Vec<TargetInformation>},
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

    /// Details about the log backend used by the target
    pub backend: LogBackendInformation,

    pub rtt_thread_handle: Option<JoinHandle<()>>,

    pub rtt_stream_thread_tx: Option<Sender<bool>>,
}

pub struct Commander {

    /// Connected target information
    pub probes: Vec<TargetMcu>,

    /// Egui context for redraw
    pub egui_ctx: Option<egui::Context>,

    /// Configuration
    pub cfg: Configuration,

    /// Log streaming information
    pub stream_logs: bool,
    pub stream_logs_file_handle: Option<std::fs::File>,

    // Temporary log storage
    pub log_raw: Vec<u8>,

    /// Command input
    pub command_rx: Receiver<Command>,
    /// Command output (provided to thread creates by Commander)
    pub command_tx: Sender<Command>,

    /// Command response output (to send response to other modules)
    pub command_response_tx: Sender<CommandResponse>,

    // Store the rtt_tx channel for cloning purposes, will not use it directly
    pub rtt_tx: Sender<String>
}


impl Commander {
    /// Create a new Commander
    /// 
    /// Intended to be used in the beginning of the aplication, to create the single commander that
    /// will handle all the connected probes and related targets
    pub fn new(command_tx: Sender<Command>, command_rx: Receiver<Command>, command_response_tx: Sender<CommandResponse>, rtt_tx: Sender<String>, cfg: Configuration) -> Commander {
        let mut ret = Commander {
            probes: Vec::new(),
            egui_ctx: None,
            cfg,
            command_rx,
            command_tx,
            command_response_tx,
            rtt_tx,
            stream_logs: false,
            stream_logs_file_handle: None,
            log_raw: Vec::new(),
        };
        ret.init();
        ret
    }

    /// Open log file
    /// 
    /// Open a file with text data and parse it
    fn cmd_open_file(&mut self, path: PathBuf) -> Result<(), String> {

        if let Ok(content) = read_to_string(path) {
            for line in content.lines() {
                self.rtt_tx.send(line.to_string());

                if let Some(ectx) = &self.egui_ctx {
                    ectx.request_repaint()
                }
            }
        }

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
            if let Ok(p) = std::fs::File::create(path) {
                self.stream_logs_file_handle = Some(p);
                self.stream_logs = true;
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
        }
        
        Ok(())
    }

    /// Process incoming commands
    /// 
    /// Core of this module, this function is designed in a way that a thread is to be calling it periodically
    /// forever. It will block waiting for commands and then process them as required.
    pub fn process(&mut self) -> Result<(), String> {
        if let Ok(response) = self.command_rx.recv() {
            match response {
                Command::GetProbes => {
                    self.cmd_get_probes();
                }
                Command::Disconnect(probe_serial) => {
                    return self.cmd_disconnect(probe_serial);
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
                Command::ParseLogBytes(bytes) => {
                    return self.cmd_parse_bytes(bytes);
                }
                Command::ReceiveDrawContext(ctx) => {
                    self.egui_ctx = Some(ctx);
                }
                Command::OpenFile(path) => {
                    return self.cmd_open_file(path);
                }
            }
        }
        Ok(())
    }

    fn cmd_parse_bytes(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        info!("Received <-- {:?} -->", bytes);
                
        // TODO: all this should be done over message, not self.log_raw
        self.log_raw.extend(bytes);

        // Remove ansi colors...
        self.log_raw = strip_ansi_escapes::strip(&self.log_raw);

        // ... and remove strange characters
        self.log_raw.retain(|c| c.is_ascii());

        // The remaining part should be utf8, so transform it
        let s = match std::str::from_utf8(&self.log_raw) {
            Ok(v) => v,
            Err(e) => {
                error!("Invalid UTF-8 sequence: {}", e);
                return Err("Invalid characters".to_string());
            }
        };

        let mut count = 0;
        // Iterate over every line
        for line in s.split_inclusive('\n') {
            if !line.contains('\n') {
                break;
            }
            count = count + line.len();

            info!("Processing line <-- {} -->", line);
            self.rtt_tx.send(line.to_string());

            // If we are streaming logs to a file, add the line to it
            if let Some(handle) =  &mut self.stream_logs_file_handle {
                if !self.stream_logs {
                    error!("Handle not null even though streaming is disabled!!");
                }

                handle.write_all(line.as_bytes());
            }

            // And request redraw if the egui context is available (should)
            match &self.egui_ctx {
                Some(ctx) => {
                    info!("Repaint");
                    ctx.request_repaint()
                },
                None => (),
            };
        }

        // Let's try to be as ineficient as possible
        let (_, b) = self.log_raw.split_at(count);
        self.log_raw = Vec::from(b);

        Ok(())
    }

    /// Reset target
    /// 
    /// Issue a reset on the MCU connected to the indicated probe
    fn cmd_reset(&mut self, probe_serial: String) -> Result<(), String> {
        let probe_info = self.probes.iter_mut().find(|target_mcu| *target_mcu.probe_info.serial_number.as_ref().unwrap() == probe_serial).ok_or("Unable to find probe")?;

        if probe_info.rtt_stream_thread_tx.is_some() {
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

        if probe_info.rtt_stream_thread_tx.is_none() {
            let (rtt_stream_control_tx, rtt_stream_control_rx) = std::sync::mpsc::channel();
            probe_info.rtt_stream_thread_tx = Some(rtt_stream_control_tx);

            let tx = self.command_tx.clone();

            // Actual thread
            let handle = std::thread::spawn(move || {

                info!("Thread started");

                info!("Opening serial port {} with baud rate {}", dev_path, baud);
                let mut port = serialport::new(dev_path, baud)
                    .timeout(std::time::Duration::from_secs(3600))
                    .open().expect("Failed to open port");
                info!("Serial port opened");


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
                            error!("{}", e);
                            continue;
                        },
                        Ok(count) => count,
                    };

                    // If there is data, clean and send it
                    if count > 0 {

                        info!("Read {} bytes", count);
                        // Take the part with data
                        let (buf, _) = buf.split_at(count);

                        // Send the message
                        info!("Sending: <-- {:?} -->", buf);
                        match tx.send(Command::ParseLogBytes(Vec::from(buf))) {
                            Ok(_) => (),
                            Err(e) => {
                                error!("{}", e);
                                continue;
                            }
                        }
                    }
                    thread::sleep(time::Duration::from_millis(10));
                }
            });
            probe_info.rtt_thread_handle = Some(handle);
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
        
        if probe_info.rtt_stream_thread_tx.is_none() {
            let (rtt_stream_control_tx, rtt_stream_control_rx) = std::sync::mpsc::channel();
            probe_info.rtt_stream_thread_tx = Some(rtt_stream_control_tx);

            let tx = self.command_tx.clone();
            info!("Converting {}", rtt_address);

            let egui_ctx = self.egui_ctx.clone();

            let handle = std::thread::spawn(move || {

                info!("Thread started");

                let mut core = session.core(0).expect("OOPS");
                info!("Core open");
                // Attach to RTT
                //let mut rtt = match Rtt::attach(&mut core, &memory_map) {
                let mut rtt = match Rtt::attach_region(&mut core,&ScanRegion::Exact(rtt_address)) {
                    Ok(val) => val,
                    Err(e) => {
                        error!("{}", e);
                        return;
                    }
                };
                info!("Region attached");
                info!("There are {} channels", rtt.up_channels().len());

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
                                error!("{}", e);
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
                            match tx.send(Command::ParseLogBytes(Vec::from(buf))) {
                                Ok(_) => (),
                                Err(e) => {
                                    error!("{}", e);
                                    continue;
                                }
                            }
                        }
                        thread::sleep(time::Duration::from_millis(10));
                    }
                }
            });
            probe_info.rtt_thread_handle = Some(handle);
        };                    
        Ok(())
    }


    /// Implementation for Command::GetProbes
    /// 
    /// Reinitialize all the probe/target information and use it to generate a vector of `TargetInformation`, which
    /// is later sent via a mpsc channel to the entity that queried it
    fn cmd_get_probes (&mut self) {
        // Re-initialize
        self.init();

        // Create output
        let return_value = CommandResponse::ProbeInformation{
            probes: self.probes.iter().map(|target| TargetInformation::from(target)).collect()
        };

        // Send the message
        let _ = self.command_response_tx.send(return_value);
    }

    /// Disconnect the logging backend
    /// 
    /// Depending on the used log backend, either disconnect the RTT channel and kill the thread
    /// or close the /dev/ttyXXX port
    fn cmd_disconnect(&mut self, probe_serial: String) -> Result<(), String> {
        let probe_info = self.probes.iter_mut().find(|target_mcu| *target_mcu.probe_info.serial_number.as_ref().unwrap() == probe_serial).ok_or("Unable to find probe")?;

        let handle = probe_info.rtt_thread_handle.take().ok_or("Streaming was not active, or something really bad is happening (thread leaked)")?;
        let channel = probe_info.rtt_stream_thread_tx.take().ok_or("Thread is running but the SPMC channel is down!")?;
        match channel.send(false) {
            Ok(_) => (),
            Err(e) => return Err(format!("{}", e)),
        }

        // Wait for the thread to die, and remove the session
        match handle.join() {
            Ok(_) => (),
            Err(e) => return Err(format!("{:?}", e)),
        }

        info!("Disconnected");
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
                    Err(e) => (),
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

            if let Some(target) = self.cfg.targets.iter().filter(|t| t.probe_id == *probe.serial_number.as_ref().unwrap()).next() {
                
            self.probes.push(TargetMcu {
                    name: target.name.clone(),
                    mcu: target.processor.clone(),
                    probe_info: probe.clone(),
                    backend: match &target.log_backend {
                        LogBackend::Rtt{elf_path} => LogBackendInformation::Rtt(Commander::rtt_block_from_elf(elf_path)?),
                        LogBackend::Uart{dev, baud}  => LogBackendInformation::Uart(dev.clone(), *baud),
                    },
                    rtt_thread_handle: None,
                    rtt_stream_thread_tx: None,
                });
            }
        }

        Ok(())
    }

}
