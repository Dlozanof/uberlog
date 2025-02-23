use std::sync::mpsc::Sender;

use crossterm::event::KeyCode;
use ratatui::{layout::Rect, style::Style, text::Line, widgets::{Block, Borders, Paragraph}, Frame};

use crate::{commander::{Command, LogBackendInformation, TargetInformation}, layout_section::LayoutSection};

pub struct SectionProbes {
    pub targets: Vec<TargetInformation>,
    pub selected_probe: usize,
    pub command_tx: Sender<Command>,
}

impl LayoutSection for SectionProbes {
    fn ui(&mut self, frame: &mut Frame, area: Rect) {
        // Probe information
        let mut probe_list_lines = Vec::new();
        for info in &self.targets {
            probe_list_lines.push(Line::from(format!("{} [{}]", info.probe_name, info.probe_serial)));
            match &info.backend {
                LogBackendInformation::Rtt(elfpath) => {
                    probe_list_lines.push(Line::from(format!("-> {} {}", info.mcu, elfpath)));
                }
                LogBackendInformation::Uart(port, baud) => {
                    probe_list_lines.push(Line::from(format!("-> {} {} ({} bauds)", info.mcu, port, baud)));
                }
            }
        }
        let probse_block_title = Line::from("Debug probes");
        let probes_block = Block::default()
            .title(probse_block_title)
            .borders(Borders::ALL)
            .style(Style::default());

        let probe_list  = Paragraph::new(probe_list_lines).block(probes_block);
        frame.render_widget(probe_list, area);
    }

    fn process_key(&mut self, key: crossterm::event::KeyCode) {
        match key {
            KeyCode::Char('c') => {
                if self.targets.is_empty() {
                    let _ = self.command_tx.send(Command::PrintMessage(String::from("No probes detected")));
                    return;
                }
                let _ = self.command_tx.send(Command::Connect(self.targets[0].clone()));
            }
            KeyCode::Char('d') => {
                if self.targets.is_empty() {
                    let _ = self.command_tx.send(Command::PrintMessage(String::from("No probes detected")));
                    return;
                }
                let _ = self.command_tx.send(Command::Disconnect(self.targets[0].probe_serial.clone()));
            }
            KeyCode::Char('r') => {
                let _ = self.command_tx.send(Command::GetProbes);
                let _ = self.command_tx.send(Command::PrintMessage(String::from("Probe list updated")));
            }
            _ => ()
        }
    }

    fn min_lines(&self) -> usize {
        return 2 /*borders */ + self.targets.len().max(1) * 2;
    }
}
