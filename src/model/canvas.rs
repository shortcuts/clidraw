use super::geom::{center, route, side_mid, straight, Axis};
use super::{Command, Element, End, Id, Line, Router, Routing, Shape, ShapeKind, Text, VERSION};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Canvas {
    pub version: u32,
    pub elements: Vec<Element>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Vec<Id>>,
}

impl Default for Canvas {
    fn default() -> Canvas {
        Canvas {
            version: VERSION,
            elements: vec![],
            groups: vec![],
        }
    }
}

/// A shape lookup by id, borrowed from the canvas it serves.
pub type Find<'a> = dyn Fn(&Id) -> Option<&'a Shape> + 'a;

/// A batch longer than this builds an id index; a shorter one scans faster.
pub(crate) const BULK: usize = 8;

pub(crate) type Index = HashMap<Id, usize>;

impl Canvas {
    pub(crate) fn index_map(&self) -> Index {
        let ids = self.elements.iter().enumerate();
        ids.map(|(i, e)| (e.id().clone(), i)).collect()
    }

    /// The top-left corner of the shapes and text, `(0, 0)` when there are none.
    pub fn origin(&self) -> (i32, i32) {
        let corners = self.elements.iter().filter_map(|e| match e {
            Element::Shape(s) => Some((s.x, s.y)),
            Element::Text(t) => Some((t.x, t.y)),
            Element::Line(_) => None,
        });
        corners
            .reduce(|m, p| (m.0.min(p.0), m.1.min(p.1)))
            .unwrap_or((0, 0))
    }

    pub(crate) fn position(&self, id: &Id, index: &Option<Index>) -> Option<usize> {
        match index {
            Some(map) => map.get(id).copied(),
            None => self.index_of(id),
        }
    }

