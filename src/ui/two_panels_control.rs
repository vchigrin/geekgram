use super::control::Control;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use eyre::eyre;
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::Block;
use ratatui::Frame;
use std::cmp;
use std::collections::HashMap;

enum Focused {
    Left,
    Right,
}

#[derive(Clone, Copy)]
enum Action {
    FocusLeft,
    FocusRight,
    IncreaseLeftWidth,
    DecreaseLeftWidth,
}

const WIDTH_STEP_PERCENT: u16 = 1;

pub struct TwoPanelsControl {
    left_child: Box<dyn Control>,
    right_child: Box<dyn Control>,
    left_title: Option<String>,
    right_title: Option<String>,
    focused: Focused,
    left_width_percent: u16,
    keymap: HashMap<KeyEvent, Action>,
}

fn default_keymap() -> HashMap<KeyEvent, Action> {
    HashMap::<KeyEvent, Action>::from([
        (KeyCode::Char('h').into(), Action::FocusLeft),
        (KeyCode::Char('l').into(), Action::FocusRight),
        (
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
            Action::IncreaseLeftWidth,
        ),
        (
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
            Action::DecreaseLeftWidth,
        ),
    ])
}

impl TwoPanelsControl {
    pub fn new(
        left_child: Box<dyn Control>,
        right_child: Box<dyn Control>,
        left_title: Option<String>,
        right_title: Option<String>,
    ) -> Self {
        Self {
            left_child,
            right_child,
            left_title,
            right_title,
            focused: Focused::Left,
            left_width_percent: 50,
            keymap: default_keymap(),
        }
    }

    fn handle_action(&mut self, action: Action) -> Result<()> {
        match action {
            Action::FocusLeft => self.focused = Focused::Left,
            Action::FocusRight => self.focused = Focused::Right,
            Action::IncreaseLeftWidth => {
                self.left_width_percent = cmp::min(
                    self.left_width_percent + WIDTH_STEP_PERCENT,
                    100 - WIDTH_STEP_PERCENT,
                );
            }
            Action::DecreaseLeftWidth => {
                self.left_width_percent = cmp::max(
                    self.left_width_percent - WIDTH_STEP_PERCENT,
                    WIDTH_STEP_PERCENT,
                );
            }
        }
        Ok(())
    }

    fn compute_child_rects(&self, rect: Rect) -> Option<(Rect, Rect)> {
        // 2 positions for left and right borders and at least one for
        // inner content.
        const MIN_AREA_WIDTH: u16 = 3;
        if rect.width < 2 * MIN_AREA_WIDTH {
            // Not enough space to draw anything.
            return None;
        }
        let mut left_width = cmp::max(MIN_AREA_WIDTH, (rect.width * self.left_width_percent) / 100);
        let mut right_width = rect.width - left_width;
        if right_width < MIN_AREA_WIDTH {
            right_width = MIN_AREA_WIDTH;
            left_width = rect.width - right_width;
        }
        let mut left_area = rect;
        let mut right_area = rect;
        left_area.width = left_width;
        right_area.x = left_width;
        right_area.width = right_width;
        Some((left_area, right_area))
    }
}

impl Control for TwoPanelsControl {
    fn handle_keyboard(&mut self, event: KeyEvent) -> Result<()> {
        if let Some(action) = self.keymap.get(&event) {
            self.handle_action(*action)
        } else {
            match self.focused {
                Focused::Left => self.left_child.handle_keyboard(event),
                Focused::Right => self.right_child.handle_keyboard(event),
            }
        }
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        let left_area;
        let right_area;
        if let Some(rects) = self.compute_child_rects(rect) {
            (left_area, right_area) = rects;
        } else {
            return Err(eyre!("Rect too thin"));
        }

        let margins = Margin::new(1, 1);

        let left_color;
        let right_color;
        match self.focused {
            Focused::Left => {
                left_color = Color::Yellow;
                right_color = Color::White;
            }
            Focused::Right => {
                left_color = Color::White;
                right_color = Color::Yellow;
            }
        }
        let mut left_border = Block::bordered().style(Style::default().fg(left_color));
        if let Some(title) = self.left_title.as_ref() {
            left_border = left_border.title(title.as_ref());
        }

        frame.render_widget(left_border, left_area);

        let mut right_border = Block::bordered().style(Style::default().fg(right_color));
        if let Some(title) = self.right_title.as_ref() {
            right_border = right_border.title(title.as_ref());
        }
        frame.render_widget(right_border, right_area);

        self.left_child.render(frame, left_area.inner(margins))?;
        self.right_child.render(frame, right_area.inner(margins))?;
        Ok(())
    }
}
