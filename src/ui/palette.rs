use super::grid::{Grid, REVERSE, UNDERLINE};
use crate::input::{Session, Tool};
use crate::model::{Align, Border, Element, Id, LineKind, Paint, Routing, Stroke, TextAlign, Z};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteHit {
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
    Align(TextAlign),
    Ink,
    Fill,
    Order(Z),
    Place(Align),
    Group,
    Ungroup,
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
            Choice::Align(TextAlign::Left) => "<< left",
            Choice::Align(TextAlign::Center) => "<> mid",
            Choice::Align(TextAlign::Right) => ">> right",
            Choice::Ink => "ink >",
            Choice::Fill => "fill >",
            Choice::Routing(Routing::Elbow) => "|_ elbow",
            Choice::Routing(Routing::Straight) => "/ direct",
            Choice::Head(LineKind::Line) => "-- none",
            Choice::Head(LineKind::Arrow) => "-> end",
            Choice::Head(LineKind::ArrowStart) => "<- start",
            Choice::Head(LineKind::ArrowBoth) => "<-> both",
            Choice::Order(Z::Back) => "{ back",
            Choice::Order(Z::Backward) => "[ lower",
            Choice::Order(Z::Forward) => "] raise",
            Choice::Order(Z::Front) => "} front",
            Choice::Place(Align::Left) => "< align-l",
            Choice::Place(Align::Center) => "| align-c",
            Choice::Place(Align::Right) => "> align-r",
            Choice::Place(Align::Top) => "^ align-t",
            Choice::Place(Align::Middle) => "- align-m",
            Choice::Place(Align::Bottom) => "_ align-b",
            Choice::Place(Align::DistributeH) => "= spread-h",
            Choice::Place(Align::DistributeV) => "# spread-v",
            Choice::Group => "g group",
            Choice::Ungroup => "G ungroup",
        }
    }

    /// Word and key of a one-shot action; the other choices have no key.
    fn hint(self) -> Option<(&'static str, &'static str)> {
        match self {
            Choice::Order(_) | Choice::Place(_) | Choice::Group | Choice::Ungroup => {
                self.label().split_once(' ').map(|(key, word)| (word, key))
            }
            _ => None,
        }
    }
}

/// The selected elements, found with one id map instead of a scan per selected id.
pub fn selected(s: &Session) -> Vec<&Element> {
    let by_id: HashMap<&Id, &Element> = s.canvas.elements.iter().map(|e| (e.id(), e)).collect();
    s.selection
        .iter()
        .filter_map(|id| by_id.get(id).copied())
        .collect()
}

/// Choices that apply to the selection; empty when nothing is selected.
pub fn choices(els: &[&Element], groups: &[Vec<Id>]) -> Vec<Choice> {
    if els.is_empty() {
        return vec![];
    }
    let has = |f: fn(&Element) -> bool| els.iter().any(|e| f(e));
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
    if let Some(Element::Shape(s)) = els.iter().find(|e| matches!(e, Element::Shape(_))) {
        out.push(Choice::Align(s.align));
    }
    out.push(Choice::Ink);
    if has(|e| matches!(e, Element::Shape(_))) {
        out.push(Choice::Fill);
    }
    out.extend([Z::Back, Z::Backward, Z::Forward, Z::Front].map(Choice::Order));
    if els.len() > 1 {
        out.extend(
            [
                Align::Left,
                Align::Center,
                Align::Right,
                Align::Top,
                Align::Middle,
                Align::Bottom,
                Align::DistributeH,
                Align::DistributeV,
            ]
            .map(Choice::Place),
        );
        out.push(Choice::Group);
    }
    if groups
        .iter()
        .flatten()
        .any(|m| els.iter().any(|e| e.id() == m))
    {
        out.push(Choice::Ungroup);
    }
    out
}

/// The color a selected element holds for this choice; `None` when the element is not eligible.
pub fn paint(e: &Element, c: Choice) -> Option<Option<Paint>> {
    match (c, e) {
        (Choice::Ink, Element::Shape(s)) => Some(s.color),
        (Choice::Ink, Element::Line(l)) => Some(l.color),
        (Choice::Ink, Element::Text(t)) => Some(t.color),
        (Choice::Fill, Element::Shape(s)) => Some(s.fill),
        _ => None,
    }
}

fn active(els: &[&Element], c: Choice) -> bool {
    let els = els.iter().copied();
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
        Choice::Align(a) => a != TextAlign::Center,
        Choice::Routing(r) => els
            .filter_map(|e| match e {
                Element::Line(l) => Some(l.routing),
                _ => None,
            })
            .all(|k| k == r),
        Choice::Ink | Choice::Fill => els.filter_map(|e| paint(e, c)).any(|p| p.is_some()),
        Choice::Order(_) | Choice::Place(_) | Choice::Group | Choice::Ungroup => false,
    }
}

/// Every clickable cell run as (hit, x, y, width). Tools fill row 0; options start on row 1 and wrap at `width`.
pub fn cells(opts: &[Choice], width: i32) -> Vec<(PaletteHit, i32, i32, i32)> {
    let mut out = vec![];
    let mut x = 0;
    for t in Tool::ALL {
        let w = t.label().len() as i32 + 2;
        out.push((PaletteHit::Tool(t), x, 0, w));
        x += w;
    }
    let (mut x, mut y) = (0, 1);
    for c in opts {
        let w = c.label().len() as i32 + 2;
        if x > 0 && x + w > width {
            (x, y) = (0, y + 1);
        }
        out.push((PaletteHit::Option(*c), x, y, w));
        x += w;
    }
    out
}

pub fn hit(s: &Session, col: i32, row: i32) -> Option<PaletteHit> {
    cells(
        &choices(&selected(s), &s.canvas.groups),
        i32::from(s.screen.0),
    )
    .into_iter()
    .find(|&(_, x, y, w)| row == y && col >= x && col < x + w)
    .map(|c| c.0)
}

pub fn draw(grid: &mut Grid, s: &Session) {
    let els = selected(s);
    let cells = cells(&choices(&els, &s.canvas.groups), grid.w);
    for &(h, x, y, w) in &cells {
        let (text, flags) = match h {
            PaletteHit::Tool(t) => (t.label().to_string(), if t == s.tool { REVERSE } else { 0 }),
            PaletteHit::Option(c) => (
                c.label().to_string(),
                if active(&els, c) { REVERSE } else { 0 },
            ),
        };
        let flags = flags
            | if s.palette_hover == Some(h) {
                UNDERLINE
            } else {
                0
            };
        let pad = " ".repeat((w as usize).saturating_sub(text.len() + 1));
        grid.text(x, y, &format!(" {text}{pad}"), flags);
    }
    if let (true, Some(h)) = (s.tip, s.palette_hover) {
        tooltip(grid, h, &cells);
    }
}

/// Draws the cell's name and key below it, when it has a key.
fn tooltip(grid: &mut Grid, h: PaletteHit, cells: &[(PaletteHit, i32, i32, i32)]) {
    let Some(&(_, x, y, _)) = cells.iter().find(|c| c.0 == h) else {
        return;
    };
    let text = match h {
        PaletteHit::Tool(t) => match t.key() {
            Some(k) => format!(" {}  {} ", t.label(), k),
            None => format!(" {} ", t.label()),
        },
        PaletteHit::Option(c) => match c.hint() {
            Some((word, key)) => format!(" {word}  {key} "),
            None => return,
        },
    };
    grid.text(x, y + 1, &text, REVERSE);
}
