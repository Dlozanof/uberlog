use std::sync::mpsc::Sender;

use crossterm::event::KeyCode;
use ratatui::{layout::Rect, style::Style, text::Line, widgets::{Block, Borders, Paragraph}, Frame};

use crate::{commander::Command, layout_section::LayoutSection, LogMessage};


enum SearchDirection {
    FOWARD,
    BACKWARD,
}

pub struct SectionLogs {

    /// Log offset
    pub vertical_scroll: usize,
    
    /// Maximum offset
    vertical_scroll_limit: usize,

    /// Log message storage
    pub logs: Vec<LogMessage>,

    /// Log search feature
    search_string: String,
    search_string_log_idx: usize,

    /// How many lines are displayed in a page, depends on screen size
    page_size: usize,

    /// Should the offset be updated automatically when a new log message comes
    sticky: bool,

    /// Send commands to the Commander
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

    pub fn update_search_log(&mut self, log: String) {
        self.search_string = log.clone();
        self.find_log(log, SearchDirection::FOWARD);
    }

    /// Search a log containing the search_string text
    /// 
    /// If the search_string is empty, the search is disabled
    fn find_log(&mut self, log: String, direction: SearchDirection) {

        // If search_string_log_idx is not within view, update it
        if self.search_string_log_idx < self.vertical_scroll || self.search_string_log_idx > (self.vertical_scroll + self.page_size) {
            self.search_string_log_idx = self.vertical_scroll;
        }

        let start_idx = match direction{
            SearchDirection::FOWARD => self.search_string_log_idx.saturating_add(1).min(self.logs.len() - 1),
            SearchDirection::BACKWARD => self.search_string_log_idx.saturating_sub(1),
        };

        if start_idx == self.search_string_log_idx {
            return;
        }

        let end_idx = match direction {
            SearchDirection::FOWARD => self.logs.len() - 1,
            SearchDirection::BACKWARD => 0,
        };

        if end_idx == self.search_string_log_idx {
            return;
        }

        
        let mut i = start_idx;
        while i != end_idx {
            if self.logs[i].message.contains(&log) {

                self.search_string_log_idx = i;

                let offset = self.page_size / 2;
                let offset = match offset >= i {
                    true => i,
                    false => offset,
                };
                
                self.vertical_scroll = self.search_string_log_idx - offset;
                break;
            }

            i = match direction {
                SearchDirection::FOWARD => i.saturating_add(1).min(self.logs.len() - 1),
                SearchDirection::BACKWARD => i.saturating_sub(1),                
            }
        }
    }
}

impl SectionLogs {
    pub fn new(command_tx: Sender<Command>) -> SectionLogs {
        SectionLogs {
            command_tx,
            logs: Vec::new(),
            search_string: String::new(),
            search_string_log_idx: 0,
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
        for (idx, log) in self.logs.iter().enumerate() {
            let log_style = match idx == self.search_string_log_idx && !self.search_string.is_empty() {
                false => log.style,
                true =>  Style {
                    fg: log.style.bg,
                    bg: log.style.fg,
                    ..Default::default()
                }
            };
            log_lines.push(Line::from(log.message.clone()).style(log_style));
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
            // Movement
            KeyCode::Char('j') | KeyCode::Down => {
                self.vertical_scroll = self.vertical_scroll.saturating_add(1).min(self.vertical_scroll_limit);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.vertical_scroll = self.vertical_scroll.saturating_sub(1);
                self.sticky = false;
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.vertical_scroll = 0;
                self.sticky = false;
            }
            KeyCode::PageDown => {
                self.vertical_scroll = self.vertical_scroll.saturating_add(self.page_size).min(self.vertical_scroll_limit);
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.vertical_scroll = self.vertical_scroll.saturating_add(self.vertical_scroll_limit).min(self.vertical_scroll_limit);
            }
            KeyCode::PageUp => {
                self.vertical_scroll = self.vertical_scroll.saturating_sub(self.page_size);
                self.sticky = false;
            }

            // Clear screent
            KeyCode::Char('C') => {
                let _ = self.command_tx.send(Command::ClearLogs);
            },

            // Search log
            KeyCode::Char('n') => {
                if !self.search_string.is_empty() {
                    self.find_log(self.search_string.clone(), SearchDirection::FOWARD);
                    self.sticky = false;
                }
            }
            KeyCode::Char('N') => {
                if !self.search_string.is_empty() {
                    self.find_log(self.search_string.clone(), SearchDirection::BACKWARD);
                    self.sticky = false;
                }
            }
            _ => ()
        }
    }


    fn min_lines(&self) -> usize {
        return self.logs.len().min(1);
    }
}
