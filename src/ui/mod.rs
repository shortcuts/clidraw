pub mod actions;
mod canvas_view;
pub mod colors;
mod grid;
pub mod help;
mod infobar;
pub mod kinds;
mod layout;
pub mod list;
pub mod menu;
pub mod minimap;
pub mod notes;
pub mod palette;
pub mod paths;
mod perf;
pub mod prompt;
pub mod registry;
pub mod scope;
mod style;
pub mod switcher;
pub mod theme;

use crate::input::Session;
use grid::Grid;
pub use infobar::{hit as button_at, Button};
pub use layout::{refresh, Layout};
pub use perf::Perf;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub enum Overlay {
    None,
    Context(list::List<menu::ContextItem>),
    Formats(list::List<scope::ExportFormat>),
    Scope(scope::ExportFormat, list::List<scope::ScopeItem>),
    Colors(palette::Choice, list::List<Option<crate::model::Paint>>),
    Kinds(
        Vec<crate::input::Origin>,
        list::List<crate::model::ShapeKind>,
    ),
    Switcher(switcher::Switcher),
    Notes(prompt::Extent, notes::Notes),
    Prompt(prompt::Prompt),
    Help(usize, String),
    Actions(list::List<actions::Action>),
    Map(minimap::Target),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    Notice,
    Error,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Status {
    pub text: String,
    pub severity: Severity,
}

impl Status {
    pub fn notice(text: impl Into<String>) -> Status {
        Status {
            text: text.into(),
            severity: Severity::Notice,
        }
    }

    pub fn error(text: impl Into<String>) -> Status {
        Status {
            text: text.into(),
            severity: Severity::Error,
        }
    }
}

pub struct Doc<'a> {
    pub name: &'a str,
    pub saved: bool,
    pub picking: bool,
}

pub fn render(
    buf: &mut Buffer,
    area: Rect,
    s: &Session,
    overlay: &Overlay,
    status: Option<&Status>,
    perf: Option<&Perf>,
    doc: &Doc,
) {
    let mut grid = Grid::new(area.width, area.height);
    let (no_color, colorterm) = (
        std::env::var_os("NO_COLOR").is_some(),
        std::env::var("COLORTERM").ok(),
    );
    let colors = theme::colors(no_color, colorterm.as_deref());
    let accent = theme::accent(s.theme, no_color, colorterm.as_deref());
    grid.accent = colors.then(|| s.chrome_accent.unwrap_or_else(|| s.theme.accent()));
    canvas_view::draw(&mut grid, s, accent, colors);
    if !s.present {
        let (w, h) = (i32::from(area.width), i32::from(area.height));
        grid.fill(0, 0, w, palette::rows(s));
        grid.fill(0, h - 1, w, 1);
        palette::draw(&mut grid, s);
        infobar::draw(&mut grid, s, doc, overlay);
    }
    if !matches!(overlay, Overlay::None) {
        grid.dim_all();
    }
    match overlay {
        Overlay::None => {}
        Overlay::Context(c) => c.draw(&mut grid),
        Overlay::Formats(l) => l.draw(&mut grid),
        Overlay::Scope(_, l) => l.draw(&mut grid),
        Overlay::Colors(_, l) => l.draw(&mut grid),
        Overlay::Kinds(_, l) => l.draw(&mut grid),
        Overlay::Switcher(sw) => sw.draw(&mut grid),
        Overlay::Notes(_, n) => n.draw(&mut grid),
        Overlay::Prompt(p)
            if matches!(
                p.kind,
                prompt::PromptKind::Command | prompt::PromptKind::Mermaid(_)
            ) =>
        {
            prompt::draw_command(&mut grid, p)
        }
        Overlay::Prompt(p) => prompt::draw(&mut grid, p),
        Overlay::Help(p, q) => help::draw(&mut grid, *p, q),
        Overlay::Actions(a) => a.draw(&mut grid),
        Overlay::Map(t) => minimap::draw(&mut grid, s, *t),
    }
    if let Some(st) = status.filter(|_| !s.present) {
        infobar::status(&mut grid, s, doc, st);
    }
    if let Some(p) = perf {
        p.draw(&mut grid, s.canvas.elements.len(), s.viewport().1);
    }
    grid.copy_to(buf, area);
}

/// Centered box (x, y, w, h) clamped to the screen; shared by the overlays.
pub(crate) fn centered(screen: (u16, u16), w: i32, h: i32) -> (i32, i32, i32, i32) {
    let (sw, sh) = (i32::from(screen.0), i32::from(screen.1));
    let w = w.min(sw);
    let h = h.min(sh);
    ((sw - w) / 2, (sh - h) / 2, w, h)
}

pub(crate) fn boxed(grid: &mut Grid, (x, y, w, h): (i32, i32, i32, i32), title: &str) {
    grid.fill(x, y, w, h);
    for i in 1..w - 1 {
        grid.set(x + i, y, '─', 0);
        grid.set(x + i, y + h - 1, '─', 0);
    }
    for j in 1..h - 1 {
        grid.set(x, y + j, '│', 0);
        grid.set(x + w - 1, y + j, '│', 0);
    }
    for (cx, cy, c) in [
        (x, y, '┌'),
        (x + w - 1, y, '┐'),
        (x, y + h - 1, '└'),
        (x + w - 1, y + h - 1, '┘'),
    ] {
        grid.set(cx, cy, c, 0);
    }
    grid.text(x + 2, y, &format!(" {title} "), 0);
}
