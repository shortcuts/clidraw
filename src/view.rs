/// Integer zoom steps: below `One` a cell shrinks to a block, above it a cell stretches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Zoom {
    Quarter,
    Half,
    #[default]
    One,
    Two,
    Three,
}

impl Zoom {
    const ALL: [Zoom; 5] = [Zoom::Quarter, Zoom::Half, Zoom::One, Zoom::Two, Zoom::Three];

    /// World cells per screen cell when zoomed out.
    fn d(self) -> i32 {
        match self {
            Zoom::Quarter => 4,
            Zoom::Half => 2,
            _ => 1,
        }
    }

    /// Screen cells per world cell when zoomed in.
    fn k(self) -> i32 {
        match self {
            Zoom::Two => 2,
            Zoom::Three => 3,
            _ => 1,
        }
    }

    /// The step `n` places along the scale, clamped at both ends.
    pub fn step(self, n: i32) -> Zoom {
        let i = Zoom::ALL.iter().position(|&z| z == self).unwrap_or(2) as i32;
        Zoom::ALL[(i + n).clamp(0, 4) as usize]
    }

    /// Info bar text; none at 1x.
    pub fn label(self) -> Option<&'static str> {
        match self {
            Zoom::Quarter => Some("25%"),
            Zoom::Half => Some("50%"),
            Zoom::One => None,
            Zoom::Two => Some("200%"),
            Zoom::Three => Some("300%"),
        }
    }
}

/// `ox, oy` is the world cell under screen cell 0,0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub ox: i32,
    pub oy: i32,
    pub zoom: Zoom,
}

impl View {
    pub fn to_screen(&self, wx: i32, wy: i32) -> (i32, i32) {
        let (k, d) = (self.zoom.k(), self.zoom.d());
        (
            ((wx - self.ox) * k).div_euclid(d),
            ((wy - self.oy) * k).div_euclid(d),
        )
    }

    pub fn to_world(&self, sx: i32, sy: i32) -> (i32, i32) {
        let (k, d) = (self.zoom.k(), self.zoom.d());
        (
            self.ox + (sx * d).div_euclid(k),
            self.oy + (sy * d).div_euclid(k),
        )
    }

    /// The last screen cell a world cell covers; differs from `to_screen` only when zoomed in.
    pub fn cell_end(&self, wx: i32, wy: i32) -> (i32, i32) {
        let (sx, sy) = self.to_screen(wx, wy);
        (sx + self.zoom.k() - 1, sy + self.zoom.k() - 1)
    }

    /// The screen rect (x, y, w, h) a world rect covers.
    pub fn rect(&self, x: i32, y: i32, w: i32, h: i32) -> (i32, i32, i32, i32) {
        let (x0, y0) = self.to_screen(x, y);
        let (x1, y1) = self.cell_end(x + w - 1, y + h - 1);
        (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
    }

    /// World cells a screen of this size shows.
    pub fn span(&self, screen: (u16, u16)) -> (i32, i32) {
        let (k, d) = (self.zoom.k(), self.zoom.d());
        let n = |s: u16| (i32::from(s) * d + k - 1) / k;
        (n(screen.0), n(screen.1))
    }

    /// A world route as screen cells, with the gaps a zoom-in opens between neighbours filled.
    pub fn path(&self, route: &[(i32, i32)]) -> Vec<(i32, i32)> {
        let k = self.zoom.k();
        let mut out: Vec<(i32, i32)> = Vec::with_capacity(route.len() * k as usize);
        for &c in route {
            let (sx, sy) = self.to_screen(c.0, c.1);
            if let Some(&(px, py)) = out.last() {
                let (dx, dy) = ((sx - px).signum(), (sy - py).signum());
                out.extend((1..k).map(|n| (px + dx * n, py + dy * n)));
            }
            out.push((sx, sy));
        }
        out
    }

    /// World cells that `n` screen cells of pan cover; never zero for a non-zero `n`.
    pub fn world_step(&self, n: i32) -> i32 {
        let (k, d) = (self.zoom.k(), self.zoom.d());
        if k > 1 {
            n.signum() * (n.abs() / k).max(1)
        } else {
            n * d
        }
    }

    pub fn pan(&mut self, dx: i32, dy: i32) {
        self.ox += dx;
        self.oy += dy;
    }

    /// Pans by whole world cells for a drag of `dx, dy` screen cells; returns the screen cells it used.
    pub fn pan_screen(&mut self, dx: i32, dy: i32) -> (i32, i32) {
        let (k, d) = (self.zoom.k(), self.zoom.d());
        let (wx, wy) = if k > 1 {
            (dx / k, dy / k)
        } else {
            (dx * d, dy * d)
        };
        self.pan(wx, wy);
        (wx * k / d, wy * k / d)
    }

    /// Switches zoom and keeps the world cell under screen cell `col, row` in place.
    pub fn zoom_at(&mut self, zoom: Zoom, col: i32, row: i32) {
        let p = self.to_world(col, row);
        self.zoom = zoom;
        self.ox = 0;
        self.oy = 0;
        let q = self.to_world(col, row);
        self.ox = p.0 - q.0;
        self.oy = p.1 - q.1;
    }

    /// Picks the largest zoom up to 1x that shows the box (x, y, w, h) and centers it.
    pub fn fit_all(&mut self, bounds: Option<(i32, i32, i32, i32)>, screen: (u16, u16)) {
        let Some((x, y, w, h)) = bounds else {
            *self = View::default();
            return;
        };
        // The last row belongs to the info bar.
        let rows = i32::from(screen.1) - 1;
        self.zoom = [Zoom::One, Zoom::Half]
            .into_iter()
            .find(|z| w <= i32::from(screen.0) * z.d() && h <= rows * z.d())
            .unwrap_or(Zoom::Quarter);
        let (sw, sh) = self.span(screen);
        self.ox = x + w / 2 - sw / 2;
        self.oy = y + h / 2 - sh / 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_negative_coordinates() {
        let v = View {
            ox: -30,
            oy: 12,
            ..View::default()
        };
        let (sx, sy) = v.to_screen(-35, -4);
        assert_eq!((sx, sy), (-5, -16));
        assert_eq!(v.to_world(sx, sy), (-35, -4));
    }

    #[test]
    fn fit_all_centers_the_box() {
        let mut v = View::default();
        v.fit_all(Some((100, 50, 10, 4)), (80, 24));
        assert_eq!(v.to_screen(105, 52), (40, 12));
    }

    fn at(zoom: Zoom) -> View {
        View {
            ox: -7,
            oy: 5,
            zoom,
        }
    }

    #[test]
    fn round_trips_at_every_zoom() {
        for z in Zoom::ALL {
            let v = at(z);
            for (wx, wy) in [(-35, -4), (0, 0), (13, 29)] {
                let (sx, sy) = v.to_screen(wx, wy);
                // Zoomed out, a screen cell stands for a block that starts at the world cell.
                let (bx, by) = v.to_world(sx, sy);
                assert!(bx <= wx && wx < bx + z.d(), "{z:?} {wx}");
                assert!(by <= wy && wy < by + z.d(), "{z:?} {wy}");
                let (ex, ey) = v.cell_end(wx, wy);
                assert_eq!(v.to_world(ex, ey), v.to_world(sx, sy), "{z:?}");
            }
        }
    }

    #[test]
    fn rect_stretches_by_k_and_shrinks_by_d() {
        let v = View {
            zoom: Zoom::Three,
            ..View::default()
        };
        assert_eq!(v.rect(2, 1, 40, 10), (6, 3, 120, 30));
        let v = View {
            zoom: Zoom::Half,
            ..View::default()
        };
        assert_eq!(v.rect(2, 2, 40, 10), (1, 1, 20, 5));
    }

    #[test]
    fn path_fills_the_gaps_between_cells() {
        let v = View {
            zoom: Zoom::Three,
            ..View::default()
        };
        assert_eq!(
            v.path(&[(0, 0), (1, 0), (2, 1)]),
            vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 1), (5, 2), (6, 3)]
        );
        assert_eq!(
            View::default().path(&[(0, 0), (1, 0)]),
            vec![(0, 0), (1, 0)]
        );
    }

