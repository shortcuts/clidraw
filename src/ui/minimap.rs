use super::grid::Grid;
use super::layout::{refresh, Layout};
use super::style::ACTIVE;
use crate::input::Session;
use crate::model::{Element, Id, ShapeKind};
use crate::view::fit_divisor;
use std::collections::HashSet;
use unicode_width::UnicodeWidthStr;

type Rect = (i32, i32, i32, i32);

/// What the overview shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Selection,
    Frame,
    Canvas,
}

impl Target {
    pub fn next(self) -> Target {
        match self {
            Target::Canvas => Target::Selection,
            Target::Selection => Target::Frame,
            Target::Frame => Target::Canvas,
        }
    }
}

/// The full-screen box, its inner cells and the world cells per screen cell.
struct Map {
    rect: Rect,
    inner: Rect,
    d: i32,
    ox: i32,
    oy: i32,
}

fn union(rects: impl Iterator<Item = Rect>) -> Option<Rect> {
    let (x0, y0, x1, y1) = rects.fold(
        (i32::MAX, i32::MAX, i32::MIN, i32::MIN),
        |a, (x, y, w, h)| (a.0.min(x), a.1.min(y), a.2.max(x + w), a.3.max(y + h)),
    );
    (x0 <= x1).then(|| (x0, y0, x1 - x0, y1 - y0))
}

fn is_frame(e: &Element) -> bool {
    matches!(e, Element::Shape(sh) if sh.kind == ShapeKind::Frame)
}

/// The smallest unselected frame holding the selection, else the viewport centre; the later one wins a tie.
fn frame_around(s: &Session, layout: &Layout, selection: Option<Rect>) -> Option<Rect> {
    let (vx, vy) = s.viewport_center();
    let (cx, cy) = s.view.to_world(vx, vy);
    s.canvas
        .elements
        .iter()
        .zip(&layout.bounds)
        .rev()
        .filter(|(e, _)| is_frame(e) && !s.selection.contains(e.id()))
        .map(|(_, &b)| b)
        .filter(|&(x, y, w, h)| match selection {
            Some((sx, sy, sw, sh)) => x <= sx && y <= sy && sx + sw <= x + w && sy + sh <= y + h,
            None => (x..x + w).contains(&cx) && (y..y + h).contains(&cy),
        })
        .min_by_key(|&(_, _, w, h)| i64::from(w) * i64::from(h))
}

/// The world rect a target covers; an empty target falls back to the canvas, then the viewport.
fn area(s: &Session, layout: &Layout, target: Target) -> Rect {
    let picked: HashSet<&Id> = s.selection.iter().collect();
    let selected = union(
        s.canvas
            .elements
            .iter()
            .zip(&layout.bounds)
            .filter(|(e, _)| picked.contains(e.id()))
            .map(|(_, &b)| b),
    );
    let chosen = match target {
        Target::Selection => selected,
        Target::Frame => frame_around(s, layout, selected),
        Target::Canvas => None,
    };
    chosen
        .or_else(|| union(layout.bounds.iter().copied()))
        .unwrap_or_else(|| s.visible_world())
}

impl Map {
    /// None when the screen is too small.
    fn new(s: &Session, layout: &Layout, target: Target) -> Option<Map> {
        let (sw, sh) = (i32::from(s.screen.0), i32::from(s.screen.1));
        if sw < 3 || sh < 4 {
            return None;
        }
        let (x, y, w, h) = area(s, layout, target);
        let (iw, ih) = (sw - 2, sh - 3);
        let d = fit_divisor((w, h), (iw, ih));
        Some(Map {
            rect: (0, 0, sw, sh - 1),
            inner: (1, 1, iw, ih),
            d,
            ox: x - (iw * d - w).div_euclid(2),
            oy: y - (ih * d - h).div_euclid(2),
        })
    }

    /// Builds the map for `target` and hands it to `f` with the layout it came from.
    fn with<R>(s: &Session, target: Target, f: impl FnOnce(&Map, &Layout) -> R) -> Option<R> {
        let mut slot = s.layout.borrow_mut();
        let layout = refresh(&mut slot, &s.canvas, s.rev);
        Map::new(s, layout, target).map(|m| f(&m, layout))
    }

    fn cell(&self, wx: i32, wy: i32) -> (i32, i32) {
        (
            self.inner.0 + (wx - self.ox).div_euclid(self.d),
            self.inner.1 + (wy - self.oy).div_euclid(self.d),
        )
    }

    /// The map cells (x0, y0, x1, y1) a world rect covers.
    fn cells(&self, (x, y, w, h): Rect) -> Rect {
        let (a, b) = (self.cell(x, y), self.cell(x + w - 1, y + h - 1));
        (a.0, a.1, b.0, b.1)
    }

    /// The world cell under a screen cell inside the box.
    fn world(&self, col: i32, row: i32) -> (i32, i32) {
        let (ix, iy, iw, ih) = self.inner;
        (
            self.ox + (col - ix).clamp(0, iw - 1) * self.d + self.d / 2,
            self.oy + (row - iy).clamp(0, ih - 1) * self.d + self.d / 2,
        )
    }

