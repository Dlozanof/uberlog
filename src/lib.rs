use ratatui::style;

pub mod tui;
pub mod commander;
pub mod configuration;
pub mod layout_section;
pub mod command_parser;

#[derive(Clone)]
pub struct LogMessage {
    pub message: String,
    pub color: style::Color,
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
    pub color: style::Color,
}
