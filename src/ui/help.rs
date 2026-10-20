use super::grid::REVERSE;
use super::menu::ITEMS;
use super::{boxed, centered, Grid};
use crate::input::Tool;

pub const SCOPES: [&str; 4] = ["Canvas", "Menu", "Text edit", "Tools"];

const CANVAS: [(&str, &str); 15] = [
    ("Esc", "Cancel, else open menu"),
    ("Delete, Backspace", "Delete the selection"),
    ("Ctrl-a", "Select everything"),
    ("Arrows", "Move the selection one cell"),
    ("] [", "Raise, lower the selection"),
    ("} {", "Send to front, to back"),
    ("g, G", "Group, ungroup the selection"),
    ("c, Ctrl-x", "Copy, cut the selection"),
    ("Ctrl-v", "Paste, offset from the last"),
    ("Ctrl-d", "Duplicate the selection"),
    ("Ctrl-z, Ctrl-y", "Undo, redo"),
    ("z", "Overview of the whole diagram"),
    ("Ctrl-c", "Quit"),
    ("?", "Show this list"),
    (":", "Commands q w wq e u redo"),
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

const WIDTH: i32 = 46;
// Two borders, the tab row, a blank row and the longest page; fixed so the box does not jump.
const HEIGHT: i32 = 19;
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
        1 => ITEMS
            .iter()
            .map(|(_, label, key)| (key.to_string(), label.to_string()))
            .collect(),
        2 => own(&TEXT_EDIT),
        _ => Tool::ALL
            .iter()
            .map(|t| (t.key().to_string(), t.label().to_string()))
            .collect(),
    }
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

pub fn draw(grid: &mut Grid, page: usize) {
    let screen = (grid.w as u16, grid.h as u16);
    let r = rect(screen);
    boxed(grid, r, "Shortcuts");
    for (i, start, _) in tabs() {
        let label = format!(" {} ", SCOPES[i]);
        grid.text(
            r.0 + start,
            r.1 + 1,
            &label,
            if i == page { REVERSE } else { 0 },
        );
    }
    for (i, (keys, action)) in rows(page).iter().enumerate() {
        let y = r.1 + 3 + i as i32;
        grid.text(r.0 + 2, y, keys, 0);
        grid.text(r.0 + ACTION_COL, y, action, 0);
    }
}