    /// Drops the ids that name no element, keeping the order of the rest.
    pub fn retain_present(&self, ids: &mut Vec<Id>) {
        if ids.len() <= BULK {
            ids.retain(|id| self.get(id).is_some());
        } else {
            let present: std::collections::HashSet<&Id> =
                self.elements.iter().map(|e| e.id()).collect();
            ids.retain(|id| present.contains(id));
        }
    }

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
            End::Bound(id, _) => find(id).map_or((0, 0), center),
        };
        let (from, to) = (raw(&l.from), raw(&l.to));
        let shape_of = |end: &End| match end {
            End::Bound(id, a) => find(id).map(|s| (s, *a)),
            End::Free(..) => None,
        };
        let anchor = |end: &End, own, toward| match shape_of(end) {
            Some((s, a)) => {
                let (p, ax) = side_mid(s, a, toward);
                (p, Some(ax))
            }
            None => (own, None),
        };
        [anchor(&l.from, from, to), anchor(&l.to, to, from)]
    }

    /// Cells of the line from start to end: two bends unless an elbow must go around an obstacle,
    /// or a diagonal for `Routing::Straight`.
    pub fn route(&self, l: &Line) -> Vec<(i32, i32)> {
        self.route_with(l, &|id| self.shape(id), &self.router())
    }

    pub fn route_with<'a>(&'a self, l: &Line, find: &Find<'a>, router: &Router) -> Vec<(i32, i32)> {
        let plain = self.plain_with(l, find);
        match l.routing {
            Routing::Elbow => router.around(self, find, l, self.anchors(l, find), plain),
            Routing::Straight => plain,
        }
    }

    /// The route ignoring obstacles.
    pub(crate) fn plain_with<'a>(&'a self, l: &Line, find: &Find<'a>) -> Vec<(i32, i32)> {
        let [(a, ax_a), (b, ax_b)] = self.anchors(l, find);
        match l.routing {
            Routing::Elbow => route(a, ax_a, b, ax_b),
            Routing::Straight => straight(a, b),
        }
    }

    pub fn router(&self) -> Router {
        let find = self.shape_lookup();
        let bounds: Vec<_> = self
            .elements
            .iter()
            .map(|e| self.bounds_with(e, &find))
            .collect();
        Router::from_bounds(self, &bounds)
    }

    /// Cells where the line starts and ends. Bound ends sit one cell outside the shape edge.
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
        let find = self.shape_lookup();
        self.elements
            .iter()
            .map(|e| self.bounds_with(e, &find))
            .reduce(|a, b| {
                let (x, y) = (a.0.min(b.0), a.1.min(b.1));
                let (r, d) = ((a.0 + a.2).max(b.0 + b.2), (a.1 + a.3).max(b.1 + b.3));
                (x, y, r - x, d - y)
            })
    }

    /// Copies of the listed elements, plus lines bound at both ends to listed shapes, shifted by
    /// (dx, dy) under fresh ids. Bound ends follow the copies; a bound end whose shape is not
    /// copied turns free at its current cell.
    pub fn clone_elements(&self, ids: &[Id], dx: i32, dy: i32) -> Vec<Element> {
        self.clone_mapped(ids, dx, dy).0
    }

    /// `clone_elements` plus the groups whose members were copied, under the fresh ids.
    pub fn clone_canvas(&self, ids: &[Id], dx: i32, dy: i32) -> Canvas {
        let (elements, fresh) = self.clone_mapped(ids, dx, dy);
        let groups = self
            .groups
            .iter()
            .map(|g| g.iter().filter_map(|id| fresh.get(id).cloned()).collect())
            .filter(|g: &Vec<Id>| g.len() > 1)
            .collect();
        Canvas {
            version: self.version,
            elements,
            groups,
        }
    }

    fn clone_mapped(&self, ids: &[Id], dx: i32, dy: i32) -> (Vec<Element>, HashMap<Id, Id>) {
        let bound_in = |end: &End| matches!(end, End::Bound(id, _) if ids.contains(id));
        let picked: Vec<&Element> = self
            .elements
            .iter()
            .filter(|e| match e {
                Element::Line(l) => ids.contains(&l.id) || (bound_in(&l.from) && bound_in(&l.to)),
                _ => ids.contains(e.id()),
            })
            .collect();
        let fresh: HashMap<&Id, Id> = picked.iter().map(|e| (e.id(), Id::random())).collect();
        let cloned = picked
            .into_iter()
            .map(|e| match e {
                Element::Shape(s) => Element::Shape(Shape {
                    id: fresh[&s.id].clone(),
                    x: s.x + dx,
                    y: s.y + dy,
                    ..s.clone()
                }),
                Element::Text(t) => Element::Text(Text {
                    id: fresh[&t.id].clone(),
                    x: t.x + dx,
                    y: t.y + dy,
                    ..t.clone()
                }),
                Element::Line(l) => {
                    let (a, b) = self.line_ends(l);
                    let end = |end: &End, at: (i32, i32)| match end {
                        End::Bound(id, a) if fresh.contains_key(id) => {
                            End::Bound(fresh[id].clone(), *a)
                        }
                        End::Free(x, y) => End::Free(x + dx, y + dy),
                        End::Bound(..) => End::Free(at.0 + dx, at.1 + dy),
                    };
                    Element::Line(Line {
                        id: fresh[&l.id].clone(),
                        from: end(&l.from, a),
                        to: end(&l.to, b),
                        ..l.clone()
                    })
                }
            })
            .collect();
        let fresh = fresh.into_iter().map(|(k, v)| (k.clone(), v)).collect();
        (cloned, fresh)
    }

    /// Adds every present member of each group that `ids` touches.
    pub fn expand_groups(&self, ids: &mut Vec<Id>) {
        if self.groups.is_empty() {
            return;
        }
        let touched: Vec<&Vec<Id>> = self
            .groups
            .iter()
            .filter(|g| g.iter().any(|m| ids.contains(m)))
            .collect();
        for m in touched.into_iter().flatten() {
            if !ids.contains(m) && self.get(m).is_some() {
                ids.push(m.clone());
            }
        }
    }

    /// Adds every element wholly inside each frame in `ids`.
    pub fn expand_frames(&self, ids: &mut Vec<Id>) {
        let frames: Vec<(i32, i32, i32, i32)> = ids
            .iter()
            .filter_map(|id| match self.get(id) {
                Some(Element::Shape(s)) if s.kind == ShapeKind::Frame => Some((s.x, s.y, s.w, s.h)),
                _ => None,
            })
            .collect();
        if frames.is_empty() {
            return;
        }
        let find = self.shape_lookup();
        for e in &self.elements {
            let (x, y, w, h) = self.bounds_with(e, &find);
            let inside = frames
                .iter()
                .any(|f| x >= f.0 && y >= f.1 && x + w <= f.0 + f.2 && y + h <= f.1 + f.3);
            if inside && !ids.contains(e.id()) {
                ids.push(e.id().clone());
            }
        }
    }

    /// Groups without the missing ids and without groups under two members.
    fn live_groups(&self, skip: &[Id]) -> Vec<Vec<Id>> {
        prune(self.groups.iter(), |m| {
            self.get(m).is_some() && !skip.contains(m)
        })
    }

    /// Joins the present, distinct `ids` in one group; `None` when fewer than two.
    pub fn group_cmd(&self, ids: &[Id]) -> Option<Command> {
        let mut members: Vec<Id> = vec![];
        for id in ids.iter().filter(|id| self.get(id).is_some()) {
            if !members.contains(id) {
                members.push(id.clone());
            }
        }
        if members.len() < 2 {
            return None;
        }
        let mut groups = self.live_groups(&members);
        groups.push(members);
        (groups != self.groups).then_some(Command::SetGroups(groups))
    }

    /// Dissolves every group that holds an id of `ids`; `None` when none does.
    pub fn ungroup_cmd(&self, ids: &[Id]) -> Option<Command> {
        let touches = |g: &Vec<Id>| g.iter().any(|m| ids.contains(m));
        if !self.groups.iter().any(touches) {
            return None;
        }
        let rest = self.groups.iter().filter(|g| !touches(g));
        Some(Command::SetGroups(prune(rest, |m| self.get(m).is_some())))
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
            End::Bound(id, _) => shapes_in.contains(&id),
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
            .collect::<Vec<Element>>();
        let groups = prune(self.groups.iter(), |m| kept.iter().any(|e| e.id() == m));
        Canvas {
            version: self.version,
            elements: kept,
            groups,
        }
    }
}

fn prune<'a>(
    groups: impl Iterator<Item = &'a Vec<Id>>,
    keep: impl Fn(&Id) -> bool,
) -> Vec<Vec<Id>> {
    groups
        .map(|g| g.iter().filter(|m| keep(m)).cloned().collect::<Vec<_>>())
        .filter(|g| g.len() > 1)
        .collect()
}
