use crate::model::{label_pos, Canvas, Element, Find, Id, ShapeKind, Side};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    Nw,
    Ne,
    Sw,
    Se,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Corner(Corner),
    End(Side),
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
        [_, _, ..] => match selection
            .iter()
            .find(|id| frame_unit(canvas, id, selection))
        {
            Some(id) => id,
            None => return vec![],
        },
        _ => return vec![],
    };
    match canvas.get(id) {
        Some(Element::Shape(s)) => {
            let (r, b) = (s.x + s.w - 1, s.y + s.h - 1);
            [
                (Corner::Nw, (s.x, s.y)),
                (Corner::Ne, (r, s.y)),
                (Corner::Sw, (s.x, b)),
                (Corner::Se, (r, b)),
            ]
            .into_iter()
            .map(|(c, p)| (Handle::Corner(c), id.clone(), p))
            .collect()
        }
        Some(Element::Line(l)) => {
            let (a, b) = canvas.line_ends(l);
            vec![
                (Handle::End(Side::From), id.clone(), a),
                (Handle::End(Side::To), id.clone(), b),
            ]
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

fn covers<'a>(canvas: &'a Canvas, find: &Find<'a>, e: &Element, p: (i32, i32)) -> bool {
    match e {
        Element::Line(l) => canvas.route_with(l, find).contains(&p),
        _ => {
            let (x, y, w, h) = canvas.bounds_with(e, find);
            let frame = matches!(e, Element::Shape(s) if s.kind == ShapeKind::Frame);
            if frame && p.0 > x && p.0 < x + w - 1 && p.1 > y && p.1 < y + h - 1 {
                return false;
            }
            p.0 >= x && p.0 < x + w && p.1 >= y && p.1 < y + h
        }
    }
}

/// The line whose label sits under `p`; only a lone selected line offers it, like its handles.
pub fn label_hit(canvas: &Canvas, selection: &[Id], p: (i32, i32)) -> Option<Id> {
    let [id] = selection else { return None };
    let Some(Element::Line(l)) = canvas.get(id) else {
        return None;
    };
    let (lx, ly) = label_pos(&canvas.route(l), l.label_at);
    let left = lx - l.label.width() as i32 / 2;
    let inside = p.1 == ly && p.0 >= left && p.0 < left + l.label.width() as i32;
    inside.then(|| id.clone())
}

pub fn test(canvas: &Canvas, selection: &[Id], p: (i32, i32)) -> Hit {
    if let Some((h, id, _)) = handles(canvas, selection).into_iter().find(|h| h.2 == p) {
        return Hit::Handle(h, id);
    }
    let find = canvas.shape_lookup();
    canvas
        .elements
        .iter()
        .rev()
        .find(|e| covers(canvas, &find, e, p))
        .map_or(Hit::Empty, |e| Hit::Element(e.id().clone()))
}

pub fn shape_at(canvas: &Canvas, p: (i32, i32)) -> Option<Id> {
    let find = canvas.shape_lookup();
    canvas
        .elements
        .iter()
        .rev()
        .find(|e| matches!(e, Element::Shape(_)) && covers(canvas, &find, e, p))
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
