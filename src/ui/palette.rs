use super::actions::Action;
use super::grid::Grid;
use super::registry;
use super::style::{ACTIVE, DISABLED, HOVER, MUTED};
use crate::input::{Session, Tool};
use crate::model::{Align, Border, Element, Id, LineKind, Paint, Stroke, TextAlign};
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteHit {
    Tool(Tool),
    Option(Choice),
    /// An edge arrow of a row; the direction is -1 or 1.
    Scroll(Strip, i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strip {
    Tools,
    Options,
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

    /// Choices of one group sit side by side; a separator divides two groups.
    fn group(self) -> u8 {
        match self {
            Choice::Head(_) | Choice::Arrange => 0,
            Choice::Stroke(_) => 1,
            Choice::Border(_) => 2,
            Choice::Align(_) => 3,
            Choice::Ink | Choice::Fill => 4,
            Choice::Place(_) => 5,
            Choice::Group | Choice::Ungroup => 6,
        }
    }

    /// The registry action a click on this choice runs.
    fn action(self) -> Option<Action> {
        match self {
            Choice::Head(_) | Choice::Stroke(_) | Choice::Border(_) | Choice::Align(_) => {
                Some(Action::Style(self))
            }
            Choice::Ink | Choice::Fill => Some(Action::Pick(self)),
            Choice::Arrange => None,
            _ => self.keyed().map(|k| k.0),
        }
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

const ARROW: i32 = 3;
const STRIPS: [Strip; 2] = [Strip::Tools, Strip::Options];

/// A clickable run: `x` and `w` are clipped to the visible area, `from` is the unclipped left edge.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub hit: PaletteHit,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub from: i32,
}

/// Geometry of both chrome rows; hit testing and drawing both read it.
#[derive(Default)]
pub struct Layout {
    pub cells: Vec<Placed>,
    seps: Vec<(i32, i32)>,
    off: Vec<(i32, i32, char)>,
    max: [i32; 2],
    starts: [Vec<i32>; 2],
}

fn lay_row(
    out: &mut Layout,
    r: usize,
    units: &[Vec<(PaletteHit, i32)>],
    width: i32,
    scroll: i32,
    seps: bool,
) {
    let y = r as i32;
    let (mut run, mut gaps, mut vx) = (vec![], vec![], 0);
    for (i, unit) in units.iter().enumerate() {
        if i > 0 && seps {
            gaps.push(vx);
            vx += 1;
        }
        out.starts[r].push(vx);
        for &(hit, w) in unit {
            run.push((hit, vx, w));
            vx += w;
        }
    }
    let over = vx > width;
    let (lo, hi) = if over {
        (ARROW, width - ARROW)
    } else {
        (0, width)
    };
    let max = if over {
        (vx - (hi - lo).max(0)).max(0)
    } else {
        0
    };
    let scroll = scroll.clamp(0, max);
    out.max[r] = max;
    let shift = lo - scroll;
    for (hit, vx, w) in run {
        let (x0, x1) = ((vx + shift).max(lo), (vx + shift + w).min(hi));
        if x1 > x0 {
            out.cells.push(Placed {
                hit,
                x: x0,
                y,
                w: x1 - x0,
                from: vx + shift,
            });
        }
    }
    out.seps.extend(
        gaps.into_iter()
            .map(|g| (g + shift, y))
            .filter(|&(x, _)| x >= lo && x < hi),
    );
    if over {
        for (dir, x, on, glyph) in [
            (-1, 0, scroll > 0, '‹'),
            (1, width - ARROW, scroll < max, '›'),
        ] {
            if on {
                let hit = PaletteHit::Scroll(STRIPS[r], dir);
                out.cells.push(Placed {
                    hit,
                    x,
                    y,
                    w: ARROW,
                    from: x,
                });
            } else {
                out.off.push((x, y, glyph));
            }
        }
    }
}

/// Both rows as clickable cells. Tools fill row 0, with their key when the row fits; options sit on row 1 in groups. A row wider than `width` scrolls by `scroll` columns between two arrow cells.
pub fn layout(opts: &[Choice], width: i32, scroll: (i32, i32)) -> Layout {
    let mut out = Layout::default();
    let key_w = |t: Tool| t.key().map_or(0, |_| 2);
    let keyed = Tool::ALL
        .iter()
        .map(|&t| t.label().width() as i32 + 2 + key_w(t))
        .sum::<i32>()
        <= width;
    let tools: Vec<_> = Tool::ALL
        .iter()
        .map(|&t| {
            let w = t.label().width() as i32 + 2 + if keyed { key_w(t) } else { 0 };
            vec![(PaletteHit::Tool(t), w)]
        })
        .collect();
    lay_row(&mut out, 0, &tools, width, scroll.0, false);
    let mut groups: Vec<Vec<(PaletteHit, i32)>> = vec![];
    let mut last = None;
    for c in opts {
        if last != Some(c.group()) {
            groups.push(vec![]);
            last = Some(c.group());
        }
        groups
            .last_mut()
            .unwrap()
            .push((PaletteHit::Option(*c), c.label().width() as i32 + 2));
    }
    lay_row(&mut out, 1, &groups, width, scroll.1, true);
    out
}

fn current(s: &Session) -> Layout {
    let opts = if s.zoomed_out() {
        vec![]
    } else {
        choices(&selected(s), &s.canvas.groups)
    };
    layout(
        &opts,
        i32::from(s.screen.0),
        (s.tool_scroll, s.option_scroll),
    )
}

fn slot(s: &mut Session, strip: Strip) -> &mut i32 {
    match strip {
        Strip::Tools => &mut s.tool_scroll,
        Strip::Options => &mut s.option_scroll,
    }
}

/// Scrolls the row under screen `row` by one wheel notch toward `dir`.
pub fn wheel(s: &mut Session, row: i32, dir: i32) {
    let strip = STRIPS[row as usize];
    let max = current(s).max[row as usize];
    let at = slot(s, strip);
    *at = (*at + 4 * dir).clamp(0, max);
}

/// Scrolls a strip to the next group start toward `dir`, at most one area width.
pub fn jump(s: &mut Session, strip: Strip, dir: i32) {
    let r = strip as usize;
    let lay = current(s);
    let span = (i32::from(s.screen.0) - 2 * ARROW).max(1);
    let at = *slot(s, strip);
    let at = at.clamp(0, lay.max[r]);
    let to = if dir > 0 {
        lay.starts[r]
            .iter()
            .copied()
            .find(|&x| x > at)
            .map_or(lay.max[r], |x| x.min(at + span))
    } else {
        lay.starts[r]
            .iter()
            .copied()
            .rev()
            .find(|&x| x < at)
            .map_or(0, |x| x.max(at - span))
    };
    *slot(s, strip) = to.clamp(0, lay.max[r]);
}

/// Screen rows the tool row and the option row take at the top; 0 in present mode.
pub fn rows(s: &Session) -> i32 {
    if s.present {
        0
    } else if s.zoomed_out() || !s.selection.iter().any(|id| s.canvas.get(id).is_some()) {
        1
    } else {
        2
    }
}

pub fn hit(s: &Session, col: i32, row: i32) -> Option<PaletteHit> {
    if s.zoomed_out() {
        return None;
    }
    current(s)
        .cells
        .into_iter()
        .find(|c| row == c.y && col >= c.x && col < c.x + c.w)
        .map(|c| c.hit)
}

pub fn draw(grid: &mut Grid, s: &Session) {
    let els = selected(s);
    let off = s.zoomed_out();
    let opts = if off {
        vec![]
    } else {
        choices(&els, &s.canvas.groups)
    };
    let lay = layout(&opts, grid.w, (s.tool_scroll, s.option_scroll));
    for p in &lay.cells {
        let (text, flags) = match p.hit {
            PaletteHit::Tool(t) => (
                t.label().to_string(),
                if off {
                    DISABLED
                } else if t == s.tool {
                    ACTIVE
                } else {
                    0
                },
            ),
            PaletteHit::Option(c) => (c.label(), if active(&els, c) { ACTIVE } else { 0 }),
            PaletteHit::Scroll(_, dir) => (if dir < 0 { "‹" } else { "›" }.to_string(), 0),
        };
        let flags = flags
            | if s.palette_hover == Some(p.hit) {
                HOVER
            } else {
                0
            };
        let key = match p.hit {
            PaletteHit::Tool(t) if p.w as usize > text.width() + 2 => t.key(),
            _ => None,
        };
        let pad = " ".repeat((p.w as usize).saturating_sub(text.width() + 1));
        grid.text(p.from, p.y, &format!(" {text}{pad}"), flags);
        if let Some(k) = key {
            grid.text(p.from + p.w - 2, p.y, &k.to_string(), flags | MUTED);
        }
    }
    for &(x, y) in &lay.seps {
        grid.set(x, y, '│', MUTED);
    }
    for &(x, y, g) in &lay.off {
        grid.text(x, y, &format!(" {g} "), DISABLED);
    }
    if let (true, Some(h)) = (s.tip, s.palette_hover) {
        tooltip(grid, h, &lay.cells);
    }
}

/// Draws the cell's name and its key, when it has one, below it.
fn tooltip(grid: &mut Grid, h: PaletteHit, cells: &[Placed]) {
    let Some(p) = cells.iter().find(|c| c.hit == h) else {
        return;
    };
    let text = match h {
        PaletteHit::Tool(t) => match t.key() {
            Some(k) => format!(" {}  {} ", t.label(), k),
            None => format!(" {} ", t.label()),
        },
        PaletteHit::Option(c) => match c.action() {
            None => " Reset route ".to_string(),
            Some(a) => {
                let Some(b) = registry::for_action(a) else {
                    return;
                };
                match b.first_label().as_str() {
                    "" => format!(" {}: {} ", a.category(), b.name()),
                    k => format!(" {}: {}  {} ", a.category(), b.name(), k),
                }
            }
        },
        PaletteHit::Scroll(..) => return,
    };
    let x = p.x.min(grid.w - text.width() as i32).max(0);
    grid.text(x, p.y + 1, &text, ACTIVE);
}
