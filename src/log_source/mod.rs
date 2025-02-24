pub mod file_source;
pub mod rtt_source;
pub mod uart_source;

pub trait LogSourceTrait {
    fn connect(&mut self);
    fn disconnect(&mut self);
    fn id_eq(&self, id: u32) -> bool;
    fn id(&self) -> u32;
    fn id_string(&self) -> String;
    fn take_storage(&mut self) -> Option<Vec<u8>>;
    fn set_storage(&mut self, bytes: Vec<u8>);
}

pub enum LogSource {
    FileSource(FileSource),
    UartSource(UartSource),
    RttSource(RttSource),
}

impl LogSourceTrait for LogSource {
    fn connect(&mut self) {
        match self {
            LogSource::FileSource(s) => s.connect(),
            LogSource::UartSource(s) => s.connect(),
            LogSource::RttSource(s) => s.connect(),
        }
    }
    fn disconnect(&mut self) {
        match self {
            LogSource::FileSource(s) => s.disconnect(),
            LogSource::UartSource(s) => s.disconnect(),
            LogSource::RttSource(s) => s.disconnect(),
        }
    }
    fn id_eq(&self, id: u32) -> bool {
        match self {
            LogSource::FileSource(s) => s.id_eq(id),
            LogSource::UartSource(s) => s.id_eq(id),
            LogSource::RttSource(s) => s.id_eq(id),
        }
    }
    fn id(&self) -> u32 {
        match self {
            LogSource::FileSource(s) => s.id(),
            LogSource::UartSource(s) => s.id(),
            LogSource::RttSource(s) => s.id(),
        }
    }
    fn id_string(&self) -> String {
        match self {
            LogSource::FileSource(s) => s.id_string(),
            LogSource::UartSource(s) => s.id_string(),
            LogSource::RttSource(s) => s.id_string(),
        }
    }
    fn take_storage(&mut self) -> Option<Vec<u8>> {
        match self {
            LogSource::FileSource(s) => s.take_storage(),
            LogSource::UartSource(s) => s.take_storage(),
            LogSource::RttSource(s) => s.take_storage(),
        }
    }
    fn set_storage(&mut self, bytes: Vec<u8>) {
        match self {
            LogSource::FileSource(s) => s.set_storage(bytes),
            LogSource::UartSource(s) => s.set_storage(bytes),
            LogSource::RttSource(s) => s.set_storage(bytes),
        }
    }
}

pub use file_source::FileSource;
pub use rtt_source::RttSource;
pub use uart_source::UartSource;
