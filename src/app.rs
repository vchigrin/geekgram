use super::runtime::Runtime;
use color_eyre::Result;

pub struct App<'a> {
    app_runtime: &'a Runtime,
}

impl<'a> App<'a> {
    pub fn new(app_runtime: &'a Runtime) -> Self {
        Self { app_runtime }
    }

    pub async fn run(&mut self, mut _terminal: ratatui::DefaultTerminal) -> Result<()> {
        let _dialogs = self.app_runtime.get_dialogs();
        // TODO: Display UI.
        Ok(())
    }
}
