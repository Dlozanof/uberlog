use std::sync::mpsc::Sender;

use crossterm::event::KeyCode;
use ratatui::{layout::Rect, style::{self, Modifier, Style}, text::Line, widgets::{Block, BorderType, Borders, Paragraph}, Frame};

use crate::{commander::Command, layout_section::LayoutSection, LogFilter, LogFilterType};

pub struct SectionFilters {
    filters: Vec<LogFilter>,
    selected_filter: usize,
    command_tx: Sender<Command>,
}

impl SectionFilters {
    pub fn new(command_tx: Sender<Command>) -> SectionFilters {
        SectionFilters {
            filters: Vec::new(),
            selected_filter: 0,
            command_tx
        }
    }
    pub fn set_filters(&mut self, filters: Vec<LogFilter>) {
        self.filters = filters;
    }
}

impl LayoutSection for SectionFilters {
    fn ui(&mut self, frame: &mut Frame, area: Rect) {
        
        // Print filters
        let mut filter_list_lines = Vec::new();
        for (idx, filter) in self.filters.iter().enumerate() {

            // Map kind to text
            let type_text = match filter.kind {
                LogFilterType::Exclusion => "Exclusion",
                LogFilterType::Inclusion => "Inclusion",
                LogFilterType::Highlighter => "Highlight",
            };

            let mut line_style = filter.style;
            if idx == self.selected_filter {
                line_style = line_style.add_modifier(Modifier::BOLD);
            }

            // Print the line
            filter_list_lines.push(Line::from(format!("[{}] {} <{}>", idx, type_text, filter.msg)).style(line_style));
        }

        let filters_block_title = Line::from("Filters");
        let filters_block = Block::default()
            .title(filters_block_title)
            .borders(Borders::ALL).border_type(BorderType::Double)
            .style(Style::default());


        let filters_list  = Paragraph::new(filter_list_lines).block(filters_block);
        frame.render_widget(filters_list, area);
    }

    fn process_key(&mut self, key: crossterm::event::KeyCode) {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                // Make sure there is a filter
                if self.filters.is_empty() {
                    return;
                }
                self.selected_filter = self.selected_filter.saturating_add(1).min(self.filters.len() - 1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected_filter = self.selected_filter.saturating_sub(1);
            }
            KeyCode::Home => {
                self.selected_filter = 0;
            }
            KeyCode::End => {
                // Make sure there is a filter
                if self.filters.is_empty() {
                    return;
                }
                self.selected_filter = self.filters.len() - 1;
            }
            KeyCode::Char('d') => {
                // Make sure there is a filter
                if self.filters.is_empty() {
                    return;
                }

                // Remove selected filter
                self.filters.remove(self.selected_filter);

                // Query a filter cleanup
                let _ = self.command_tx.send(Command::ClearFilters);

                // Send all of them again
                for filter in &self.filters {
                    let _ = self.command_tx.send(Command::AddFilter(filter.clone()));
                }

                // Request list udpate
                let _ = self.command_tx.send(Command::GetFilters);

                // Update current index
                self.selected_filter = self.selected_filter.saturating_sub(1);
            }
            _ => ()
        }
    }

    fn min_lines(&self) -> usize {
        return 2 /*borders */ + self.filters.len().max(1);
    }
}
