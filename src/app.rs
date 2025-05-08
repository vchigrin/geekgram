use color_eyre::Result;

pub struct App {}

impl App {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn run(&mut self, mut _terminal: ratatui::DefaultTerminal) -> Result<()> {
        Ok(())
    }
}
