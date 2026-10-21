use super::grid::{Grid, BOLD, DIM, REVERSE, UNDERLINE};
use super::layout::refresh;
use super::theme;
use crate::input::{handles, EditTarget, Hit, Session};
use crate::model::{
    label_pos, wrap, Border, Element, End, Find, Id, Line, LineKind, Shape, ShapeKind, Stroke,
};
use ratatui::style::Color;
use std::cmp::Ordering;
use std::collections::HashSet;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn draw(grid: &mut Grid, s: &Session, accent: Option<Color>, colors: bool) {
    if s.overview {
        return overview(grid, s);
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
    for (e, &(x, y, w, h)) in s.canvas.elements.iter().zip(&layout.bounds) {
        let (sx, sy) = view.to_screen(x, y);
        if sx >= grid.w || sy >= grid.h || sx + w <= 0 || sy + h <= 0 {
            continue;
        }
        let flags = if selected.contains(e.id()) {
            REVERSE
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
            Element::Shape(sh) => shape(grid, sh, sx, sy, label(&sh.label), flags),
            Element::Line(l) => line(grid, s, &find, l, label(&l.label), flags),
            Element::Text(t) => grid.text(sx, sy, label(&t.text), flags),
        }
        if colors {
            let (ink, fill) = match e {
                Element::Shape(sh) => (sh.color, sh.fill),
                Element::Line(l) => (l.color, None),
                Element::Text(t) => (t.color, None),
            };
            if let Some(p) = ink {
                grid.tint(sx, sy, w, h, theme::ink(s.theme, p));
            }
            if let Some(p) = fill {
                grid.shade(sx, sy, w, h, theme::fill(s.theme, p));
            }
        }
        if let (Some(c), true) = (accent, selected.contains(e.id())) {
            grid.tint(sx, sy, w, h, c);
        }
    }
    edit_label(grid, s);
    for (h, id, (x, y)) in handles(&s.canvas, &s.selection) {
        let (sx, sy) = view.to_screen(x, y);
        let hovered = matches!(&s.hover, Hit::Handle(hh, hid) if *hh == h && *hid == id);
        grid.set(sx, sy, if hovered { '◆' } else { '■' }, REVERSE);
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
        welcome(grid);
    }
}

fn welcome(grid: &mut Grid) {
    let lines = ["drag to draw", "? for keys", ": for commands"];
    let y0 = grid.h / 2 - 1;
    for (y, l) in (y0..).zip(lines) {
        grid.text((grid.w - l.width() as i32) / 2, y, l, DIM);
    }
}

fn overview(grid: &mut Grid, s: &Session) {
    let Some((bx, by, bw, bh)) = s.canvas.all_bounds() else {
        return;
    };
    // The last row belongs to the status line.
    let (sw, sh) = (grid.w.max(1), (grid.h - 1).max(1));
    let d = ((bw + sw - 1) / sw).max((bh + sh - 1) / sh).max(1);
    let (ox, oy) = (bx + bw / 2 - sw * d / 2, by + bh / 2 - sh * d / 2);
    let cell = |x: i32, y: i32| ((x - ox).div_euclid(d), (y - oy).div_euclid(d));
    let mut slot = s.layout.borrow_mut();
    let layout = refresh(&mut slot, &s.canvas, s.rev);
    let find = layout.find(&s.canvas);
    for (e, &(x, y, w, h)) in s.canvas.elements.iter().zip(&layout.bounds) {
        if let Element::Line(l) = e {
            for c in s.canvas.route_with(l, &find) {
                let (sx, sy) = cell(c.0, c.1);
                grid.set(sx, sy, '·', 0);
            }
            continue;
        }
        if matches!(e, Element::Shape(sh) if sh.kind == ShapeKind::Frame) {
            continue;
        }
        let ((x0, y0), (x1, y1)) = (cell(x, y), cell(x + w - 1, y + h - 1));
        for sy in y0..=y1 {
            for sx in x0..=x1 {
                grid.set(sx, sy, '▒', 0);
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
        let (mut x, y) = s.view.to_screen(b.x, b.y);
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
        let (x, y) = s.view.to_screen(b.x, b.y);
        grid.set(x + edit.buf[b.start..b.end].width() as i32, y, ' ', REVERSE);
    }
}

fn band(grid: &mut Grid, s: &Session, a: (i32, i32), b: (i32, i32)) {
    let (x0, y0) = s.view.to_screen(a.0.min(b.0), a.1.min(b.1));
    let (x1, y1) = s.view.to_screen(a.0.max(b.0), a.1.max(b.1));
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
        grid.fill(x, y, w, h);
    }
    if s.link.is_some() {
        grid.set(x + w, y, '↗', flags);
    }
    match s.kind {
        ShapeKind::Frame => return frame(grid, s, x, y, label, flags),
        ShapeKind::Rect | ShapeKind::Ellipse => {
            let (hz, vt, corners) = match (s.border, s.kind) {
                (Border::Double, _) => ('═', '║', ['╔', '╗', '╚', '╝']),
                (Border::Heavy, _) => ('━', '┃', ['┏', '┓', '┗', '┛']),
                (Border::Rounded, _) | (Border::Single, ShapeKind::Ellipse) => {
                    ('─', '│', ['╭', '╮', '╰', '╯'])
                }
                (Border::Single, _) => ('─', '│', ['┌', '┐', '└', '┘']),
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
    // shortcut: ellipse, diamond, parallelogram and hexagon use the rect room, narrow the room per row if the text touches the outline
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

fn line<'a>(grid: &mut Grid, s: &'a Session, find: &Find<'a>, l: &Line, label: &str, flags: u8) {
    let cells = s.canvas.route_with(l, find);
    let dirs = |p: (i32, i32), q: (i32, i32)| (q.0 < p.0, q.0 > p.0, q.1 < p.1, q.1 > p.1);
    for (i, &c) in cells.iter().enumerate() {
        let skip = (i == 0 && matches!(l.from, End::Bound(..)))
            || (i == cells.len() - 1 && matches!(l.to, End::Bound(..)));
        if skip {
            continue;
        }
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
        // shortcut: diagonal glyphs have no dashed form, add one if wanted
        let glyph = diagonal.unwrap_or(match (left, right, up, down) {
            (_, _, false, false) if left || right => '─',
            (false, false, _, _) if up || down => '│',
            (true, false, false, true) => '╮',
            (false, true, false, true) => '╭',
            (true, false, true, false) => '╯',
            (false, true, true, false) => '╰',
            _ => '·',
        });
        let (sx, sy) = s.view.to_screen(c.0, c.1);
        grid.set(sx, sy, dashed(l.stroke, glyph), flags);
    }
    let last = cells.len().saturating_sub(1);
    let end = (last, last.saturating_sub(1), matches!(l.to, End::Bound(..)));
    let start = (0, 1, matches!(l.from, End::Bound(..)));
    let heads = match l.kind {
        LineKind::Arrow => vec![end],
        LineKind::ArrowStart => vec![start],
        // shortcut: heads can share a cell on a 3-cell route, split the heads if it matters
        LineKind::ArrowBoth => vec![start, end],
        LineKind::Line => vec![],
    };
    for (tip, prev, bound) in heads.into_iter().filter(|_| cells.len() >= 2) {
        let head_at = if bound && last >= 2 { prev } else { tip };
        let (p, c) = (cells[prev], cells[tip]);
        let glyph = match (c.0 - p.0, c.1 - p.1) {
            (1.., _) => '▶',
            (..=-1, _) => '◀',
            (_, 1..) => '▼',
            _ => '▲',
        };
        let (sx, sy) = s.view.to_screen(cells[head_at].0, cells[head_at].1);
        grid.set(sx, sy, glyph, flags);
    }
    if !label.is_empty() {
        let (lx, ly) = label_pos(&cells, l.label_at);
        let (sx, sy) = s.view.to_screen(lx - label.width() as i32 / 2, ly);
        grid.text(sx, sy, label, flags);
    }
}
