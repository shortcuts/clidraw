use super::{Anchor, Shape};
use std::ops::Range;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn center(s: &Shape) -> (i32, i32) {
    (s.x + s.w / 2, s.y + s.h / 2)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

type Pt = (i32, i32);

/// Anchor for a pointer on or inside `s`: the nearest side and the cell along it. The cell
/// is never a corner, and snaps to the side midpoint within 1 of it. `Auto` outside `s`.
pub fn edge_anchor(s: &Shape, (x, y): Pt) -> Anchor {
    let (right, bottom) = (s.x + s.w - 1, s.y + s.h - 1);
    if x < s.x || x > right || y < s.y || y > bottom {
        return Anchor::Auto;
    }
    let d = [y - s.y, bottom - y, x - s.x, right - x];
    let side = (0..4).min_by_key(|&i| d[i]).unwrap_or(0);
    let (off, len) = if side < 2 {
        (x - s.x, s.w)
    } else {
        (y - s.y, s.h)
    };
    let mid = len / 2;
    let off = if len < 3 || (off - mid).abs() <= 1 {
        mid
    } else {
        off.clamp(1, len - 2)
    };
    match side {
        0 => Anchor::Top(off),
        1 => Anchor::Bottom(off),
        2 => Anchor::Left(off),
        _ => Anchor::Right(off),
    }
}

/// Anchor on the side of `s` that the outside cell `c` touches, at the offset of `c` along it.
pub fn side_anchor(s: &Shape, c: Pt) -> Anchor {
    if c.1 < s.y {
        Anchor::Top(c.0 - s.x)
    } else if c.1 >= s.y + s.h {
        Anchor::Bottom(c.0 - s.x)
    } else if c.0 < s.x {
        Anchor::Left(c.1 - s.y)
    } else {
        Anchor::Right(c.1 - s.y)
    }
}

/// The cell just outside the side of `s` the anchor names, at its offset along it, or at the
/// midpoint of the side that faces `toward` for `Auto`, and the axis a line leaves it on.
// shortcut: shapes under 2 cells apart overlap the end cells, clamp if it matters
pub fn side_mid(s: &Shape, anchor: Anchor, toward: Pt) -> (Pt, Axis) {
    let (cx, cy) = center(s);
    // A corner has no side to land on, so an offset stays off it.
    let along = |o: i32, len: i32| o.clamp((len > 2) as i32, (len - 1 - (len > 2) as i32).max(0));
    let (ox, oy) = (|o: i32| s.x + along(o, s.w), |o: i32| s.y + along(o, s.h));
    match anchor {
        Anchor::Top(o) => return ((ox(o), s.y - 1), Axis::Vertical),
        Anchor::Bottom(o) => return ((ox(o), s.y + s.h), Axis::Vertical),
        Anchor::Left(o) => return ((s.x - 1, oy(o)), Axis::Horizontal),
        Anchor::Right(o) => return ((s.x + s.w, oy(o)), Axis::Horizontal),
        Anchor::Auto => {}
    }
    let (dx, dy) = (toward.0 - cx, toward.1 - cy);
    // A cell is about twice as tall as wide, so a horizontal gap counts half.
    if dx.abs() >= 2 * dy.abs() {
        let x = if dx < 0 { s.x - 1 } else { s.x + s.w };
        ((x, cy), Axis::Horizontal)
    } else {
        let y = if dy < 0 { s.y - 1 } else { s.y + s.h };
        ((cx, y), Axis::Vertical)
    }
}

/// Cells from `a` to `b` with at most two bends. An axis is set for a bound end.
pub fn route(a: Pt, ax_a: Option<Axis>, b: Pt, ax_b: Option<Axis>) -> Vec<Pt> {
    let corners = match (ax_a, ax_b) {
        (Some(Axis::Horizontal), Some(_)) => {
            let m = (a.0 + b.0) / 2;
            vec![a, (m, a.1), (m, b.1), b]
        }
        (Some(_), Some(_)) => {
            let m = (a.1 + b.1) / 2;
            vec![a, (a.0, m), (b.0, m), b]
        }
        (Some(Axis::Vertical), None) | (None, Some(Axis::Horizontal)) => vec![a, (a.0, b.1), b],
        _ => vec![a, (b.0, a.1), b],
    };
    let mut cells = vec![a];
    for w in corners.windows(2) {
        let (mut c, to) = (w[0], w[1]);
        while c != to {
            if c.0 != to.0 {
                c.0 += (to.0 - c.0).signum();
            } else {
                c.1 += (to.1 - c.1).signum();
            }
            cells.push(c);
        }
    }
    cells
}

/// First cell, last cell and every cell where the route turns.
pub fn corners(cells: &[Pt]) -> Vec<Pt> {
    let dir = |p: Pt, q: Pt| ((q.0 - p.0).signum(), (q.1 - p.1).signum());
    let mut out = vec![cells[0]];
    for w in cells.windows(3) {
        if dir(w[0], w[1]) != dir(w[1], w[2]) {
            out.push(w[1]);
        }
    }
    if cells.len() > 1 {
        out.push(cells[cells.len() - 1]);
    }
    out
}

/// The route cell at `at` thousandths of the way along; the middle of the longest straight segment for `None`.
pub fn label_pos(cells: &[Pt], at: Option<u16>) -> Pt {
    if let Some(at) = at {
        let last = cells.len().saturating_sub(1);
        return cells[(usize::from(at.min(1000)) * last + 500) / 1000];
    }
    let len = |w: &[Pt]| (w[1].0 - w[0].0).abs() + (w[1].1 - w[0].1).abs();
    let corners = corners(cells);
    let mut best = &corners[..1];
    for w in corners.windows(2) {
        if best.len() < 2 || len(w) > len(best) {
            best = w;
        }
    }
    match best {
        [p, q] => ((p.0 + q.0) / 2, (p.1 + q.1) / 2),
        [p] => *p,
        _ => unreachable!(),
    }
}

/// Byte ranges of the lines of `s` wrapped to `room` cells. They are contiguous and cover `s`;
/// each keeps its trailing `\n` or spaces, so trim them for display.
pub fn wrap(s: &str, room: usize) -> Vec<Range<usize>> {
    let (mut out, mut start, mut w, mut brk) = (vec![], 0, 0, None);
    for (i, c) in s.char_indices() {
        if c == '\n' {
            out.push(start..i + 1);
            (start, w, brk) = (i + 1, 0, None);
            continue;
        }
        let cw = c.width().unwrap_or(0);
        // A space hangs at the line end, so it never breaks the line.
        while c != ' ' && w + cw > room && i > start {
            let cut = brk.take().unwrap_or(i);
            out.push(start..cut);
            start = cut;
            w = s[start..i].width();
        }
        if c == ' ' {
            brk = Some(i + 1);
        }
        w += cw;
    }
    out.push(start..s.len());
    out
}
