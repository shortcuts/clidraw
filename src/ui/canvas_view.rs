use super::grid::{Grid, BOLD, DIM, REVERSE, UNDERLINE};
use super::layout::{refresh, seen};
use super::registry;
use super::theme;
use crate::input::{handles, EditTarget, Handle, Hit, Session};
use crate::model::{
    label_pos, wrap, Border, Element, Find, Id, Line, LineKind, Router, Shape, ShapeKind, Stroke,
};
use crate::view::Zoom;
use ratatui::style::Color;
use std::cmp::Ordering;
use std::collections::HashSet;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn draw(grid: &mut Grid, s: &Session, accent: Option<Color>, colors: bool) {
    if s.zoomed_out() {
        return blocks(grid, s);
    }
    let view = s.view;
    let editing = s.edit.as_ref().and_then(|e| match &e.target {
        EditTarget::Existing(id) => Some(id),
        EditTarget::New(..) => None,
    });
    let mut slot = s.layout.borrow_mut();
    let layout = refresh(&mut slot, &s.canvas, s.rev);
    let find = layout.find(&s.canvas);
    let selected: HashSet<&Id> = s.selection.iter().collect();
    for i in s.canvas.stacking() {
        let e = &s.canvas.elements[i];
        let (x, y, w, h) = seen(e, layout.bounds[i]);
        let (sx, sy, sw, sh) = view.rect(x, y, w, h);
        if sx >= grid.w || sy >= grid.h || sx + sw <= 0 || sy + sh <= 0 {
            continue;
        }
        let (x, y, w, h) = layout.bounds[i];
        let (sx, sy, w, h) = view.rect(x, y, w, h);
        let is_selected = selected.contains(e.id());
        let flags = if is_selected {
            if colors {
                0
            } else {
                REVERSE
            }
        } else if s.snap.as_ref() == Some(e.id()) {
            BOLD
        } else if matches!(&s.hover, Hit::Element(id) if id == e.id()) {
            UNDERLINE
        } else {
            0
        };
        let hide = editing == Some(e.id());
        let label = |own| if hide { "" } else { own };
        match e {
            Element::Shape(sh) if view.zoom > Zoom::One => {
                let (sx, sy, w, h) = view.rect(sh.x, sh.y, sh.w, sh.h);
                let big = Shape { w, h, ..sh.clone() };
                shape(grid, &big, sx, sy, label(&sh.label), flags)
            }
            Element::Shape(sh) => shape(grid, sh, sx, sy, label(&sh.label), flags),
            Element::Line(l) => line(grid, s, &find, &layout.router, l, label(&l.label), flags),
            Element::Text(t) => grid.text(sx, sy, label(&t.text), flags),
        }
        if colors {
            let (ink, fill) = match e {
                Element::Shape(sh) => (sh.color, sh.fill),
                Element::Line(l) => (l.color, None),
                Element::Text(t) => (t.color, None),
            };
            if is_selected {
                grid.tint(sx, sy, w, h, theme::selected(s.theme, ink));
            } else if let Some(p) = ink {
                grid.tint(sx, sy, w, h, theme::ink(s.theme, p));
            }
            if let Some(p) = fill {
                grid.shade(sx, sy, w, h, theme::fill(s.theme, p));
            }
        }
    }
    edit_label(grid, s);
    for (h, id, (x, y)) in handles(&s.canvas, &s.selection) {
        if h != Handle::Resize {
            continue;
        }
        let (sx, sy) = view.cell_end(x, y);
        let hovered = matches!(&s.hover, Hit::Handle(hh, hid) if *hh == h && *hid == id);
        grid.set(sx, sy, if hovered { '◆' } else { '⤡' }, REVERSE);
        if let Some(c) = accent {
            grid.tint(sx, sy, 1, 1, c);
        }
    }
    if let Some((x, y)) = s.snap_at {
        let (sx, sy) = view.to_screen(x, y);
        grid.set(sx, sy, '◆', REVERSE);
        if let Some(c) = accent {
            grid.tint(sx, sy, 1, 1, c);
        }
    }
    if let Some((a, b)) = s.band {
        band(grid, s, a, b);
    }
    guides(grid, s);
    if s.rev == 0 && s.canvas.elements.is_empty() {
        welcome(grid, s.viewport());
    }
}

fn welcome(grid: &mut Grid, (vx, vy, vw, vh): (i32, i32, i32, i32)) {
    let lines = [
        "Drag to draw".to_string(),
        format!("{} for keys", registry::by_id("help").first_label()),
        format!("{} for commands", registry::by_id("command").first_label()),
    ];
    let y0 = vy + vh / 2 - 1;
    for (y, l) in (y0..).zip(&lines) {
        grid.text(vx + (vw - l.width() as i32) / 2, y, l, DIM);
    }
}

