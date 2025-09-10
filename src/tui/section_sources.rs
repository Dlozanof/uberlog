use std::sync::mpsc::Sender;

use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect}, style::{Modifier, Style}, text::Line, widgets::{Block, BorderType, Borders, Gauge, Paragraph}, Frame
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

    /// Some sources have operations that take time, like flashing new code,
    /// this variable holds the state of the progress and the name of the stage
    progress: u16,
    progress_stage: String,
}

impl SourceInformation {
    fn new(id: u32, name: String) -> Self {
        SourceInformation {
            id,
            name,
            connected: false,
            progress: 0,
            progress_stage: String::new(),
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

    fn set_progress(&mut self, progress: u16, progress_stage: String) {
        self.progress = progress;
        self.progress_stage = progress_stage;
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

    pub fn source_set_progress(&mut self, id: u32, progress: u16, progress_stage: String) {
        if let Some(idx) = self.get_source_idx(id) {
            self.sources[idx].set_progress(progress, progress_stage);
            return;
        } else {
            error!("Unable to set progress of source with ID {}, does not exist", id);
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

    fn draw_source_info(&mut self, frame: &mut Frame, area: Rect, idx: usize) {

        let source_info = self.sources.get(idx).unwrap();
        let status = match source_info.is_connected() {
            true => "Connected",
            false => "Not connected",
        };

        // Create line and make it Bold if it is the currently selected source
        let mut line = Line::from(format!(" {} | {} {}", status, source_info.get_name(), source_info.progress_stage));
        if idx == self.selected_source_idx {
            line.style = line.style.add_modifier(Modifier::BOLD);
        }

        frame.render_widget(line, area);
    }

    fn draw_source_extra_info(&mut self, frame: &mut Frame, area: Rect, idx: usize) {

        let source_info = self.sources.get(idx).unwrap();
        if source_info.progress != 0 {
            let gauge = Gauge::default()
                //.gauge_style(GAUGE1_COLOR)
                .percent(source_info.progress);
            frame.render_widget(gauge, area);
        }

    }
}

impl LayoutSection for SectionSources {

    fn ui(&mut self, frame: &mut Frame, area: Rect) {

        // Block
        let probse_block_title = Line::from("Log Sources");
        let probes_block = Block::default()
            .title(probse_block_title)
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .style(Style::default());

        // Get the inner area before consuming probes_block
        let inner_area = probes_block.inner(area);

        // Render the block
        frame.render_widget(probes_block, area);

        // Early return if there is nothing to do
        if self.sources.is_empty() {
            return;
        }
        
        // Otherwise fill the contents of the block
        let row_height = 1;

        let constraints: Vec<Constraint> = (0..self.sources.len())
            .map(|_| Constraint::Length(row_height)).collect();

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner_area);
        
        for (idx, row_area) in rows.iter().enumerate() {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Min(area.width / 2),  // Section 1
                    Constraint::Min(1),               // Section 2
                ])
                .split(*row_area);
            // Draw the Section 1
            self.draw_source_info(frame, chunks[0], idx);
            // Draw the Section 2
            self.draw_source_extra_info(frame, chunks[1], idx);
        }
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
            KeyCode::Char('l') => {
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
