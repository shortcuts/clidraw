use super::Session;
use crate::model::{
    label_pos, side_label_at, Canvas, Edge, Element, Find, Id, Line, Router, Shape, ShapeKind,
    Side, REACH,
};
use crate::ui::{refresh, Layout};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Resize,
    Rotate,
    End(Side),
    Bend(usize),
    Mid(usize, usize),
    Whole,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hit {
    Handle(Handle, Id),
    Element(Id),
    Empty,
}

/// Handles exist only while exactly one element is selected, or one frame with its contents.
pub fn handles(canvas: &Canvas, selection: &[Id]) -> Vec<(Handle, Id, (i32, i32))> {
    let id = match selection {
        [id] => id,
        [_, _, ..] => match canvas
            .elements
            .iter()
            .filter(|e| matches!(e, Element::Shape(s) if s.kind == ShapeKind::Frame))
            .map(Element::id)
            .find(|id| selection.contains(id) && frame_unit(canvas, id, selection))
        {
            Some(id) => id,
            None => return vec![],
        },
        _ => return vec![],
    };
    match canvas.get(id) {
        Some(Element::Shape(s)) => {
            let (r, b) = (s.x + s.w - 1, s.y + s.h - 1);
            let mut hs = vec![(Handle::Resize, id.clone(), (r, b))];
            if matches!(s.kind, ShapeKind::Rect | ShapeKind::Cylinder) {
                hs.push((Handle::Rotate, id.clone(), (s.x + (s.w - 1) / 2, s.y - 2)));
            }
            hs
        }
        Some(Element::Line(l)) => {
            let (a, b) = canvas.line_ends(l);
            let bends = l.bends.iter().enumerate();
            let bends = bends.map(|(i, &p)| (Handle::Bend(i), id.clone(), p));
            [
                (Handle::End(Side::From), id.clone(), a),
                (Handle::End(Side::To), id.clone(), b),
            ]
            .into_iter()
            .chain(bends)
            .collect()
        }
        _ => vec![],
    }
}

/// Dots of a lone selected line; dragging one inserts a bend, or moves the whole line for `Whole`.
pub fn mid_dots<'a>(
    canvas: &'a Canvas,
    find: &Find<'a>,
    router: &Router,
    selection: &[Id],
) -> Vec<(Handle, Id, (i32, i32))> {
    let [id] = selection else { return vec![] };
    let Some(Element::Line(l)) = canvas.get(id) else {
        return vec![];
    };
    canvas
        .mid_cells(l, find, router)
        .into_iter()
        .map(|(m, p)| {
            (
                m.map_or(Handle::Whole, |(i, pos)| Handle::Mid(i, pos)),
                id.clone(),
                p,
            )
        })
        .collect()
}

/// True when `id` is a frame whose frame-unit is exactly `selection`.
fn frame_unit(canvas: &Canvas, id: &Id, selection: &[Id]) -> bool {
    let Some(Element::Shape(s)) = canvas.get(id) else {
        return false;
    };
    if s.kind != ShapeKind::Frame {
        return false;
    }
    let mut unit = vec![id.clone()];
    canvas.expand_frames(&mut unit);
    unit.len() == selection.len() && unit.iter().all(|u| selection.contains(u))
}

/// True when `p` lies in the rect grown by `slop` cells on every side.
fn in_rect((x, y, w, h): (i32, i32, i32, i32), p: (i32, i32), slop: i32) -> bool {
    p.0 >= x - slop && p.0 < x + w + slop && p.1 >= y - slop && p.1 < y + h + slop
}

/// A detour stays within REACH of the end cells; routing every line per hover is too slow.
fn route_near(
    canvas: &Canvas,
    layout: &Layout,
    i: usize,
    l: &Line,
    p: (i32, i32),
) -> Option<Vec<(i32, i32)>> {
    in_rect(layout.bounds[i], p, REACH)
        .then(|| canvas.route_with(l, &layout.find(canvas), &layout.router))
}

fn covers(canvas: &Canvas, layout: &Layout, i: usize, p: (i32, i32)) -> bool {
    let e = &canvas.elements[i];
    let Element::Line(l) = e else {
        return border_covers(e, p);
    };
    route_near(canvas, layout, i, l, p).is_some_and(|r| r.contains(&p))
}

/// True when `p` lies in the 1-cell ring outside a shape's box, or around a text row.
fn near(canvas: &Canvas, layout: &Layout, i: usize, p: (i32, i32)) -> bool {
    match &canvas.elements[i] {
        Element::Shape(s) => {
            if slanted(s) {
                return (-1..=1)
                    .flat_map(|dy| (-1..=1).map(move |dx| (p.0 + dx, p.1 + dy)))
                    .any(|c| s.on_slanted_outline(c));
            }
            let rect = (s.x, s.y, s.w, s.h);
            in_rect(rect, p, 1) && !in_rect(rect, p, 0)
        }
        Element::Text(t) => in_rect((t.x, t.y, t.width(), 1), p, 1),
        Element::Line(l) => route_near(canvas, layout, i, l, p).is_some_and(|r| {
            r.iter()
                .any(|c| (c.0 - p.0).abs() <= 1 && (c.1 - p.1).abs() <= 1)
        }),
    }
}

fn slanted(s: &Shape) -> bool {
    matches!(
        s.kind,
        ShapeKind::Diamond | ShapeKind::Parallelogram | ShapeKind::Hexagon
    )
}

