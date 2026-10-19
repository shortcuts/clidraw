use super::geom::{center, route, side_mid, Axis};
use super::{Element, End, Id, Line, Shape, VERSION};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Canvas {
    pub version: u32,
    pub elements: Vec<Element>,
}

impl Default for Canvas {
    fn default() -> Canvas {
        Canvas {
            version: VERSION,
            elements: vec![],
        }
    }
}

/// A shape lookup by id, borrowed from the canvas it serves.
pub type Find<'a> = dyn Fn(&Id) -> Option<&'a Shape> + 'a;

impl Canvas {
    pub fn index_of(&self, id: &Id) -> Option<usize> {
        self.elements.iter().position(|e| e.id() == id)
    }

    pub fn get(&self, id: &Id) -> Option<&Element> {
        self.index_of(id).map(|i| &self.elements[i])
    }

    pub fn shape(&self, id: &Id) -> Option<&Shape> {
        match self.get(id) {
            Some(Element::Shape(s)) => Some(s),
            _ => None,
        }
    }

    /// Shape lookup in O(1). A frame draws every line, and `shape` scans all elements.
    pub fn shape_lookup<'a>(&'a self) -> impl Fn(&Id) -> Option<&'a Shape> + 'a {
        let by_id: HashMap<&Id, &Shape> = self
            .elements
            .iter()
            .filter_map(|e| match e {
                Element::Shape(s) => Some((&s.id, s)),
                _ => None,
            })
            .collect();
        move |id| by_id.get(id).copied()
    }

    fn anchors<'a>(&'a self, l: &Line, find: &Find<'a>) -> [((i32, i32), Option<Axis>); 2] {
        let raw = |end: &End| match end {
            End::Free(x, y) => (*x, *y),
            End::Bound(id) => find(id).map_or((0, 0), center),
        };
        let (from, to) = (raw(&l.from), raw(&l.to));
        let shape_of = |end: &End| match end {
            End::Bound(id) => find(id),
            End::Free(..) => None,
        };
        let anchor = |end: &End, own, toward| match shape_of(end) {
            Some(s) => {
                let (p, ax) = side_mid(s, toward);
                (p, Some(ax))
            }
            None => (own, None),
        };
        [anchor(&l.from, from, to), anchor(&l.to, to, from)]
    }

    /// Cells of the line from start to end, at most two bends.
    pub fn route(&self, l: &Line) -> Vec<(i32, i32)> {
        self.route_with(l, &|id| self.shape(id))
    }

    pub fn route_with<'a>(&'a self, l: &Line, find: &Find<'a>) -> Vec<(i32, i32)> {
        let [(a, ax_a), (b, ax_b)] = self.anchors(l, find);
        route(a, ax_a, b, ax_b)
    }

    /// Cells where the line starts and ends. Bound ends sit on the shape edge.
    pub fn line_ends(&self, l: &Line) -> ((i32, i32), (i32, i32)) {
        self.line_ends_with(l, &|id| self.shape(id))
    }

    fn line_ends_with<'a>(&'a self, l: &Line, find: &Find<'a>) -> ((i32, i32), (i32, i32)) {
        let [(a, _), (b, _)] = self.anchors(l, find);
        (a, b)
    }

    /// Smallest box (x, y, w, h) holding the element.
    pub fn bounds(&self, e: &Element) -> (i32, i32, i32, i32) {
        self.bounds_with(e, &|id| self.shape(id))
    }

    pub fn bounds_with<'a>(&'a self, e: &Element, find: &Find<'a>) -> (i32, i32, i32, i32) {
        match e {
            Element::Shape(s) => (s.x, s.y, s.w, s.h),
            Element::Text(t) => (t.x, t.y, t.width(), 1),
            Element::Line(l) => {
                let (a, b) = self.line_ends_with(l, find);
                let (x, y) = (a.0.min(b.0), a.1.min(b.1));
                (x, y, (a.0 - b.0).abs() + 1, (a.1 - b.1).abs() + 1)
            }
        }
    }

    pub fn all_bounds(&self) -> Option<(i32, i32, i32, i32)> {
        self.elements.iter().map(|e| self.bounds(e)).reduce(|a, b| {
            let (x, y) = (a.0.min(b.0), a.1.min(b.1));
            let (r, d) = ((a.0 + a.2).max(b.0 + b.2), (a.1 + a.3).max(b.1 + b.3));
            (x, y, r - x, d - y)
        })
    }

    /// Elements whose whole box lies in the area (x, y, w, h), in order.
    /// A line also needs its bound shapes inside: its ends sit on the shape edges.
    pub fn within(&self, (ax, ay, aw, ah): (i32, i32, i32, i32)) -> Canvas {
        let inside = |e: &Element| {
            let (x, y, w, h) = self.bounds(e);
            x >= ax && y >= ay && x + w <= ax + aw && y + h <= ay + ah
        };
        let shapes_in: Vec<&Id> = self
            .elements
            .iter()
            .filter(|e| !matches!(e, Element::Line(_)) && inside(e))
            .map(|e| e.id())
            .collect();
        let end_in = |end: &End| match end {
            End::Bound(id) => shapes_in.contains(&id),
            End::Free(..) => true,
        };
        let kept = self
            .elements
            .iter()
            .filter(|e| match e {
                Element::Line(l) => inside(e) && end_in(&l.from) && end_in(&l.to),
                _ => shapes_in.contains(&e.id()),
            })
            .cloned()
            .collect();
        Canvas {
            version: self.version,
            elements: kept,
        }
    }
}
