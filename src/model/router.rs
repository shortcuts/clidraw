use super::geom::Axis;
use super::{Canvas, Element, End, Find, Line, ShapeKind};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

type Pt = (i32, i32);
type Rect = (i32, i32, i32, i32);

/// Cells the search may leave beyond the box of the two end cells.
pub const MARGIN: i32 = 3;
const STUB: i32 = 2;
const LANE: i32 = 4;
const MAX_AREA: i32 = 1000;
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
    Line,
}

struct Item {
    at: usize,
    rect: Rect,
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
                Element::Shape(s) if s.kind == ShapeKind::Frame => continue,
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
            r.items.push(Item { at, rect });
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

    fn shares_segment<'a>(
        &self,
        canvas: &'a Canvas,
        find: &Find<'a>,
        own: &Line,
        plain: &[Pt],
    ) -> bool {
        let xs = plain.iter().map(|p| p.0);
        let ys = plain.iter().map(|p| p.1);
        let (x0, y0) = (xs.clone().min().unwrap_or(0), ys.clone().min().unwrap_or(0));
        let (x1, y1) = (xs.max().unwrap_or(0), ys.max().unwrap_or(0));
        let near = self.near(&self.lines, (x0, y0, x1 - x0 + 1, y1 - y0 + 1));
        let line = |i: &Item| match &canvas.elements[i.at] {
            Element::Line(l) => Some(l),
            _ => None,
        };
        let me = near
            .iter()
            .find(|i| line(i).is_some_and(|l| l.id == own.id))
            .map_or(usize::MAX, |i| i.at);
        // Only the later line detours: if both did, each would dodge the other's plain route and still overlap.
        near.into_iter()
            .filter(|i| i.at < me)
            .filter(|i| {
                plain
                    .windows(2)
                    .any(|w| has(i.rect, w[0]) && has(i.rect, w[1]))
            })
            .filter_map(|i| line(i).map(|l| canvas.plain_with(l, find)))
            .any(|theirs| {
                let theirs: HashSet<Pt> = theirs.into_iter().collect();
                plain
                    .windows(2)
                    .any(|w| theirs.contains(&w[0]) && theirs.contains(&w[1]))
            })
    }

    /// Whether another line ends on the same side of the same shape as `own` does at `b`.
    fn lane_sibling<'a>(
        &self,
        canvas: &'a Canvas,
        find: &Find<'a>,
        own: &Line,
        shape: Rect,
        b: Pt,
        out: Pt,
    ) -> bool {
        let End::Bound(id, _) = &own.to else {
            return false;
        };
        let (x, y, w, h) = shape;
        self.near(&self.lines, (x - 1, y - 1, w + 2, h + 2))
            .into_iter()
            .filter_map(|i| match &canvas.elements[i.at] {
                Element::Line(l) if l.id != own.id => Some(l),
                _ => None,
            })
            .filter(|l| matches!(&l.to, End::Bound(other, _) if other == id))
            .map(|l| canvas.line_ends_with(l, find).1)
            .any(|e| if out.0 != 0 { e.0 == b.0 } else { e.1 == b.1 })
    }

    /// `plain` when no cell of it lies in an obstacle and it shares no segment with an earlier
    /// line, else the cheapest path in the box of the end cells plus `MARGIN`. Obstacles are the other shapes and texts, minus the two
    /// end shapes (a bent line keeps them as obstacles). A frame is no obstacle.
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
        let skip = |i: &Item| is_end(i) && !bent;
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
        // The cells before the end cell run straight away from the shape, so the head points at it.
        // Arrows sharing a side get a longer run, so they look merged while keeping their own end cells.
        let stub: Option<Vec<Pt>> = ax_b.and_then(|_| {
            let out = DIRS.iter().find(|d| has(held[1], (b.0 + d.0, b.1 + d.1)))?;
            let len = if self.lane_sibling(canvas, find, own, held[1], b, *out) {
                LANE
            } else {
                STUB
            };
            let cells: Vec<Pt> = (1..=len)
                .map(|k| (b.0 - out.0 * k, b.1 - out.1 * k))
                .collect();
            (!cells.iter().any(|&p| blocked(p) || p == a) && a != b).then_some(cells)
        });
        let straight = stub.as_ref().is_none_or(|s| {
            let tail: Vec<Pt> = s.iter().rev().copied().chain([b]).collect();
            plain.ends_with(&tail)
        });
        if straight
            && !plain.iter().any(|&p| blocked(p))
            && !self.shares_segment(canvas, find, own, &plain)
        {
            return plain;
        }
        if a == b {
            return vec![a];
        }
        let t = stub.as_ref().map_or(b, |s| s[s.len() - 1]);
        let base = (
            a.0.min(t.0) - MARGIN,
            a.1.min(t.1) - MARGIN,
            a.0.max(t.0) + MARGIN,
            a.1.max(t.1) + MARGIN,
        );
        let (mut x0, mut y0, mut x1, mut y1) = base;
        // An obstacle that spans the whole box blocks every way round inside it, so widen the box past it.
        for i in self.near(&self.solid, (x0, y0, x1 - x0 + 1, y1 - y0 + 1)) {
            let r = i.rect;
            if is_end(i) {
                continue;
            }
            if r.1 <= base.1 && r.1 + r.3 > base.3 {
                (y0, y1) = (y0.min(r.1 - MARGIN), y1.max(r.1 + r.3 - 1 + MARGIN));
            }
            if r.0 <= base.0 && r.0 + r.2 > base.2 {
                (x0, x1) = (x0.min(r.0 - MARGIN), x1.max(r.0 + r.2 - 1 + MARGIN));
            }
        }
        if i64::from(x1 - x0 + 1) * i64::from(y1 - y0 + 1) > i64::from(MAX_AREA) {
            (x0, y0, x1, y1) = base;
        }
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
        for p in stub.iter().flatten().chain([&b]) {
            grid[cell(*p)] |= 4;
        }
        let goal = cell(t);
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
        // Every step costs at least STEP, so the Manhattan distance never overestimates.
        let est = |c: usize| {
            let (dx, dy) = (
                (c as i32 % w - t.0 + x0).abs(),
                (c as i32 / w - t.1 + y0).abs(),
            );
            STEP * (dx + dy) as u64
        };
        let step = |c: usize, d: usize| {
            let (x, y) = (c as i32 % w + DIRS[d].0, c as i32 / w + DIRS[d].1);
            ((0..w).contains(&x) && (0..h).contains(&y)).then(|| (y * w + x) as usize)
        };
        let start = cell(a);
        for &d in first {
            if let Some(n) = step(start, d) {
                if grid[n] & 4 == 0 || n == goal {
                    dist[n * 4 + d] = enter(n, false);
                    heap.push(Reverse((dist[n * 4 + d] + est(n), (n * 4 + d) as u32)));
                }
            }
        }
        while let Some(Reverse((f, s))) = heap.pop() {
            let (c, d) = (s as usize / 4, s as usize % 4);
            let cost = dist[s as usize];
            if f > cost + est(c) {
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
                if let Some(s) = &stub {
                    path.extend(s.iter().rev().skip(1).chain([&b]));
                }
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
                    heap.push(Reverse((next + est(n), t as u32)));
                }
            }
        }
        plain
    }
}
