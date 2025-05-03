use probe_rs::{
    Permissions,
    probe::DebugProbeInfo,
    rtt::{Rtt, ScanRegion},
};
use tracing::{debug, error, info, warn};

use crate::commander::{Command, LogBackendInformation, TargetMcu};

use super::LogSourceTrait;

use core::time;
use std::{
    sync::mpsc::Sender,
    thread::{self, JoinHandle},
};

pub struct RttSource {
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

    /// Name of the target
    target_name: String,
}

impl RttSource {
    pub fn new(
        id: u32,
        mcu_info: TargetMcu,
        command_tx: Sender<Command>,
        target_name: String,
    ) -> RttSource {
        RttSource {
            id,
            mcu_info,
            command_tx,
            target_name,
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

impl LogSourceTrait for RttSource {
    fn connect(&mut self) {
        if self.is_connected {
            warn!("Already connected ({})", &self.mcu_info.name);
        }

        if self.handle.is_some() || self.thread_control_tx.is_some() {
            warn!(
                "Unexpected state, app thinks this source is disconnected but the thread and/or metadata still exists. Disconnecting for cleanup"
            );
            self.disconnect();
        }

        // Update RAM state
        self.is_connected = true;

        // In order to interact with a device using probe-rs a probe/session are needed
        info!("Opening probe...");
        let probe = match self.mcu_info.probe_info.open() {
            Err(e) => {
                error!("{}", e);
                return;
            }
            Ok(val) => val,
        };

        info!("Session...");
        let mut session = match probe.attach(self.mcu_info.mcu.clone(), Permissions::default()) {
            Err(e) => {
                error!("{}", e);
                return;
            }
            Ok(val) => val,
        };
        let rtt_address = match self.mcu_info.backend {
            LogBackendInformation::Rtt(addr) => addr,
            LogBackendInformation::Uart(_, _) => {
                error!("Trying to connect to RTT a target that uses UART");
                return;
            }
        };

        // Create communication channel for sending data to the thread
        let (tx, rx) = std::sync::mpsc::channel();
        self.thread_control_tx = Some(tx);

        // Copy data that is to be used by the thread
        let id = self.id;
        let commander_tx = self.command_tx.clone();
        let thread_rx = rx;
        let source_name = self.id_string();

        let handle = std::thread::spawn(move || {
            info!("Thread started - RttSource \"{}\"", source_name);

            // Create the core
            let mut core = session.core(0).expect("OOPS");
            info!("Core open");

            // Attach to RTT
            let mut rtt = match Rtt::attach_region(&mut core, &ScanRegion::Exact(rtt_address)) {
                Ok(val) => val,
                Err(e) => {
                    error!("Attach region error: {}", e);
                    return;
                }
            };
            info!("Region attached");
            info!("There are {} channels", rtt.up_channels().len());

            let input = &mut rtt.up_channels()[0];
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
                let count = match input.read(&mut core, &mut buf) {
                    Ok(val) => val,
                    Err(e) => {
                        error!("Port read error: {}", e);
                        let _ = commander_tx
                            .send(Command::PrintMessage(format!("Error reading port {}", e)));
                        continue;
                    }
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
                            let _ = commander_tx
                                .send(Command::PrintMessage(String::from("Internal error!")));
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
        info!("Disconnecting {}", self.target_name);

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
        format!("{} (RTT - {})", self.target_name, self.mcu_info.name)
    }

    fn take_storage(&mut self) -> Option<Vec<u8>> {
        self.storage.take()
    }

    fn set_storage(&mut self, bytes: Vec<u8>) {
        self.storage = Some(bytes);
    }
}
