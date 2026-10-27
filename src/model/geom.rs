use super::{Anchor, Edge, Shape, SideLabel};
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
    let (lo, hi) = s.row_span(y);
    let d = [y - s.y, bottom - y, x - lo, hi - x];
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

/// Whether `p` lies strictly inside the drawn outline of `s`.
pub fn inside(s: &Shape, p: Pt) -> bool {
    let (lo, hi) = s.row_span(p.1);
    s.y < p.1 && p.1 < s.y + s.h - 1 && lo < p.0 && p.0 < hi
}

/// Anchor on the side of `s` that the cell `c` touches, at the offset of `c` along it.
/// A cell inside the outline snaps to the nearest side.
pub fn side_anchor(s: &Shape, c: Pt) -> Anchor {
    if inside(s, c) {
        edge_anchor(s, c)
    } else if c.1 < s.y {
        Anchor::Top(c.0 - s.x)
    } else if c.1 >= s.y + s.h {
        Anchor::Bottom(c.0 - s.x)
    } else if c.0 <= s.row_span(c.1).0 {
        Anchor::Left(c.1 - s.y)
    } else {
        Anchor::Right(c.1 - s.y)
    }
}

/// The cell next to the drawn outline of `s` on the side the anchor names, at its offset
/// along it, or at the midpoint of the side that faces `toward` for `Auto`, and the axis a line
/// leaves it on. The cell is outside the outline, or inside when `toward` lies in the box of `s`.
pub fn side_mid(s: &Shape, anchor: Anchor, toward: Pt) -> (Pt, Axis) {
    side_cell(s, anchor, toward, toward)
}

/// `side_mid` where `face` decides outer or inner and `toward` picks the side of an `Auto` anchor.
// shortcut: shapes under 2 cells apart overlap the end cells, clamp if it matters
pub(super) fn side_cell(s: &Shape, anchor: Anchor, toward: Pt, face: Pt) -> (Pt, Axis) {
    let (cx, cy) = center(s);
    let inner = s.w >= 3
        && s.h >= 3
        && (s.x..s.x + s.w).contains(&face.0)
        && (s.y..s.y + s.h).contains(&face.1);
    let roomy = |row: i32| {
        let (lo, hi) = s.row_span(row);
        inner && hi - lo >= 2
    };
    // A corner has no side to land on, so an offset stays off it.
    let along = |o: i32, len: i32| o.clamp((len > 2) as i32, (len - 1 - (len > 2) as i32).max(0));
    let oy = |o: i32| s.y + along(o, s.h);
    // The border of the row above or below touches the outer cell, so the cell steps out one more.
    let touches = |row: i32, x: i32, dir: i32| {
        [row - 1, row + 1].into_iter().any(|r| {
            (s.y..s.y + s.h).contains(&r) && {
                let (lo, hi) = s.row_span(r);
                if dir < 0 {
                    lo == x
                } else {
                    hi == x
                }
            }
        })
    };
    let side = |row: i32, dir: i32| {
        let inset = if inner {
            row.clamp(s.y + 1, s.y + s.h - 2)
        } else {
            row
        };
        let (row, o) = if roomy(inset) {
            (inset, -dir)
        } else {
            (row, dir)
        };
        let (lo, hi) = s.row_span(row);
        let x = (if dir < 0 { lo } else { hi }) + o;
        let x = if !inner && touches(row, x, dir) {
            x + dir
        } else {
            x
        };
        ((x, row), Axis::Horizontal)
    };
    let (left, right) = (|row| side(row, -1), |row| side(row, 1));
    let edge = |x: i32, border: i32, out_y: i32, in_row: i32| {
        let (row, y) = if roomy(in_row) {
            (in_row, in_row)
        } else {
            (border, out_y)
        };
        let (lo, hi) = s.row_span(row);
        let pad = (hi - lo > 2) as i32;
        ((x.clamp(lo + pad, hi - pad), y), Axis::Vertical)
    };
    let (top, bottom) = (s.y, s.y + s.h - 1);
    let (up, down) = (
        |x| edge(x, top, s.y - 1, s.y + 1),
        |x| edge(x, bottom, s.y + s.h, s.y + s.h - 2),
    );
    match anchor {
        Anchor::Top(o) => return up(s.x + o),
        Anchor::Bottom(o) => return down(s.x + o),
        Anchor::Left(o) => return left(oy(o)),
        Anchor::Right(o) => return right(oy(o)),
        Anchor::Auto => {}
    }
    let (dx, dy) = (toward.0 - cx, toward.1 - cy);
    // A cell is about twice as tall as wide, so a horizontal gap counts half.
    if dx.abs() >= 2 * dy.abs() {
        if dx < 0 {
            left(cy)
        } else {
            right(cy)
        }
    } else if dy < 0 {
        up(cx)
    } else {
        down(cx)
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

/// The `at` value that puts the label on the route cell nearest `p`.
pub fn label_at_nearest(cells: &[Pt], p: Pt) -> Option<u16> {
    let near = |c: &Pt| (c.0 - p.0).abs() + (c.1 - p.1).abs();
    let i = cells
        .iter()
        .enumerate()
        .min_by_key(|(_, c)| near(c))
        .map(|(i, _)| i)?;
    let last = cells.len().saturating_sub(1).max(1);
    Some(((i * 1000 + last / 2) / last) as u16)
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

/// Index of the border cell `at` thousandths along a side whose last index is `span`.
pub fn side_index(span: i32, at: u16) -> i32 {
    (i32::from(at.min(1000)) * span + 500) / 1000
}

/// Thousandths along a side for the border cell `offset`, clamped to `0..=span`.
pub fn side_at(span: i32, offset: i32) -> u16 {
    if span <= 0 {
        return 500;
    }
    ((offset.clamp(0, span) * 1000 + span / 2) / span) as u16
}

/// Start column, row and shown text of a side label, drawn over the border cells of `s`.
pub fn side_label_at<'a>(s: &Shape, l: &'a SideLabel) -> (i32, i32, &'a str) {
    let horizontal = matches!(l.edge, Edge::Top | Edge::Bottom);
    let span = (if horizontal { s.w } else { s.h } - 3).max(0);
    let i = l.at.map_or(span / 2, |at| side_index(span, at));
    let room = if horizontal { s.w - 2 } else { s.w }.max(0) as usize;
    let (mut used, mut end) = (0, 0);
    for (b, c) in l.text.char_indices() {
        used += c.width().unwrap_or(0);
        if used > room {
            break;
        }
        end = b + c.len_utf8();
    }
    let text = &l.text[..end];
    let tw = text.width() as i32;
    match l.edge {
        Edge::Top | Edge::Bottom => {
            let row = if l.edge == Edge::Top {
                s.y
            } else {
                s.y + s.h - 1
            };
            let at = s.x + 1 + i - (tw - 1).max(0) / 2;
            (
                at.clamp(s.x + 1, (s.x + s.w - 1 - tw).max(s.x + 1)),
                row,
                text,
            )
        }
        Edge::Left => (s.x, s.y + 1 + i, text),
        Edge::Right => (s.x + s.w - tw, s.y + 1 + i, text),
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
