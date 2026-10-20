mod canvas_view;
mod grid;
pub mod help;
mod infobar;
mod layout;
pub mod menu;
pub mod palette;
mod perf;
pub mod prompt;
pub mod scope;
pub mod switcher;
mod theme;

use crate::input::Session;
use grid::Grid;
pub use layout::Layout;
pub use perf::Perf;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub enum Overlay {
    None,
    Menu,
    Context(menu::Context),
    Scope(scope::ExportFormat),
    Switcher(switcher::Switcher),
    Prompt(prompt::Prompt),
    Help(usize),
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

pub fn render(
    buf: &mut Buffer,
    area: Rect,
    s: &Session,
    overlay: &Overlay,
    status: Option<&Status>,
    perf: Option<&Perf>,
    saved: bool,
) {
    let mut grid = Grid::new(area.width, area.height);
    let (no_color, colorterm) = (
        std::env::var_os("NO_COLOR").is_some(),
        std::env::var("COLORTERM").ok(),
    );
    let colors = theme::colors(no_color, colorterm.as_deref());
    let accent = theme::accent(no_color, colorterm.as_deref());
    canvas_view::draw(&mut grid, s, accent, colors);
    if !s.overview {
        palette::draw(&mut grid, s);
    }
    infobar::draw(&mut grid, s, overlay, saved);
    if !matches!(overlay, Overlay::None) {
        grid.dim_all();
    }
    match overlay {
        Overlay::None => {}
        Overlay::Menu => menu::draw(&mut grid),
        Overlay::Context(c) => c.draw(&mut grid),
        Overlay::Scope(_) => scope::draw(&mut grid),
        Overlay::Switcher(sw) => switcher::draw(&mut grid, sw),
        Overlay::Prompt(p) => prompt::draw(&mut grid, p),
        Overlay::Help(p) => help::draw(&mut grid, *p),
    }
    if let Some(st) = status {
        infobar::status(&mut grid, s, st, saved);
    }
    if let Some(p) = perf {
        p.draw(&mut grid, s.canvas.elements.len());
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
