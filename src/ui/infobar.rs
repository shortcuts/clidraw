use super::grid::{BOLD, DIM, REVERSE};
use super::{Grid, Overlay, Severity, Status};
use crate::input::{Session, Tool};

type Hints = &'static [(&'static str, &'static str)];

const GAP: i32 = 2;

fn hints(s: &Session, overlay: &Overlay) -> Hints {
    match overlay {
        Overlay::Prompt(_) => &[("Enter", "confirm"), ("Esc", "cancel")],
        Overlay::None if s.edit.is_some() => &[
            ("Enter", "commit"),
            ("Esc", "discard"),
            ("Alt-Enter", "new line"),
        ],
        Overlay::None if s.overview => &[("z, Esc", "back"), ("?", "help")],
        Overlay::None if s.dragging() => &[("Esc", "cancel")],
        Overlay::None if s.tool == Tool::Hand => {
            &[("drag", "pan"), ("Esc", "cancel"), ("v", "select")]
        }
        Overlay::None if s.tool != Tool::Select => {
            &[("drag", "draw"), ("Esc", "cancel"), ("v", "select")]
        }
        Overlay::None if !s.selection.is_empty() => &[
            ("Del", "delete"),
            ("Shift+arrows", "connect"),
            ("Ctrl-d", "dup"),
            ("g", "group"),
        ],
        Overlay::None => &[
            ("?", "help"),
            (":", "command"),
            ("Ctrl-f", "find"),
            ("Esc", "menu"),
            ("z", "overview"),
        ],
        _ => &[("Esc", "close")],
    }
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
        ("saved", DIM)
    } else {
        ("unsaved", BOLD)
    };
    let mut parts = [
        Some((s.tool.label().to_string(), 0)),
        (!s.selection.is_empty()).then(|| (format!("{} sel", s.selection.len()), 0)),
        s.overview.then(|| ("Overview".to_string(), 0)),
        Some((marker.0.to_string(), marker.1)),
    ];
    let total = |p: &[Option<(String, u8)>]| -> i32 {
        let shown = p.iter().flatten();
        shown.clone().map(|(t, _)| width(t)).sum::<i32>() + GAP * (shown.count() as i32 - 1)
    };
    for i in [1, 2, 0] {
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
    let full = hs.iter().map(|&p| pair_width(p)).sum::<i32>() + GAP * (hs.len() as i32 - 1);
    let mut parts = right(s, saved, grid.w, full);
    let mut room = grid.w - right_width(&parts) - GAP;
    if room < pair_width(hs[0]) {
        parts.clear();
        room = grid.w;
    }
    draw_right(grid, y, &parts);
    let mut x = 0;
    for &(k, a) in hs {
        if x > 0 && x + GAP + pair_width((k, a)) > room {
            break;
        }
        if x > 0 {
            x += GAP;
        }
        let key: String = k.chars().take((room - x).max(0) as usize).collect();
        grid.text(x, y, &key, BOLD);
        let rest = (room - x - width(k) - 1).max(0) as usize;
        grid.text(
            x + width(k) + 1,
            y,
            &a.chars().take(rest).collect::<String>(),
            DIM,
        );
        x += pair_width((k, a));
    }
}

pub(super) fn status(grid: &mut Grid, s: &Session, st: &Status, saved: bool) {
    let y = grid.h - 1;
    grid.fill(0, y, grid.w, 1);
    let parts = right(s, saved, grid.w, width(&st.text));
    let room = (grid.w - right_width(&parts) - GAP).max(0);
    draw_right(grid, y, &parts);
    let flags = match st.severity {
        Severity::Notice => 0,
        Severity::Error => REVERSE,
    };
    let text: String = st.text.chars().take(room as usize).collect();
    grid.text(0, y, &text, flags);
}
