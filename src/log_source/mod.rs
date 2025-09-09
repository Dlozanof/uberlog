pub mod file_source;
pub mod rtt_source;
pub mod uart_source;
pub mod stdin_source;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum LogSourceError {
    #[error(transparent)]
    DebugProbeError(#[from] probe_rs::probe::DebugProbeError),
    #[error(transparent)]
    ProbeRsError(#[from] probe_rs::Error),
    #[error(transparent)]
    FlashingError(#[from] probe_rs::flashing::FileDownloadError),
    #[error("This function is not implemented")]
    NotImplemented,
}

pub trait LogSourceTrait {
    fn connect(&mut self);
    fn disconnect(&mut self);
    fn id_eq(&self, id: u32) -> bool;
    fn id(&self) -> u32;
    fn id_string(&self) -> String;
    fn take_storage(&mut self) -> Option<Vec<u8>>;
    fn set_storage(&mut self, bytes: Vec<u8>);
    fn reflash(&self) -> Result<(), LogSourceError>;
}

pub enum LogSource {
    FileSource(FileSource),
    UartSource(UartSource),
    RttSource(RttSource),
    StdinSource(StdinSource),
}

impl LogSourceTrait for LogSource {
    fn connect(&mut self) {
        match self {
            LogSource::FileSource(s) => s.connect(),
            LogSource::UartSource(s) => s.connect(),
            LogSource::RttSource(s) => s.connect(),
            LogSource::StdinSource(s) => s.connect(),
        }
    }
    fn disconnect(&mut self) {
        match self {
            LogSource::FileSource(s) => s.disconnect(),
            LogSource::UartSource(s) => s.disconnect(),
            LogSource::RttSource(s) => s.disconnect(),
            LogSource::StdinSource(s) => s.disconnect(),
        }
    }
    fn id_eq(&self, id: u32) -> bool {
        match self {
            LogSource::FileSource(s) => s.id_eq(id),
            LogSource::UartSource(s) => s.id_eq(id),
            LogSource::RttSource(s) => s.id_eq(id),
            LogSource::StdinSource(s) => s.id_eq(id),
        }
    }
    fn id(&self) -> u32 {
        match self {
            LogSource::FileSource(s) => s.id(),
            LogSource::UartSource(s) => s.id(),
            LogSource::RttSource(s) => s.id(),
            LogSource::StdinSource(s) => s.id(),
        }
    }
    fn id_string(&self) -> String {
        match self {
            LogSource::FileSource(s) => s.id_string(),
            LogSource::UartSource(s) => s.id_string(),
            LogSource::RttSource(s) => s.id_string(),
            LogSource::StdinSource(s) => s.id_string(),
        }
    }
    fn take_storage(&mut self) -> Option<Vec<u8>> {
        match self {
            LogSource::FileSource(s) => s.take_storage(),
            LogSource::UartSource(s) => s.take_storage(),
            LogSource::RttSource(s) => s.take_storage(),
            LogSource::StdinSource(s) => s.take_storage(),
        }
    }
    fn set_storage(&mut self, bytes: Vec<u8>) {
        match self {
            LogSource::FileSource(s) => s.set_storage(bytes),
            LogSource::UartSource(s) => s.set_storage(bytes),
            LogSource::RttSource(s) => s.set_storage(bytes),
            LogSource::StdinSource(s) => s.set_storage(bytes),
        }
    }
    fn reflash(&self) -> Result<(), LogSourceError> {
        match self {
            LogSource::FileSource(s) => Err(LogSourceError::NotImplemented),
            LogSource::UartSource(s) => Err(LogSourceError::NotImplemented),
            LogSource::RttSource(s) => s.reflash(),
            LogSource::StdinSource(s) => Err(LogSourceError::NotImplemented),
        }
    }
}

pub use file_source::FileSource;
pub use rtt_source::RttSource;
pub use uart_source::UartSource;
pub use stdin_source::StdinSource;
