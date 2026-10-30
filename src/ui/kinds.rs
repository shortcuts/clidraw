use super::canvas_view::shape;
use super::list::{Click, Outcome};
use super::style::ACTIVE;
use super::{boxed, centered, Grid};
use crate::model::{Shape, ShapeKind};
use crossterm::event::{KeyCode, KeyEvent};

const CELL_W: i32 = 25;
const CELL_H: i32 = 3;

const ALL: [(ShapeKind, &str); 8] = [
    (ShapeKind::Rect, "Rect"),
    (ShapeKind::Diamond, "Diamond"),
    (ShapeKind::Cylinder, "Cylinder"),
    (ShapeKind::Parallelogram, "Parallelogram"),
    (ShapeKind::Hexagon, "Hexagon"),
    (ShapeKind::Stadium, "Stadium"),
    (ShapeKind::Subroutine, "Subroutine"),
    (ShapeKind::Frame, "Frame"),
];

/// Floating grid of drawn shape glyphs with their names; the current kind carries a check mark.
#[derive(Clone, Debug)]
pub struct KindPicker {
    current: ShapeKind,
    cols: usize,
    focus: usize,
}

impl KindPicker {
    pub fn new(current: ShapeKind, screen: (u16, u16)) -> KindPicker {
        KindPicker {
            current,
            cols: usize::from(screen.0.saturating_sub(4) / CELL_W as u16).clamp(1, 2),
            focus: ALL.iter().position(|(k, _)| *k == current).unwrap_or(0),
        }
    }

    pub fn selected(&self) -> ShapeKind {
        ALL[self.focus].0
    }

    fn visible(&self, screen: (u16, u16)) -> usize {
        let fit = (usize::from(screen.1).saturating_sub(2) / CELL_H as usize).max(1);
        ALL.len().div_ceil(self.cols).min(fit)
    }

    fn first_row(&self, visible: usize) -> usize {
        (self.focus / self.cols).saturating_sub(visible - 1)
    }

    fn rect(&self, screen: (u16, u16)) -> (i32, i32, i32, i32) {
        let rows = self.visible(screen) as i32;
        centered(screen, self.cols as i32 * CELL_W + 4, rows * CELL_H + 2)
    }

    pub fn draw(&self, grid: &mut Grid) {
        let screen = (grid.w as u16, grid.h as u16);
        let r = self.rect(screen);
        boxed(grid, r, "Change shape");
        let visible = self.visible(screen);
        let first = self.first_row(visible);
        for (i, &(kind, name)) in ALL.iter().enumerate() {
            let row = i / self.cols;
            if row < first || row >= first + visible {
                continue;
            }
            let cx = r.0 + 2 + (i % self.cols) as i32 * CELL_W;
            let cy = r.1 + 1 + (row - first) as i32 * CELL_H;
            let flags = if i == self.focus { ACTIVE } else { 0 };
            if flags != 0 {
                for dy in 0..CELL_H {
                    grid.text(cx, cy + dy, &" ".repeat(CELL_W as usize - 1), flags);
                }
            }
            let w = Shape::ratio_width(kind, CELL_H).unwrap_or(6);
            let glyph = Shape::new(kind, cx + 1, cy, w, CELL_H, "");
            shape(grid, &glyph, glyph.x, glyph.y, "", None, flags);
            let mark = if kind == self.current { "✓ " } else { "  " };
            grid.text(cx + 9, cy + 1, &format!("{mark}{name}"), flags);
        }
    }

    pub fn key(&mut self, k: KeyEvent) -> Outcome<ShapeKind> {
        let (last, cols) = (ALL.len() - 1, self.cols);
        match k.code {
            KeyCode::Esc => return Outcome::Close,
            KeyCode::Enter => return Outcome::Run(self.selected()),
            KeyCode::Left | KeyCode::Char('h') => self.focus = self.focus.saturating_sub(1),
            KeyCode::Right | KeyCode::Char('l') => self.focus = (self.focus + 1).min(last),
            KeyCode::Up | KeyCode::Char('k') if self.focus >= cols => self.focus -= cols,
            KeyCode::Down | KeyCode::Char('j') if self.focus + cols <= last => self.focus += cols,
            _ => {}
        }
        Outcome::Idle
    }

