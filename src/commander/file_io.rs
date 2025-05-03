use std::io::Write;

use tracing::error;

use crate::log_source::{FileSource, LogSource, LogSourceTrait};

pub use super::Commander;
use super::UiCommand;

impl Commander {
    /// Stream file
    pub(crate) fn cmd_stream_file(&mut self, path: String) -> Result<(), String> {
        // Get new source ID
        let id = self.get_new_source_id();

        // Create and connect it
        let mut new_source = FileSource::new(id, path.clone(), self.command_tx.clone());
        new_source.connect();

        // Store it
        self.log_sources.push(LogSource::FileSource(new_source));

        // Let UI know of the change
        let _ = self.command_response_tx.send(UiCommand::AddNewSource(
            id,
            self.log_sources.last().unwrap().id_string(),
        ));
        let _ = self
            .command_response_tx
            .send(UiCommand::SetConnectionSource(id, true));

        Ok(())
    }

    /// Configure output log streaming
    ///
    /// Receive a status update and a path where to stream
    pub(crate) fn cmd_log_stream(&mut self, streaming: bool, path: String) -> Result<(), String> {
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
                let _ = self.command_response_tx.send(UiCommand::TextMessage {
                    message: format!("Saved/streaming data into <{}>", path),
                });
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

            let _ = self.command_response_tx.send(UiCommand::TextMessage {
                message: "Streaming stopped".to_string(),
            });
        }

        Ok(())
    }
}
