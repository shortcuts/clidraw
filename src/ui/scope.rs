use super::{boxed, centered, Grid};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Svg,
    Mermaid,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeItem {
    Whole,
    Area,
    Selection,
}

const ITEMS: [(ScopeItem, &str); 3] = [
    (ScopeItem::Whole, "Whole canvas"),
    (ScopeItem::Area, "Area"),
    (ScopeItem::Selection, "Selection"),
];

fn rect(screen: (u16, u16)) -> (i32, i32, i32, i32) {
    centered(screen, 22, ITEMS.len() as i32 + 2)
}

/// None when the click lands outside every entry.
pub fn entry_at(screen: (u16, u16), col: i32, row: i32) -> Option<ScopeItem> {
    let (x, y, w, _) = rect(screen);
    let i = usize::try_from(row - y - 1).ok()?;
    (col > x && col < x + w - 1).then(|| ITEMS.get(i).map(|e| e.0))?
}

pub fn inside(screen: (u16, u16), col: i32, row: i32) -> bool {
    let (x, y, w, h) = rect(screen);
    col >= x && col < x + w && row >= y && row < y + h
}

pub fn draw(grid: &mut Grid) {
    let r = rect((grid.w as u16, grid.h as u16));
    boxed(grid, r, "Export");
    for (i, (_, label)) in ITEMS.iter().enumerate() {
        grid.text(r.0 + 2, r.1 + 1 + i as i32, label, 0);
    }
}
