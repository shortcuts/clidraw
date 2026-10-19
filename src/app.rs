use crate::input::{handle_key, handle_mouse, handle_paste, KeyOutcome, Session};
use crate::model::Canvas;
use crate::store::{State, Store, StoreError};
use crate::ui::menu::{self, MenuItem};
use crate::ui::prompt::{self, Prompt, PromptHit, PromptKind};
use crate::ui::scope::{self, ExportFormat};
use crate::ui::switcher::{self, Switcher, SwitcherHit};
use crate::ui::{self, Overlay};
use crate::view::View;
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

mod export;

const AUTOSAVE: Duration = Duration::from_millis(500);

pub struct App {
    pub session: Session,
    pub overlay: Overlay,
    pub status: Option<String>,
    pub name: String,
    pub quit: bool,
    pick: Option<export::Pick>,
    store: Store,
    state: State,
    saved_rev: u64,
    seen_rev: u64,
    last_edit: Instant,
}

impl App {
    pub fn open(store: Store, name: Option<String>, screen: (u16, u16)) -> Result<App, StoreError> {
        store.list()?;
        let state = store.load_state();
        let name = name
            .or_else(|| state.last.clone())
            .unwrap_or_else(|| "untitled".into());
        let mut app = App {
            session: Session::new(Canvas::default(), View::default(), screen),
            overlay: Overlay::None,
            status: None,
            name: String::new(),
            quit: false,
            pick: None,
            store,
            state,
            saved_rev: 0,
            seen_rev: 0,
            last_edit: Instant::now(),
        };
        app.load_into(&name)?;
        Ok(app)
    }

    fn load_into(&mut self, name: &str) -> Result<(), StoreError> {
        let canvas = if self.store.exists(name) {
            self.store.load(name)?
        } else {
            self.store.create(name)?;
            Canvas::default()
        };
        let (ox, oy) = self.state.offsets.get(name).copied().unwrap_or_default();
        self.session.replace_canvas(canvas, View { ox, oy });
        self.name = name.to_string();
        self.saved_rev = 0;
        self.seen_rev = 0;
        self.state.last = Some(name.to_string());
        self.store.save_state(&self.state)
    }

    fn switch_to(&mut self, name: &str) {
        self.save();
        let result = self.load_into(name);
        self.report(result);
    }

    fn report(&mut self, r: Result<(), StoreError>) {
        if let Err(e) = r {
            self.status = Some(e.to_string());
        }
    }

    pub fn save(&mut self) {
        let offset = (self.session.view.ox, self.session.view.oy);
        if self.state.offsets.insert(self.name.clone(), offset) != Some(offset) {
            let _ = self.store.save_state(&self.state);
        }
        if self.saved_rev == self.session.rev {
            return;
        }
        self.saved_rev = self.session.rev;
        let r = self
            .store
            .save(&self.name, &self.session.canvas)
            .map(|_| ());
        self.report(r);
    }

    pub fn tick(&mut self, now: Instant) {
        if self.seen_rev != self.session.rev {
            self.seen_rev = self.session.rev;
            self.last_edit = now;
        }
        if self.saved_rev != self.session.rev && now.duration_since(self.last_edit) >= AUTOSAVE {
            self.save();
        }
    }

    pub fn render(&self, buf: &mut Buffer, area: Rect) {
        ui::render(
            buf,
            area,
            &self.session,
            &self.overlay,
            self.status.as_deref(),
        );
    }

