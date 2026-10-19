use super::Shape;

pub fn center(s: &Shape) -> (i32, i32) {
    (s.x + s.w / 2, s.y + s.h / 2)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

type Pt = (i32, i32);

/// Midpoint of the side of `s` that faces `toward`, and the axis a line leaves it on.
pub fn side_mid(s: &Shape, toward: Pt) -> (Pt, Axis) {
    let (cx, cy) = center(s);
    let (dx, dy) = (toward.0 - cx, toward.1 - cy);
    // A cell is about twice as tall as wide, so a horizontal gap counts half.
    if dx.abs() >= 2 * dy.abs() {
        let x = if dx < 0 { s.x } else { s.x + s.w - 1 };
        ((x, cy), Axis::Horizontal)
    } else {
        let y = if dy < 0 { s.y } else { s.y + s.h - 1 };
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
        (Some(Axis::Vertical), None) | (None, Some(Axis::Vertical)) => vec![a, (a.0, b.1), b],
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

/// Middle cell of the longest straight segment.
pub fn label_pos(cells: &[Pt]) -> Pt {
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