fn blocks(grid: &mut Grid, s: &Session) {
    let mut slot = s.layout.borrow_mut();
    let layout = refresh(&mut slot, &s.canvas, s.rev);
    let find = layout.find(&s.canvas);
    for (e, &b) in s.canvas.elements.iter().zip(&layout.bounds) {
        let (x, y, w, h) = seen(e, b);
        let (sx, sy, sw, sh) = s.view.rect(x, y, w, h);
        if sx >= grid.w || sy >= grid.h || sx + sw <= 0 || sy + sh <= 0 {
            continue;
        }
        match e {
            Element::Line(l) => {
                for c in s.canvas.route_with(l, &find, &layout.router) {
                    let (sx, sy) = s.view.to_screen(c.0, c.1);
                    grid.set(sx, sy, '·', 0);
                }
            }
            Element::Shape(sh) if sh.kind == ShapeKind::Frame => {}
            _ => {
                for sy in sy.max(0)..(sy + sh).min(grid.h) {
                    for sx in sx.max(0)..(sx + sw).min(grid.w) {
                        grid.set(sx, sy, '▒', 0);
                    }
                }
            }
        }
    }
}

fn edit_label(grid: &mut Grid, s: &Session) {
    let Some(edit) = &s.edit else {
        return;
    };
    let boxes = s.edit_boxes();
    let sel = edit.selection();
    for b in &boxes {
        let (mut x, y) = (b.x, b.y);
        for (i, ch) in edit.buf[b.start..b.end].char_indices() {
            let i = b.start + i;
            let hit = match sel {
                Some((from, to)) => (from..to).contains(&i),
                None => i == edit.caret,
            };
            let mut one = [0; 4];
            grid.text(
                x,
                y,
                ch.encode_utf8(&mut one),
                if hit { BOLD | REVERSE } else { BOLD },
            );
            x += ch.width().unwrap_or(0) as i32;
        }
    }
    let Some(b) = boxes.iter().rev().find(|b| b.start <= edit.caret) else {
        return;
    };
    if sel.is_none() && edit.caret >= b.end {
        // shortcut: a caret at the end of a full line draws on the border, reserve a cell if it looks wrong
        grid.set(
            b.x + edit.buf[b.start..b.end].width() as i32,
            b.y,
            ' ',
            REVERSE,
        );
    }
}

fn band(grid: &mut Grid, s: &Session, a: (i32, i32), b: (i32, i32)) {
    let (x0, y0) = s.view.to_screen(a.0.min(b.0), a.1.min(b.1));
    let (x1, y1) = s.view.cell_end(a.0.max(b.0), a.1.max(b.1));
    for x in x0..=x1 {
        grid.set(x, y0, '┄', DIM);
        grid.set(x, y1, '┄', DIM);
    }
    for y in y0..=y1 {
        grid.set(x0, y, '┆', DIM);
        grid.set(x1, y, '┆', DIM);
    }
}

fn guides(grid: &mut Grid, s: &Session) {
    for &(a, b) in &s.guides {
        let (x0, y0) = s.view.to_screen(a.0, a.1);
        let (x1, y1) = s.view.to_screen(b.0, b.1);
        let ch = if x0 == x1 { '┆' } else { '┄' };
        for (x, y) in (x0..=x1).flat_map(|x| (y0..=y1).map(move |y| (x, y))) {
            if grid.get(x, y).is_some_and(|c| c.ch == ' ') {
                grid.set(x, y, ch, DIM);
            }
        }
    }
}

fn dashed(stroke: Stroke, glyph: char) -> char {
    match (stroke, glyph) {
        (Stroke::Dashed, '─') => '╌',
        (Stroke::Dashed, '│') => '╎',
        (Stroke::Dotted, '─') => '┄',
        (Stroke::Dotted, '│') => '┆',
        _ => glyph,
    }
}

