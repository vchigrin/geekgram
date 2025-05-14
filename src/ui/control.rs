use color_eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;

pub trait Control {
    fn handle_keyboard(&mut self, event: KeyEvent) -> Result<()>;
    fn render(&mut self, frame: &mut Frame, rect: Rect) -> Result<()>;
}