    pub fn click(&self, screen: (u16, u16), col: i32, row: i32) -> Click<ShapeKind> {
        let (x, y, w, h) = self.rect(screen);
        if col < x || col >= x + w || row < y || row >= y + h {
            return Click::Outside;
        }
        let (dx, dy) = (col - (x + 2), row - (y + 1));
        let visible = self.visible(screen);
        let hit = (dx >= 0 && dy >= 0 && dx < self.cols as i32 * CELL_W)
            .then(|| {
                (self.first_row(visible) + (dy / CELL_H) as usize) * self.cols
                    + (dx / CELL_W) as usize
            })
            .and_then(|i| ALL.get(i));
        hit.map_or(Click::Inside, |&(kind, _)| Click::Row(kind, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn press(p: &mut KindPicker, keys: &[KeyCode]) -> Outcome<ShapeKind> {
        keys.iter().fold(Outcome::Idle, |_, &c| {
            p.key(KeyEvent::new(c, KeyModifiers::NONE))
        })
    }

    fn picked(o: Outcome<ShapeKind>) -> Option<ShapeKind> {
        match o {
            Outcome::Run(k) => Some(k),
            _ => None,
        }
    }

    #[test]
    fn arrows_and_hjkl_move_and_clamp() {
        let mut p = KindPicker::new(ShapeKind::Rect, (80, 24));
        let o = press(&mut p, &[KeyCode::Left, KeyCode::Up, KeyCode::Enter]);
        assert_eq!(picked(o), Some(ShapeKind::Rect));
        let o = press(&mut p, &[KeyCode::Char('l'), KeyCode::Enter]);
        assert_eq!(picked(o), Some(ShapeKind::Diamond));
        let o = press(&mut p, &[KeyCode::Down, KeyCode::Enter]);
        assert_eq!(picked(o), Some(ShapeKind::Parallelogram));
        let o = press(
            &mut p,
            &[KeyCode::Char('k'), KeyCode::Char('h'), KeyCode::Enter],
        );
        assert_eq!(picked(o), Some(ShapeKind::Rect));
        let o = press(
            &mut p,
            &[KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down],
        );
        assert!(matches!(o, Outcome::Idle));
        let o = press(&mut p, &[KeyCode::Right, KeyCode::Right, KeyCode::Enter]);
        assert_eq!(picked(o), Some(ShapeKind::Frame));
        assert!(matches!(press(&mut p, &[KeyCode::Esc]), Outcome::Close));
    }

    #[test]
    fn the_current_kind_starts_focused() {
        let mut p = KindPicker::new(ShapeKind::Hexagon, (80, 24));
        assert_eq!(
            picked(press(&mut p, &[KeyCode::Enter])),
            Some(ShapeKind::Hexagon)
        );
    }

    #[test]
    fn narrow_screens_use_one_column() {
        assert_eq!(KindPicker::new(ShapeKind::Rect, (80, 24)).cols, 2);
        assert_eq!(KindPicker::new(ShapeKind::Rect, (40, 24)).cols, 1);
    }

    #[test]
    fn clicks_hit_cells_inside_or_outside() {
        let p = KindPicker::new(ShapeKind::Rect, (80, 24));
        let (x, y, ..) = p.rect((80, 24));
        let at = |dx, dy| p.click((80, 24), x + 2 + dx, y + 1 + dy);
        assert!(matches!(at(0, 0), Click::Row(ShapeKind::Rect, 0)));
        assert!(matches!(
            at(CELL_W + 24, 2),
            Click::Row(ShapeKind::Diamond, 0)
        ));
        assert!(matches!(at(0, 3), Click::Row(ShapeKind::Cylinder, 0)));
        assert!(matches!(p.click((80, 24), 0, 0), Click::Outside));
        assert!(matches!(at(CELL_W * 2, 0), Click::Inside));
    }

    #[test]
    fn scrolling_keeps_the_focused_cell_visible_and_clickable() {
        let mut p = KindPicker::new(ShapeKind::Rect, (40, 24));
        press(&mut p, &[KeyCode::Char('j'); 7]);
        let (x, y, w, h) = p.rect((40, 24));
        assert!(y >= 0 && y + h <= 24 && x + w <= 40);
        let last = y + h - 2;
        assert!(matches!(
            p.click((40, 24), x + 2, last),
            Click::Row(ShapeKind::Frame, 0)
        ));
    }
}
