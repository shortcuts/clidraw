use crate::input::{handle_key, handle_mouse, handle_paste, scroll_step, KeyOutcome, Session};
use crate::model::Z;
use crate::names;
use crate::store::{State, Store, StoreError};
use crate::ui::list::Click;
use crate::ui::menu::{self, ContextItem, MenuItem};
use crate::ui::minimap;
use crate::ui::prompt::{self, Prompt, PromptHit, PromptKind};
use crate::ui::scope::ExportFormat;
use crate::ui::switcher::{Switcher, SwitcherHit};
use crate::ui::{self, Overlay};
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

mod actions;
mod command;
mod export;
mod help;
mod import;
mod library;
mod list;
mod open;

pub struct App {
    pub session: Session,
    pub overlay: Overlay,
    pub status: Option<ui::Status>,
    pub perf: Option<ui::Perf>,
    pub name: String,
    pub quit: bool,
    pub dirty: bool,
    pointer: &'static str,
    pointer_sent: &'static str,
    pick: Option<export::Pick>,
    store: Store,
    state: State,
    saved_rev: u64,
    seen_rev: u64,
    last_edit: Instant,
    autosave: Duration,
    default_canvas: String,
    export_padding: i32,
    quick_fixes: bool,
    mermaid_header: String,
}

impl App {
    fn switch_to(&mut self, name: &str) {
        self.save();
        let result = self.load_into(name);
        self.report(result);
    }

    fn report(&mut self, r: Result<(), StoreError>) {
        if let Err(e) = r {
            self.dirty = true;
            self.status = Some(ui::Status::error(e.to_string()));
        }
    }

    pub fn save(&mut self) {
        if let Overlay::Kinds(origins, _) = &self.overlay {
            self.session.restore_kinds(origins);
        }
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
            .and_then(|_| {
                let undo = self.session.history.commands();
                self.store.save_history(&self.name, undo)
            });
        self.report(r);
    }

    pub fn tick(&mut self, now: Instant) {
        if self.seen_rev != self.session.rev {
            self.seen_rev = self.session.rev;
            self.last_edit = now;
        }
        self.dirty |= self.session.edge_pan(now);
        self.dirty |= self.session.glide_step(now);
        self.dirty |= self.session.tip_step(now);
        if self.saved_rev != self.session.rev
            && !self.session.dragging()
            && !matches!(self.overlay, Overlay::Kinds(..))
            && now.duration_since(self.last_edit) >= self.autosave
        {
            self.save();
        }
    }

    pub fn next_wake(&self, now: Instant) -> Option<Duration> {
        if let wait @ Some(_) = self.session.edge_wait(now) {
            return wait;
        }
        if let wait @ Some(_) = self.session.glide_wait() {
            return wait;
        }
        if let wait @ Some(_) = self.session.tip_wait(now) {
            return wait;
        }
        if self.saved_rev == self.session.rev || self.session.dragging() {
            return None;
        }
        let deadline = self.last_edit + self.autosave;
        Some(deadline.saturating_duration_since(now))
    }

    pub fn render(&self, buf: &mut Buffer, area: Rect) {
        ui::render(
            buf,
            area,
            &self.session,
            &self.overlay,
            self.status.as_ref(),
            self.perf.as_ref(),
            &self.doc(),
        );
    }

