use std::sync::mpsc::Sender;

use crossterm::event::KeyCode;
use ratatui::{layout::Rect, style::Style, text::Line, widgets::{Block, Borders, Paragraph}, Frame};

use crate::{commander::Command, layout_section::LayoutSection, LogMessage};

pub struct SectionLogs {
    //pub vertical_scroll_state: ScrollbarState,
    pub vertical_scroll: usize,
    pub logs: Vec<LogMessage>,
    vertical_scroll_limit: usize,
    page_size: usize,
    sticky: bool,
    command_tx: Sender<Command>,
}

impl SectionLogs {
    pub fn clear_logs(&mut self) {
        self.logs.clear();
        self.vertical_scroll = 0;
    }

    pub fn update_logs(&mut self, new_logs: Vec<LogMessage>) {
        self.logs = new_logs;
    }
}

impl SectionLogs {
    pub fn new(command_tx: Sender<Command>) -> SectionLogs {
        SectionLogs {
            command_tx,
            logs: Vec::new(),
            page_size: 0,
            sticky: true,
            vertical_scroll: 0,
            vertical_scroll_limit: 0,
        }
    }
}

impl LayoutSection for SectionLogs {
    fn ui(&mut self, frame: &mut Frame, area: Rect) {

        // Update scroll limit value (+2 to take into account borders)
        if area.height as usize <= self.logs.len() {
            self.vertical_scroll_limit = 2 + self.logs.len() - area.height as usize;
        } else {
            self.vertical_scroll_limit = 0
        }

        // Update page size
        self.page_size = area.height as usize;

        // Scroll to bottom if sticky, otherwise check if sticky
        if self.sticky {
            self.vertical_scroll = self.vertical_scroll_limit;
        } else if self.vertical_scroll == self.vertical_scroll_limit {
            self.sticky = true;
        }

        // Draw ui
        let mut log_lines = Vec::new();
        for log in &self.logs {
            log_lines.push(Line::from(log.message.clone()).style(log.style));
        }
        let log_block_title = Line::from("Logs");
        let log_block = Block::default()
            .title(log_block_title)
            .borders(Borders::ALL)
            .style(Style::default());
        let log_content  = Paragraph::new(log_lines).block(log_block)
            .scroll((self.vertical_scroll as u16, 0));

        // Render
        frame.render_widget(log_content, area);

    }

    fn process_key(&mut self, key: crossterm::event::KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                self.vertical_scroll = self.vertical_scroll.saturating_add(1).min(self.vertical_scroll_limit);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.vertical_scroll = self.vertical_scroll.saturating_sub(1);
                self.sticky = false;
            }
            KeyCode::Home => {
                self.vertical_scroll = 0;
                self.sticky = false;
            }
            KeyCode::PageDown => {
                self.vertical_scroll = self.vertical_scroll.saturating_add(self.page_size).min(self.vertical_scroll_limit);
            }
            KeyCode::End => {
                self.vertical_scroll = self.vertical_scroll.saturating_add(self.vertical_scroll_limit).min(self.vertical_scroll_limit);
            }
            KeyCode::PageUp => {
                self.vertical_scroll = self.vertical_scroll.saturating_sub(self.page_size);
                self.sticky = false;
            }
            KeyCode::Char('C') => {
                let _ = self.command_tx.send(Command::ClearLogs);
            },

            _ => ()
        }
    }

    fn min_lines(&self) -> usize {
        return self.logs.len().min(1);
    }
}
