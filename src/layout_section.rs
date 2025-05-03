use ratatui::{Frame, layout::Rect};

pub trait LayoutSection {
    /// Draw the rectangle
    fn ui(&mut self, frame: &mut Frame, area: Rect);

    /// Handle user input
    fn process_key(&mut self, key: crossterm::event::KeyCode);

    /// How many lines would it like to have in this frame
    fn min_lines(&self) -> usize;
}
