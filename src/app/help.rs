use super::App;
use crate::ui::help::{self, SCOPES};
use crate::ui::Overlay;
use crossterm::event::KeyCode;

impl App {
    pub(super) fn help_key(&mut self, code: KeyCode) {
        let Overlay::Help(page) = &mut self.overlay else {
            return;
        };
        match code {
            KeyCode::Esc | KeyCode::Char('?') => self.overlay = Overlay::None,
            KeyCode::Left => *page = page.saturating_sub(1),
            KeyCode::Right => *page = (*page + 1).min(SCOPES.len() - 1),
            _ => {}
        }
    }

    pub(super) fn help_click(&mut self, col: i32, row: i32) {
        let screen = self.session.screen;
        match help::tab_at(screen, col, row) {
            Some(p) => self.overlay = Overlay::Help(p),
            None if !help::inside(screen, col, row) => self.overlay = Overlay::None,
            None => {}
        }
    }
}
