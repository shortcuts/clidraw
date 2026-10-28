use super::Session;
use crate::model::{
    label_pos, side_label_at, Canvas, Edge, Element, Find, Id, Shape, ShapeKind, Side, REACH,
};
use crate::ui::{refresh, Layout};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Resize,
    End(Side),
    Bend(usize),
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
            vec![(Handle::Resize, id.clone(), (r, b))]
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

fn covers(canvas: &Canvas, layout: &Layout, i: usize, p: (i32, i32)) -> bool {
    let e = &canvas.elements[i];
    let Element::Line(l) = e else {
        return border_covers(e, p);
    };
    // A detour stays within REACH of the end cells; routing every line per hover is too slow.
    let (x, y, w, h) = layout.bounds[i];
    p.0 >= x - REACH
        && p.0 < x + w + REACH
        && p.1 >= y - REACH
        && p.1 < y + h + REACH
        && canvas
            .route_with(l, &layout.find(canvas), &layout.router)
            .contains(&p)
}

fn border_covers(e: &Element, p: (i32, i32)) -> bool {
    let s = match e {
        Element::Shape(s) => s,
        Element::Text(t) => return p.1 == t.y && p.0 >= t.x && p.0 < t.x + t.width(),
        _ => return false,
    };
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

pub fn test(s: &Session, selection: &[Id], p: (i32, i32)) -> Hit {
    let canvas = &s.canvas;
    if let Some((h, id, _)) = handles(canvas, selection).into_iter().find(|h| h.2 == p) {
        return Hit::Handle(h, id);
    }
    let mut slot = s.layout.borrow_mut();
    let layout = refresh(&mut slot, canvas, s.rev);
    canvas
        .stacking()
        .rev()
        .find(|&i| covers(canvas, layout, i, p))
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
