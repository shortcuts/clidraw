use crate::model::{Canvas, Element, Id, Shape};
use std::collections::HashMap;

/// Shape index and element bounds, reused until the canvas changes.
pub struct Layout {
    key: (u64, usize),
    index: HashMap<Id, usize>,
    pub bounds: Vec<(i32, i32, i32, i32)>,
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
pub fn refresh<'a>(slot: &'a mut Option<Layout>, canvas: &Canvas, rev: u64) -> &'a Layout {
    let key = (rev, canvas.elements.len());
    if slot.as_ref().is_none_or(|l| l.key != key) {
        let mut layout = Layout {
            key,
            index: canvas.index_map(),
            bounds: vec![],
        };
        layout.bounds = {
            let find = layout.find(canvas);
            canvas
                .elements
                .iter()
                .map(|e| canvas.bounds_with(e, &find))
                .collect()
        };
        *slot = Some(layout);
    }
    slot.as_ref().expect("filled above")
}
