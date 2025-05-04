use chrono::Timelike;
use ratatui::style::Style;

pub mod command_parser;
pub mod commander;
pub mod configuration;
pub mod log_source;
pub mod tui;

#[derive(Clone, Copy)]
pub struct LogTimestamp {
    hour: u32,
    minute: u32,
    second: u32,
    ms: u32,
}

impl LogTimestamp {
    /// Get string representation from timestamp
    pub fn to_string(&self) -> String {
        format!(
            "{:02}:{:02}:{:02}.{:03}",
            self.hour, self.minute, self.second, self.ms
        )
    }

    /// Get current timestamp
    pub fn now() -> Self {
        let now = chrono::Local::now();
        Self {
            hour: now.hour(),
            minute: now.minute(),
            second: now.second(),
            ms: now.timestamp_subsec_millis(),
        }
    }

    pub fn second_count(&self) -> u32 {
        self.hour * 3600 + self.minute * 60 + self.second
    }
}

#[derive(Clone)]
pub struct LogMessage {
    pub timestamp: LogTimestamp,
    pub source_id: i32,
    pub message: String,
    pub style: Style,
}

#[derive(Clone, PartialEq, Debug)]
pub enum LogFilterType {
    Exclusion,
    Inclusion,
    Highlighter,
}

#[derive(Clone, Debug)]
pub struct LogFilter {
    pub kind: LogFilterType,
    pub msg: String,
    pub style: Style,
}