    #[test]
    fn zoom_at_keeps_the_anchor_cell() {
        for from in Zoom::ALL {
            for to in Zoom::ALL {
                let mut v = at(from);
                let before = v.to_world(31, 9);
                v.zoom_at(to, 31, 9);
                assert_eq!(v.to_world(31, 9), before, "{from:?} {to:?}");
            }
        }
    }

    #[test]
    fn zoom_in_at_the_cursor_keeps_its_world_cell() {
        let mut v = View::default();
        v.zoom_at(Zoom::Two, 10, 5);
        assert_eq!(v.zoom, Zoom::Two);
        assert_eq!(v.to_world(10, 5), (10, 5));
    }

    #[test]
    fn pan_screen_carries_the_remainder() {
        let mut v = View {
            zoom: Zoom::Three,
            ..View::default()
        };
        assert_eq!(v.pan_screen(5, -4), (3, -3));
        assert_eq!((v.ox, v.oy), (1, -1));
        let mut v = View {
            zoom: Zoom::Half,
            ..View::default()
        };
        assert_eq!(v.pan_screen(5, 1), (5, 1));
        assert_eq!((v.ox, v.oy), (10, 2));
    }

    #[test]
    fn world_step_never_rounds_to_zero() {
        let mut v = View {
            zoom: Zoom::Three,
            ..View::default()
        };
        assert_eq!(
            (v.world_step(2), v.world_step(-2), v.world_step(0)),
            (1, -1, 0)
        );
        v.zoom = Zoom::Quarter;
        assert_eq!(v.world_step(2), 8);
    }

    #[test]
    fn span_counts_the_world_cells_on_screen() {
        let v = |zoom| {
            View {
                zoom,
                ..View::default()
            }
            .span((80, 24))
        };
        assert_eq!(v(Zoom::One), (80, 24));
        assert_eq!(v(Zoom::Quarter), (320, 96));
        assert_eq!(v(Zoom::Two), (40, 12));
        assert_eq!(v(Zoom::Three), (27, 8));
    }

    #[test]
    fn fit_all_picks_half_for_a_box_wider_than_the_screen() {
        let mut v = View::default();
        v.fit_all(Some((0, 0, 120, 10)), (80, 24));
        assert_eq!(v.zoom, Zoom::Half);
        assert_eq!(v.to_screen(60, 5), (40, 12));
        v.fit_all(Some((0, 0, 400, 10)), (80, 24));
        assert_eq!(v.zoom, Zoom::Quarter);
        v.fit_all(Some((0, 0, 10, 10)), (80, 24));
        assert_eq!(v.zoom, Zoom::One);
    }

    #[test]
    fn step_clamps_at_both_ends() {
        assert_eq!(Zoom::Three.step(1), Zoom::Three);
        assert_eq!(Zoom::Quarter.step(-1), Zoom::Quarter);
        assert_eq!(Zoom::One.step(1), Zoom::Two);
    }
}
