use super::{boxed, centered, Grid};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    Canvases,
    FitAll,
    ExportSvg,
    ExportMermaid,
    Quit,
}

const ITEMS: [(MenuItem, &str); 5] = [
    (MenuItem::Canvases, "Canvases"),
    (MenuItem::FitAll, "Fit all"),
    (MenuItem::ExportSvg, "Export SVG"),
    (MenuItem::ExportMermaid, "Export Mermaid"),
    (MenuItem::Quit, "Quit"),
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

pub fn inside(screen: (u16, u16), col: i32, row: i32) -> bool {
    let (x, y, w, h) = rect(screen);
    col >= x && col < x + w && row >= y && row < y + h
}

pub fn draw(grid: &mut Grid) {
    let screen = (grid.w as u16, grid.h as u16);
    let r = rect(screen);
    boxed(grid, r, "Menu");
    for (i, (_, label)) in ITEMS.iter().enumerate() {
        grid.text(r.0 + 2, r.1 + 1 + i as i32, label, 0);
    }
}
