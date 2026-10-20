use super::grid::{Grid, REVERSE};
use crate::input::{Session, Tool};
use crate::model::{Border, Element, LineKind, Routing, Stroke};

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
    Stroke(Stroke),
    Routing(Routing),
    Border(Border),
}

impl Choice {
    fn label(self) -> &'static str {
        match self {
            Choice::Stroke(Stroke::Solid) => "__ solid",
            Choice::Stroke(Stroke::Dashed) => "-- dashed",
            Choice::Stroke(Stroke::Dotted) => ".. dotted",
            Choice::Border(Border::Single) => "┌┐ single",
            Choice::Border(Border::Rounded) => "╭╮ round",
            Choice::Border(Border::Double) => "╔╗ double",
            Choice::Border(Border::Heavy) => "┏┓ heavy",
            Choice::Routing(Routing::Elbow) => "|_ elbow",
            Choice::Routing(Routing::Straight) => "/ direct",
            Choice::Head(LineKind::Line) => "-- none",
            Choice::Head(LineKind::Arrow) => "-> end",
            Choice::Head(LineKind::ArrowStart) => "<- start",
            Choice::Head(LineKind::ArrowBoth) => "<-> both",
        }
    }
}

/// Choices that apply to the selection; empty when the selection has none.
pub fn choices(s: &Session) -> Vec<Choice> {
    let has =
        |f: fn(&Element) -> bool| s.selection.iter().any(|id| s.canvas.get(id).is_some_and(f));
    let mut out = vec![];
    if has(|e| matches!(e, Element::Line(_))) {
        out.extend(
            [
                LineKind::Line,
                LineKind::Arrow,
                LineKind::ArrowStart,
                LineKind::ArrowBoth,
            ]
            .map(Choice::Head),
        );
    }
    if has(|e| matches!(e, Element::Line(_) | Element::Shape(_))) {
        out.extend([Stroke::Solid, Stroke::Dashed, Stroke::Dotted].map(Choice::Stroke));
    }
    if has(|e| matches!(e, Element::Line(_))) {
        out.extend([Routing::Elbow, Routing::Straight].map(Choice::Routing));
    }
    if has(|e| matches!(e, Element::Shape(s) if s.has_border())) {
        out.extend(
            [
                Border::Single,
                Border::Rounded,
                Border::Double,
                Border::Heavy,
            ]
            .map(Choice::Border),
        );
    }
    out
}

fn active(s: &Session, c: Choice) -> bool {
    let els = s.selection.iter().filter_map(|id| s.canvas.get(id));
    match c {
        Choice::Head(kind) => els
            .filter_map(|e| match e {
                Element::Line(l) => Some(l.kind),
                _ => None,
            })
            .all(|k| k == kind),
        Choice::Stroke(st) => els
            .filter_map(|e| match e {
                Element::Line(l) => Some(l.stroke),
                Element::Shape(s) => Some(s.stroke),
                Element::Text(_) => None,
            })
            .all(|k| k == st),
        Choice::Border(b) => els
            .filter_map(|e| match e {
                Element::Shape(s) if s.has_border() => Some(s.border),
                _ => None,
            })
            .all(|k| k == b),
        Choice::Routing(r) => els
            .filter_map(|e| match e {
                Element::Line(l) => Some(l.routing),
                _ => None,
            })
            .all(|k| k == r),
    }
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
