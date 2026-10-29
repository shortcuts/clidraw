use super::list::{List, Row};
use super::Grid;
use std::ops::Range;

pub struct Switcher {
    pub names: Vec<String>,
    pub list: List<SwitcherHit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitcherHit {
    New,
    Open(usize),
    Rename(usize),
    Delete(usize),
}

const KEYS: &str = "^r rename  ^x delete";
pub const RENAME: Range<i32> = 0..9;
pub const DELETE: Range<i32> = 11..20;

impl Switcher {
    pub fn new(names: Vec<String>, current: &str) -> Switcher {
        let new = Row {
            value: SwitcherHit::New,
            label: "+ New".to_string(),
            key: String::new(),
        };
        let opens = names.iter().enumerate().map(|(i, n)| Row {
            value: SwitcherHit::Open(i),
            label: if n == current {
                format!("{n} (current)")
            } else {
                n.clone()
            },
            key: KEYS.to_string(),
        });
        let rows: Vec<_> = std::iter::once(new).chain(opens).collect();
        let at = names.iter().position(|n| n == current).map_or(0, |i| i + 1);
        Switcher {
            names,
            list: List::new("Canvases", rows)
                .with_query()
                .min_width(44)
                .select(at),
        }
    }

    pub fn draw(&self, grid: &mut Grid) {
        self.list.draw(grid);
    }
}
