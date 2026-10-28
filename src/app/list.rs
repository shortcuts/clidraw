use super::App;
use crate::ui::list::{Click, Outcome};
use crate::ui::switcher::{self, SwitcherHit};
use crate::ui::Overlay;
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub(super) fn list_key(&mut self, k: KeyEvent) {
        match &mut self.overlay {
            Overlay::Context(l) => match l.key(k) {
                Outcome::Run(item) => self.context_item(item),
                Outcome::Close => self.overlay = Overlay::None,
                _ => {}
            },
            Overlay::Formats(l) => match l.key(k) {
                Outcome::Run(format) => self.pick_scope(format),
                Outcome::Close => self.overlay = Overlay::None,
                _ => {}
            },
            Overlay::Scope(format, l) => match l.key(k) {
                Outcome::Run(item) => {
                    let format = *format;
                    self.scope_item(item, format)
                }
                Outcome::Close => self.overlay = Overlay::None,
                _ => {}
            },
            Overlay::Colors(c, l) => match l.key(k) {
                Outcome::Run(p) => {
                    let c = *c;
                    self.session.set_paint(c, p);
                    self.overlay = Overlay::None;
                }
                Outcome::Close => self.overlay = Overlay::None,
                _ => {}
            },
            Overlay::Kinds(origins, l) => match l.key(k) {
                Outcome::Run(kind) => {
                    self.session.set_kind(origins, kind);
                    self.overlay = Overlay::None;
                }
                Outcome::Close => {
                    self.session.restore_kinds(origins);
                    self.overlay = Overlay::None;
                }
                Outcome::Idle => {
                    if let Some(kind) = l.selected() {
                        self.session.preview_kind(origins, kind);
                    }
                }
                _ => {}
            },
            Overlay::Notes(x, n) => match n.list.key(k) {
                Outcome::Run(hit) => {
                    let x = *x;
                    self.note_hit(hit, x)
                }
                Outcome::Close => self.overlay = Overlay::None,
                _ => {}
            },
            Overlay::Actions(l) => {
                if let (KeyCode::Enter, Some(line)) = (k.code, l.ex_line().map(String::from)) {
                    self.overlay = Overlay::None;
                    self.run_command(&line);
                } else {
                    match l.key(k) {
                        Outcome::Run(action) => self.run_action(action),
                        Outcome::Close => self.overlay = Overlay::None,
                        _ => {}
                    }
                }
            }
            Overlay::Switcher(sw) => match sw.list.key(k) {
                Outcome::Run(hit) => self.switcher_hit(hit),
                Outcome::Close => self.overlay = Overlay::None,
                Outcome::Key('r', Some(SwitcherHit::Open(i))) => {
                    self.switcher_hit(SwitcherHit::Rename(i))
                }
                Outcome::Key('x', Some(SwitcherHit::Open(i))) => {
                    self.switcher_hit(SwitcherHit::Delete(i))
                }
                _ => {}
            },
            _ => {}
        }
    }

    pub(super) fn list_click(&mut self, col: i32, row: i32) {
        let screen = self.session.screen;
        match &self.overlay {
            Overlay::Context(l) => match l.click(screen, col, row) {
                Click::Row(item, _) => self.context_item(item),
                Click::Inside => {}
                Click::Outside => self.overlay = Overlay::None,
            },
            Overlay::Formats(l) => match l.click(screen, col, row) {
                Click::Row(format, _) => self.pick_scope(format),
                Click::Inside => {}
                Click::Outside => self.overlay = Overlay::None,
            },
            Overlay::Scope(format, l) => match l.click(screen, col, row) {
                Click::Row(item, _) => {
                    let format = *format;
                    self.scope_item(item, format)
                }
                Click::Inside => {}
                Click::Outside => self.overlay = Overlay::None,
            },
            Overlay::Colors(c, l) => match l.click(screen, col, row) {
                Click::Row(p, _) => {
                    let c = *c;
                    self.session.set_paint(c, p);
                    self.overlay = Overlay::None;
                }
                Click::Inside => {}
                Click::Outside => self.overlay = Overlay::None,
            },
            Overlay::Kinds(origins, l) => match l.click(screen, col, row) {
                Click::Row(kind, _) => {
                    self.session.set_kind(origins, kind);
                    self.overlay = Overlay::None;
                }
                Click::Inside => {}
                Click::Outside => {
                    self.session.restore_kinds(origins);
                    self.overlay = Overlay::None;
                }
            },
            Overlay::Notes(x, n) => match n.list.click(screen, col, row) {
                Click::Row(hit, _) => {
                    let x = *x;
                    self.note_hit(hit, x)
                }
                Click::Inside => {}
                Click::Outside => self.overlay = Overlay::None,
            },
            Overlay::Actions(l) => match l.click(screen, col, row) {
                Click::Row(action, _) => self.run_action(action),
                Click::Inside => {}
                Click::Outside => self.overlay = Overlay::None,
            },
            Overlay::Switcher(sw) => match sw.list.click(screen, col, row) {
                Click::Row(SwitcherHit::Open(i), off) => {
                    let hit = match off {
                        o if switcher::RENAME.contains(&o) => SwitcherHit::Rename(i),
                        o if switcher::DELETE.contains(&o) => SwitcherHit::Delete(i),
                        _ => SwitcherHit::Open(i),
                    };
                    self.switcher_hit(hit)
                }
                Click::Row(hit, _) => self.switcher_hit(hit),
                Click::Inside => {}
                Click::Outside => self.overlay = Overlay::None,
            },
            _ => {}
        }
    }
}
