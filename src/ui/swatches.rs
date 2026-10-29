use super::infobar::fit;
use super::list::{Click, Outcome};
use super::style::{ACTIVE, MUTED};
use super::theme::{self, Theme};
use super::{boxed, centered, Grid};
use crate::model::Paint;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;

const CELL: i32 = 8;

/// Floating grid of color squares for ink and fill; the cell `None` clears the color.
#[derive(Clone, Debug)]
pub struct Swatches {
    title: &'static str,
    cells: Vec<Option<Paint>>,
    current: Option<Option<Paint>>,
    cols: usize,
    focus: usize,
    hex: Option<String>,
}

fn label(p: Option<Paint>) -> String {
    p.map_or("none".into(), Paint::label)
}

fn mark_fg(c: Color) -> Color {
    match c {
        Color::Rgb(r, g, b)
            if 299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b) > 128_000 =>
        {
            Color::Black
        }
        _ => Color::White,
    }
}

impl Swatches {
    /// `current` is the color every selected element shares; `None` when they differ.
    pub fn new(title: &'static str, current: Option<Option<Paint>>, screen_w: u16) -> Swatches {
        let mut cells: Vec<Option<Paint>> =
            std::iter::once(None).chain(Paint::ALL.map(Some)).collect();
        if let Some(c @ Some(Paint::Rgb(..))) = current {
            cells.push(c);
        }
        let focus = cells.iter().position(|&c| Some(c) == current).unwrap_or(0);
        Swatches {
            title,
            cells,
            current,
            cols: usize::from(screen_w.saturating_sub(4) / CELL as u16).clamp(1, 4),
            focus,
            hex: None,
        }
    }

    fn rows(&self) -> usize {
        self.cells.len().div_ceil(self.cols)
    }

    fn rect(&self, screen: (u16, u16)) -> (i32, i32, i32, i32) {
        centered(screen, self.cols as i32 * CELL + 4, self.rows() as i32 + 3)
    }

    pub fn draw(&self, grid: &mut Grid, theme: Theme) {
        let r = self.rect((grid.w as u16, grid.h as u16));
        boxed(grid, r, self.title);
        let color = theme::colors(grid.depth);
        for (i, &cell) in self.cells.iter().enumerate() {
            let cx = r.0 + 2 + (i % self.cols) as i32 * CELL;
            let cy = r.1 + 1 + (i / self.cols) as i32;
            let (on, mine) = (i == self.focus, Some(cell) == self.current);
            if !color {
                let name = label(cell);
                let pad = (CELL as usize - name.len()) / 2;
                let text = format!("{:pad$}{name:<w$}", "", w = CELL as usize - pad);
                grid.text(cx, cy, &text, if on { ACTIVE } else { 0 });
                if mine {
                    grid.text(cx, cy, "*", if on { ACTIVE } else { 0 });
                }
                continue;
            }
            if on {
                grid.text(cx, cy, "[", ACTIVE);
                grid.text(cx + CELL - 1, cy, "]", ACTIVE);
            }
            match cell {
                None if mine => grid.text(cx + 1, cy, "✓ none", MUTED),
                None => grid.text(cx + 2, cy, "none", MUTED),
                Some(p) => {
                    let ink = theme::ink(theme, p);
                    if mine {
                        grid.text(cx + 3, cy, "✓", 0);
                        grid.tint(cx + 3, cy, 1, 1, mark_fg(ink));
                    }
                    grid.swatch(cx + 1, cy, CELL - 2, ink);
                }
            }
        }
        let line = match &self.hex {
            Some(h) if Paint::parse(h).is_some() => format!("hex: {h}_ Enter: use"),
            Some(h) => format!("hex: {h}_"),
            None if Some(self.cells[self.focus]) == self.current => {
                format!("{} (current)", label(self.cells[self.focus]))
            }
            None => label(self.cells[self.focus]),
        };
        grid.text(
            r.0 + 2,
            r.1 + 1 + self.rows() as i32,
            &fit(&line, (r.2 - 4).max(0)),
            0,
        );
    }

    pub fn key(&mut self, k: KeyEvent) -> Outcome<Option<Paint>> {
        let plain = !k
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        if let Some(h) = &mut self.hex {
            match k.code {
                KeyCode::Esc => self.hex = None,
                KeyCode::Backspace => {
                    h.pop();
                    if h.is_empty() {
                        self.hex = None;
                    }
                }
                KeyCode::Char(c) if plain && c.is_ascii_hexdigit() && h.len() < 7 => h.push(c),
                KeyCode::Enter => {
                    return Paint::parse(h).map_or(Outcome::Idle, |p| Outcome::Run(Some(p)))
                }
                _ => {}
            }
            return Outcome::Idle;
        }
        let (last, cols) = (self.cells.len() - 1, self.cols);
        match k.code {
            KeyCode::Esc => return Outcome::Close,
            KeyCode::Enter => return Outcome::Run(self.cells[self.focus]),
            KeyCode::Char('#') if plain => self.hex = Some("#".into()),
            KeyCode::Left | KeyCode::Char('h') => self.focus = self.focus.saturating_sub(1),
            KeyCode::Right | KeyCode::Char('l') => self.focus = (self.focus + 1).min(last),
            KeyCode::Up | KeyCode::Char('k') if self.focus >= cols => self.focus -= cols,
            KeyCode::Down | KeyCode::Char('j') if self.focus + cols <= last => self.focus += cols,
            _ => {}
        }
        Outcome::Idle
    }

