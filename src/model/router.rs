use super::geom::Axis;
use super::{Canvas, Element, End, Find, Id, Line, ShapeKind};
use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::hash::{BuildHasherDefault, Hasher};

type Pt = (i32, i32);
type Rect = (i32, i32, i32, i32);

/// Cells the search may leave beyond the box of the two end cells.
pub const MARGIN: i32 = 3;
const STUB: i32 = 2;
const LANE: i32 = 4;
/// How far a route strays from the box of its end cells, walls aside.
pub const REACH: i32 = LANE + MARGIN;
const MAX_AREA: i32 = 2000;
const BUCKET: i32 = 16;

// Packed cost, compared as (obstacle cells, line cells, length, bends): a path has at most
// MAX_AREA steps, so no lower term outweighs one unit of a higher one.
// Entering the end against its axis counts as a line cell, so a bent stub beats a crossed shape.
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

/// Hashes a bucket key by multiplication: SipHash dominates the per-cell obstacle lookups.
#[derive(Default)]
struct CellHasher(u64);

impl Hasher for CellHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(u64::from(b));
        }
    }

    fn write_i32(&mut self, n: i32) {
        self.write_u64(n as u32 as u64);
    }

    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

type Buckets = HashMap<Pt, Vec<u32>, BuildHasherDefault<CellHasher>>;

/// The cells of an earlier line's route inside `area`, as a route read them.
pub struct Dep {
    at: usize,
    id: Id,
    area: Rect,
    seen: Vec<Pt>,
}

/// A routed line, the box outside which no obstacle can alter it, and the earlier routes it read.
/// `stale` marks a route whose `deps` need a check before it is reused.
struct Cached {
    path: Vec<Pt>,
    foot: Rect,
    deps: Vec<Dep>,
    stale: bool,
}

/// Spatial index of the elements a routed line avoids, built once per canvas state.
#[derive(Default)]
pub struct Router {
    items: Vec<Item>,
    solid: Buckets,
    lines: Buckets,
    routes: RefCell<HashMap<Id, Cached>>,
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

fn meets(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && a.0 + a.2 > b.0 && a.1 < b.1 + b.3 && a.1 + a.3 > b.1
}

pub(super) fn union(a: Rect, b: Rect) -> Rect {
    let (x, y) = (a.0.min(b.0), a.1.min(b.1));
    (
        x,
        y,
        (a.0 + a.2).max(b.0 + b.2) - x,
        (a.1 + a.3).max(b.1 + b.3) - y,
    )
}

fn cells_in(path: &[Pt], area: Rect) -> Vec<Pt> {
    path.iter().copied().filter(|&p| has(area, p)).collect()
}

pub(super) fn grow((x, y, w, h): Rect, by: i32) -> Rect {
    (x - by, y - by, w + 2 * by, h + 2 * by)
}

impl Router {
    /// `bounds[i]` is the box of `canvas.elements[i]`.
    pub fn from_bounds(canvas: &Canvas, bounds: &[Rect]) -> Router {
        let mut r = Router::default();
        for (at, (e, &rect)) in canvas.elements.iter().zip(bounds).enumerate() {
            let kind = match e {
                Element::Shape(s) if s.kind == ShapeKind::Frame => continue,
                Element::Shape(_) | Element::Text(_) => Kind::Solid,
                Element::Line(_) => Kind::Line,
            };
            // shortcut: a detour past a wall wider than the search box leaves this rect, upgrade when a test shows the overlap
            let (map, rect) = if kind == Kind::Line {
                (&mut r.lines, grow(rect, REACH))
            } else {
                (&mut r.solid, rect)
            };
            for b in buckets(rect) {
                map.entry(b).or_default().push(r.items.len() as u32);
            }
            r.items.push(Item { at, rect });
        }
        r
    }

    pub fn cached(&self, id: &Id) -> Option<Vec<Pt>> {
        self.routes.borrow().get(id).map(|c| c.path.clone())
    }

    pub fn store(&self, id: Id, path: Vec<Pt>, foot: Rect, deps: Vec<Dep>) {
        let cached = Cached {
            path,
            foot,
            deps,
            stale: false,
        };
        self.routes.borrow_mut().insert(id, cached);
    }

    /// Takes the routes of `old` that no `dirty` rect touches. A route that read an earlier line
    /// stays until `settled` finds that line took other cells where it looked.
    pub fn carry(&mut self, old: &mut Router, dirty: &[Rect]) {
        let mut routes = std::mem::take(old.routes.get_mut());
        routes.retain(|_, c| !dirty.iter().any(|&d| meets(c.foot, d)));
        for c in routes.values_mut() {
            c.stale = !c.deps.is_empty();
        }
        *self.routes.get_mut() = routes;
    }

