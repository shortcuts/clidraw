use super::App;
use crate::config::Config;
use crate::input::Session;
use crate::model::Canvas;
use crate::store::{Store, StoreError};
use crate::ui::theme::{Theme, ThemeSetting};
use crate::ui::{self, Overlay};
use crate::view::View;
use std::time::Instant;

fn apply_session(s: &mut Session, c: &Config) {
    s.double_click = c.double_click;
    s.grid = c.grid;
    s.chrome_accent = c.chrome_accent;
    s.motion = c.motion;
    s.quick_connect = c.quick_connect;
    s.command_ui = c.command_ui;
    s.scroll_speed = c.scroll_speed;
    s.history.limit = c.history_limit;
}

impl App {
    pub fn open(store: Store, name: Option<String>, screen: (u16, u16)) -> Result<App, StoreError> {
        App::open_with(store, name, screen, Config::default(), vec![])
    }

    pub fn open_with(
        store: Store,
        name: Option<String>,
        screen: (u16, u16),
        config: Config,
        warnings: Vec<String>,
    ) -> Result<App, StoreError> {
        store.list()?;
        let state = store.load_state();
        let name = name
            .or_else(|| state.last.clone())
            .unwrap_or_else(|| config.default_canvas.clone());
        let mut session = Session::new(Canvas::default(), View::default(), screen);
        apply_session(&mut session, &config);
        let mut app = App {
            session,
            overlay: Overlay::None,
            status: (!warnings.is_empty()).then(|| ui::Status::error(warnings.join("; "))),
            perf: None,
            name: String::new(),
            quit: false,
            dirty: true,
            pointer: "default",
            pointer_sent: "default",
            copy: None,
            pick: None,
            store,
            state,
            saved_rev: 0,
            seen_rev: 0,
            disk: None,
            last_edit: Instant::now(),
            autosave: config.autosave,
            default_canvas: config.default_canvas,
            export_padding: config.export_padding,
            quick_fixes: config.quick_fixes,
            mermaid_header: "flowchart TD".into(),
            obsidian_vault: config.obsidian_vault,
            pending_canvas: None,
        };
        app.load_into(&name)?;
        Ok(app)
    }

    pub(super) fn apply_config(&mut self, c: &Config) {
        apply_session(&mut self.session, c);
        self.autosave = c.autosave;
        self.default_canvas = c.default_canvas.clone();
        self.export_padding = c.export_padding;
        self.quick_fixes = c.quick_fixes;
        self.obsidian_vault = c.obsidian_vault.clone();
        match c.theme {
            ThemeSetting::Dark => self.session.theme = Theme::Dark,
            ThemeSetting::Light => self.session.theme = Theme::Light,
            ThemeSetting::Auto => {}
        }
    }

    pub(super) fn load_into(&mut self, name: &str) -> Result<(), StoreError> {
        let canvas = if self.store.exists(name) {
            self.store.load(name)?
        } else {
            self.store.create(name)?;
            Canvas::default()
        };
        let (ox, oy) = self.state.offsets.get(name).copied().unwrap_or_default();
        self.install(
            name,
            canvas,
            View {
                ox,
                oy,
                ..View::default()
            },
        );
        self.state.last = Some(name.to_string());
        self.store.save_state(&self.state)
    }

    pub(super) fn reload(&mut self) -> Result<(), StoreError> {
        let canvas = self.store.load(&self.name)?;
        let name = self.name.clone();
        self.install(&name, canvas, self.session.view);
        Ok(())
    }

    fn install(&mut self, name: &str, canvas: Canvas, view: View) {
        self.session.replace_canvas(canvas, view);
        let (undo, redo) = self.store.load_history(name);
        self.session.history.restore(undo, redo);
        self.name = name.to_string();
        self.saved_rev = 0;
        self.seen_rev = 0;
        self.disk = self.store.canvas_hash(name);
    }
}
