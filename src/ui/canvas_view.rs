use super::grid::{Grid, BOLD, DIM, REVERSE};
use crate::input::{handles, EditTarget, Session};
use crate::model::{label_pos, Element, End, Find, Line, LineKind, Shape, ShapeKind};
use ratatui::style::Color;
use unicode_width::UnicodeWidthStr;

pub fn draw(grid: &mut Grid, s: &Session, accent: Option<Color>) {
    let view = s.view;
    let editing = s.edit.as_ref().and_then(|e| match &e.target {
        EditTarget::Existing(id) => Some((id, format!("{}_", e.buf))),
        EditTarget::New(..) => None,
    });
    let find = s.canvas.shape_lookup();
    for e in &s.canvas.elements {
        let (x, y, w, h) = s.canvas.bounds_with(e, &find);
        let (sx, sy) = view.to_screen(x, y);
        if sx >= grid.w || sy >= grid.h || sx + w <= 0 || sy + h <= 0 {
            continue;
        }
        let flags = if s.selection.contains(e.id()) {
            REVERSE
        } else if s.snap.as_ref() == Some(e.id()) {
            BOLD
        } else {
            0
        };
        let label = |own: &str| match &editing {
            Some((id, buf)) if *id == e.id() => buf.clone(),
            _ => own.to_string(),
        };
        match e {
            Element::Shape(sh) => shape(grid, sh, sx, sy, &label(&sh.label), flags),
            Element::Line(l) => line(grid, s, &find, l, &label(&l.label), flags),
            Element::Text(t) => grid.text(sx, sy, &label(&t.text), flags),
        }
        if let (Some(c), true) = (accent, s.selection.contains(e.id())) {
            grid.tint(sx, sy, w, h, c);
        }
    }
    if let Some(edit) = &s.edit {
        if let EditTarget::New(x, y) = edit.target {
            let (sx, sy) = view.to_screen(x, y);
            grid.text(sx, sy, &format!("{}_", edit.buf), BOLD);
        }
    }
    for (_, _, (x, y)) in handles(&s.canvas, &s.selection) {
        let (sx, sy) = view.to_screen(x, y);
        grid.set(sx, sy, '■', REVERSE);
        if let Some(c) = accent {
            grid.tint(sx, sy, 1, 1, c);
        }
    }
    if let Some((a, b)) = s.band {
        band(grid, s, a, b);
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

fn shape(grid: &mut Grid, s: &Shape, x: i32, y: i32, label: &str, flags: u8) {
    let (w, h) = (s.w, s.h);
    grid.fill(x, y, w, h);
    match s.kind {
        ShapeKind::Rect | ShapeKind::Ellipse => {
            let corners = if s.kind == ShapeKind::Rect {
                ['┌', '┐', '└', '┘']
            } else {
                ['╭', '╮', '╰', '╯']
            };
            for i in 1..w - 1 {
                grid.set(x + i, y, '─', flags);
                grid.set(x + i, y + h - 1, '─', flags);
            }
            for j in 1..h - 1 {
                grid.set(x, y + j, '│', flags);
                grid.set(x + w - 1, y + j, '│', flags);
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
    }
    let room = (w - 2).max(0) as usize;
    let mut shown = String::new();
    for ch in label.chars() {
        if (shown.clone() + &ch.to_string()).width() > room {
            break;
        }
        shown.push(ch);
    }
    grid.text(
        x + 1 + (room as i32 - shown.width() as i32) / 2,
        y + h / 2,
        &shown,
        flags,
    );
}

fn line<'a>(grid: &mut Grid, s: &'a Session, find: &Find<'a>, l: &Line, label: &str, flags: u8) {
    let cells = s.canvas.route_with(l, find);
    let dirs = |p: (i32, i32), q: (i32, i32)| (q.0 < p.0, q.0 > p.0, q.1 < p.1, q.1 > p.1);
    for (i, &c) in cells.iter().enumerate() {
        let skip = (i == 0 && matches!(l.from, End::Bound(_)))
            || (i == cells.len() - 1 && matches!(l.to, End::Bound(_)));
        if skip {
            continue;
        }
        let (mut left, mut right, mut up, mut down) = (false, false, false, false);
        for n in [
            i.checked_sub(1).map(|j| cells[j]),
            cells.get(i + 1).copied(),
        ]
        .into_iter()
        .flatten()
        {
            let d = dirs(c, n);
            (left, right, up, down) = (left | d.0, right | d.1, up | d.2, down | d.3);
        }
        let glyph = match (left, right, up, down) {
            (_, _, false, false) if left || right => '─',
            (false, false, _, _) if up || down => '│',
            (true, false, false, true) => '╮',
            (false, true, false, true) => '╭',
            (true, false, true, false) => '╯',
            (false, true, true, false) => '╰',
            _ => '·',
        };
        let (sx, sy) = s.view.to_screen(c.0, c.1);
        grid.set(sx, sy, glyph, flags);
    }
    let last = cells.len().saturating_sub(1);
    let end = (last, last.saturating_sub(1), matches!(l.to, End::Bound(_)));
    let start = (0, 1, matches!(l.from, End::Bound(_)));
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
        let (lx, ly) = label_pos(&cells);
        let (sx, sy) = s.view.to_screen(lx - label.width() as i32 / 2, ly);
        grid.text(sx, sy, label, flags);
    }
}
