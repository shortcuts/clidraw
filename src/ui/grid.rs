use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthChar;

pub const REVERSE: u8 = 1;
pub const BOLD: u8 = 2;
pub const DIM: u8 = 4;

#[derive(Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub flags: u8,
}

/// Our own cell grid: integer cells, no floating-point dots.
pub struct Grid {
    pub w: i32,
    pub h: i32,
    cells: Vec<Cell>,
}

impl Grid {
    pub fn new(w: u16, h: u16) -> Grid {
        let blank = Cell { ch: ' ', flags: 0 };
        Grid {
            w: i32::from(w),
            h: i32::from(h),
            cells: vec![blank; usize::from(w) * usize::from(h)],
        }
    }

    pub fn set(&mut self, x: i32, y: i32, ch: char, flags: u8) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.cells[(y * self.w + x) as usize] = Cell { ch, flags };
        }
    }

    pub fn get(&self, x: i32, y: i32) -> Option<Cell> {
        (x >= 0 && y >= 0 && x < self.w && y < self.h)
            .then(|| self.cells[(y * self.w + x) as usize])
    }

    /// Draws text; a double-width char claims the next cell with a NUL placeholder.
    pub fn text(&mut self, x: i32, y: i32, s: &str, flags: u8) {
        let mut cx = x;
        for ch in s.chars() {
            let w = ch.width().unwrap_or(0) as i32;
            if w == 0 {
                continue;
            }
            self.set(cx, y, ch, flags);
            if w == 2 {
                self.set(cx + 1, y, '\0', flags);
            }
            cx += w;
        }
    }

    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, ' ', 0);
            }
        }
    }

    pub fn dim_all(&mut self) {
        for c in &mut self.cells {
            c.flags |= DIM;
        }
    }

    pub fn copy_to(&self, buf: &mut Buffer, area: Rect) {
        for y in 0..self.h.min(i32::from(area.height)) {
            for x in 0..self.w.min(i32::from(area.width)) {
                let c = self.cells[(y * self.w + x) as usize];
                if c.ch == '\0' {
                    continue;
                }
                let mut style = Style::default();
                for (flag, m) in [
                    (REVERSE, Modifier::REVERSED),
                    (BOLD, Modifier::BOLD),
                    (DIM, Modifier::DIM),
                ] {
                    if c.flags & flag != 0 {
                        style = style.add_modifier(m);
                    }
                }
                let mut tmp = [0u8; 4];
                buf[(area.x + x as u16, area.y + y as u16)]
                    .set_symbol(c.ch.encode_utf8(&mut tmp))
                    .set_style(style);
            }
        }
    }
}
