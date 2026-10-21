use super::App;
use crate::config::Config;
use crate::input::Session;
use crate::model::Canvas;
use crate::store::{Store, StoreError};
use crate::ui::{self, Overlay};
use crate::view::View;
use std::time::Instant;

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
        session.dock = config.dock;
        session.double_click = config.double_click;
        session.grid = config.grid;
        session.motion = config.motion;
        session.history.limit = config.history_limit;
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
            pick: None,
            store,
            state,
            saved_rev: 0,
            seen_rev: 0,
            last_edit: Instant::now(),
            autosave: config.autosave,
            default_canvas: config.default_canvas,
            export_padding: config.export_padding,
        };
        app.load_into(&name)?;
        Ok(app)
    }

    pub(super) fn load_into(&mut self, name: &str) -> Result<(), StoreError> {
        let canvas = if self.store.exists(name) {
            self.store.load(name)?
        } else {
            self.store.create(name)?;
            Canvas::default()
        };
        let (ox, oy) = self.state.offsets.get(name).copied().unwrap_or_default();
        self.session.replace_canvas(canvas, View { ox, oy });
        self.session.history.restore(self.store.load_history(name));
        self.name = name.to_string();
        self.saved_rev = 0;
        self.seen_rev = 0;
        self.state.last = Some(name.to_string());
        self.store.save_state(&self.state)
    }
}
