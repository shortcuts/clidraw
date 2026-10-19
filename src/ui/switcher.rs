use super::grid::REVERSE;
use super::{boxed, centered, Grid};

pub struct Switcher {
    pub names: Vec<String>,
    pub current: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitcherHit {
    New,
    Open(usize),
    Rename(usize),
    Delete(usize),
}

const WIDTH: i32 = 44;
const ACTIONS: &str = "[r] [x]";

fn rect(screen: (u16, u16), rows: usize) -> (i32, i32, i32, i32) {
    centered(screen, WIDTH, rows as i32 + 3)
}

pub fn hit(sw: &Switcher, screen: (u16, u16), col: i32, row: i32) -> Option<SwitcherHit> {
    let (x, y, w, h) = rect(screen, sw.names.len());
    if col <= x || col >= x + w - 1 || row <= y || row >= y + h - 1 {
        return None;
    }
    let r = (row - y - 1) as usize;
    if r == 0 {
        return Some(SwitcherHit::New);
    }
    let i = r - 1;
    if i >= sw.names.len() {
        return None;
    }
    let actions = x + w - 1 - ACTIONS.len() as i32;
    Some(match col - actions {
        0..=2 => SwitcherHit::Rename(i),
        4..=6 => SwitcherHit::Delete(i),
        _ => SwitcherHit::Open(i),
    })
}

pub fn draw(grid: &mut Grid, sw: &Switcher) {
    let screen = (grid.w as u16, grid.h as u16);
    let r = rect(screen, sw.names.len());
    boxed(grid, r, "Canvases");
    grid.text(r.0 + 2, r.1 + 1, "+ New", REVERSE);
    let name_room = (r.2 - 4 - ACTIONS.len() as i32 - 1).max(0) as usize;
    for (i, name) in sw.names.iter().enumerate() {
        let y = r.1 + 2 + i as i32;
        let shown: String = name.chars().take(name_room).collect();
        grid.text(
            r.0 + 2,
            y,
            &shown,
            if *name == sw.current { REVERSE } else { 0 },
        );
        grid.text(r.0 + r.2 - 1 - ACTIONS.len() as i32, y, ACTIONS, 0);
    }
}
