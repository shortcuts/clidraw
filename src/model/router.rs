use super::geom::Axis;
use super::{Canvas, Element, End, Find, Line, ShapeKind};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

type Pt = (i32, i32);
type Rect = (i32, i32, i32, i32);

/// Cells the search may leave beyond the box of the two end cells.
pub const MARGIN: i32 = 3;
const MAX_AREA: i32 = 600;
const BUCKET: i32 = 16;

// Packed cost, compared as (obstacle cells, line cells, length, bends): a path has at most
// MAX_AREA steps, so no lower term outweighs one unit of a higher one.
const BEND: u64 = 1;
const STEP: u64 = 1 << 12;
const LINE: u64 = 1 << 24;
const SOLID: u64 = 1 << 40;

const DIRS: [Pt; 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Solid,
    Frame,
    Line,
}

struct Item {
    at: usize,
    rect: Rect,
    kind: Kind,
}

type Buckets = HashMap<Pt, Vec<u32>>;

/// Spatial index of the elements a routed line avoids, built once per canvas state.
#[derive(Default)]
pub struct Router {
    items: Vec<Item>,
    solid: Buckets,
    lines: Buckets,
}

fn buckets((x, y, w, h): Rect) -> impl Iterator<Item = Pt> {
    let (x0, x1) = (x.div_euclid(BUCKET), (x + w - 1).div_euclid(BUCKET));
    let (y0, y1) = (y.div_euclid(BUCKET), (y + h - 1).div_euclid(BUCKET));
    (x0..=x1).flat_map(move |bx| (y0..=y1).map(move |by| (bx, by)))
}

fn holds((x, y, w, h): Rect, (px, py, pw, ph): Rect) -> bool {
    px >= x && py >= y && px + pw <= x + w && py + ph <= y + h
}

fn has((x, y, w, h): Rect, p: Pt) -> bool {
    holds((x, y, w, h), (p.0, p.1, 1, 1))
}

impl Router {
    /// `bounds[i]` is the box of `canvas.elements[i]`.
    pub fn from_bounds(canvas: &Canvas, bounds: &[Rect]) -> Router {
        let mut r = Router {
            items: vec![],
            solid: Buckets::new(),
            lines: Buckets::new(),
        };
        for (at, (e, &rect)) in canvas.elements.iter().zip(bounds).enumerate() {
            let kind = match e {
                Element::Shape(s) if s.kind == ShapeKind::Frame => Kind::Frame,
                Element::Shape(_) | Element::Text(_) => Kind::Solid,
                Element::Line(_) => Kind::Line,
            };
            let map = if kind == Kind::Line {
                &mut r.lines
            } else {
                &mut r.solid
            };
            for b in buckets(rect) {
                map.entry(b).or_default().push(r.items.len() as u32);
            }
            r.items.push(Item { at, rect, kind });
        }
        r
    }

    fn near(&self, map: &Buckets, area: Rect) -> Vec<&Item> {
        let mut ids: Vec<u32> = buckets(area)
            .filter_map(|b| map.get(&b))
            .flatten()
            .copied()
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let (x, y, w, h) = area;
        let items = ids.into_iter().map(|i| &self.items[i as usize]);
        items
            .filter(|i| {
                let r = i.rect;
                r.0 < x + w && r.0 + r.2 > x && r.1 < y + h && r.1 + r.3 > y
            })
            .collect()
    }