    pub fn on_event(&mut self, ev: Event, now: Instant) {
        match ev {
            Event::Resize(w, h) => self.session.screen = (w, h),
            Event::Key(k) => self.on_key(k),
            Event::Paste(text) => match &mut self.overlay {
                Overlay::Prompt(p) => p.buf.extend(text.chars().filter(|c| !c.is_control())),
                Overlay::None => handle_paste(&mut self.session, &text),
                _ => {}
            },
            Event::Mouse(m) => {
                if matches!(m.kind, MouseEventKind::Down(_)) {
                    self.status = None;
                }
                if matches!(self.overlay, Overlay::None) && self.pick_mouse(m) {
                    return;
                }
                match self.overlay {
                    Overlay::None => handle_mouse(&mut self.session, m, now),
                    _ if m.kind == MouseEventKind::Down(MouseButton::Left) => {
                        self.overlay_click(i32::from(m.column), i32::from(m.row))
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn on_key(&mut self, k: crossterm::event::KeyEvent) {
        self.status = None;
        if matches!(self.overlay, Overlay::None) {
            if self.pick_key(k) {
                return;
            }
            match handle_key(&mut self.session, k) {
                KeyOutcome::Quit => self.quit = true,
                KeyOutcome::Menu => self.overlay = Overlay::Menu,
                KeyOutcome::None => {}
            }
            return;
        }
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        match (k.code, &mut self.overlay) {
            (KeyCode::Esc, _) => self.overlay = Overlay::None,
            (KeyCode::Char(c), Overlay::Menu) => {
                if let Some(item) = menu::by_key(c) {
                    self.menu_item(item);
                }
            }
            (KeyCode::Char(c), Overlay::Prompt(p)) => p.buf.push(c),
            (KeyCode::Backspace, Overlay::Prompt(p)) => {
                p.buf.pop();
            }
            (KeyCode::Enter, Overlay::Prompt(_)) => self.confirm(),
            _ => {}
        }
    }

    fn overlay_click(&mut self, col: i32, row: i32) {
        let screen = self.session.screen;
        match &self.overlay {
            Overlay::Menu => match menu::entry_at(screen, col, row) {
                Some(item) => self.menu_item(item),
                None if !menu::inside(screen, col, row) => self.overlay = Overlay::None,
                None => {}
            },
            Overlay::Scope(format) => match scope::entry_at(screen, col, row) {
                Some(item) => self.scope_item(item, *format),
                None if !scope::inside(screen, col, row) => self.overlay = Overlay::None,
                None => {}
            },
            Overlay::Switcher(sw) => match switcher::hit(sw, screen, col, row) {
                Some(h) => self.switcher_hit(h),
                None => self.overlay = Overlay::None,
            },
            Overlay::Prompt(_) => match prompt::hit(screen, col, row) {
                Some(PromptHit::Ok) => self.confirm(),
                Some(PromptHit::Cancel) => self.overlay = Overlay::None,
                None => {}
            },
            Overlay::None => {}
        }
    }

    fn menu_item(&mut self, item: MenuItem) {
        self.overlay = Overlay::None;
        match item {
            MenuItem::Canvases => self.open_switcher(),
            MenuItem::FitAll => {
                let bounds = self.session.canvas.all_bounds();
                self.session.view.fit_all(bounds, self.session.screen);
            }
            MenuItem::ExportSvg => self.overlay = Overlay::Scope(ExportFormat::Svg),
            MenuItem::ExportMermaid => self.overlay = Overlay::Scope(ExportFormat::Mermaid),
            MenuItem::Quit => self.quit = true,
        }
    }

    fn open_switcher(&mut self) {
        self.save();
        let names = self.store.list().unwrap_or_default();
        self.overlay = Overlay::Switcher(Switcher {
            names,
            current: self.name.clone(),
        });
    }

    fn switcher_hit(&mut self, h: SwitcherHit) {
        let Overlay::Switcher(sw) = &self.overlay else {
            return;
        };
        let name = |i: usize| sw.names[i].clone();
        let prompt = |kind, buf| Overlay::Prompt(Prompt::new(kind, buf));
        match h {
            SwitcherHit::New => self.overlay = prompt(PromptKind::NewCanvas, String::new()),
            SwitcherHit::Rename(i) => self.overlay = prompt(PromptKind::Rename(name(i)), name(i)),
            SwitcherHit::Delete(i) => {
                self.overlay = prompt(PromptKind::ConfirmDelete(name(i)), String::new())
            }
            SwitcherHit::Open(i) => {
                let n = name(i);
                self.overlay = Overlay::None;
                if n != self.name {
                    self.switch_to(&n);
                }
            }
        }
    }

    fn confirm(&mut self) {
        let Overlay::Prompt(p) = std::mem::replace(&mut self.overlay, Overlay::None) else {
            return;
        };
        match p.kind {
            PromptKind::ExportSvg(a) => self.export(&p.buf, ExportFormat::Svg, a),
            PromptKind::ExportMermaid(a) => self.export(&p.buf, ExportFormat::Mermaid, a),
            PromptKind::NewCanvas => match self.store.create(&p.buf) {
                Ok(()) => self.switch_to(&p.buf),
                Err(e) => self.status = Some(e.to_string()),
            },
            PromptKind::Rename(old) => {
                self.save();
                let r = self.store.rename(&old, &p.buf);
                if r.is_ok() && old == self.name {
                    self.name = p.buf.clone();
                    self.state.last = Some(p.buf.clone());
                    self.session.rev += 1;
                }
                self.report(r);
                self.open_switcher();
            }
            PromptKind::ConfirmDelete(name) => {
                let r = self.store.delete(&name);
                if r.is_ok() && name == self.name {
                    self.saved_rev = self.session.rev;
                    let next = self.store.list().ok().and_then(|l| l.into_iter().next());
                    let r = self.load_into(&next.unwrap_or_else(|| "untitled".into()));
                    self.report(r);
                }
                self.report(r);
                self.open_switcher();
            }
        }
    }
}
