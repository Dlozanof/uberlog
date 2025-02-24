use std::sync::mpsc::Sender;

use ratatui::style::{self, Modifier, Style};
use tracing::debug;

use crate::{commander::UiCommand, LogFilter, LogFilterType, LogMessage, LogTimestamp};

use super::{Command, Commander};

impl Commander {

    /// Clear filters
    ///
    /// Clear the available filters, and reprocess the log messages
    pub(crate) fn clear_filters(&mut self) -> Result<(), String> {
        // Clear filters
        self.filters.clear();

        // Empty current log list
        let filtered_messages: Vec<LogMessage> = self
            .log_messages
            .iter()
            .map(|msg| self.apply_filters(msg.timestamp, msg.source_id, msg.message.to_string()))
            .map(|msg| msg.unwrap())
            .collect();

        let _ = self
            .command_response_tx
            .send(UiCommand::UpdateLogs(filtered_messages));
        Ok(())
    }

    /// Add a new filter
    ///
    /// Not only store the new filter, but also regenerate the filtered log list and send it to the
    /// application so it can update the log view
    pub(crate) fn add_filter(&mut self, filter: LogFilter) -> Result<(), String> {
        // Add new filter
        self.filters.push(filter.clone());

        // Empty current log list
        let filtered_messages: Vec<LogMessage> = self
            .log_messages
            .iter()
            .map(|msg| self.apply_filters(msg.timestamp, msg.source_id, msg.message.to_string()))
            .filter(|msg| msg.is_some())
            .map(|msg| msg.unwrap())
            .collect();

        let _ = self
            .command_response_tx
            .send(UiCommand::UpdateLogs(filtered_messages));
        debug!("Added {:?}", filter);

        Ok(())
    }


    /// Apply filters to a log message
    pub(crate) fn apply_filters(&self, timestamp: LogTimestamp, id: i32, log: String) -> Option<LogMessage> {
        let mut log = Some(LogMessage {
            timestamp: timestamp.clone(),
            style: Style::default().add_modifier(Modifier::DIM),
            message: log,
            source_id: id,
        });

        for current_filter in &self.filters {
            if log.is_none() {
                return log;
            }
            match current_filter.kind {
                LogFilterType::Inclusion => {
                    let tmp_log = log.clone().unwrap();
                    let retain_it = tmp_log.message.contains(&current_filter.msg)
                        && !current_filter.msg.is_empty();
                    if retain_it {
                        continue;
                    } else {
                        log = None;
                    }
                }
                LogFilterType::Exclusion => {
                    let tmp_log = log.clone().unwrap();
                    let retain_it = !tmp_log.message.contains(&current_filter.msg)
                        && !current_filter.msg.is_empty();
                    if retain_it {
                        continue;
                    } else {
                        log = None;
                    }
                }
                LogFilterType::Highlighter => {
                    let tmp_log = log.clone().unwrap();
                    let matches_msg = tmp_log.message.contains(&current_filter.msg)
                        && !current_filter.msg.is_empty();
                    if matches_msg {
                        log = Some(LogMessage {
                            timestamp: timestamp.clone(),
                            message: log.unwrap().message,
                            style: current_filter.style,
                            source_id: id,
                        });
                    }
                }
            }
        }
        log
    }

}


/// Add filter callback
///
/// Add a filter by parsing the `input` field. It has the general form:
/// {h/i/e} (optional)color word
///
/// Examples:
///     h red wrn -> add highlight filter (color red) for lines containing "wrn"
///     i tempo -> add inclusion filter for lines containing "tempo"
///     e tempo -> add exclusion filter for lines containing "tempo"
pub fn add_filter(sender: &Sender<Command>, input: Vec<String>) -> Result<(), String> {
    if input.is_empty() {
        return Err(String::from("Filter information missing"));
    }

    if input.len() < 2 {
        return Err(String::from(
            "Wrong arguments. Expected \'/{h,i,e} {color} word\'",
        ));
    }

    let mut idx = 0;

    let kind = match input[idx].chars().next() {
        Some('h') => LogFilterType::Highlighter,
        Some('i') => LogFilterType::Inclusion,
        Some('e') => LogFilterType::Exclusion,
        _ => {
            return Err("Wrong argument".to_owned());
        }
    };
    idx = idx + 1;

    // Inclusion/exclusion do not change color
    let mut color = style::Color::Blue;
    if input.len() == 3 {
        match input[idx].as_str() {
            "red" => color = style::Color::Red,
            "green" => color = style::Color::Green,
            "yellow" => color = style::Color::Yellow,
            "white" => color = style::Color::White,
            "blue" => color = style::Color::Blue,
            "magenta" => color = style::Color::Magenta,
            _ => (),
        }
        idx = idx + 1;
    }

    let filter_style = Style {
        fg: Some(color),
        ..Default::default()
    };

    let _ = sender.send(Command::AddFilter(LogFilter {
        style: filter_style,
        kind,
        msg: input[idx].clone(),
    }));

    Ok(())
}
