use super::list::{Anchor, List, Row};
use crate::model::ShapeKind;

const ALL: [(ShapeKind, &str); 7] = [
    (ShapeKind::Rect, "Rect"),
    (ShapeKind::Diamond, "Diamond"),
    (ShapeKind::Cylinder, "Cylinder"),
    (ShapeKind::Parallelogram, "Parallelogram"),
    (ShapeKind::Hexagon, "Hexagon"),
    (ShapeKind::Stadium, "Stadium"),
    (ShapeKind::Frame, "Frame"),
];

pub fn list(current: ShapeKind, screen_rows: i32) -> List<ShapeKind> {
    let rows = ALL
        .map(|(value, name)| Row {
            value,
            label: name.to_string(),
            key: String::new(),
        })
        .to_vec();
    let at = ALL.iter().position(|(k, _)| *k == current).unwrap_or(0);
    List::new("Change shape", rows)
        .select(at)
        .anchor(Anchor::Bottom(screen_rows - 1))
}
