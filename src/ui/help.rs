use super::grid::{DIM, REVERSE};
use super::{boxed, centered, Grid};
use crate::input::Tool;

pub const SCOPES: [&str; 4] = ["Canvas", "Text edit", "Tools", "Project"];

const CANVAS: [(&str, &str); 18] = [
    ("Esc", "Cancel, deselect"),
    ("Delete, Backspace", "Delete the selection"),
    ("Ctrl-a", "Select everything"),
    ("Arrows, h j k l", "Move one cell; Shift adds linked shape"),
    ("] [", "Raise, lower the selection"),
    ("} {", "Send to front, to back"),
    ("< | > ^ - _", "Align the selection"),
    ("= #", "Distribute evenly, H or V"),
    ("g, G", "Group, ungroup the selection"),
    ("y, x", "Copy, cut the selection"),
    ("p", "Paste, offset from the last"),
    ("Ctrl-b", "Bold labels"),
    ("u, Ctrl-r", "Undo, redo"),
    ("z", "Fit all"),
    ("Ctrl+wheel", "Zoom, 25% to 300%"),
    ("Ctrl-c", "Quit"),
    ("?, Ctrl-p", "Help, command palette"),
    (":", "Commands q w wq e u redo layout"),
];

const TEXT_EDIT: [(&str, &str); 10] = [
    ("Type, paste", "Insert at the caret"),
    ("Left, Right", "Move the caret"),
    ("Home, End", "Jump to the start, end"),
    ("Ctrl-Left/Right", "Jump by word"),
    ("Shift + move", "Select text"),
    ("Backspace, Delete", "Delete a character"),
    ("Ctrl-w, Ctrl-u", "Delete word, to start"),
    ("Alt-Enter, Ctrl-j", "New line in a shape label"),
    ("Enter", "Commit the label"),
    ("Esc", "Discard the edit"),
];

const PROJECT: [(&str, &str); 2] = [
    ("Repository", "github.com/shortcuts/clidraw"),
    ("Contributing", "github.com/shortcuts/clidraw#contributing"),
];

/// Query word and the word the help text uses instead; matched both ways.
pub const SYNONYMS: [(&str, &str); 9] = [
    ("remove", "delete"),
    ("erase", "delete"),
    ("exit", "quit"),
    ("close", "quit"),
    ("front", "raise"),
    ("back", "lower"),
    ("redo", "undo"),
    ("box", "rect"),
    ("text", "label"),
];

const MAX_ROWS: usize = 18;
const WIDTH: i32 = 64;
// Two borders, the tab row, a blank row and the longest page; fixed so the box does not jump.
const HEIGHT: i32 = 22;
const ACTION_COL: i32 = 20;

/// Keys and action for each row of a page.
pub fn rows(page: usize) -> Vec<(String, String)> {
    let own = |t: &[(&str, &str)]| {
        t.iter()
            .map(|(k, a)| (k.to_string(), a.to_string()))
            .collect()
    };
    match page {
        0 => own(&CANVAS),
        1 => own(&TEXT_EDIT),
        3 => own(&PROJECT),
        _ => Tool::ALL
            .iter()
            .filter_map(|t| Some((t.key()?.to_string(), t.label().to_string())))
            .collect(),
    }
}

fn fuzzy(needle: &str, hay: &str) -> bool {
    let mut hay = hay.chars();
    needle.chars().all(|n| hay.any(|h| h == n))
}

/// Rows of every page matching the query: substring hits first, then subsequence hits.
pub fn search(query: &str) -> Vec<(String, String)> {
    let query = query.to_lowercase();
    let mut wanted = vec![query.clone()];
    for (a, b) in SYNONYMS {
        if query == a || query == b {
            wanted.push(if query == a { b } else { a }.to_string());
        }
    }
    let (mut exact, mut loose) = (Vec::new(), Vec::new());
    for row in (0..SCOPES.len()).flat_map(rows) {
        let hay = format!("{} {}", row.0, row.1).to_lowercase();
        if wanted.iter().any(|w| hay.contains(w.as_str())) {
            exact.push(row);
        } else if wanted.iter().any(|w| fuzzy(w, &hay)) {
            loose.push(row);
        }
    }
    exact.extend(loose);
    exact.truncate(MAX_ROWS);
    exact
}

fn rect(screen: (u16, u16)) -> (i32, i32, i32, i32) {
    centered(screen, WIDTH, HEIGHT)
}

/// Column offset from the box edge and width of each tab, drawn as " name ".
fn tabs() -> impl Iterator<Item = (usize, i32, i32)> {
    SCOPES.iter().enumerate().scan(2, |at, (i, name)| {
        let w = name.chars().count() as i32 + 2;
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
        grid.text(r.0 + start, r.1 + 1, &label, if on { REVERSE } else { 0 });
    }
    if query.is_empty() {
        grid.text(r.0 + 2, r.1 + 2, "Type to filter", DIM);
    } else {
        grid.text(r.0 + 2, r.1 + 2, &format!("> {query}"), 0);
    }
    let shown = if query.is_empty() {
        rows(page)
    } else {
        search(query)
    };
    if shown.is_empty() {
        grid.text(r.0 + 2, r.1 + 3, "No match", DIM);
    }
    for (i, (keys, action)) in shown.iter().enumerate() {
        let y = r.1 + 3 + i as i32;
        grid.text(r.0 + 2, y, keys, 0);
        grid.text(r.0 + ACTION_COL, y, action, 0);
    }
}
