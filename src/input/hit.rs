use crate::model::{Canvas, Element, Find, Id, Side};

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

#[derive(Debug, PartialEq, Eq)]
pub enum Hit {
    Handle(Handle, Id),
    Element(Id),
    Empty,
}

/// Handles exist only while exactly one element is selected.
pub fn handles(canvas: &Canvas, selection: &[Id]) -> Vec<(Handle, Id, (i32, i32))> {
    let [id] = selection else { return vec![] };
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

fn covers<'a>(canvas: &'a Canvas, find: &Find<'a>, e: &Element, p: (i32, i32)) -> bool {
    match e {
        Element::Line(l) => canvas.route_with(l, find).contains(&p),
        _ => {
            let (x, y, w, h) = canvas.bounds_with(e, find);
            p.0 >= x && p.0 < x + w && p.1 >= y && p.1 < y + h
        }
    }
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
