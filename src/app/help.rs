use super::App;
use crate::ui::help::{self, SCOPES};
use crate::ui::Overlay;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub(super) fn help_key(&mut self, k: KeyEvent) {
        let Overlay::Help(page, query) = &mut self.overlay else {
            return;
        };
        let typing = !k
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        match k.code {
            KeyCode::Esc if !query.is_empty() => query.clear(),
            KeyCode::Esc => self.overlay = Overlay::None,
            KeyCode::Char('?') if query.is_empty() => self.overlay = Overlay::None,
            KeyCode::Char(c) if typing => query.push(c),
            KeyCode::Backspace => {
                query.pop();
            }
            KeyCode::Left if query.is_empty() => *page = page.saturating_sub(1),
            KeyCode::Right if query.is_empty() => *page = (*page + 1).min(SCOPES.len() - 1),
            _ => {}
        }
    }

    pub(super) fn help_click(&mut self, col: i32, row: i32) {
        let screen = self.session.screen;
        match help::tab_at(screen, col, row) {
            Some(p) => self.overlay = Overlay::Help(p, String::new()),
            None if !help::inside(screen, col, row) => self.overlay = Overlay::None,
            None => {}
        }
    }
}
