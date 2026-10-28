use super::list::{List, Row};
use super::Grid;

pub struct Notes {
    pub paths: Vec<String>,
    pub list: List<NoteHit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteHit {
    New,
    Open(usize),
}

impl Notes {
    pub fn new(paths: Vec<String>) -> Notes {
        let new = Row {
            value: NoteHit::New,
            label: "+ New note".to_string(),
            key: String::new(),
        };
        let opens = paths.iter().enumerate().map(|(i, p)| Row {
            value: NoteHit::Open(i),
            label: p.clone(),
            key: String::new(),
        });
        let rows: Vec<_> = std::iter::once(new).chain(opens).collect();
        Notes {
            paths,
            list: List::new("Obsidian notes", rows).with_query().min_width(44),
        }
    }

    pub fn draw(&self, grid: &mut Grid) {
        self.list.draw(grid);
    }
}
