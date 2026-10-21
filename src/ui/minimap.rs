use super::grid::{Grid, REVERSE};
use super::layout::refresh;
use super::palette::Dock;
use crate::input::Session;

const W: i32 = 24;
const H: i32 = 8;

/// The corner box and the world area it maps onto.
pub struct Map {
    pub rect: (i32, i32, i32, i32),
    area: (i32, i32, i32, i32),
}

impl Map {
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
    pub fn world(&self, col: i32, row: i32) -> (i32, i32) {
        let (ix, iy, iw, ih) = self.inner();
        let (ax, ay, aw, ah) = self.area;
        (
            ax + ((col - ix).clamp(0, iw - 1) * aw + aw / 2) / iw,
            ay + ((row - iy).clamp(0, ih - 1) * ah + ah / 2) / ih,
        )
    }
}

/// Some while the canvas has content outside the viewport and the screen has room for the box.
pub fn map(s: &Session) -> Option<Map> {
    let (sw, sh) = (i32::from(s.screen.0), i32::from(s.screen.1));
    if s.overview || sw < 3 * W || sh < 3 * H {
        return None;
    }
    let mut slot = s.layout.borrow_mut();
    let layout = refresh(&mut slot, &s.canvas, s.rev);
    let (x0, y0, x1, y1) = layout
        .bounds
        .iter()
        .map(|&(x, y, w, h)| (x, y, x + w, y + h))
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))?;
    let (vx, vy) = (s.view.ox, s.view.oy);
    if x0 >= vx && y0 >= vy && x1 <= vx + sw && y1 <= vy + sh {
        return None;
    }
    let (ax, ay) = (x0.min(vx), y0.min(vy));
    let area = (ax, ay, x1.max(vx + sw) - ax, y1.max(vy + sh) - ay);
    let y = match s.dock {
        Dock::Left | Dock::Top => sh - 1 - H,
        Dock::Right | Dock::Bottom => 1,
    };
    let x = match s.dock {
        Dock::Left | Dock::Top => sw - W,
        Dock::Right | Dock::Bottom => 0,
    };
    Some(Map {
        rect: (x, y, W, H),
        area,
    })
}

pub fn hit(s: &Session, col: i32, row: i32) -> Option<Map> {
    map(s).filter(|m| {
        let (x, y, w, h) = m.rect;
        col >= x && col < x + w && row >= y && row < y + h
    })
}

pub fn draw(grid: &mut Grid, s: &Session) {
    let Some(m) = map(s) else {
        return;
    };
    super::boxed(grid, m.rect, "map");
    let mut slot = s.layout.borrow_mut();
    let layout = refresh(&mut slot, &s.canvas, s.rev);
    for &(x, y, w, h) in &layout.bounds {
        let ((x0, y0), (x1, y1)) = (m.cell(x, y), m.cell(x + w - 1, y + h - 1));
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                grid.set(cx, cy, '▒', 0);
            }
        }
    }
    let ((x0, y0), (x1, y1)) = (
        m.cell(s.view.ox, s.view.oy),
        m.cell(
            s.view.ox + i32::from(s.screen.0) - 1,
            s.view.oy + i32::from(s.screen.1) - 1,
        ),
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