    fn doc(&self) -> ui::Doc<'_> {
        ui::Doc {
            name: &self.name,
            saved: self.saved_rev == self.session.rev,
            picking: self.pick.is_some(),
        }
    }

    /// Handles a queue of events, keeping only the last of each run of drags or moves
    /// and summing each run of same-direction scrolls.
    pub fn on_events(&mut self, evs: Vec<Event>, now: Instant) {
        let kind = |e: Option<&Event>| match e {
            Some(Event::Mouse(m)) => Some(m.kind),
            _ => None,
        };
        let step_of = |e: Option<&Event>| match e {
            Some(Event::Mouse(m)) => scroll_step(m),
            _ => None,
        };
        let same_run = |a, b| {
            matches!(
                (a, b),
                (Some(MouseEventKind::Drag(_)), Some(MouseEventKind::Drag(_)))
                    | (Some(MouseEventKind::Moved), Some(MouseEventKind::Moved))
            )
        };
        let mut evs = evs.into_iter().peekable();
        while let Some(ev) = evs.next() {
            if self.quit {
                break;
            }
            let k = kind(Some(&ev));
            if same_run(k, kind(evs.peek())) {
                continue;
            }
            if let Some(step) = step_of(Some(&ev)) {
                let mut n = 1;
                while step_of(evs.peek()) == Some(step) {
                    evs.next();
                    n += 1;
                }
                if matches!(self.overlay, Overlay::None) {
                    self.dirty = true;
                    self.session.scroll(step, n, now);
                }
                continue;
            }
            self.on_event(ev, now);
        }
    }

    pub fn on_event(&mut self, ev: Event, now: Instant) {
        self.dispatch(ev, now);
        let shape = self.pointer_shape();
        self.dirty |= shape != std::mem::replace(&mut self.pointer, shape);
    }

    fn pointer_shape(&self) -> &'static str {
        match self.overlay {
            Overlay::None => self.session.pointer(),
            _ => "default",
        }
    }

    /// The pointer shape to send, or `None` when the terminal already shows it.
    pub fn take_pointer(&mut self) -> Option<&'static str> {
        (self.pointer != std::mem::replace(&mut self.pointer_sent, self.pointer))
            .then_some(self.pointer)
    }

    fn dispatch(&mut self, ev: Event, now: Instant) {
        self.dirty |= !matches!(&ev, Event::Mouse(m) if m.kind == MouseEventKind::Moved);
        match ev {
            Event::Resize(w, h) => self.session.screen = (w, h),
            Event::Key(k) => self.on_key(k),
            Event::Paste(text) => match &mut self.overlay {
                Overlay::Prompt(p) => p
                    .edit
                    .insert(&text.chars().filter(|c| !c.is_control()).collect::<String>()),
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
                if m.kind == MouseEventKind::Down(MouseButton::Right)
                    && matches!(self.overlay, Overlay::None | Overlay::Context(_))
                {
                    return self.open_context(i32::from(m.column), i32::from(m.row));
                }
                if m.kind == MouseEventKind::Down(MouseButton::Left)
                    && matches!(self.overlay, Overlay::None)
                    && !self.session.present
                {
                    let (col, row) = (i32::from(m.column), i32::from(m.row));
                    if let Some(b) = ui::button_at(&self.session, &self.doc(), col, row) {
                        return self.button(b);
                    }
                }
                match self.overlay {
                    Overlay::None => {
                        let before = (self.session.hover.clone(), self.session.palette_hover);
                        handle_mouse(&mut self.session, m, now);
                        self.dirty |=
                            (self.session.hover.clone(), self.session.palette_hover) != before;
                        if self.session.palette_hover != before.1 {
                            self.session.tip_reset(now);
                        }
                    }
                    _ if m.kind == MouseEventKind::Down(MouseButton::Left) => {
                        self.overlay_click(i32::from(m.column), i32::from(m.row))
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn button(&mut self, b: ui::Button) {
        use crate::ui::actions::Action;
        let action = match b {
            ui::Button::ZoomOut => Action::ZoomOut,
            ui::Button::ZoomIn => Action::ZoomIn,
            ui::Button::Fit => Action::Menu(MenuItem::FitAll),
            ui::Button::Undo | ui::Button::Redo if !self.session.can_edit() => return,
            ui::Button::Undo => Action::Ex("u"),
            ui::Button::Redo => Action::Ex("redo"),
        };
        self.run_action(action);
    }

    fn on_key(&mut self, k: crossterm::event::KeyEvent) {
        self.status = None;
        if matches!(self.overlay, Overlay::None) {
            if self.pick_key(k) {
                return;
            }
            match handle_key(&mut self.session, k) {
                KeyOutcome::Quit => self.quit = true,
                KeyOutcome::Help => self.overlay = Overlay::Help(0, String::new()),
                KeyOutcome::Actions => {
                    if self.pick.is_none() {
                        self.overlay = Overlay::Actions(ui::actions::list(&self.session));
                    }
                }
                KeyOutcome::Command => {
                    self.overlay = Overlay::Prompt(Prompt::new(PromptKind::Command, String::new()))
                }
                KeyOutcome::Find => {
                    self.overlay = Overlay::Prompt(Prompt::new(PromptKind::Command, "/".into()))
                }
                KeyOutcome::Notice(text) => self.status = Some(ui::Status::notice(text)),
                KeyOutcome::None => {}
            }
            return;
        }
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if matches!(self.overlay, Overlay::Help(..)) {
            self.help_key(k);
            return;
        }
        if matches!(
            self.overlay,
            Overlay::Context(_)
                | Overlay::Formats(_)
                | Overlay::Scope(..)
                | Overlay::Colors(..)
                | Overlay::Kinds(..)
                | Overlay::Switcher(_)
                | Overlay::Actions(_)
        ) {
            self.list_key(k);
            return;
        }
        match (k.code, &self.overlay) {
            (_, Overlay::Prompt(_)) => self.prompt_key(k),
            (KeyCode::Tab, Overlay::Map(t)) => self.overlay = Overlay::Map(t.next()),
            (KeyCode::Esc, _) => self.overlay = Overlay::None,
            _ => {}
        }
    }

    fn overlay_click(&mut self, col: i32, row: i32) {
        let screen = self.session.screen;
        match &self.overlay {
            Overlay::Context(_)
            | Overlay::Formats(_)
            | Overlay::Scope(..)
            | Overlay::Colors(..)
            | Overlay::Kinds(..)
            | Overlay::Switcher(_)
            | Overlay::Actions(_) => self.list_click(col, row),
            Overlay::Prompt(p)
                if matches!(p.kind, PromptKind::Command | PromptKind::Mermaid(_)) =>
            {
                let bar = i32::from(screen.1) - 1;
                match prompt::command_list(p, bar).click(screen, col, row) {
                    Click::Row(b, _) => self.complete(b),
                    Click::Outside if row != bar => self.overlay = Overlay::None,
                    _ => {}
                }
            }
            Overlay::Prompt(_) => match prompt::hit(screen, col, row) {
                Some(PromptHit::Ok) => self.confirm(),
                Some(PromptHit::Cancel) => self.overlay = Overlay::None,
                None => {}
            },
            Overlay::Help(..) => self.help_click(col, row),
            Overlay::Map(t) => {
                minimap::click(&mut self.session, col, row, *t);
                self.overlay = Overlay::None;
            }
            Overlay::None => {}
        }
    }

    fn menu_item(&mut self, item: MenuItem) {
        self.overlay = Overlay::None;
        match item {
            MenuItem::Canvases => self.open_switcher(),
            MenuItem::FitAll => self.fit_all(),
            MenuItem::ExportSvg => self.pick_scope(ExportFormat::Svg),
            MenuItem::ExportMermaid => self.pick_scope(ExportFormat::Mermaid),
            MenuItem::ExportClidraw => self.pick_scope(ExportFormat::Clidraw),
            MenuItem::Quit => self.quit = true,
        }
    }

    fn fit_all(&mut self) {
        let bounds = self.session.canvas.all_bounds();
        self.session.fit(bounds);
    }

    fn open_context(&mut self, col: i32, row: i32) {
        self.overlay = Overlay::None;
        if self.pick.is_some() {
            return;
        }
        let at = (col, row);
        self.overlay = match self.session.right_down(col, row) {
            None => return,
            Some(true) => Overlay::Context(menu::element(
                at,
                self.session.selection.len() == 1,
                self.session.line_has_bends(),
            )),
            Some(false) => {
                let can_paste = !self.session.clipboard.elements.is_empty();
                Overlay::Context(menu::canvas(at, can_paste))
            }
        };
    }

    fn context_item(&mut self, item: ContextItem) {
        self.overlay = Overlay::None;
        let s = &mut self.session;
        match item {
            ContextItem::EditLabel => {
                if let Some(id) = s.selection.first().cloned() {
                    s.start_edit_existing(id);
                }
            }
            ContextItem::Duplicate => s.duplicate(),
            ContextItem::Delete => s.delete_selection(),
            ContextItem::Front => s.reorder(Z::Front),
            ContextItem::Forward => s.reorder(Z::Forward),
            ContextItem::Backward => s.reorder(Z::Backward),
            ContextItem::Back => s.reorder(Z::Back),
            ContextItem::ResetRoute => s.reset_route(),
            ContextItem::Paste => s.paste(),
            ContextItem::SelectAll => s.select_all(),
            ContextItem::FitAll => self.fit_all(),
        }
    }

    fn open_switcher(&mut self) {
        self.save();
        let names = self.store.list().unwrap_or_default();
        self.overlay = Overlay::Switcher(Switcher::new(names, &self.name));
    }

    fn switcher_hit(&mut self, h: SwitcherHit) {
        let Overlay::Switcher(sw) = &self.overlay else {
            return;
        };
        let name = |i: usize| sw.names[i].clone();
        let prompt = |kind, buf| Overlay::Prompt(Prompt::new(kind, buf));
        match h {
            SwitcherHit::New => {
                let fresh = names::generate(|n| self.store.exists(n));
                self.overlay = prompt(PromptKind::NewCanvas, fresh)
            }
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
        let buf = p.edit.buf;
        match p.kind {
            PromptKind::Command => self.run_command(&buf),
            PromptKind::Mermaid(header) => {
                if !buf.trim().is_empty() {
                    self.import_text(&format!("{header}\n{buf}"));
                }
            }
            PromptKind::ExportSvg(x) => self.export(&buf, ExportFormat::Svg, x),
            PromptKind::ExportMermaid(x) => self.export(&buf, ExportFormat::Mermaid, x),
            PromptKind::ExportClidraw(x) => self.export(&buf, ExportFormat::Clidraw, x),
            PromptKind::NewCanvas => match self.store.create(&buf) {
                Ok(()) => self.switch_to(&buf),
                Err(e) => self.status = Some(ui::Status::error(e.to_string())),
            },
            PromptKind::Rename(old) => {
                self.save();
                let r = self.store.rename(&old, &buf);
                if r.is_ok() && old == self.name {
                    self.name = buf.clone();
                    self.state.last = Some(buf.clone());
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
                    let r = self.load_into(&next.unwrap_or_else(|| self.default_canvas.clone()));
                    self.report(r);
                }
                self.report(r);
                self.open_switcher();
            }
        }
    }
}