    /// `plain` when no cell of it lies in an obstacle, else the cheapest path in the box of the
    /// end cells plus `MARGIN`. Obstacles are the other shapes, texts and frames, minus the two
    /// end shapes (a bent line keeps them as obstacles) and the frames holding an end.
    // shortcut: other lines count by their plain route, a long line or a wide obstacle is not avoided
    pub fn around<'a>(
        &self,
        canvas: &'a Canvas,
        find: &Find<'a>,
        own: &Line,
        ends: [(Pt, Option<Axis>); 2],
        plain: Vec<Pt>,
    ) -> Vec<Pt> {
        let [(a, ax_a), (b, ax_b)] = ends;
        let end_rect = |e: &End, at: Pt| match e {
            End::Bound(id, _) => find(id).map_or((at.0, at.1, 1, 1), |s| (s.x, s.y, s.w, s.h)),
            End::Free(x, y) => (*x, *y, 1, 1),
        };
        let held = [end_rect(&own.from, a), end_rect(&own.to, b)];
        let is_end = |i: &Item| match &canvas.elements[i.at] {
            Element::Shape(s) => [&own.from, &own.to]
                .iter()
                .any(|e| matches!(e, End::Bound(id, _) if *id == s.id)),
            _ => false,
        };
        let bent = !own.bends.is_empty();
        let skip = |i: &Item| {
            if is_end(i) {
                return !bent;
            }
            i.kind == Kind::Frame && held.iter().any(|&h| holds(i.rect, h))
        };
        let blocked = |p: Pt| {
            if p == a || p == b {
                return false;
            }
            let at = (p.0.div_euclid(BUCKET), p.1.div_euclid(BUCKET));
            let items = self.solid.get(&at).map_or(&[][..], |v| v);
            items.iter().any(|&i| {
                let i = &self.items[i as usize];
                has(i.rect, p) && !skip(i)
            })
        };
        if !plain.iter().any(|&p| blocked(p)) {
            return plain;
        }
        if a == b {
            return vec![a];
        }
        let (x0, y0) = (a.0.min(b.0) - MARGIN, a.1.min(b.1) - MARGIN);
        let (x1, y1) = (a.0.max(b.0) + MARGIN, a.1.max(b.1) + MARGIN);
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        if i64::from(w) * i64::from(h) > i64::from(MAX_AREA) {
            return plain;
        }
        let area = (x0, y0, w, h);
        let cell = |p: Pt| ((p.1 - y0) * w + p.0 - x0) as usize;
        // 1: obstacle, 2: other line, 4: end shape, impassable.
        let mut grid = vec![0u8; (w * h) as usize];
        let mut stamp = |r: Rect, v: u8| {
            for y in r.1.max(y0)..(r.1 + r.3).min(y0 + h) {
                for x in r.0.max(x0)..(r.0 + r.2).min(x0 + w) {
                    grid[cell((x, y))] |= v;
                }
            }
        };
        for i in self.near(&self.solid, area) {
            if is_end(i) {
                stamp(i.rect, 4);
            } else if !skip(i) {
                stamp(i.rect, 1);
            }
        }
        for i in self.near(&self.lines, area) {
            if let Element::Line(l) = &canvas.elements[i.at] {
                if l.id != own.id {
                    for p in canvas.plain_with(l, find) {
                        if has(area, p) {
                            grid[cell(p)] |= 2;
                        }
                    }
                }
            }
        }
        let goal = cell(b);
        let first: &[usize] = match ax_a {
            Some(Axis::Horizontal) => &[0, 1],
            Some(Axis::Vertical) => &[2, 3],
            None => &[0, 1, 2, 3],
        };
        let last = |d: usize| match ax_b {
            Some(Axis::Horizontal) => d < 2,
            Some(Axis::Vertical) => d >= 2,
            None => true,
        };
        let enter = |c: usize, turn: bool| {
            let v = grid[c];
            STEP + u64::from(turn) * BEND
                + if v & 2 != 0 { LINE } else { 0 }
                + if v & 1 != 0 { SOLID } else { 0 }
        };
        let mut dist = vec![u64::MAX; grid.len() * 4];
        let mut from = vec![u32::MAX; grid.len() * 4];
        let mut heap = BinaryHeap::new();
        let step = |c: usize, d: usize| {
            let (x, y) = (c as i32 % w + DIRS[d].0, c as i32 / w + DIRS[d].1);
            ((0..w).contains(&x) && (0..h).contains(&y)).then(|| (y * w + x) as usize)
        };
        let start = cell(a);
        for &d in first {
            if let Some(n) = step(start, d) {
                if grid[n] & 4 == 0 || n == goal {
                    dist[n * 4 + d] = enter(n, false);
                    heap.push(Reverse((dist[n * 4 + d], (n * 4 + d) as u32)));
                }
            }
        }
        while let Some(Reverse((cost, s))) = heap.pop() {
            let (c, d) = (s as usize / 4, s as usize % 4);
            if cost > dist[s as usize] {
                continue;
            }
            if c == goal && last(d) {
                let mut path = vec![];
                let mut at = s;
                while at != u32::MAX {
                    let c = at as usize / 4;
                    path.push(((c as i32 % w) + x0, (c as i32 / w) + y0));
                    at = from[at as usize];
                }
                path.push(a);
                path.reverse();
                return path;
            }
            for nd in (0..4).filter(|&nd| nd != d ^ 1) {
                let Some(n) = step(c, nd) else { continue };
                if grid[n] & 4 != 0 && n != goal {
                    continue;
                }
                let next = cost + enter(n, nd != d);
                let t = n * 4 + nd;
                if next < dist[t] {
                    dist[t] = next;
                    from[t] = s;
                    heap.push(Reverse((next, t as u32)));
                }
            }
        }
        plain
    }
}