fn border_covers(e: &Element, p: (i32, i32)) -> bool {
    let s = match e {
        Element::Shape(s) => s,
        Element::Text(t) => return p.1 == t.y && p.0 >= t.x && p.0 < t.x + t.width(),
        _ => return false,
    };
    if slanted(s) {
        return s.on_slanted_outline(p);
    }
    let inside = p.0 > s.x && p.0 < s.x + s.w - 1 && p.1 > s.y && p.1 < s.y + s.h - 1;
    box_covers(e, p) && !inside
}

fn box_covers(e: &Element, p: (i32, i32)) -> bool {
    let (x, y, w, h, frame) = match e {
        Element::Shape(s) => (s.x, s.y, s.w, s.h, s.kind == ShapeKind::Frame),
        _ => return false,
    };
    if frame && p.0 > x && p.0 < x + w - 1 && p.1 > y && p.1 < y + h - 1 {
        return false;
    }
    p.0 >= x && p.0 < x + w && p.1 >= y && p.1 < y + h
}

/// The border side `p` lies on, corners excluded; only a rect has sides.
pub fn edge_at(s: &Shape, p: (i32, i32)) -> Option<Edge> {
    if !s.has_border() {
        return None;
    }
    let (r, b) = (s.x + s.w - 1, s.y + s.h - 1);
    let across = p.0 > s.x && p.0 < r;
    let down = p.1 > s.y && p.1 < b;
    match (p.0, p.1) {
        (_, y) if y == s.y && across => Some(Edge::Top),
        (_, y) if y == b && across => Some(Edge::Bottom),
        (x, _) if x == s.x && down => Some(Edge::Left),
        (x, _) if x == r && down => Some(Edge::Right),
        _ => None,
    }
}

/// The edge whose side label is drawn over `p`.
pub fn side_label_cell(s: &Shape, p: (i32, i32)) -> Option<Edge> {
    s.side_labels.iter().find_map(|l| {
        let (x, y, text) = side_label_at(s, l);
        (p.1 == y && p.0 >= x && p.0 < x + text.width() as i32).then_some(l.edge)
    })
}

/// The element whose label sits under `p`, with the side for a shape's side label; only a lone
/// selected line or rect offers it, like their handles.
pub fn label_hit(s: &Session, selection: &[Id], p: (i32, i32)) -> Option<(Id, Option<Edge>)> {
    let [id] = selection else { return None };
    let l = match s.canvas.get(id) {
        Some(Element::Line(l)) => l,
        Some(Element::Shape(sh)) => return Some((id.clone(), Some(side_label_cell(sh, p)?))),
        _ => return None,
    };
    let mut slot = s.layout.borrow_mut();
    let layout = refresh(&mut slot, &s.canvas, s.rev);
    let route = s
        .canvas
        .route_with(l, &layout.find(&s.canvas), &layout.router);
    let (lx, ly) = label_pos(&route, l.label_at);
    let left = lx - l.label.width() as i32 / 2;
    let inside = p.1 == ly && p.0 >= left && p.0 < left + l.label.width() as i32;
    inside.then(|| (id.clone(), None))
}

/// `slop` 1 also picks the topmost element within 1 cell when none covers `p`.
pub(super) fn at(s: &Session, selection: &[Id], p: (i32, i32), slop: i32) -> Hit {
    let canvas = &s.canvas;
    if let Some((h, id, _)) = handles(canvas, selection).into_iter().find(|h| h.2 == p) {
        return Hit::Handle(h, id);
    }
    let mut slot = s.layout.borrow_mut();
    let layout = refresh(&mut slot, canvas, s.rev);
    let dots = mid_dots(canvas, &layout.find(canvas), &layout.router, selection);
    if let Some((h, id, _)) = dots.into_iter().find(|d| d.2 == p) {
        return Hit::Handle(h, id);
    }
    canvas
        .stacking()
        .rev()
        .find(|&i| covers(canvas, layout, i, p))
        .or_else(|| {
            canvas
                .stacking()
                .rev()
                .find(|&i| slop >= 1 && near(canvas, layout, i, p))
        })
        .map_or(Hit::Empty, |i| {
            Hit::Element(canvas.elements[i].id().clone())
        })
}

pub fn shape_at(canvas: &Canvas, p: (i32, i32)) -> Option<Id> {
    canvas
        .stacking()
        .rev()
        .map(|i| &canvas.elements[i])
        .find(|e| box_covers(e, p))
        .map(|e| e.id().clone())
}

pub fn intersects<'a>(
    canvas: &'a Canvas,
    find: &Find<'a>,
    e: &Element,
    a: (i32, i32),
    b: (i32, i32),
) -> bool {
    let (x, y, w, h) = canvas.bounds_with(e, find);
    let (x0, x1, y0, y1) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1));
    x <= x1 && x + w > x0 && y <= y1 && y + h > y0
}

/// Band rule: elements touch the band, a frame needs its whole box inside.
pub fn swept<'a>(
    canvas: &'a Canvas,
    find: &Find<'a>,
    e: &Element,
    a: (i32, i32),
    b: (i32, i32),
) -> bool {
    if !matches!(e, Element::Shape(s) if s.kind == ShapeKind::Frame) {
        return intersects(canvas, find, e, a, b);
    }
    let (x, y, w, h) = canvas.bounds_with(e, find);
    let (x0, x1, y0, y1) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1));
    x >= x0 && x + w - 1 <= x1 && y >= y0 && y + h - 1 <= y1
}
