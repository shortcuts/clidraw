use super::Shape;

pub fn center(s: &Shape) -> (i32, i32) {
    (s.x + s.w / 2, s.y + s.h / 2)
}

// Leaves sideways from the middle row unless the target sits straight above or below.
pub fn anchor_from(s: &Shape, to: (i32, i32)) -> (i32, i32) {
    let (cx, cy) = center(s);
    let columns = s.x..s.x + s.w;
    let rows = s.y..s.y + s.h;
    if columns.contains(&to.0) && !rows.contains(&to.1) {
        return (to.0, if to.1 < s.y { s.y } else { s.y + s.h - 1 });
    }
    (if to.0 < cx { s.x } else { s.x + s.w - 1 }, cy)
}

// Arrives vertically at the middle column unless the source row crosses the shape.
pub fn anchor_to(s: &Shape, from: (i32, i32)) -> (i32, i32) {
    if (s.y..s.y + s.h).contains(&from.1) {
        return (if from.0 < s.x { s.x } else { s.x + s.w - 1 }, from.1);
    }
    (center(s).0, if from.1 < s.y { s.y } else { s.y + s.h - 1 })
}

/// Cells of the one-bend path: horizontal first, then vertical.
pub fn path(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let step = |from: i32, to: i32| if to >= from { 1 } else { -1 };
    let mut cells = vec![];
    let mut x = a.0;
    while x != b.0 {
        cells.push((x, a.1));
        x += step(a.0, b.0);
    }
    let mut y = a.1;
    while y != b.1 {
        cells.push((b.0, y));
        y += step(a.1, b.1);
    }
    cells.push(b);
    cells
}

/// Middle cell of the longest segment.
pub fn label_pos(a: (i32, i32), b: (i32, i32)) -> (i32, i32) {
    let (dx, dy) = ((b.0 - a.0).abs(), (b.1 - a.1).abs());
    if dx >= dy {
        ((a.0 + b.0) / 2, a.1)
    } else {
        (b.0, (a.1 + b.1) / 2)
    }
}
