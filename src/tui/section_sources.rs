use std::sync::mpsc::Sender;

use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Paragraph},
};
use tracing::error;

use crate::commander::Command;

use super::LayoutSection;

struct SourceInformation {
    /// Whether it is currently connected or not
    connected: bool,

    /// Name to display about the source
    name: String,

    /// ID coming from Commander, use to identify it
    id: u32,
}

impl SourceInformation {
    fn new(id: u32, name: String) -> Self {
        SourceInformation {
            id,
            name,
            connected: false,
        }
    }

    fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
}

pub struct SectionSources {
    pub command_tx: Sender<Command>,

    /// Store available sources
    sources: Vec<SourceInformation>,

    /// Currently selected source
    selected_source_idx: usize,
}

impl SectionSources {
    pub fn new(command_tx: Sender<Command>) -> Self {
        SectionSources {
            command_tx,
            sources: Vec::new(),
            selected_source_idx: 0,
        }
    }

    pub fn set_connected(&mut self, id: u32, is_connected: bool) {
        if let Some(idx) = self.get_source_idx(id) {
            self.sources[idx].set_connected(is_connected);
        } else {
            error!("Unable to update source with ID {}, does not exist", id);
        }
    }

    pub fn add_source(&mut self, id: u32, name: String) {
        self.sources.push(SourceInformation::new(id, name));
    }

    pub fn delete_source(&mut self, id: u32) {
        if let Some(idx) = self.get_source_idx(id) {
            self.sources.remove(idx);
            return;
        } else {
            error!("Unable to delete source with ID {}, does not exist", id);
        }
    }

    fn get_source_idx(&self, id: u32) -> Option<usize> {
        for (idx, source) in self.sources.iter().enumerate() {
            if source.id == id {
                return Some(idx);
            }
        }

        return None;
    }
}

impl LayoutSection for SectionSources {
    fn ui(&mut self, frame: &mut Frame, area: Rect) {
        // Probe information
        let mut source_list_lines = Vec::new();

        for (idx, info) in self.sources.iter().enumerate() {
            let status = match info.is_connected() {
                true => "Connected",
                false => "Not connected",
            };

            // Create line and make it Bold if it is the currently selected source
            let mut line = Line::from(format!(" {} | {}", status, info.get_name()));
            if idx == self.selected_source_idx {
                line.style = line.style.add_modifier(Modifier::BOLD);
            }

            source_list_lines.push(line);
        }
        let probse_block_title = Line::from("Log Sources");
        let probes_block = Block::default()
            .title(probse_block_title)
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .style(Style::default());

        let probe_list = Paragraph::new(source_list_lines).block(probes_block);
        frame.render_widget(probe_list, area);
    }

    fn process_key(&mut self, key: crossterm::event::KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                // Make sure there is a filter
                if self.sources.is_empty() {
                    return;
                }
                self.selected_source_idx = self
                    .selected_source_idx
                    .saturating_add(1)
                    .min(self.sources.len() - 1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected_source_idx = self.selected_source_idx.saturating_sub(1);
            }
            KeyCode::Home => {
                self.selected_source_idx = 0;
            }
            KeyCode::End => {
                // Make sure there is a filter
                if self.sources.is_empty() {
                    return;
                }
                self.selected_source_idx = self.sources.len() - 1;
            }

            KeyCode::Char('c') => {
                if self.sources.is_empty() {
                    let _ = self
                        .command_tx
                        .send(Command::PrintMessage(String::from("No probes detected")));
                    return;
                }
                let _ = self.command_tx.send(Command::ConnectLogSource(
                    self.sources[self.selected_source_idx].id,
                ));
            }
            KeyCode::Char('d') => {
                if self.sources.is_empty() {
                    let _ = self
                        .command_tx
                        .send(Command::PrintMessage(String::from("No probes detected")));
                    return;
                }
                let _ = self.command_tx.send(Command::DisconnectLogSource(
                    self.sources[self.selected_source_idx].id,
                ));
            }
            KeyCode::Char('r') => {
                let _ = self.command_tx.send(Command::RefreshProbeInfo);
            }
            KeyCode::Char('R') => {
                let _ = self
                    .command_tx
                    .send(Command::Reset(self.sources[self.selected_source_idx].id));
            }
            KeyCode::Char('F') => {
                let _ = self
                    .command_tx
                    .send(Command::Reflash(self.sources[self.selected_source_idx].id));
            }
            _ => (),
        }
    }

    fn min_lines(&self) -> usize {
        return 2 /*borders */ + self.sources.len().max(1);
    }
}
