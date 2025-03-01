use ratatui::style::{self, Style};

pub mod tui;
pub mod commander;
pub mod configuration;
pub mod layout_section;
pub mod command_parser;

#[derive(Clone)]
pub struct LogMessage {
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
