use super::{hit, Hit, Session};
use crate::model::{
    Anchor, Command, Edge, Element, End, Id, Line, LineKind, Shape, ShapeKind, GAP_H, GAP_V,
};
use crate::view::Zoom;

/// How far outside a side the pointer still offers a copy.
const REACH: i32 = 2;

/// The copy and arrow offered beside the selected shape `from`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quick {
    pub from: Id,
    pub edge: Edge,
    pub rect: (i32, i32, i32, i32),
}

/// What the pointer at world cell `p` offers: a copy of the lone selected shape at the layout gap.
pub fn at(s: &Session, p: (i32, i32)) -> Option<Quick> {
    if !(s.quick_connect
        && s.tool.is_pointer()
        && s.edit.is_none()
        && !s.shift
        && s.view.zoom == Zoom::One)
    {
        return None;
    }
    let [id] = s.selection.as_slice() else {
        return None;
    };
    let Some(Element::Shape(src)) = s.canvas.get(id) else {
        return None;
    };
    if src.kind == ShapeKind::Frame {
        return None;
    }
    let (x, y, w, h) = (src.x, src.y, src.w, src.h);
    let along_x = (x..x + w).contains(&p.0);
    let along_y = (y..y + h).contains(&p.1);
    let edge = if along_x && (y - REACH..y).contains(&p.1) {
        Edge::Top
    } else if along_x && (y + h..y + h + REACH).contains(&p.1) {
        Edge::Bottom
    } else if along_y && (x - REACH..x).contains(&p.0) {
        Edge::Left
    } else if along_y && (x + w..x + w + REACH).contains(&p.0) {
        Edge::Right
    } else {
        return None;
    };
    let (rx, ry) = match edge {
        Edge::Top => (x, y - GAP_V - h),
        Edge::Bottom => (x, y + h + GAP_V),
        Edge::Left => (x - GAP_H - w, y),
        Edge::Right => (x + w + GAP_H, y),
    };
    let find = s.canvas.shape_lookup();
    let taken = s.canvas.elements.iter().any(|e| {
        let b = s.canvas.bounds_with(e, &find);
        b.0 < rx + w && rx < b.0 + b.2 && b.1 < ry + h && ry < b.1 + b.3
    });
    if taken || hit::test(s, &s.selection, p) != Hit::Empty {
        return None;
    }
    Some(Quick {
        from: id.clone(),
        edge,
        rect: (rx, ry, w, h),
    })
}

/// A copy of `src` at `rect`, without its text, link and side labels.
pub fn clone_of(src: &Shape, rect: (i32, i32, i32, i32)) -> Shape {
    Shape {
        id: Id::random(),
        x: rect.0,
        y: rect.1,
        w: rect.2,
        h: rect.3,
        label: String::new(),
        link: None,
        side_labels: vec![],
        ..src.clone()
    }
}

/// Inserts the copy and an arrow bound to both shapes; one undo step.
pub fn insert_cmd(s: &Session, q: &Quick) -> Option<(Command, Id)> {
    let Some(Element::Shape(src)) = s.canvas.get(&q.from) else {
        return None;
    };
    let copy = clone_of(src, q.rect);
    let id = copy.id.clone();
    let arrow = Line {
        id: Id::random(),
        kind: LineKind::Arrow,
        from: End::Bound(q.from.clone(), Anchor::Auto),
        to: End::Bound(id.clone(), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
    };
    let at = s.canvas.elements.len();
    let cmd = Command::Batch(vec![
        Command::Insert(at, Element::Shape(copy)),
        Command::Insert(at + 1, Element::Line(arrow)),
    ]);
    Some((cmd, id))
}
