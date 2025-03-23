use tracing::{error, info, warn, debug};

use crate::commander::{Command, CommandResponse};

use super::log_source::LogSource;

use core::time;
use std::{io::{BufRead, BufReader}, path::PathBuf, sync::mpsc::Sender, thread::{self, JoinHandle}};


pub struct FileSource {

    /// Handle of the thread reading data
    handle: Option<JoinHandle<()>>,

    /// Send channel to gracefully shutdown the thread
    thread_control_tx: Option<Sender<bool>>,
    
    /// Send channel to Commander 
    command_tx: Sender<Command>,

    /// Holds state
    is_connected: bool,

    /// File that is opened
    file_name: String,
}

impl FileSource {
    pub fn new(file_name: String, command_tx: Sender<Command>) -> FileSource {
        FileSource {
            handle: None,
            thread_control_tx: None,
            command_tx,
            file_name,
            is_connected: false
        }
    }
}

impl LogSource for FileSource {
    fn connect(&mut self) {

        // Validate status
        if self.is_connected {
            warn!("Already connected!");
            return;
        }

        // TODO: check the rest of the state, should be None

        // Try to open the file
        info!("Open file");
        let file_path = PathBuf::from(&self.file_name); 
        if !file_path.exists() {
            error!("File {} does not exist", self.file_name);
            let _ = self.command_tx.send(Command::PrintMessage( format!("`{}` does not exist", self.file_name) ));
        }
        let file = std::fs::File::open(file_path).expect("Something real wrong happened");
        let mut buffered_reader = BufReader::new(file);

        // Populate thread control channel 
        let (control_tx, control_rx) = std::sync::mpsc::channel();
        self.thread_control_tx = Some(control_tx);
        
        // Provide a command_tx copy to the thread
        let command_tx = self.command_tx.clone();

        let _ = command_tx.send(Command::PrintMessage( format!("Streaming from `{}`", self.file_name) ));

        // Define the thread
        let handle = std::thread::spawn(move || {

            info!("Thread started - FileSource");

            loop {
                // Check no message was received
                if let Ok(response) = control_rx.try_recv() {
                    if !response {
                        info!("Stop streaming thread");
                        break;
                    }
                }

                // Fill vector 
                loop {
                    let mut out_bytes = Vec::new();
                    match buffered_reader.read_until(0xA, &mut out_bytes) {
                        Ok(nbytes) => {
                            if nbytes > 0 {
                                // Send the message
                                debug!("Sending: <-- {:?} -->", out_bytes);
                                match command_tx.send(Command::ParseLogBytes(Vec::from(out_bytes))) {
                                    Ok(_) => (),
                                    Err(e) => {
                                        error!("Send error: {}", e);
                                        let _ = command_tx.send(Command::PrintMessage("Internal error!!".to_string()));
                                        continue;
                                    }
                                }
                            } else {
                                break;
                            }

                        },
                        Err(e) => {
                            error!("File read error <{}>", e);
                            break;
                        }
                    }
                }

                thread::sleep(time::Duration::from_millis(100));
            }
        });
        self.handle = Some(handle);
    }

    fn disconnect() {}
}
