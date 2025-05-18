use super::control::Control;
use crate::runtime::Runtime;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use grammers_client::types::{Chat, Dialog};
use grammers_tl_types as tl_types;
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::style::Style;
use ratatui::widgets::{List, ListDirection, ListState};
use ratatui::Frame;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Copy)]
enum Action {
    SelectNext,
    SelectPrev,
    Activate,
    SortByName,
    SortByType,
    SortByUnreadCount,
}

enum SortOrder {
    Name,
    Type,
    UnreadCount,
}

fn default_keymap() -> HashMap<KeyEvent, Action> {
    HashMap::<KeyEvent, Action>::from([
        (KeyCode::Char('j').into(), Action::SelectNext),
        (KeyCode::Char('k').into(), Action::SelectPrev),
        (
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            Action::SortByName,
        ),
        (
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
            Action::SortByType,
        ),
        (
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
            Action::SortByUnreadCount,
        ),
        (KeyCode::Enter.into(), Action::Activate),
    ])
}

#[derive(Clone)]
struct ListItem {
    chat_id: i64,
    display_content: Text<'static>,
}

impl From<ListItem> for ratatui::widgets::ListItem<'static> {
    fn from(value: ListItem) -> ratatui::widgets::ListItem<'static> {
        ratatui::widgets::ListItem::new(value.display_content)
    }
}

pub struct DialogsListControl {
    keymap: HashMap<KeyEvent, Action>,
    app_runtime: Arc<Runtime>,
    sort_order: SortOrder,
    list_state: ListState,
    last_drawn_items: Vec<ListItem>,
}

impl DialogsListControl {
    pub fn new(app_runtime: Arc<Runtime>) -> Self {
        Self {
            keymap: default_keymap(),
            list_state: ListState::default(),
            app_runtime,
            sort_order: SortOrder::Name,
            last_drawn_items: Vec::new(),
        }
    }

    fn handle_action(&mut self, action: Action) -> Result<()> {
        match action {
            Action::SelectNext => {
                self.list_state.select_next();
            }
            Action::SelectPrev => {
                self.list_state.select_previous();
            }
            Action::SortByName => {
                self.sort_order = SortOrder::Name;
            }
            Action::SortByType => {
                self.sort_order = SortOrder::Type;
            }
            Action::SortByUnreadCount => {
                self.sort_order = SortOrder::UnreadCount;
            }
            Action::Activate => {
                if let Some(selected) = self.list_state.selected() {
                    let chat_id = self.last_drawn_items[selected].chat_id;
                    self.app_runtime.set_active_dialog(chat_id);
                }
            }
        }
        Ok(())
    }

    fn get_raw_dialog(dialog: &Dialog) -> &tl_types::types::Dialog {
        match &dialog.raw {
            tl_types::enums::Dialog::Dialog(d) => d,
            tl_types::enums::Dialog::Folder(_) => panic!("Unexpected type"),
        }
    }

    fn get_type_for_sort(dialog: &Dialog) -> i32 {
        match dialog.chat {
            Chat::User(_) => 0,
            Chat::Group(_) => 1,
            Chat::Channel(_) => 2,
        }
    }

    fn is_muted(raw_dialog: &tl_types::types::Dialog) -> bool {
        let tl_types::enums::PeerNotifySettings::Settings(settings) = &raw_dialog.notify_settings;
        if let Some(mute_until) = settings.mute_until {
            let now = chrono::Utc::now();
            let mute_untile_dt = chrono::DateTime::from_timestamp(mute_until.into(), 0).unwrap();
            now < mute_untile_dt
        } else {
            false
        }
    }

    fn make_list_items(&self, mut dialogs: Vec<Dialog>) -> Vec<ListItem> {
        match self.sort_order {
            SortOrder::Name => {
                dialogs.sort_by(|first, second| first.chat().name().cmp(second.chat().name()));
            }
            SortOrder::Type => {
                dialogs.sort_by(|first, second| {
                    let first_type_for_sort = Self::get_type_for_sort(first);
                    let second_type_for_sort = Self::get_type_for_sort(second);
                    first_type_for_sort.cmp(&second_type_for_sort)
                });
            }
            SortOrder::UnreadCount => {
                dialogs.sort_by(|first, second| {
                    let first_raw = Self::get_raw_dialog(first);
                    let second_raw = Self::get_raw_dialog(second);
                    first_raw.unread_count.cmp(&second_raw.unread_count)
                });
            }
        }

        let mut items = Vec::<ListItem>::with_capacity(dialogs.capacity());
        for d in dialogs {
            let dialog = Self::get_raw_dialog(&d);
            let mut components = Vec::<Span>::new();
            let main_text_style = match &d.chat {
                Chat::User(_) => Style::new(),
                Chat::Group(_) => Style::new().italic(),
                Chat::Channel(_) => Style::new().underlined(),
            };
            components.push(Span::from(d.chat().name().to_owned()).style(main_text_style));

            if dialog.unread_mentions_count > 0 {
                let text = format!(" @{}", dialog.unread_mentions_count);
                components.push(Span::from(text).style(Style::new().blue()));
            }
            if dialog.unread_count > 0 {
                let style = if Self::is_muted(dialog) {
                    Style::new().dark_gray()
                } else {
                    Style::new().red()
                };
                let text = format!(" {}", dialog.unread_count);
                components.push(Span::from(text).style(style));
            }
            items.push(ListItem {
                chat_id: d.chat().id(),
                display_content: Text::from(Line::from(components)),
            });
        }
        items
    }
}

impl Control for DialogsListControl {
    fn handle_keyboard(&mut self, event: KeyEvent) -> Result<()> {
        if let Some(action) = self.keymap.get(&event) {
            self.handle_action(*action)?;
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        let dialogs = self.app_runtime.get_dialogs()?;
        self.last_drawn_items = self.make_list_items(dialogs);
        let list = List::new(self.last_drawn_items.clone())
            .style(Style::new().white())
            .highlight_style(Style::new().yellow())
            .repeat_highlight_symbol(true)
            .direction(ListDirection::TopToBottom);
        frame.render_stateful_widget(list, rect, &mut self.list_state);
        Ok(())
    }
}
