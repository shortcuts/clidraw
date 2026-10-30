use super::geom::{center, contour, corners, route, side_anchor, side_cell, Axis};
use super::router::{grow, union};
use super::{
    Anchor, Command, Dir, Element, End, Id, Line, Router, Shape, ShapeKind, Side, Text, VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

type Stop = ((i32, i32), Option<Axis>);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Canvas {
    pub version: u32,
    pub elements: Vec<Element>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Vec<Id>>,
    #[serde(default, skip_serializing_if = "Dir::is_td")]
    pub dir: Dir,
}

impl Default for Canvas {
    fn default() -> Canvas {
        Canvas {
            version: VERSION,
            elements: vec![],
            groups: vec![],
            dir: Dir::TopDown,
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

    /// Element indices in draw order: lines draw above shapes so a crossing shows, and a frame
    /// groups visually, so it draws above everything else.
    pub fn stacking(&self) -> impl DoubleEndedIterator<Item = usize> + '_ {
        let rank = |i: usize| match &self.elements[i] {
            Element::Shape(s) if s.kind == ShapeKind::Frame => 2,
            Element::Line(_) => 1,
            _ => 0,
        };
        (0..3).flat_map(move |r| (0..self.elements.len()).filter(move |&i| rank(i) == r))
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
        let shape_of = |end: &End| match end {
            End::Bound(id, a) => find(id).map(|s| (s, *a)),
            End::Free(..) => None,
        };
        let shapes = [shape_of(&l.from), shape_of(&l.to)];
        let raw = |end: &End, found: Option<(&Shape, Anchor)>| match (end, found) {
            (End::Free(x, y), _) => (*x, *y),
            (End::Bound(..), found) => found.map_or((0, 0), |(s, _)| center(s)),
        };
        let (from, to) = (raw(&l.from, shapes[0]), raw(&l.to, shapes[1]));
        let (face_from, face_to) = (
            l.bends.first().copied().unwrap_or(to),
            l.bends.last().copied().unwrap_or(from),
        );
        // A pinned far end counts by its outer cell, so a shape inside the other still leaves outward.
        let face = |bend: Option<(i32, i32)>, far: Option<(&Shape, Anchor)>, toward, other| {
            bend.or_else(|| {
                let (s, a) = far.filter(|&(_, a)| a != Anchor::Auto)?;
                Some(side_cell(s, a, other, (i32::MIN, i32::MIN)).0)
            })
            .unwrap_or(toward)
        };
        let anchor = |found: Option<(&Shape, Anchor)>, own, toward, face| match found {
            Some((s, a)) => {
                let (p, ax) = side_cell(s, a, toward, face);
                (p, Some(ax))
            }
            None => (own, None),
        };
        [
            anchor(
                shapes[0],
                from,
                face_from,
                face(l.bends.first().copied(), shapes[1], face_from, from),
            ),
            anchor(
                shapes[1],
                to,
                face_to,
                face(l.bends.last().copied(), shapes[0], face_to, to),
            ),
        ]
    }

    /// Cells of the line from start to end. A line with bends routes plain; one without goes around obstacles.
    pub fn route(&self, l: &Line) -> Vec<(i32, i32)> {
        self.route_with(l, &|id| self.shape(id), &self.router())
    }

    pub fn route_with<'a>(&'a self, l: &Line, find: &Find<'a>, router: &Router) -> Vec<(i32, i32)> {
        if let Some(path) = router.settled(self, find, &l.id, <[_]>::to_vec) {
            return path;
        }
        let (path, foot, deps, cross) = if l.bends.is_empty() {
            router.around(
                self,
                find,
                l,
                self.anchors(l, find),
                self.plain_with(l, find),
            )
        } else {
            let path = self.plain_with(l, find);
            let foot = path
                .iter()
                .map(|&(x, y)| (x, y, 1, 1))
                .reduce(union)
                .map(|r| grow(r, 1))
                .expect("a route has a cell");
            (path, foot, vec![], 0)
        };
        router.store(l.id.clone(), path.clone(), foot, deps, cross);
        path
    }

    /// The route ignoring obstacles, except that a bent line contours its own end shapes.
    pub(crate) fn plain_with<'a>(&'a self, l: &Line, find: &Find<'a>) -> Vec<(i32, i32)> {
        let mut cells: Vec<(i32, i32)> = vec![];
        for leg in self.legs(l, find) {
            cells.extend(leg.into_iter().skip(usize::from(!cells.is_empty())));
        }
        cells
    }

    /// Cells of each leg between consecutive stops: ends and bends.
    fn legs<'a>(&'a self, l: &Line, find: &Find<'a>) -> Vec<Vec<(i32, i32)>> {
        let [a, b] = self.anchors(l, find);
        let mut stops = vec![a];
        stops.extend(l.bends.iter().map(|&p| (p, None)));
        stops.push(b);
        let mut legs: Vec<_> = stops
            .windows(2)
            .map(|w| route(w[0].0, w[0].1, w[1].0, w[1].1))
            .collect();
        if let (Some(&first), Some(&last)) = (l.bends.first(), l.bends.last()) {
            let end_leg = |end: &End, (p, ax): Stop, bend| match (end, ax) {
                (End::Bound(id, _), Some(ax)) => find(id).map(|s| contour(s, p, ax, bend)),
                _ => None,
            };
            if let Some(cells) = end_leg(&l.from, a, first) {
                legs[0] = cells;
            }
            if let Some(mut cells) = end_leg(&l.to, b, last) {
                cells.reverse();
                *legs.last_mut().expect("a line has a leg") = cells;
            }
        }
        legs
    }

    /// Index in `l.bends` where a bend dragged from cell `p` belongs: the leg that holds `p`, else the nearest leg.
    pub fn bend_slot(&self, l: &Line, p: (i32, i32)) -> usize {
        let find = |id: &Id| self.shape(id);
        let gap = |c: &(i32, i32)| (c.0 - p.0).abs().max((c.1 - p.1).abs());
        self.legs(l, &find)
            .iter()
            .enumerate()
            .min_by_key(|(_, leg)| leg.iter().map(gap).min())
            .map_or(0, |(i, _)| i)
    }

    #[allow(clippy::type_complexity)]
    /// Dot cells as `(Some((slot, pos)), cell)` for a bend insert, `(None, cell)` for moving the whole line.
    /// A bent line gets the middle of each leg; an unbent line gets a move dot flanked by two insert dots.
    pub fn mid_cells<'a>(
        &'a self,
        l: &Line,
        find: &Find<'a>,
        router: &Router,
    ) -> Vec<(Option<(usize, usize)>, (i32, i32))> {
        if l.bends.is_empty() {
            let leg = self.route_with(l, find, router);
            let n = leg.len();
            let mut dots = vec![];
            if n >= 5 {
                dots.push((Some((0, n / 4)), leg[n / 4]));
            }
            if n >= 3 {
                dots.push((None, leg[n / 2]));
            }
            if n >= 5 {
                dots.push((Some((0, 3 * n / 4)), leg[3 * n / 4]));
            }
            return dots;
        }
        self.legs(l, find)
            .iter()
            .enumerate()
            .filter(|(_, leg)| leg.len() >= 3)
            .map(|(i, leg)| (Some((i, leg.len() / 2)), leg[leg.len() / 2]))
            .collect()
    }

    /// Lines in `ids` and lines bound to a shape in `ids`.
    pub fn touching<'a>(&'a self, ids: &[Id]) -> Vec<&'a Line> {
        let ids: std::collections::HashSet<&Id> = ids.iter().collect();
        let bound = |e: &End| matches!(e, End::Bound(id, _) if ids.contains(id));
        self.elements
            .iter()
            .filter_map(|e| match e {
                Element::Line(l) if ids.contains(&l.id) || bound(&l.from) || bound(&l.to) => {
                    Some(l)
                }
                _ => None,
            })
            .collect()
    }

    /// One `SetBends` per line `touching` `ids` that stores its automatic route as bends.
    pub fn pin_cmds(&self, ids: &[Id]) -> Vec<Command> {
        let find = self.shape_lookup();
        let router = self.router();
        self.touching(ids)
            .into_iter()
            .filter_map(|l| {
                let bare = Line {
                    bends: vec![],
                    ..l.clone()
                };
                let cells = self.route_with(&bare, &find, &router);
                let turns = corners(&cells);
                let mut bends = turns[1..turns.len().saturating_sub(1).max(1)].to_vec();
                // An unbent line reroutes on its own, so a straight one keeps a collinear bend.
                if bends.is_empty() && cells.len() >= 3 {
                    bends.push(cells[cells.len() / 2]);
                }
                let pinned = Line {
                    bends: bends.clone(),
                    ..l.clone()
                };
                // An `Auto` end picks its side from the first bend, which can differ from the fresh route.
                let mut cmds = vec![];
                if self.plain_with(&pinned, &find) != cells {
                    for (side, end, cell) in [
                        (Side::From, &l.from, cells[0]),
                        (Side::To, &l.to, cells[cells.len() - 1]),
                    ] {
                        if let End::Bound(id, Anchor::Auto) = end {
                            if let Some(s) = find(id) {
                                let end = End::Bound(id.clone(), side_anchor(s, cell));
                                cmds.push(Command::SetEnd {
                                    id: l.id.clone(),
                                    side,
                                    end,
                                });
                            }
                        }
                    }
                }
                if bends != l.bends {
                    cmds.push(Command::SetBends {
                        id: l.id.clone(),
                        bends,
                    });
                }
                match cmds.len() {
                    0 => None,
                    1 => cmds.pop(),
                    _ => Some(Command::Batch(cmds)),
                }
            })
            .collect()
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

    pub(crate) fn line_ends_with<'a>(
        &'a self,
        l: &Line,
        find: &Find<'a>,
    ) -> ((i32, i32), (i32, i32)) {
        let [(a, _), (b, _)] = self.anchors(l, find);
        (a, b)
    }

    /// Smallest box (x, y, w, h) holding the element. A line's box spans its end cells and bends, not its detour.
    pub fn bounds(&self, e: &Element) -> (i32, i32, i32, i32) {
        self.bounds_with(e, &|id| self.shape(id))
    }

    pub fn bounds_with<'a>(&'a self, e: &Element, find: &Find<'a>) -> (i32, i32, i32, i32) {
        match e {
            Element::Shape(s) => (s.x, s.y, s.w, s.h),
            Element::Text(t) => (t.x, t.y, t.width(), 1),
            Element::Line(l) => {
                let (a, b) = self.line_ends_with(l, find);
                let pts = || l.bends.iter().copied().chain([a, b]);
                let (x, y) = (
                    pts().map(|p| p.0).min().unwrap(),
                    pts().map(|p| p.1).min().unwrap(),
                );
                let (r, d) = (
                    pts().map(|p| p.0).max().unwrap(),
                    pts().map(|p| p.1).max().unwrap(),
                );
                (x, y, r - x + 1, d - y + 1)
            }
        }
    }

    /// Like `bounds_with`, but a line's box spans the route it takes.
    pub fn extent_with<'a>(
        &'a self,
        e: &Element,
        find: &Find<'a>,
        router: &Router,
    ) -> (i32, i32, i32, i32) {
        let Element::Line(l) = e else {
            return self.bounds_with(e, find);
        };
        self.route_with(l, find, router)
            .into_iter()
            .map(|(x, y)| (x, y, 1, 1))
            .reduce(union)
            .expect("a route has a cell")
    }

    pub fn all_bounds(&self) -> Option<(i32, i32, i32, i32)> {
        self.all_bounds_with(&self.shape_lookup(), &self.router())
    }

    pub fn all_bounds_with<'a>(
        &'a self,
        find: &Find<'a>,
        router: &Router,
    ) -> Option<(i32, i32, i32, i32)> {
        self.elements
            .iter()
            .map(|e| self.extent_with(e, find, router))
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
            dir: Dir::TopDown,
        }
    }

    fn clone_mapped(&self, ids: &[Id], dx: i32, dy: i32) -> (Vec<Element>, HashMap<Id, Id>) {
        let ids: HashSet<&Id> = ids.iter().collect();
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
                        bends: l.bends.iter().map(|p| (p.0 + dx, p.1 + dy)).collect(),
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
        let (find, router) = (self.shape_lookup(), self.router());
        let inside = |e: &Element| {
            let (x, y, w, h) = self.extent_with(e, &find, &router);
            x >= ax && y >= ay && x + w <= ax + aw && y + h <= ay + ah
        };
        let shapes_in: HashSet<&Id> = self
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
        let kept_ids: HashSet<&Id> = kept.iter().map(|e| e.id()).collect();
        let groups = prune(self.groups.iter(), |m| kept_ids.contains(m));
        Canvas {
            version: self.version,
            elements: kept,
            groups,
            dir: Dir::TopDown,
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
