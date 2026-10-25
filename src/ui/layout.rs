use crate::model::{Canvas, Element, Id, Router, Shape, ShapeKind, REACH};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Shape index, element bounds and obstacle index, reused until the canvas changes.
pub struct Layout {
    key: (u64, usize),
    index: HashMap<Id, usize>,
    pub bounds: Vec<(i32, i32, i32, i32)>,
    /// What a route reads from an element besides its bounds.
    sigs: Vec<u64>,
    pub router: Router,
}

fn sig(e: &Element) -> u64 {
    let mut h = DefaultHasher::new();
    e.id().hash(&mut h);
    match e {
        Element::Shape(s) => (s.kind == ShapeKind::Frame).hash(&mut h),
        Element::Line(l) => (&l.from, &l.to, &l.bends).hash(&mut h),
        Element::Text(_) => {}
    }
    h.finish()
}

/// The box a frame must draw for an element: a line also covers the reach of its detour.
pub fn seen(e: &Element, (x, y, w, h): (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    match e {
        Element::Line(_) => (x - REACH, y - REACH, w + 2 * REACH, h + 2 * REACH),
        _ => (x, y, w, h),
    }
}

impl Layout {
    pub fn find<'a>(&'a self, canvas: &'a Canvas) -> impl Fn(&Id) -> Option<&'a Shape> + 'a {
        move |id| match canvas.elements.get(*self.index.get(id)?) {
            Some(Element::Shape(s)) => Some(s),
            _ => None,
        }
    }
}

/// Rebuilds the slot when `rev` or the element count differs from the cached key.
/// Routes far from every changed element carry over to the new router.
pub fn refresh<'a>(slot: &'a mut Option<Layout>, canvas: &Canvas, rev: u64) -> &'a Layout {
    let key = (rev, canvas.elements.len());
    if slot.as_ref().is_none_or(|l| l.key != key) {
        let mut layout = Layout {
            key,
            index: canvas.index_map(),
            bounds: vec![],
            sigs: canvas.elements.iter().map(sig).collect(),
            router: Router::default(),
        };
        layout.bounds = {
            let find = layout.find(canvas);
            canvas
                .elements
                .iter()
                .map(|e| canvas.bounds_with(e, &find))
                .collect()
        };
        layout.router = Router::from_bounds(canvas, &layout.bounds);
        if let Some(mut old) = slot.take().filter(|o| o.sigs.len() == layout.sigs.len()) {
            let dirty: Vec<_> = (0..old.sigs.len())
                .filter(|&i| old.sigs[i] != layout.sigs[i] || old.bounds[i] != layout.bounds[i])
                .flat_map(|i| [old.bounds[i], layout.bounds[i]])
                .collect();
            layout.router.carry(&mut old.router, &dirty);
        }
        *slot = Some(layout);
    }
    slot.as_ref().expect("filled above")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Anchor, End, Line, LineKind};

    fn shape(id: &str, x: i32, y: i32) -> Element {
        Element::Shape(Shape {
            id: Id::from(id),
            kind: ShapeKind::Rect,
            x,
            y,
            w: 3,
            h: 3,
            label: String::new(),
            stroke: Default::default(),
            border: Default::default(),
            color: None,
            fill: None,
            align: Default::default(),
            bold: false,
            link: None,
        })
    }

    fn arrow(id: &str, from: &str, to: &str) -> Element {
        let end = |s: &str| End::Bound(Id::from(s), Anchor::Auto);
        Element::Line(Line {
            id: Id::from(id),
            kind: LineKind::Arrow,
            from: end(from),
            to: end(to),
            label: String::new(),
            label_at: None,
            stroke: Default::default(),
            bends: vec![],
            color: None,
        })
    }

    /// Routes every line through the layout of `rev`, which fills the router cache.
    fn route_all(slot: &mut Option<Layout>, canvas: &Canvas, rev: u64) {
        let layout = refresh(slot, canvas, rev);
        let find = layout.find(canvas);
        for e in &canvas.elements {
            if let Element::Line(l) = e {
                canvas.route_with(l, &find, &layout.router);
            }
        }
    }

    #[test]
    fn seen_grows_a_line_by_the_reach_and_leaves_a_shape() {
        let canvas = two_pairs();
        let mut slot = None;
        let layout = refresh(&mut slot, &canvas, 0);
        for (e, &b) in canvas.elements.iter().zip(&layout.bounds) {
            match e {
                Element::Line(_) => assert_eq!(
                    seen(e, b),
                    (b.0 - REACH, b.1 - REACH, b.2 + 2 * REACH, b.3 + 2 * REACH)
                ),
                _ => assert_eq!(seen(e, b), b),
            }
        }
    }

    fn cached(slot: &Option<Layout>, id: &str) -> bool {
        let layout = slot.as_ref().unwrap();
        layout.router.cached(&Id::from(id)).is_some()
    }

    fn two_pairs() -> Canvas {
        Canvas {
            elements: vec![
                shape("a0000000", 0, 0),
                shape("b0000000", 20, 0),
                shape("c0000000", 0, 100),
                shape("d0000000", 20, 100),
                arrow("near0000", "a0000000", "b0000000"),
                arrow("far00000", "c0000000", "d0000000"),
            ],
            ..Canvas::default()
        }
    }

    #[test]
    fn a_cached_route_survives_a_far_move_and_drops_on_a_near_move() {
        let mut canvas = two_pairs();
        let mut slot = None;
        route_all(&mut slot, &canvas, 0);
        assert!(cached(&slot, "near0000") && cached(&slot, "far00000"));

        let Element::Shape(s) = &mut canvas.elements[3] else {
            unreachable!()
        };
        s.y += 5;
        refresh(&mut slot, &canvas, 1);
        assert!(cached(&slot, "near0000"));
        assert!(!cached(&slot, "far00000"));

        let Element::Shape(s) = &mut canvas.elements[1] else {
            unreachable!()
        };
        s.y += 2;
        refresh(&mut slot, &canvas, 2);
        assert!(!cached(&slot, "near0000"));
    }

    #[test]
    fn a_new_element_empties_the_cache() {
        let mut canvas = two_pairs();
        let mut slot = None;
        route_all(&mut slot, &canvas, 0);
        canvas.elements.push(shape("e0000000", 200, 200));
        refresh(&mut slot, &canvas, 1);
        assert!(!cached(&slot, "near0000") && !cached(&slot, "far00000"));
    }
}
