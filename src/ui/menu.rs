use super::grid::DIM;
use super::{boxed, centered, Grid};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    Canvases,
    FitAll,
    ExportSvg,
    ExportMermaid,
    Quit,
}

/// Item, label, hotkey. The shortcuts modal can list this table.
pub const ITEMS: [(MenuItem, &str, char); 5] = [
    (MenuItem::Canvases, "Canvases", 'c'),
    (MenuItem::FitAll, "Fit all", 'f'),
    (MenuItem::ExportSvg, "Export SVG", 's'),
    (MenuItem::ExportMermaid, "Export Mermaid", 'm'),
    (MenuItem::Quit, "Quit", 'q'),
];

fn rect(screen: (u16, u16)) -> (i32, i32, i32, i32) {
    centered(screen, 22, ITEMS.len() as i32 + 2)
}

/// None when the click lands outside every entry.
pub fn entry_at(screen: (u16, u16), col: i32, row: i32) -> Option<MenuItem> {
    let (x, y, w, _) = rect(screen);
    let i = usize::try_from(row - y - 1).ok()?;
    (col > x && col < x + w - 1).then(|| ITEMS.get(i).map(|e| e.0))?
}

pub fn by_key(c: char) -> Option<MenuItem> {
    ITEMS.iter().find(|e| e.2 == c).map(|e| e.0)
}

pub fn inside(screen: (u16, u16), col: i32, row: i32) -> bool {
    let (x, y, w, h) = rect(screen);
    col >= x && col < x + w && row >= y && row < y + h
}

pub fn draw(grid: &mut Grid) {
    let screen = (grid.w as u16, grid.h as u16);
    let r = rect(screen);
    boxed(grid, r, "Menu");
    for (i, (_, label, key)) in ITEMS.iter().enumerate() {
        let y = r.1 + 1 + i as i32;
        grid.text(r.0 + 2, y, label, 0);
        grid.text(r.0 + r.2 - 3, y, &key.to_string(), DIM);
    }
}
