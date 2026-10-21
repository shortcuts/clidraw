use super::grid::{Grid, REVERSE, UNDERLINE};
use crate::input::{Session, Tool};
use crate::model::{Border, Element, Id, LineKind, Paint, Routing, Stroke, TextAlign};
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

/// Choices that apply to the selection; empty when the selection has none.
pub fn choices(els: &[&Element]) -> Vec<Choice> {
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
    }
}

/// Every clickable cell run as (hit, x, y, width). Tools fill row 0, options row 1.
pub fn cells(opts: &[Choice]) -> Vec<(PaletteHit, i32, i32, i32)> {
    let run = |labels: Vec<(PaletteHit, usize)>, y: i32| {
        let mut x = 0;
        labels
            .into_iter()
            .map(|(h, len)| {
                let w = len as i32 + 2;
                x += w;
                (h, x - w, y, w)
            })
            .collect::<Vec<_>>()
    };
    let tools = Tool::ALL
        .iter()
        .map(|t| (PaletteHit::Tool(*t), t.label().len()));
    let opts = opts
        .iter()
        .map(|c| (PaletteHit::Option(*c), c.label().len()));
    let mut out = run(tools.collect(), 0);
    out.extend(run(opts.collect(), 1));
    out
}

pub fn hit(s: &Session, col: i32, row: i32) -> Option<PaletteHit> {
    cells(&choices(&selected(s)))
        .into_iter()
        .find(|&(_, x, y, w)| row == y && col >= x && col < x + w)
        .map(|c| c.0)
}

pub fn draw(grid: &mut Grid, s: &Session) {
    let els = selected(s);
    for (h, x, y, w) in cells(&choices(&els)) {
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
    if let (true, Some(PaletteHit::Tool(t))) = (s.tip, s.palette_hover) {
        tooltip(grid, t, &cells(&[]));
    }
}

/// Draws the tool's name and key beside its cell, on the side facing the canvas.
fn tooltip(grid: &mut Grid, t: Tool, cells: &[(PaletteHit, i32, i32, i32)]) {
    let Some(&(_, x, y, _)) = cells.iter().find(|c| c.0 == PaletteHit::Tool(t)) else {
        return;
    };
    let text = match t.key() {
        Some(k) => format!(" {}  {} ", t.label(), k),
        None => format!(" {} ", t.label()),
    };
    grid.text(x, y + 1, &text, REVERSE);
}
