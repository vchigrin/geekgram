use super::runtime::Runtime;
use super::ui;
use color_eyre::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent};
use futures::StreamExt;
use ratatui::style::Color;
use ratatui::{DefaultTerminal, Frame};
use std::sync::Arc;

pub struct App {
    app_runtime: Arc<Runtime>,
    event_stream: EventStream,
    should_run: bool,
    root_control: Box<dyn ui::Control>,
}
// TODO: remove after finish debugging.
struct DummyControl {
    color: Color,
}

impl ui::Control for DummyControl {
    fn handle_keyboard(&mut self, _event: KeyEvent) -> Result<()> {
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame, rect: ratatui::layout::Rect) -> Result<()> {
        let b =
            ratatui::widgets::Block::new().style(ratatui::style::Style::default().bg(self.color));
        frame.render_widget(b, rect);
        Ok(())
    }
}

impl App {
    pub fn new(app_runtime: Arc<Runtime>) -> Self {
        let left = Box::new(ui::DialogsListControl::new(app_runtime.clone()));
        let right = Box::new(DummyControl { color: Color::Blue });
        let root_control = ui::TwoPanelsControl::new(
            left,
            right,
            Some("Dialogs".to_string()),
            Some("Right panel".to_string()),
        );
        Self {
            app_runtime,
            event_stream: EventStream::new(),
            should_run: true,
            root_control: Box::new(root_control),
        }
    }

    pub async fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        // TODO(vchigrin): Remove after implementing Messages list.
        let dialogs = self.app_runtime.get_dialogs()?;
        if !dialogs.is_empty() {
            self.app_runtime
                .start_message_refreshing(dialogs[0].chat.clone())
                .await?;
        }
        while self.should_run {
            terminal.draw(|frame| self.render(frame))?;
            if let Some(read_result) = self.event_stream.next().await {
                match read_result {
                    Err(e) => {
                        log::error!("Keyboard read failed {:?}", e);
                        self.should_run = true;
                        break;
                    }
                    Ok(event) => {
                        if let Event::Key(kbd_event) = event {
                            // TODO(vchigrin): Remove this hardcode.
                            if kbd_event.code == KeyCode::Esc {
                                self.should_run = false;
                                break;
                            }
                            if let Err(e) = self.root_control.handle_keyboard(kbd_event) {
                                log::error!("Failed handle keyboard; Error {:?}", e);
                            }
                        }
                    }
                }
            } else {
                break;
            }
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
        if let Err(e) = self.root_control.render(frame, frame.area()) {
            log::error!("Failed render; Error {:?}", e);
        }
    }
}
