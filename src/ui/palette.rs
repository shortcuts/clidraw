use super::actions::Action;
use super::grid::Grid;
use super::registry;
use super::style::{ACTIVE, HOVER, MUTED};
use crate::input::{Session, Tool};
use crate::model::{Align, Border, Element, Id, LineKind, Paint, Stroke, TextAlign};
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

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
    Border(Border),
    Align(TextAlign),
    Ink,
    Fill,
    Place(Align),
    Group,
    Arrange,
    Ungroup,
}

impl Choice {
    /// Registry action and word of the choices that run a keyed action.
    fn keyed(self) -> Option<(Action, &'static str)> {
        Some(match self {
            Choice::Place(a) => (
                Action::Align(a),
                match a {
                    Align::Left => "align-l",
                    Align::Center => "align-c",
                    Align::Right => "align-r",
                    Align::Top => "align-t",
                    Align::Middle => "align-m",
                    Align::Bottom => "align-b",
                    Align::DistributeH => "spread-h",
                    Align::DistributeV => "spread-v",
                },
            ),
            Choice::Group => (Action::Group, "group"),
            Choice::Ungroup => (Action::Ungroup, "ungroup"),
            _ => return None,
        })
    }

    fn label(self) -> String {
        if let Some((word, key)) = self.hint() {
            return format!("{key} {word}");
        }
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
            Choice::Head(LineKind::Line) => "-- none",
            Choice::Head(LineKind::Arrow) => "-> end",
            Choice::Head(LineKind::ArrowStart) => "<- start",
            Choice::Head(LineKind::ArrowBoth) => "<-> both",
            _ => "~ arrange",
        }
        .to_string()
    }

    /// Word and key of a one-shot action; the other choices have no key.
    fn hint(self) -> Option<(&'static str, String)> {
        let (action, word) = self.keyed()?;
        Some((word, registry::for_action(action)?.first_label()))
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
        out.push(Choice::Arrange);
    }
    if has(|e| matches!(e, Element::Line(_) | Element::Shape(_))) {
        out.extend([Stroke::Solid, Stroke::Dashed, Stroke::Dotted].map(Choice::Stroke));
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
        Choice::Ink | Choice::Fill => els.filter_map(|e| paint(e, c)).any(|p| p.is_some()),
        Choice::Arrange | Choice::Place(_) | Choice::Group | Choice::Ungroup => false,
    }
}

/// Every clickable cell run as (hit, x, y, width). Tools fill row 0, with their key when the row fits; options start on row 1 and wrap at `width`.
pub fn cells(opts: &[Choice], width: i32) -> Vec<(PaletteHit, i32, i32, i32)> {
    let mut out = vec![];
    let mut x = 0;
    let key_w = |t: Tool| t.key().map_or(0, |_| 2);
    let keyed = Tool::ALL
        .iter()
        .map(|&t| t.label().width() as i32 + 2 + key_w(t))
        .sum::<i32>()
        <= width;
    for t in Tool::ALL {
        let w = t.label().width() as i32 + 2 + if keyed { key_w(t) } else { 0 };
        out.push((PaletteHit::Tool(t), x, 0, w));
        x += w;
    }
    let (mut x, mut y) = (0, 1);
    for c in opts {
        let w = c.label().width() as i32 + 2;
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
            PaletteHit::Tool(t) => (t.label().to_string(), if t == s.tool { ACTIVE } else { 0 }),
            PaletteHit::Option(c) => (c.label(), if active(&els, c) { ACTIVE } else { 0 }),
        };
        let flags = flags | if s.palette_hover == Some(h) { HOVER } else { 0 };
        let key = match h {
            PaletteHit::Tool(t) if w as usize > text.width() + 2 => t.key(),
            _ => None,
        };
        let pad = " ".repeat((w as usize).saturating_sub(text.width() + 1));
        grid.text(x, y, &format!(" {text}{pad}"), flags);
        if let Some(k) = key {
            grid.text(x + w - 2, y, &k.to_string(), flags | MUTED);
        }
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
    grid.text(x, y + 1, &text, ACTIVE);
}