    /// Reads the cached route of `id` with `read`, or returns `None` when an earlier line it read has moved on.
    pub fn settled<'a, T>(
        &self,
        canvas: &'a Canvas,
        find: &Find<'a>,
        id: &Id,
        read: impl FnOnce(&[Pt]) -> T,
    ) -> Option<T> {
        let deps = {
            let mut routes = self.routes.borrow_mut();
            let c = routes.get_mut(id)?;
            if !c.stale {
                return Some(read(&c.path));
            }
            std::mem::take(&mut c.deps)
        };
        let holds = deps.iter().all(|d| match canvas.elements.get(d.at) {
            Some(Element::Line(l)) if l.id == d.id => {
                cells_in(&canvas.route_with(l, find, self), d.area) == d.seen
            }
            _ => false,
        });
        let mut routes = self.routes.borrow_mut();
        if !holds {
            routes.remove(id);
            return None;
        }
        let c = routes.get_mut(id)?;
        c.deps = deps;
        c.stale = false;
        Some(read(&c.path))
    }

    /// The cells of the route `l` really takes inside `area`. The caller's route drops when they change.
    fn earlier<'a, 'd>(
        &self,
        canvas: &'a Canvas,
        find: &Find<'a>,
        (i, l): (&Item, &Line),
        area: Rect,
        deps: &'d mut Vec<Dep>,
    ) -> &'d [Pt] {
        let seen = self
            .settled(canvas, find, &l.id, |p| cells_in(p, area))
            .unwrap_or_else(|| cells_in(&canvas.route_with(l, find, self), area));
        deps.push(Dep {
            at: i.at,
            id: l.id.clone(),
            area,
            seen,
        });
        &deps[deps.len() - 1].seen
    }

    fn near(&self, map: &Buckets, area: Rect) -> Vec<&Item> {
        let (x, y, w, h) = area;
        let (ax, ay) = (x.div_euclid(BUCKET), y.div_euclid(BUCKET));
        // An item sits in several buckets: keep it in the first one the area also covers.
        buckets(area)
            .flat_map(|b| {
                let ids = map.get(&b).map_or(&[][..], |v| v);
                ids.iter().map(move |&i| (b, &self.items[i as usize]))
            })
            .filter(|&(b, i)| {
                let r = i.rect;
                r.0 < x + w
                    && r.0 + r.2 > x
                    && r.1 < y + h
                    && r.1 + r.3 > y
                    && b == (
                        ax.max(r.0.div_euclid(BUCKET)),
                        ay.max(r.1.div_euclid(BUCKET)),
                    )
            })
            .map(|(_, i)| i)
            .collect()
    }

    /// Whether `plain`, bar its end cells `a` and `b`, meets the route of an earlier line.
    fn crosses_line<'a>(
        &self,
        canvas: &'a Canvas,
        find: &Find<'a>,
        (a, b): (Pt, Pt),
        plain: &[Pt],
        me: usize,
        deps: &mut Vec<Dep>,
    ) -> bool {
        let xs = plain.iter().map(|p| p.0);
        let ys = plain.iter().map(|p| p.1);
        let (x0, y0) = (xs.clone().min().unwrap_or(0), ys.clone().min().unwrap_or(0));
        let (x1, y1) = (xs.max().unwrap_or(0), ys.max().unwrap_or(0));
        let region = (x0, y0, x1 - x0 + 1, y1 - y0 + 1);
        let near = self.near(&self.lines, region);
        // Only the later line detours: if both did, each would dodge the other's plain route and still cross.
        near.into_iter()
            .filter(|i| i.at < me)
            .filter(|i| plain.iter().any(|&p| has(i.rect, p)))
            .filter_map(|i| match &canvas.elements[i.at] {
                Element::Line(l) => Some((i, l)),
                _ => None,
            })
            .any(|(i, l)| {
                let theirs = self.earlier(canvas, find, (i, l), region, deps);
                plain
                    .iter()
                    .any(|&p| p != a && p != b && theirs.contains(&p))
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

    /// `plain` when no cell of it lies in an obstacle and it crosses no earlier line, else the
    /// cheapest path in the box of the end cells plus `MARGIN`. Obstacles are the other shapes and
    /// texts, minus the two end shapes. A frame is no obstacle.
    /// Also returns a box: an obstacle change outside it leaves the path as is, and the earlier routes it read.
    // shortcut: a later line is read by its plain route, only earlier lines count by their actual route
    pub fn around<'a>(
        &self,
        canvas: &'a Canvas,
        find: &Find<'a>,
        own: &Line,
        ends: [(Pt, Option<Axis>); 2],
        plain: Vec<Pt>,
    ) -> (Vec<Pt>, Rect, Vec<Dep>) {
        let [(a, ax_a), (b, ax_b)] = ends;
        let end_rect = |e: &End, at: Pt| match e {
            End::Bound(id, _) => find(id).map_or((at.0, at.1, 1, 1), |s| (s.x, s.y, s.w, s.h)),
            End::Free(x, y) => (*x, *y, 1, 1),
        };
        let held = [end_rect(&own.from, a), end_rect(&own.to, b)];
        // The end stub and the lines ending beside an end shape reach LANE cells past it.
        let foot = |searched: Rect| {
            let ends = held.map(|r| grow(r, LANE + 1));
            union(union(searched, ends[0]), ends[1])
        };
        let plain_foot = || {
            let xs = plain.iter().map(|p| p.0);
            let ys = plain.iter().map(|p| p.1);
            let (x0, y0) = (xs.clone().min().unwrap_or(0), ys.clone().min().unwrap_or(0));
            let (x1, y1) = (xs.max().unwrap_or(0), ys.max().unwrap_or(0));
            foot(grow((x0, y0, x1 - x0 + 1, y1 - y0 + 1), MARGIN))
        };
        let is_end = |i: &Item| match &canvas.elements[i.at] {
            Element::Shape(s) => [&own.from, &own.to]
                .iter()
                .any(|e| matches!(e, End::Bound(id, _) if *id == s.id)),
            _ => false,
        };
        let blocked = |p: Pt| {
            if p == a || p == b {
                return false;
            }
            let at = (p.0.div_euclid(BUCKET), p.1.div_euclid(BUCKET));
            let items = self.solid.get(&at).map_or(&[][..], |v| v);
            items.iter().any(|&i| {
                let i = &self.items[i as usize];
                has(i.rect, p) && !is_end(i)
            })
        };
        // A start beside the end cell cannot clear a long stub without a parallel U.
        let close = (a.0 - b.0).abs().max((a.1 - b.1).abs()) <= LANE;
        // The cells before the end cell run straight away from the shape, so the head points at it.
        // Arrows sharing a side get a longer run, so they look merged while keeping their own end cells.
        let stub: Option<Vec<Pt>> = ax_b.and_then(|_| {
            let out = DIRS.iter().find(|d| has(held[1], (b.0 + d.0, b.1 + d.1)))?;
            let len = if close {
                1
            } else if self.lane_sibling(canvas, find, own, held[1], b, *out) {
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
        let me = self
            .near(&self.lines, (a.0, a.1, 1, 1))
            .iter()
            .find(|i| matches!(&canvas.elements[i.at], Element::Line(l) if l.id == own.id))
            .map_or(0, |i| i.at);
        let mut deps = vec![];
        let ok = straight
            && !plain.iter().any(|&p| blocked(p))
            && !self.crosses_line(canvas, find, (a, b), &plain, me, &mut deps);
        if ok {
            let area = plain_foot();
            return (plain, area, deps);
        }
        if a == b {
            return (vec![a], plain_foot(), deps);
        }
        let t = stub.as_ref().map_or(b, |s| s[s.len() - 1]);
        let base = (
            a.0.min(t.0).min(b.0) - MARGIN,
            a.1.min(t.1).min(b.1) - MARGIN,
            a.0.max(t.0).max(b.0) + MARGIN,
            a.1.max(t.1).max(b.1) + MARGIN,
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
            let area = plain_foot();
            return (plain, area, deps);
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
            } else {
                stamp(i.rect, 1);
            }
        }
        for i in self.near(&self.lines, area) {
            if let Element::Line(l) = &canvas.elements[i.at] {
                if l.id != own.id {
                    let later;
                    let theirs: &[Pt] = if i.at < me {
                        self.earlier(canvas, find, (i, l), area, &mut deps)
                    } else if meets(grow(i.rect, -REACH), area) {
                        later = canvas.plain_with(l, find);
                        &later[..]
                    } else {
                        &[][..]
                    };
                    for &p in theirs {
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
        let enter = |c: usize, turn: bool, d: usize| {
            let v = grid[c];
            STEP + u64::from(turn) * BEND
                + if c == goal && !close && !last(d) {
                    LINE
                } else {
                    0
                }
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
                    dist[n * 4 + d] = enter(n, false, d);
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
            if c == goal {
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
                return (path, foot(area), deps);
            }
            for nd in (0..4).filter(|&nd| nd != d ^ 1) {
                let Some(n) = step(c, nd) else { continue };
                if grid[n] & 4 != 0 && n != goal {
                    continue;
                }
                let next = cost + enter(n, nd != d, nd);
                let t = n * 4 + nd;
                if next < dist[t] {
                    dist[t] = next;
                    from[t] = s;
                    heap.push(Reverse((next + est(n), t as u32)));
                }
            }
        }
        (plain, foot(area), deps)
    }
}
