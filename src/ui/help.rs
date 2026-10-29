use super::registry::{Scope, BINDINGS};
use super::style::{ACTIVE, MUTED};
use super::{boxed, centered, Grid};
use unicode_width::UnicodeWidthStr;

pub const SCOPES: [&str; 6] = [
    "Canvas",
    "Selection",
    "Text edit",
    "Tools",
    "Commands",
    "Project",
];

const PROJECT: [(&str, &str); 2] = [
    ("Repository", "github.com/shortcuts/clidraw"),
    ("Contributing", "github.com/shortcuts/clidraw#contributing"),
];

const MAX_ROWS: usize = 18;
const WIDTH: i32 = 64;
const CHROME_ROWS: i32 = 4;
const ACTION_COL: i32 = 20;

/// Keys and action for each row of a page.
pub fn rows(page: usize) -> Vec<(String, String)> {
    let scoped = |scopes: &[Scope]| -> Vec<(String, String)> {
        BINDINGS
            .iter()
            .filter(|b| scopes.contains(&b.scope) && (b.shown.is_some() || !b.keys.is_empty()))
            .map(|b| (b.keys_label(), b.name().to_string()))
            .collect()
    };
    let project = || PROJECT.map(|(k, a)| (k.to_string(), a.to_string()));
    match page {
        0 => scoped(&[Scope::Canvas, Scope::ZoomedOut]),
        1 => scoped(&[Scope::Selection]),
        2 => scoped(&[Scope::TextEdit]),
        3 => scoped(&[Scope::Tool]),
        4 => scoped(&[Scope::Command]),
        _ => project().into(),
    }
}

/// Rows of every page matching the query, best match first.
pub fn search(query: &str) -> Vec<(String, String)> {
    let all: Vec<_> = (0..SCOPES.len()).flat_map(rows).collect();
    let hay: Vec<String> = all.iter().map(|r| format!("{} {}", r.0, r.1)).collect();
    crate::search::find(query, &hay)
        .into_iter()
        .take(MAX_ROWS)
        .map(|i| all[i].clone())
        .collect()
}

/// Fits the longest page so the box does not jump between tabs.
fn rect(screen: (u16, u16)) -> (i32, i32, i32, i32) {
    let longest = (0..SCOPES.len()).map(|p| rows(p).len()).max().unwrap_or(0);
    centered(screen, WIDTH, CHROME_ROWS + longest as i32)
}

/// Column offset from the box edge and width of each tab, drawn as " name ".
fn tabs() -> impl Iterator<Item = (usize, i32, i32)> {
    SCOPES.iter().enumerate().scan(2, |at, (i, name)| {
        let w = name.width() as i32 + 2;
        let start = *at;
        *at += w + 1;
        Some((i, start, w))
    })
}

pub fn tab_at(screen: (u16, u16), col: i32, row: i32) -> Option<usize> {
    let (x, y, _, _) = rect(screen);
    if row != y + 1 {
        return None;
    }
    tabs()
        .find(|&(_, start, w)| col >= x + start && col < x + start + w)
        .map(|t| t.0)
}

pub fn inside(screen: (u16, u16), col: i32, row: i32) -> bool {
    let (x, y, w, h) = rect(screen);
    col >= x && col < x + w && row >= y && row < y + h
}

pub fn draw(grid: &mut Grid, page: usize, query: &str) {
    let screen = (grid.w as u16, grid.h as u16);
    let r = rect(screen);
    boxed(grid, r, "Shortcuts");
    for (i, start, _) in tabs() {
        let label = format!(" {} ", SCOPES[i]);
        let on = i == page && query.is_empty();
        grid.text(r.0 + start, r.1 + 1, &label, if on { ACTIVE } else { 0 });
    }
    if query.is_empty() {
        grid.text(r.0 + 2, r.1 + 2, "Type to filter", MUTED);
    } else {
        grid.text(r.0 + 2, r.1 + 2, &format!("> {query}"), 0);
    }
    let shown = if query.is_empty() {
        rows(page)
    } else {
        search(query)
    };
    if shown.is_empty() {
        grid.text(r.0 + 2, r.1 + 3, "No match", MUTED);
    }
    for (i, (keys, action)) in shown.iter().take((r.3 - CHROME_ROWS) as usize).enumerate() {
        let y = r.1 + 3 + i as i32;
        grid.text(r.0 + 2, y, keys, 0);
        grid.text(r.0 + ACTION_COL, y, action, 0);
    }
}