fn shape(grid: &mut Grid, s: &Shape, x: i32, y: i32, label: &str, flags: u8) {
    let (w, h) = (s.w, s.h);
    if s.kind != ShapeKind::Frame {
        grid.clear_inside(x, y, w, h);
    }
    if s.link.is_some() {
        grid.set(x + w, y, '↗', flags);
    }
    match s.kind {
        ShapeKind::Frame => return frame(grid, s, x, y, label, flags),
        ShapeKind::Rect => {
            let (hz, vt, corners) = match s.border {
                Border::Double => ('═', '║', ['╔', '╗', '╚', '╝']),
                Border::Heavy => ('━', '┃', ['┏', '┓', '┗', '┛']),
                Border::Rounded => ('─', '│', ['╭', '╮', '╰', '╯']),
                Border::Single => ('─', '│', ['┌', '┐', '└', '┘']),
            };
            for i in 1..w - 1 {
                grid.set(x + i, y, dashed(s.stroke, hz), flags);
                grid.set(x + i, y + h - 1, dashed(s.stroke, hz), flags);
            }
            for j in 1..h - 1 {
                grid.set(x, y + j, dashed(s.stroke, vt), flags);
                grid.set(x + w - 1, y + j, dashed(s.stroke, vt), flags);
            }
            for (c, (cx, cy)) in corners.into_iter().zip([
                (x, y),
                (x + w - 1, y),
                (x, y + h - 1),
                (x + w - 1, y + h - 1),
            ]) {
                grid.set(cx, cy, c, flags);
            }
        }
        ShapeKind::Diamond => {
            let mid = f64::from(h - 1) / 2.0;
            for j in 0..h {
                let spread = 1.0 - (f64::from(j) - mid).abs() / mid;
                let off = (f64::from(w - 1) / 2.0 * spread).round() as i32;
                let (l, r) = (x + (w - 1) / 2 - off, x + w / 2 + off);
                let upper = f64::from(j) < mid;
                grid.set(l, y + j, if upper { '╱' } else { '╲' }, flags);
                grid.set(r, y + j, if upper { '╲' } else { '╱' }, flags);
            }
        }
        ShapeKind::Cylinder => {
            let h_edge = |grid: &mut Grid, row: i32| {
                grid.set(x, row, if row == y { '╭' } else { '╰' }, flags);
                grid.set(x + w - 1, row, if row == y { '╮' } else { '╯' }, flags);
                for i in 1..w - 1 {
                    grid.set(x + i, row, dashed(s.stroke, '─'), flags);
                }
            };
            for row in [y, y + 1, y + h - 1] {
                h_edge(grid, row);
            }
            for j in 2..h - 1 {
                grid.set(x, y + j, dashed(s.stroke, '│'), flags);
                grid.set(x + w - 1, y + j, dashed(s.stroke, '│'), flags);
            }
        }
        ShapeKind::Parallelogram => {
            let i = s.inset();
            for j in 0..h {
                let t = j.min(i);
                let (l, r) = (x + i - t, x + w - 1 - t);
                if j == 0 || j == h - 1 {
                    for cx in l + 1..r {
                        grid.set(cx, y + j, dashed(s.stroke, '─'), flags);
                    }
                }
                grid.set(l, y + j, '╱', flags);
                grid.set(r, y + j, '╱', flags);
            }
        }
        ShapeKind::Hexagon => {
            let m = s.inset();
            let mid = (h - 1) / 2;
            for cx in x + m..x + w - m {
                grid.set(cx, y, dashed(s.stroke, '─'), flags);
                grid.set(cx, y + h - 1, dashed(s.stroke, '─'), flags);
            }
            for j in 1..h - 1 {
                let (d, l, r) = match j.cmp(&mid) {
                    Ordering::Equal => (0, '<', '>'),
                    Ordering::Less => ((m - j).max(0), '╱', '╲'),
                    Ordering::Greater => ((j - (h - 1 - m)).max(0), '╲', '╱'),
                };
                let (l, r) = if d == 0 && j != mid {
                    ('│', '│')
                } else {
                    (l, r)
                };
                grid.set(x + d, y + j, l, flags);
                grid.set(x + w - 1 - d, y + j, r, flags);
            }
        }
    }
    // shortcut: diamond, parallelogram and hexagon use the rect room, narrow the room per row if the text touches the outline
    let room = (w - 2).max(1) as usize;
    let lines = wrap(label, room);
    let top = y + h / 2 - (lines.len() as i32 - 1) / 2;
    for (r, row) in lines.into_iter().zip(top..) {
        let lid = if s.kind == ShapeKind::Cylinder {
            y + 1
        } else {
            y
        };
        if row <= lid || row >= y + h - 1 {
            continue;
        }
        let text = label[r].trim_end_matches([' ', '\n']);
        let used = text.width();
        let ink = if s.bold { flags | BOLD } else { flags };
        grid.text(x + 1 + s.align.offset(room, used), row, text, ink);
    }
}

