use super::grid::{Grid, REVERSE};
use crate::input::{Session, Tool};
use crate::model::{Element, LineKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dock {
    Left,
    Top,
    Right,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteHit {
    Title,
    Tool(Tool),
    Option(Choice),
}

/// One entry of the contextual submenu for the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Head(LineKind),
}

impl Choice {
    fn label(self) -> &'static str {
        match self {
            Choice::Head(LineKind::Line) => "-- none",
            Choice::Head(LineKind::Arrow) => "-> end",
            Choice::Head(LineKind::ArrowStart) => "<- start",
            Choice::Head(LineKind::ArrowBoth) => "<-> both",
        }
    }
}

/// Choices that apply to the selection; empty when the selection has none.
pub fn choices(s: &Session) -> Vec<Choice> {
    let has_line = s
        .selection
        .iter()
        .any(|id| matches!(s.canvas.get(id), Some(Element::Line(_))));
    if has_line {
        [
            LineKind::Line,
            LineKind::Arrow,
            LineKind::ArrowStart,
            LineKind::ArrowBoth,
        ]
        .map(Choice::Head)
        .to_vec()
    } else {
        vec![]
    }
}

fn active(s: &Session, c: Choice) -> bool {
    let Choice::Head(kind) = c;
    s.selection
        .iter()
        .filter_map(|id| match s.canvas.get(id) {
            Some(Element::Line(l)) => Some(l.kind),
            _ => None,
        })
        .all(|k| k == kind)
}

const COLUMN: i32 = 9;
const TITLE: &str = "::";

/// Every clickable cell run as (hit, x, y, width). One row high.
pub fn cells(dock: Dock, screen: (u16, u16), opts: &[Choice]) -> Vec<(PaletteHit, i32, i32, i32)> {
    let (sw, sh) = (i32::from(screen.0), i32::from(screen.1));
    let mut out = vec![];
    match dock {
        Dock::Left | Dock::Right => {
            let x = if dock == Dock::Left {
                0
            } else {
                (sw - COLUMN).max(0)
            };
            out.push((PaletteHit::Title, x, 0, COLUMN));
            for (i, t) in Tool::ALL.iter().enumerate() {
                out.push((PaletteHit::Tool(*t), x, i as i32 + 1, COLUMN));
            }
            for (i, c) in opts.iter().enumerate() {
                let row = Tool::ALL.len() as i32 + 2 + i as i32;
                out.push((PaletteHit::Option(*c), x, row, COLUMN));
            }
        }
        Dock::Top | Dock::Bottom => {
            let y = if dock == Dock::Top {
                0
            } else {
                (sh - 1).max(0)
            };
            out.push((PaletteHit::Title, 0, y, 3));
            let mut x = 3;
            for t in Tool::ALL {
                let w = t.label().len() as i32 + 2;
                out.push((PaletteHit::Tool(t), x, y, w));
                x += w;
            }
            x += 1;
            for c in opts {
                let w = c.label().len() as i32 + 2;
                out.push((PaletteHit::Option(*c), x, y, w));
                x += w;
            }
        }
    }
    out
}

pub fn hit(s: &Session, col: i32, row: i32) -> Option<PaletteHit> {
    cells(s.dock, s.screen, &choices(s))
        .into_iter()
        .find(|&(_, x, y, w)| row == y && col >= x && col < x + w)
        .map(|c| c.0)
}

pub fn nearest_dock(col: i32, row: i32, screen: (u16, u16)) -> Dock {
    let (sw, sh) = (i32::from(screen.0), i32::from(screen.1));
    let edges = [
        (col, Dock::Left),
        (row, Dock::Top),
        (sw - 1 - col, Dock::Right),
        (sh - 1 - row, Dock::Bottom),
    ];
    edges
        .into_iter()
        .min_by_key(|e| e.0)
        .map_or(Dock::Left, |e| e.1)
}

pub fn draw(grid: &mut Grid, s: &Session) {
    for (h, x, y, w) in cells(s.dock, s.screen, &choices(s)) {
        let (text, flags) = match h {
            PaletteHit::Title => (TITLE.to_string(), REVERSE),
            PaletteHit::Tool(t) => (t.label().to_string(), if t == s.tool { REVERSE } else { 0 }),
            PaletteHit::Option(c) => (
                c.label().to_string(),
                if active(s, c) { REVERSE } else { 0 },
            ),
        };
        let pad = " ".repeat((w as usize).saturating_sub(text.len() + 1));
        grid.text(x, y, &format!(" {text}{pad}"), flags);
    }
}