    pub fn click(&self, screen: (u16, u16), col: i32, row: i32) -> Click<Option<Paint>> {
        let (x, y, w, h) = self.rect(screen);
        if col < x || col >= x + w || row < y || row >= y + h {
            return Click::Outside;
        }
        let (dx, dy) = (col - (x + 2), row - (y + 1));
        let cell = (dx >= 0 && dy >= 0 && dx < self.cols as i32 * CELL)
            .then(|| dy as usize * self.cols + (dx / CELL) as usize)
            .and_then(|i| self.cells.get(i));
        cell.map_or(Click::Inside, |&c| Click::Row(c, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn run(s: &mut Swatches, keys: &[KeyCode]) -> Outcome<Option<Paint>> {
        keys.iter().fold(Outcome::Idle, |_, &c| s.key(key(c)))
    }

    fn picked(o: Outcome<Option<Paint>>) -> Option<Option<Paint>> {
        match o {
            Outcome::Run(p) => Some(p),
            _ => None,
        }
    }

    #[test]
    fn arrows_and_hjkl_move_and_clamp() {
        let mut s = Swatches::new("Ink", None, 80);
        let o = run(&mut s, &[KeyCode::Left, KeyCode::Up, KeyCode::Enter]);
        assert_eq!(picked(o), Some(None));
        let o = run(
            &mut s,
            &[KeyCode::Char('l'), KeyCode::Char('l'), KeyCode::Enter],
        );
        assert_eq!(picked(o), Some(Some(Paint::Orange)));
        let o = run(&mut s, &[KeyCode::Down, KeyCode::Enter]);
        assert_eq!(picked(o), Some(Some(Paint::Violet)));
        let o = run(
            &mut s,
            &[
                KeyCode::Down,
                KeyCode::Right,
                KeyCode::Right,
                KeyCode::Enter,
            ],
        );
        assert_eq!(picked(o), Some(Some(Paint::Violet)));
        let o = run(
            &mut s,
            &[KeyCode::Char('k'), KeyCode::Char('h'), KeyCode::Enter],
        );
        assert_eq!(picked(o), Some(Some(Paint::Red)));
    }

    #[test]
    fn the_current_color_starts_focused_and_a_mix_starts_at_none() {
        let mut s = Swatches::new("Ink", Some(Some(Paint::Blue)), 80);
        assert_eq!(
            picked(run(&mut s, &[KeyCode::Enter])),
            Some(Some(Paint::Blue))
        );
        let mut s = Swatches::new("Ink", None, 80);
        assert_eq!(picked(run(&mut s, &[KeyCode::Enter])), Some(None));
        let hex = Some(Paint::Rgb(1, 2, 3));
        let mut s = Swatches::new("Ink", Some(hex), 80);
        assert_eq!(picked(run(&mut s, &[KeyCode::Enter])), Some(hex));
    }

    #[test]
    fn typed_hex_applies_only_when_complete_and_esc_clears_before_closing() {
        let mut s = Swatches::new("Ink", None, 80);
        let digits = "#00ff8".chars().map(KeyCode::Char).collect::<Vec<_>>();
        assert!(matches!(run(&mut s, &digits), Outcome::Idle));
        assert!(matches!(run(&mut s, &[KeyCode::Enter]), Outcome::Idle));
        let o = run(&mut s, &[KeyCode::Char('8'), KeyCode::Enter]);
        assert_eq!(picked(o), Some(Some(Paint::Rgb(0, 255, 136))));
        let mut s = Swatches::new("Ink", None, 80);
        assert!(matches!(
            run(&mut s, &[KeyCode::Char('#'), KeyCode::Esc]),
            Outcome::Idle
        ));
        assert!(matches!(run(&mut s, &[KeyCode::Esc]), Outcome::Close));
    }

    #[test]
    fn clicks_hit_cells_inside_or_outside() {
        let s = Swatches::new("Ink", None, 80);
        let (x, y, ..) = s.rect((80, 24));
        let cell = |dx, dy| s.click((80, 24), x + 2 + dx, y + 1 + dy);
        assert!(matches!(cell(0, 0), Click::Row(None, 0)));
        assert!(matches!(cell(9, 0), Click::Row(Some(Paint::Red), 0)));
        assert!(matches!(cell(8 * 2, 1), Click::Row(Some(Paint::Violet), 0)));
        assert!(matches!(
            cell(8 * 2 + 7, 1),
            Click::Row(Some(Paint::Violet), 0)
        ));
        assert!(matches!(cell(8 * 3, 1), Click::Inside));
        assert!(matches!(s.click((80, 24), 0, 0), Click::Outside));
    }

    #[test]
    fn narrow_screens_use_fewer_columns() {
        assert_eq!(Swatches::new("Ink", None, 80).cols, 4);
        assert_eq!(Swatches::new("Ink", None, 20).cols, 2);
        assert_eq!(Swatches::new("Ink", None, 2).cols, 1);
    }
}
