use super::registry;
use super::style::{DISABLED, ERROR, KEY, MUTED};
use super::{Grid, Overlay, Severity, Status};
use crate::input::{Session, Tool};

const GAP: i32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    ZoomOut,
    ZoomIn,
    Fit,
    Undo,
    Redo,
}

const BUTTONS: [(Button, &str); 5] = [
    (Button::ZoomOut, "[-]"),
    (Button::ZoomIn, "[+]"),
    (Button::Fit, "[Fit]"),
    (Button::Undo, "[Undo]"),
    (Button::Redo, "[Redo]"),
];

/// Below this width the hints and the right side get the whole bar.
const BUTTONS_MIN_W: i32 = 60;

/// Columns the buttons and the gap after them take, or 0 on a narrow screen.
fn offset(w: i32) -> i32 {
    if w < BUTTONS_MIN_W {
        return 0;
    }
    BUTTONS.iter().map(|&(_, l)| width(l) + 1).sum::<i32>() + GAP - 1
}

/// The button under a cell of the bottom row.
pub fn hit(w: i32, col: i32, row: i32, h: i32) -> Option<Button> {
    if row != h - 1 || col >= offset(w) {
        return None;
    }
    let mut x = 0;
    for &(b, l) in &BUTTONS {
        if (x..x + width(l)).contains(&col) {
            return Some(b);
        }
        x += width(l) + 1;
    }
    None
}

fn draw_buttons(grid: &mut Grid, y: i32, s: &Session) {
    if offset(grid.w) == 0 {
        return;
    }
    let mut x = 0;
    for &(b, l) in &BUTTONS {
        let off = matches!(b, Button::Undo | Button::Redo) && !s.can_edit();
        grid.text(x, y, l, if off { DISABLED } else { KEY });
        x += width(l) + 1;
    }
}

fn hints(s: &Session, overlay: &Overlay) -> Vec<(String, &'static str)> {
    let ids: &[&str] = match overlay {
        Overlay::Prompt(_) => &["confirm", "cancel_prompt"],
        Overlay::None if s.edit.is_some() => &["commit_edit", "discard_edit", "newline"],
        Overlay::None if s.zoomed_out() => &["reset_zoom", "zoom_wheel", "help"],
        Overlay::None if s.dragging() => &["cancel"],
        Overlay::None if s.tool != Tool::Select => &["draw", "cancel", "tool_select"],
        Overlay::None if !s.selection.is_empty() => &["delete", "connect", "copy", "group"],
        Overlay::None => &["help", "command", "find", "fit"],
        Overlay::Actions(_) => &["run", "select", "close"],
        Overlay::Map => &["jump", "close"],
        _ => &["close"],
    };
    ids.iter()
        .map(|id| {
            let b = registry::by_id(id);
            (b.first_label(), b.hint.unwrap_or_default())
        })
        .collect()
}

fn width(s: &str) -> i32 {
    s.chars().count() as i32
}

fn pair_width((k, a): (&str, &str)) -> i32 {
    width(k) + 1 + width(a)
}

/// Right-side parts that fit beside `left_w` columns; the save marker always stays.
fn right(s: &Session, saved: bool, w: i32, left_w: i32) -> Vec<(String, u8)> {
    let marker = if saved {
        ("saved", MUTED)
    } else {
        ("unsaved", KEY)
    };
    let mut parts = [
        s.readout().map(|r| (r, KEY)),
        Some((s.tool.label().to_string(), 0)),
        (!s.selection.is_empty()).then(|| (format!("{} sel", s.selection.len()), 0)),
        s.view.zoom.label().map(|l| (l.to_string(), 0)),
        Some((marker.0.to_string(), marker.1)),
    ];
    let total = |p: &[Option<(String, u8)>]| -> i32 {
        let shown = p.iter().flatten();
        shown.clone().map(|(t, _)| width(t)).sum::<i32>() + GAP * (shown.count() as i32 - 1)
    };
    for i in [2, 3, 1, 0] {
        if left_w + GAP + total(&parts) <= w {
            break;
        }
        parts[i] = None;
    }
    parts.into_iter().flatten().collect()
}

fn draw_right(grid: &mut Grid, y: i32, parts: &[(String, u8)]) {
    let mut x = grid.w - right_width(parts);
    for (t, f) in parts {
        grid.text(x, y, t, *f);
        x += width(t) + GAP;
    }
}

fn right_width(parts: &[(String, u8)]) -> i32 {
    parts.iter().map(|(t, _)| width(t) + GAP).sum::<i32>() - GAP
}

pub(super) fn draw(grid: &mut Grid, s: &Session, overlay: &Overlay, saved: bool) {
    if grid.h < 2 {
        return;
    }
    let y = grid.h - 1;
    let hs = hints(s, overlay);
    let full =
        hs.iter().map(|(k, a)| pair_width((k, a))).sum::<i32>() + GAP * (hs.len() as i32 - 1);
    let off = offset(grid.w);
    let mut parts = right(s, saved, grid.w, off + full);
    let mut room = grid.w - right_width(&parts) - GAP;
    if room < off + pair_width((&hs[0].0, hs[0].1)) {
        parts.clear();
        room = grid.w;
    }
    draw_right(grid, y, &parts);
    draw_buttons(grid, y, s);
    let mut x = off;
    for (k, a) in &hs {
        if x > off && x + GAP + pair_width((k, a)) > room {
            break;
        }
        if x > off {
            x += GAP;
        }
        let key: String = k.chars().take((room - x).max(0) as usize).collect();
        grid.text(x, y, &key, KEY);
        let rest = (room - x - width(k) - 1).max(0) as usize;
        grid.text(
            x + width(k) + 1,
            y,
            &a.chars().take(rest).collect::<String>(),
            MUTED,
        );
        x += pair_width((k, a));
    }
}

pub(super) fn status(grid: &mut Grid, s: &Session, st: &Status, saved: bool) {
    let y = grid.h - 1;
    grid.fill(0, y, grid.w, 1);
    let off = offset(grid.w);
    let parts = right(s, saved, grid.w, off + width(&st.text));
    let room = (grid.w - right_width(&parts) - GAP - off).max(0);
    draw_right(grid, y, &parts);
    draw_buttons(grid, y, s);
    let flags = match st.severity {
        Severity::Notice => 0,
        Severity::Error => ERROR,
    };
    let text: String = st.text.chars().take(room as usize).collect();
    grid.text(off, y, &text, flags);
}