fn frame(grid: &mut Grid, s: &Shape, x: i32, y: i32, label: &str, flags: u8) {
    let (w, h) = (s.w, s.h);
    for i in 1..w - 1 {
        grid.set(x + i, y, dashed(s.stroke, '─'), flags);
        grid.set(x + i, y + h - 1, dashed(s.stroke, '─'), flags);
    }
    for j in 1..h - 1 {
        grid.set(x, y + j, dashed(s.stroke, '│'), flags);
        grid.set(x + w - 1, y + j, dashed(s.stroke, '│'), flags);
    }
    for (c, (cx, cy)) in ['┌', '┐', '└', '┘'].into_iter().zip([
        (x, y),
        (x + w - 1, y),
        (x, y + h - 1),
        (x + w - 1, y + h - 1),
    ]) {
        grid.set(cx, cy, c, flags);
    }
    let room = (w - 4).max(0) as usize;
    let mut text = String::new();
    for c in label.chars().map(|c| if c == '\n' { ' ' } else { c }) {
        if text.width() + c.width().unwrap_or(0) > room {
            break;
        }
        text.push(c);
    }
    if !text.is_empty() {
        let ink = if s.bold { flags | BOLD } else { flags };
        grid.text(x + 1, y, &format!(" {text} "), ink);
    }
}

/// Line glyphs and the sides (left, right, up, down) they connect.
const JOINTS: [(char, (bool, bool, bool, bool)); 11] = [
    ('─', (true, true, false, false)),
    ('│', (false, false, true, true)),
    ('╮', (true, false, false, true)),
    ('╭', (false, true, false, true)),
    ('╯', (true, false, true, false)),
    ('╰', (false, true, true, false)),
    ('┬', (true, true, false, true)),
    ('┴', (true, true, true, false)),
    ('├', (false, true, true, true)),
    ('┤', (true, false, true, true)),
    ('┼', (true, true, true, true)),
];

fn line<'a>(
    grid: &mut Grid,
    s: &'a Session,
    find: &Find<'a>,
    router: &Router,
    l: &Line,
    label: &str,
    flags: u8,
) {
    let cells = s.view.path(&s.canvas.route_with(l, find, router));
    let dirs = |p: (i32, i32), q: (i32, i32)| (q.0 < p.0, q.0 > p.0, q.1 < p.1, q.1 > p.1);
    for (i, &c) in cells.iter().enumerate() {
        let (mut left, mut right, mut up, mut down) = (false, false, false, false);
        let mut diagonal = None;
        for n in [
            i.checked_sub(1).map(|j| cells[j]),
            cells.get(i + 1).copied(),
        ]
        .into_iter()
        .flatten()
        {
            let d = dirs(c, n);
            if (d.0 | d.1) && (d.2 | d.3) {
                diagonal = Some(if d.0 == d.2 { '╲' } else { '╱' });
            }
            (left, right, up, down) = (left | d.0, right | d.1, up | d.2, down | d.3);
        }
        let ends = i == 0 || i + 1 == cells.len();
        if let Some(g) = grid.get(c.0, c.1).filter(|_| ends) {
            if let Some(&(_, e)) = JOINTS.iter().find(|j| j.0 == g.ch) {
                (left, right, up, down) = (left | e.0, right | e.1, up | e.2, down | e.3);
            }
        }
        // shortcut: diagonal glyphs have no dashed form, add one if wanted
        let glyph = diagonal.unwrap_or_else(|| match (left, right, up, down) {
            (_, _, false, false) if left || right => '─',
            (false, false, _, _) if up || down => '│',
            sides => JOINTS.iter().find(|j| j.1 == sides).map_or('·', |j| j.0),
        });
        grid.set(c.0, c.1, dashed(l.stroke, glyph), flags);
    }
    let last = cells.len().saturating_sub(1);
    let end = (last, last.saturating_sub(1));
    let start = (0, 1);
    let heads = match l.kind {
        LineKind::Arrow => vec![end],
        LineKind::ArrowStart => vec![start],
        // shortcut: heads can share a cell on a 3-cell route, split the heads if it matters
        LineKind::ArrowBoth => vec![start, end],
        LineKind::Line => vec![],
    };
    for (tip, prev) in heads.into_iter().filter(|_| cells.len() >= 2) {
        let (p, c) = (cells[prev], cells[tip]);
        let glyph = match (c.0 - p.0, c.1 - p.1) {
            (1.., _) => '▶',
            (..=-1, _) => '◀',
            (_, 1..) => '▼',
            _ => '▲',
        };
        grid.set(c.0, c.1, glyph, flags);
    }
    if !label.is_empty() {
        let (lx, ly) = label_pos(&cells, l.label_at);
        grid.text(lx - label.width() as i32 / 2, ly, label, flags);
    }
}
