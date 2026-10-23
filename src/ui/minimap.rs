use super::grid::{Grid, REVERSE};
use super::layout::refresh;
use crate::input::Session;
use crate::model::{Element, ShapeKind};

/// The full-screen box and the world area it maps onto.
pub struct Map {
    rect: (i32, i32, i32, i32),
    area: (i32, i32, i32, i32),
}

/// A shape or text as the map shows it: its world bounds, map cells and label.
struct Item {
    frame: bool,
    bounds: (i32, i32, i32, i32),
    cells: (i32, i32, i32, i32),
    label: String,
}

impl Map {
    /// The world area is every element plus the viewport. None when the screen is too small.
    // shortcut: x and y scale separately, upgrade if the distortion confuses users
    fn new(s: &Session) -> Option<Map> {
        let (sw, sh) = (i32::from(s.screen.0), i32::from(s.screen.1));
        if sw < 3 || sh < 4 {
            return None;
        }
        let mut slot = s.layout.borrow_mut();
        let layout = refresh(&mut slot, &s.canvas, s.rev);
        let (vw, vh) = s.view.span(s.screen);
        let (x0, y0, x1, y1) = layout.bounds.iter().fold(
            (s.view.ox, s.view.oy, s.view.ox + vw, s.view.oy + vh),
            |a, &(x, y, w, h)| (a.0.min(x), a.1.min(y), a.2.max(x + w), a.3.max(y + h)),
        );
        Some(Map {
            rect: (0, 0, sw, sh - 1),
            area: (x0, y0, x1 - x0, y1 - y0),
        })
    }

    fn inner(&self) -> (i32, i32, i32, i32) {
        let (x, y, w, h) = self.rect;
        (x + 1, y + 1, w - 2, h - 2)
    }

    fn cell(&self, wx: i32, wy: i32) -> (i32, i32) {
        let (ix, iy, iw, ih) = self.inner();
        let (ax, ay, aw, ah) = self.area;
        (
            ix + (i64::from(wx - ax) * i64::from(iw) / i64::from(aw)).clamp(0, i64::from(iw - 1))
                as i32,
            iy + (i64::from(wy - ay) * i64::from(ih) / i64::from(ah)).clamp(0, i64::from(ih - 1))
                as i32,
        )
    }

    /// The world cell under a screen cell inside the box.
    fn world(&self, col: i32, row: i32) -> (i32, i32) {
        let (ix, iy, iw, ih) = self.inner();
        let (ax, ay, aw, ah) = self.area;
        (
            ax + ((col - ix).clamp(0, iw - 1) * aw + aw / 2) / iw,
            ay + ((row - iy).clamp(0, ih - 1) * ah + ah / 2) / ih,
        )
    }

    fn inside(&self, col: i32, row: i32) -> bool {
        let (x, y, w, h) = self.rect;
        col >= x && col < x + w && row >= y && row < y + h
    }

    /// Shapes and texts in z order. Lines are skipped: their bounds boxes hide the shapes they join.
    fn items(&self, s: &Session) -> Vec<Item> {
        let mut slot = s.layout.borrow_mut();
        let layout = refresh(&mut slot, &s.canvas, s.rev);
        s.canvas
            .elements
            .iter()
            .zip(&layout.bounds)
            .filter_map(|(e, &(x, y, w, h))| {
                let (frame, label) = match e {
                    Element::Shape(sh) => (sh.kind == ShapeKind::Frame, &sh.label),
                    Element::Text(t) => (false, &t.text),
                    Element::Line(_) => return None,
                };
                let (a, b) = (self.cell(x, y), self.cell(x + w - 1, y + h - 1));
                Some(Item {
                    frame,
                    bounds: (x, y, w, h),
                    cells: (a.0, a.1, b.0, b.1),
                    label: label.lines().next().unwrap_or_default().to_string(),
                })
            })
            .collect()
    }
}

fn cut(label: &str, n: i32) -> String {
    label.chars().take(n.max(0) as usize).collect()
}

pub fn draw(grid: &mut Grid, s: &Session) {
    let Some(m) = Map::new(s) else {
        return;
    };
    super::boxed(grid, m.rect, "Minimap");
    let items = m.items(s);
    for it in items.iter().filter(|i| i.frame) {
        let (x0, y0, x1, y1) = it.cells;
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                let ch = match (cx == x0, cx == x1, cy == y0, cy == y1) {
                    (true, _, true, _) => '┌',
                    (_, true, true, _) => '┐',
                    (true, _, _, true) => '└',
                    (_, true, _, true) => '┘',
                    (_, _, true, _) | (_, _, _, true) => '─',
                    (true, ..) | (_, true, ..) => '│',
                    _ => continue,
                };
                grid.set(cx, cy, ch, 0);
            }
        }
    }
    for it in items.iter().filter(|i| !i.frame) {
        let (x0, y0, x1, y1) = it.cells;
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                grid.set(cx, cy, '▒', 0);
            }
        }
    }
    for it in items.iter().filter(|i| i.frame) {
        let (x0, y0, x1, _) = it.cells;
        grid.text(x0 + 1, y0, &cut(&it.label, x1 - x0 - 1), 0);
    }
    for it in items.iter().filter(|i| !i.frame) {
        let (x0, y0, x1, y1) = it.cells;
        if x1 - x0 >= 2 {
            grid.text(x0, (y0 + y1) / 2, &cut(&it.label, x1 - x0 + 1), REVERSE);
        }
    }
    let (vw, vh) = s.view.span(s.screen);
    let ((x0, y0), (x1, y1)) = (
        m.cell(s.view.ox, s.view.oy),
        m.cell(s.view.ox + vw - 1, s.view.oy + vh - 1),
    );
    for cy in y0..=y1 {
        for cx in x0..=x1 {
            if cx == x0 || cx == x1 || cy == y0 || cy == y1 {
                let ch = grid.get(cx, cy).map_or(' ', |c| c.ch);
                grid.set(cx, cy, ch, REVERSE);
            }
        }
    }
}

/// Fits the view to the element under the click, else centers it on the clicked spot.
pub fn click(s: &mut Session, col: i32, row: i32) {
    let Some(m) = Map::new(s) else {
        return;
    };
    if !m.inside(col, row) {
        return;
    }
    let hit = m
        .items(s)
        .into_iter()
        .rev()
        .filter(|i| {
            let (x0, y0, x1, y1) = i.cells;
            (x0..=x1).contains(&col) && (y0..=y1).contains(&row)
        })
        .min_by_key(|i| i.frame);
    match hit {
        Some(i) => s.fit(Some(i.bounds)),
        None => {
            let (wx, wy) = m.world(col, row);
            let (sw, sh) = s.view.span(s.screen);
            s.go_to((wx - sw / 2, wy - sh / 2));
        }
    }
}
