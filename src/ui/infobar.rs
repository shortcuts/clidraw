use super::registry;
use super::style::{ACTIVE, DISABLED, ERROR, KEY, MUTED};
use super::{Doc, Grid, Overlay, Severity, Status};
use crate::input::{Session, Tool};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const GAP: i32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    ZoomOut,
    ZoomIn,
    Fit,
    Undo,
    Redo,
}

const UNDO_X: i32 = 0;
const REDO_X: i32 = 7;
const LEFT_END: i32 = 13;
const CLUSTER_W: i32 = 18;
const MARKER: &str = " ●";
const MIN_NAME_W: i32 = 2;

struct Bar {
    badge: Option<String>,
    readout: Option<(i32, String)>,
    sel: Option<(i32, String)>,
    cluster_x: i32,
    name: Option<(i32, String)>,
    hints: (i32, i32),
}

fn width(s: &str) -> i32 {
    s.width() as i32
}

fn clip(s: &str, max: i32) -> String {
    let mut used = 0;
    s.chars()
        .take_while(|c| {
            used += c.width().unwrap_or(0) as i32;
            used <= max
        })
        .collect()
}

pub(super) fn fit(s: &str, max: i32) -> String {
    if width(s) <= max {
        return s.to_string();
    }
    clip(s, max - 1) + "…"
}

fn pair_width((k, a): (&str, &str)) -> i32 {
    width(k) + 1 + width(a)
}

fn badge(s: &Session, doc: &Doc) -> String {
    let text = match () {
        _ if doc.picking => "PICK".to_string(),
        _ if s.edit.is_some() => "EDIT".to_string(),
        _ if s.zoomed_out() => "ZOOM".to_string(),
        _ => s.tool.label().to_uppercase(),
    };
    format!(" {text} ")
}

fn layout(s: &Session, doc: &Doc, w: i32) -> Bar {
    let badge = badge(s, doc);
    let marker = if doc.saved { "" } else { MARKER };
    let readout = s.readout();
    let sel = (!s.selection.is_empty()).then(|| format!("{} sel", s.selection.len()));
    let name_w = width(doc.name) + width(marker);
    let extra = |part: &Option<String>, on: bool| {
        part.as_ref().filter(|_| on).map_or(0, |p| width(p) + GAP)
    };
    let left_end = |badge_on: bool| LEFT_END + if badge_on { GAP + width(&badge) } else { 0 };
    let room = |badge_on: bool, ro: bool, se: bool| {
        w - left_end(badge_on) - GAP - CLUSTER_W - GAP - extra(&readout, ro) - extra(&sel, se)
    };
    let (mut badge_on, mut use_readout, mut use_sel) = (true, readout.is_some(), sel.is_some());
    while room(badge_on, use_readout, use_sel) < name_w {
        if use_readout {
            use_readout = false;
        } else if use_sel {
            use_sel = false;
        } else {
            break;
        }
    }
    let mut avail = room(badge_on, use_readout, use_sel);
    if avail - width(marker) < MIN_NAME_W && badge_on {
        badge_on = false;
        avail = room(false, use_readout, use_sel);
    }
    let text_w = avail.min(name_w) - width(marker);
    let name = (text_w >= MIN_NAME_W.min(width(doc.name)))
        .then(|| format!("{}{marker}", fit(doc.name, text_w)));
    let name_x = w - name.as_deref().map_or(0, width);
    let gap_name = if name.is_some() { GAP } else { 0 };
    let cluster_x = name_x - gap_name - CLUSTER_W;
    let mut x = cluster_x;
    let mut place = |text: Option<String>, on: bool| {
        text.filter(|_| on).map(|t| {
            x -= width(&t) + GAP;
            (x, t)
        })
    };
    let sel = place(sel, use_sel);
    let readout = place(readout, use_readout);
    Bar {
        hints: (left_end(badge_on) + GAP, x - GAP),
        badge: badge_on.then_some(badge),
        readout,
        sel,
        cluster_x,
        name: name.map(|n| (name_x, n)),
    }
}

