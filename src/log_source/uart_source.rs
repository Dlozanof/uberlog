use probe_rs::probe::DebugProbeInfo;
use tracing::{error, info, warn, debug};

use crate::commander::{Command, LogBackendInformation, TargetMcu};

use super::LogSourceTrait;

use core::time;
use std::{io::Read, sync::mpsc::Sender, thread::{self, JoinHandle}};


pub struct UartSource {

    /// Handle of the thread reading data
    handle: Option<JoinHandle<()>>,

    /// Send channel to gracefully shutdown the thread
    thread_control_tx: Option<Sender<bool>>,
    
    /// Send channel to Commander 
    command_tx: Sender<Command>,

    /// Holds state
    is_connected: bool,

    // probe-rs state of the connected MCU
    mcu_info: TargetMcu,

    /// Identifier of this source
    id: u32,

    /// Log processing storage
    storage: Option<Vec<u8>>,
}

impl UartSource {
    pub fn new(id: u32, mcu_info: TargetMcu, command_tx: Sender<Command>) -> UartSource {
        UartSource {
            id,
            mcu_info,
            command_tx,
            handle: None,
            thread_control_tx: None,
            is_connected: false,
            storage: None,
        }
    }

    pub fn get_probe_state(&mut self) -> &DebugProbeInfo {
        &self.mcu_info.probe_info
    }
}


impl LogSourceTrait for UartSource {
    fn connect(&mut self) {
        if self.is_connected {
            warn!("Already connected ({})", self.mcu_info.name);
        }

        if self.handle.is_some() || self.thread_control_tx.is_some() {
            warn!("Unexpected state, app thinks this source is disconnected but the thread and/or metadata still exists. Disconnecting for cleanup");
            self.disconnect();
        }

        // Create communication channel for sending data to the thread
        let (tx, rx) = std::sync::mpsc::channel();
        self.thread_control_tx = Some(tx);

        // Update RAM state
        self.is_connected = true;

        // Copy data that is to be used by the thread
        let id = self.id;
        let commander_tx = self.command_tx.clone();
        let thread_rx = rx;

        let (dev_path, baud) = match &self.mcu_info.backend {
            LogBackendInformation::Uart(path, baud) => (path.clone(), *baud),
            _ => {
                error!("UART source with RTT backend");
                return;
            }
        };

        let handle = std::thread::spawn(move || {

            info!("Thread started - UartSource \"{} - {}\"", dev_path, baud);

            let mut port = serialport::new(dev_path, baud)
                .timeout(std::time::Duration::from_secs(1))
                .open().expect("Failed to open port");

            info!("Serial port opened");

            loop {
                // Check no message was received
                if let Ok(response) = thread_rx.try_recv() {
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
                            let _ = commander_tx.send(Command::DisconnectLogSource(id));
                            let _ = commander_tx.send(Command::RefreshProbeInfo);
                            let _ = commander_tx.send(Command::PrintMessage( String::from("Serial port connection error") ));
                            break;
                        }

                        // Otherwise report it
                        error!("Port read error: {}", e);
                        let _ = commander_tx.send(Command::PrintMessage(format!("Error reading port {}", e)));
                        
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
                    match commander_tx.send(Command::ParseLogBytes(id, Vec::from(buf))) {
                        Ok(_) => (),
                        Err(e) => {
                            error!("Send error: {}", e);
                            let _ = commander_tx.send(Command::PrintMessage(String::from("Internal error!")));
                            continue;
                        }
                    }
                }
                thread::sleep(time::Duration::from_millis(10));
            }
        });
        self.handle = Some(handle);
    }

    fn disconnect(&mut self) {
        info!("Disconnecting {}", self.mcu_info.name);

        if let Some(channel) = self.thread_control_tx.take() {
            match channel.send(false) {
                Ok(_) => (),
                Err(e) => error!("{:?}", e),
            }
        } else {
            error!("Thread control channel is None");
        }

        // Wait for the thread to die, and remove the session
        if let Some(t_handle) = self.handle.take() {
            match t_handle.join() {
                Ok(_) => (),
                Err(e) => error!("{:?}", e),
            }            
        } else {
            error!("Thread handle is None");
        }
    }

    fn id_eq(&self, id: u32) -> bool {
        self.id == id
    }
    
    fn id_string(&self) -> String {
        self.mcu_info.name.clone()
    }

    fn take_storage(&mut self) -> Option<Vec<u8>> {
        self.storage.take()
    }

    fn set_storage(&mut self, bytes: Vec<u8>) {
        self.storage = Some(bytes);
    }
}