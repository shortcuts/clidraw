use super::geom::{anchor_from, anchor_to, center};
use super::{Element, End, Id, Line, Shape, VERSION};
use serde::{Deserialize, Serialize};

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

    fn raw_end(&self, end: &End) -> (i32, i32) {
        match end {
            End::Free(x, y) => (*x, *y),
            End::Bound(id) => self.shape(id).map_or((0, 0), center),
        }
    }

    /// Cells where the line starts and ends. Bound ends sit on the shape edge.
    pub fn line_ends(&self, l: &Line) -> ((i32, i32), (i32, i32)) {
        let a = match (&l.from, self.shape_of(&l.from)) {
            (End::Bound(_), Some(s)) => anchor_from(s, self.raw_end(&l.to)),
            _ => self.raw_end(&l.from),
        };
        let b = match (&l.to, self.shape_of(&l.to)) {
            (End::Bound(_), Some(s)) => anchor_to(s, a),
            _ => self.raw_end(&l.to),
        };
        (a, b)
    }

    fn shape_of(&self, end: &End) -> Option<&Shape> {
        match end {
            End::Bound(id) => self.shape(id),
            End::Free(..) => None,
        }
    }

    /// Smallest box (x, y, w, h) holding the element.
    pub fn bounds(&self, e: &Element) -> (i32, i32, i32, i32) {
        match e {
            Element::Shape(s) => (s.x, s.y, s.w, s.h),
            Element::Text(t) => (t.x, t.y, t.width(), 1),
            Element::Line(l) => {
                let (a, b) = self.line_ends(l);
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
}