fn hints(s: &Session, doc: &Doc, overlay: &Overlay) -> Vec<(String, &'static str)> {
    let ids: &[&str] = match overlay {
        _ if doc.picking => &["pick_area", "cancel"],
        Overlay::Prompt(_) => &["confirm", "cancel_prompt"],
        Overlay::None if s.edit.is_some() => &["commit_edit", "discard_edit", "newline"],
        Overlay::None if s.zoomed_out() => &["reset_zoom", "zoom_wheel", "help"],
        Overlay::None if s.dragging() => &["cancel"],
        Overlay::None if s.tool != Tool::Select => &["draw", "cancel", "tool_select"],
        Overlay::None if !s.selection.is_empty() => &["delete", "connect", "copy", "group"],
        Overlay::None => &["help", "command", "find", "fit"],
        Overlay::Context(_)
        | Overlay::Scope(..)
        | Overlay::Colors(..)
        | Overlay::Switcher(_)
        | Overlay::Actions(_) => &["run", "select", "close"],
        Overlay::Map => &["jump", "close"],
        Overlay::Help(..) => &["tabs", "close"],
    };
    ids.iter()
        .map(|id| {
            let b = registry::by_id(id);
            (b.first_label(), b.hint.unwrap_or_default())
        })
        .collect()
}

/// The button under a cell of the bottom row.
pub fn hit(s: &Session, doc: &Doc, col: i32, row: i32) -> Option<Button> {
    let (w, h) = (i32::from(s.screen.0), i32::from(s.screen.1));
    if row != h - 1 {
        return None;
    }
    let bar = layout(s, doc, w);
    let c = col - bar.cluster_x;
    [
        (col - UNDO_X, 6, Button::Undo),
        (col - REDO_X, 6, Button::Redo),
        (c, 3, Button::ZoomOut),
        (c - 9, 3, Button::ZoomIn),
        (c - 13, 5, Button::Fit),
    ]
    .into_iter()
    .find(|&(x, len, _)| (0..len).contains(&x))
    .map(|(_, _, b)| b)
}

fn draw_chrome(grid: &mut Grid, y: i32, s: &Session, bar: &Bar) {
    let off = !s.can_edit();
    grid.text(UNDO_X, y, "[Undo]", if off { DISABLED } else { KEY });
    grid.text(REDO_X, y, "[Redo]", if off { DISABLED } else { KEY });
    if let Some(b) = &bar.badge {
        grid.text(LEFT_END + GAP, y, b, ACTIVE);
    }
    if let Some((x, t)) = &bar.readout {
        grid.text(*x, y, t, KEY);
    }
    if let Some((x, t)) = &bar.sel {
        grid.text(*x, y, t, 0);
    }
    let x = bar.cluster_x;
    grid.text(x, y, "[-]", KEY);
    grid.text(x + 4, y, &format!("{:>4}", s.view.zoom.label()), MUTED);
    grid.text(x + 9, y, "[+] [Fit]", KEY);
    if let Some((x, t)) = &bar.name {
        grid.text(*x, y, t, 0);
    }
}

pub(super) fn draw(grid: &mut Grid, s: &Session, doc: &Doc, overlay: &Overlay) {
    if grid.h < 2 {
        return;
    }
    let y = grid.h - 1;
    let bar = layout(s, doc, grid.w);
    draw_chrome(grid, y, s, &bar);
    let (from, to) = bar.hints;
    let mut x = from;
    for (k, a) in hints(s, doc, overlay) {
        if x > from {
            if x + GAP + pair_width((&k, a)) > to {
                break;
            }
            x += GAP;
        }
        grid.text(x, y, &clip(&k, to - x), KEY);
        let rest = to - x - width(&k) - 1;
        grid.text(x + width(&k) + 1, y, &clip(a, rest), MUTED);
        x += pair_width((&k, a));
    }
}

pub(super) fn status(grid: &mut Grid, s: &Session, doc: &Doc, st: &Status) {
    let y = grid.h - 1;
    grid.fill(0, y, grid.w, 1);
    let bar = layout(s, doc, grid.w);
    draw_chrome(grid, y, s, &bar);
    let flags = match st.severity {
        Severity::Notice => 0,
        Severity::Error => ERROR,
    };
    let (from, to) = bar.hints;
    grid.text(from, y, &clip(&st.text, to - from), flags);
}
