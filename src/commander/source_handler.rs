use crate::{commander::UiCommand, log_source::LogSourceTrait};

use super::Commander;

impl Commander {

    /// Get a new log_source ID
    /// 
    /// Pretty basic, just increase internal counter and return the new value, used
    /// to identify a log source.
    pub(crate) fn get_new_source_id(&mut self) -> u32 {
        let ret = self.log_source_id;
        self.log_source_id = self.log_source_id + 1;
        ret
    }

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
            let _ = self
                .command_response_tx
                .send(UiCommand::SetConnectionSource(id, true));
        }

        Ok(())
    }

    /// Disconnect a log source
    ///
    /// Identify the internal log source and disconnect it
    pub(crate) fn disconnect_log_source(&mut self, id: u32) -> Result<(), String> {
        if let Some(idx) = self.get_source_idx(id) {
            self.log_sources[idx].disconnect();
            let _ = self
                .command_response_tx
                .send(UiCommand::SetConnectionSource(id, false));
        }

        Ok(())
    }
}
