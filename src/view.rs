#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub ox: i32,
    pub oy: i32,
}

impl View {
    pub fn to_screen(&self, wx: i32, wy: i32) -> (i32, i32) {
        (wx - self.ox, wy - self.oy)
    }

    pub fn to_world(&self, sx: i32, sy: i32) -> (i32, i32) {
        (sx + self.ox, sy + self.oy)
    }

    pub fn pan(&mut self, dx: i32, dy: i32) {
        self.ox += dx;
        self.oy += dy;
    }

    /// Centers the box (x, y, w, h) on a screen of the given size.
    pub fn fit_all(&mut self, bounds: Option<(i32, i32, i32, i32)>, screen: (u16, u16)) {
        let Some((x, y, w, h)) = bounds else {
            *self = View::default();
            return;
        };
        self.ox = x + w / 2 - i32::from(screen.0) / 2;
        self.oy = y + h / 2 - i32::from(screen.1) / 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_negative_coordinates() {
        let v = View { ox: -30, oy: 12 };
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
}