    fn inside(&self, col: i32, row: i32) -> bool {
        let (x, y, w, h) = self.rect;
        col >= x && col < x + w && row >= y && row < y + h
    }

    fn touches(&self, (x0, y0, x1, y1): Rect) -> bool {
        let (ix, iy, iw, ih) = self.inner;
        x0 < ix + iw && y0 < iy + ih && x1 >= ix && y1 >= iy
    }

    /// Visits the cells of `(x0, y0, x1, y1)` that fall inside the box.
    fn each(&self, (x0, y0, x1, y1): Rect, mut f: impl FnMut(i32, i32)) {
        let (ix, iy, iw, ih) = self.inner;
        for cy in y0.max(iy)..=y1.min(iy + ih - 1) {
            for cx in x0.max(ix)..=x1.min(ix + iw - 1) {
                f(cx, cy);
            }
        }
    }

    /// Draws `text` only when all of it fits inside the box.
    fn text(&self, grid: &mut Grid, x: i32, y: i32, text: &str, flags: u8) {
        let (ix, iy, iw, ih) = self.inner;
        if x >= ix && x + text.width() as i32 <= ix + iw && y >= iy && y < iy + ih {
            grid.text(x, y, text, flags);
        }
    }
}

fn first_line(e: &Element) -> &str {
    let label = match e {
        Element::Shape(sh) => &sh.label,
        Element::Text(t) => &t.text,
        Element::Line(_) => return "",
    };
    label.lines().next().unwrap_or_default()
}

pub fn draw(grid: &mut Grid, s: &Session, target: Target) {
    Map::with(s, target, |m, layout| {
        super::boxed(grid, m.rect, "Minimap");
        let find = layout.find(&s.canvas);
        let placed: Vec<_> = s
            .canvas
            .elements
            .iter()
            .zip(&layout.bounds)
            .map(|(e, &b)| (e, m.cells(b)))
            .filter(|&(_, c)| m.touches(c))
            .collect();
        for &(e, c @ (x0, y0, x1, y1)) in placed.iter().filter(|(e, _)| is_frame(e)) {
            m.each(c, |cx, cy| {
                let ch = match (cx == x0, cx == x1, cy == y0, cy == y1) {
                    (true, _, true, _) => '┌',
                    (_, true, true, _) => '┐',
                    (true, _, _, true) => '└',
                    (_, true, _, true) => '┘',
                    (_, _, true, _) | (_, _, _, true) => '─',
                    (true, ..) | (_, true, ..) => '│',
                    _ => return,
                };
                grid.set(cx, cy, ch, 0);
            });
            let name: String = first_line(e)
                .chars()
                .take((x1 - x0 - 1).max(0) as usize)
                .collect();
            m.text(grid, x0 + 1, y0, &name, 0);
        }
        for &(e, _) in &placed {
            if let Element::Line(l) = e {
                for c in s.canvas.route_with(l, &find, &layout.router) {
                    let (cx, cy) = m.cell(c.0, c.1);
                    m.each((cx, cy, cx, cy), |x, y| grid.set(x, y, '·', 0));
                }
            }
        }
        let blocks: Vec<_> = placed
            .iter()
            .filter(|(e, _)| !is_frame(e) && !matches!(e, Element::Line(_)))
            .collect();
        for &&(_, c) in &blocks {
            m.each(c, |cx, cy| grid.set(cx, cy, '▒', 0));
        }
        for &&(e, (x0, y0, x1, y1)) in &blocks {
            let label = first_line(e);
            if label.width() as i32 <= x1 - x0 + 1 {
                m.text(grid, x0, (y0 + y1) / 2, label, ACTIVE);
            }
        }
        let c @ (x0, y0, x1, y1) = m.cells(s.visible_world());
        m.each(c, |cx, cy| {
            if cx == x0 || cx == x1 || cy == y0 || cy == y1 {
                let ch = grid.get(cx, cy).map_or(' ', |c| c.ch);
                grid.set(cx, cy, ch, ACTIVE);
            }
        });
    });
}

/// Fits the view to the element under the click, else centers it on the clicked spot.
pub fn click(s: &mut Session, col: i32, row: i32, target: Target) {
    let hit = Map::with(s, target, |m, layout| {
        if !m.inside(col, row) {
            return None;
        }
        let bounds = s
            .canvas
            .elements
            .iter()
            .zip(&layout.bounds)
            .rev()
            .filter(|(e, _)| !matches!(e, Element::Line(_)))
            .filter(|(_, &b)| {
                let (x0, y0, x1, y1) = m.cells(b);
                (x0..=x1).contains(&col) && (y0..=y1).contains(&row)
            })
            .min_by_key(|(e, _)| is_frame(e))
            .map(|(_, &b)| b);
        Some((bounds, m.world(col, row)))
    })
    .flatten();
    match hit {
        Some((Some(bounds), _)) => s.fit(Some(bounds)),
        Some((None, (wx, wy))) => s.go_to(s.view.origin_for((wx, wy), s.viewport())),
        None => {}
    }
}
