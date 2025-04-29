

use crate::{commander::UiCommand, log_source::LogSourceTrait};

use super::Commander;


impl Commander {

    /// Given an id return the log source
    /// 
    /// Find the log source with the given ID
    pub(crate) fn get_source_idx(&self, id: u32) -> Option<usize> {
        for (idx, source) in self.log_sources.iter().enumerate() {
            if source.id_eq(id) {
                return Some(idx);
            }
        }
        
        None
    }

    /// Connect a log source
    /// 
    /// Identify the internal log source and connect it
    pub(crate) fn connect_log_source(&mut self, id: u32) -> Result<(), String> {

        if let Some(idx) = self.get_source_idx(id) {
            self.log_sources[idx].connect();
            let _ = self.command_response_tx.send(UiCommand::SetConnectionSource(id, true));
        }

        Ok(())
    }

    /// Disconnect a log source
    /// 
    /// Identify the internal log source and disconnect it
    pub(crate) fn disconnect_log_source(&mut self, id: u32) -> Result<(), String> {

        if let Some(idx) = self.get_source_idx(id) {
            self.log_sources[idx].disconnect();
            let _ = self.command_response_tx.send(UiCommand::SetConnectionSource(id, false));
        }

        Ok(())
    }

}