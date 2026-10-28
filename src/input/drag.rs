use super::hit::{self, swept, Handle};
use super::state::{click, Action, Gesture, Target, P};
use super::{Session, Tool};
use crate::model::{
    edge_anchor, label_at_nearest, side_at, side_mid, Anchor, Canvas, Command, Edge, Element, End,
    Id, Line, LineKind, Shape, ShapeKind, Side, SideLabel, MIN_SIZE,
};
use std::collections::HashSet;

/// Cells the pointer must travel from the press before a drag starts.
const DRAG_THRESHOLD: i32 = 2;
/// Cells within which a moved selection snaps to another shape's edge or center.
const GUIDE_RANGE: i32 = 1;

fn run(s: &mut Session, inv: &mut Vec<Command>, cmd: Command) {
    if let Ok(undo) = s.canvas.apply(cmd) {
        fold(inv, undo);
        s.rev += 1;
    }
}

/// Keeps `inv` constant-size per drag: the first `SetRect`/`SetEnd` inverse already holds the start value.
fn fold(inv: &mut Vec<Command>, undo: Command) {
    let redundant = match &undo {
        Command::SetRect { id, .. } => inv
            .iter()
            .any(|c| matches!(c, Command::SetRect { id: i, .. } if i == id)),
        Command::SetLabelAt { id, .. } => inv
            .iter()
            .any(|c| matches!(c, Command::SetLabelAt { id: i, .. } if i == id)),
        Command::SetSide { id, edge, .. } => inv
            .iter()
            .any(|c| matches!(c, Command::SetSide { id: i, edge: e, .. } if i == id && e == edge)),
        Command::SetEnd { id, side, .. } => inv
            .iter()
            .any(|c| matches!(c, Command::SetEnd { id: i, side: d, .. } if i == id && d == side)),
        Command::SetBends { id, .. } => inv
            .iter()
            .any(|c| matches!(c, Command::SetBends { id: i, .. } if i == id)),
        _ => false,
    };
    if redundant {
        return;
    }
    if let (Command::Batch(new), Some(Command::Batch(last))) = (&undo, inv.last_mut()) {
        if add_moves(last, new) {
            return;
        }
    }
    inv.push(undo);
}

/// Sums `new` into `last` when both are non-empty `Move` lists over the same ids.
fn add_moves(last: &mut [Command], new: &[Command]) -> bool {
    let same = !new.is_empty()
        && last.len() == new.len()
        && last.iter().zip(new).all(|(a, b)| {
            matches!((a, b), (Command::Move { id: x, .. }, Command::Move { id: y, .. }) if x == y)
        });
    if same {
        for (a, b) in last.iter_mut().zip(new) {
            if let (Command::Move { dx, dy, .. }, Command::Move { dx: bx, dy: by, .. }) = (a, b) {
                *dx += bx;
                *dy += by;
            }
        }
    }
    same
}

fn span(fixed: i32, cursor: i32) -> (i32, i32) {
    let len = ((cursor - fixed).abs() + 1).max(MIN_SIZE);
    (
        if cursor < fixed {
            fixed - len + 1
        } else {
            fixed
        },
        len,
    )
}

/// Moves `cursor` onto the longer axis of its offset from `start`.
fn axis_lock(start: P, cursor: P) -> P {
    if (cursor.0 - start.0).abs() >= (cursor.1 - start.1).abs() {
        (cursor.0, start.1)
    } else {
        (start.0, cursor.1)
    }
}

/// Pushes `cursor` out to the longer side of the box it makes with `fixed`.
fn square(fixed: P, cursor: P) -> P {
    let (dx, dy) = (cursor.0 - fixed.0, cursor.1 - fixed.1);
    let n = dx.abs().max(dy.abs());
    (
        fixed.0 + n * if dx < 0 { -1 } else { 1 },
        fixed.1 + n * if dy < 0 { -1 } else { 1 },
    )
}

fn snap_to(grid: i32, v: i32) -> i32 {
    (v + grid / 2).div_euclid(grid) * grid
}

fn snap_p(grid: i32, p: P) -> P {
    (snap_to(grid, p.0), snap_to(grid, p.1))
}

/// Min edge, center and max edge of a span; the center matches `geom::center`.
fn marks(at: i32, len: i32) -> [i32; 3] {
    [at, at + len / 2, at + len - 1]
}

/// The correction and matched coordinate of the closest pair within `GUIDE_RANGE`; the first pair wins a tie.
fn nearest(mine: [i32; 3], theirs: [i32; 3]) -> Option<(i32, i32)> {
    mine.iter()
        .flat_map(|m| theirs.iter().map(move |t| (t - m, *t)))
        .filter(|(d, _)| d.abs() <= GUIDE_RANGE)
        .min_by_key(|(d, _)| d.abs())
}

type Match = (i32, i32, (i32, i32, i32, i32));

fn closer(best: &mut Option<Match>, hit: Option<(i32, i32)>, other: &Shape) {
    if let Some((d, at)) = hit {
        if best.is_none_or(|b| d.abs() < b.0.abs()) {
            *best = Some((d, at, (other.x, other.y, other.w, other.h)));
        }
    }
}

/// Snaps the selection box, moved by `(dx, dy)`, to the nearest other shape per axis.
fn align(s: &Session, dx: i32, dy: i32) -> (i32, i32, Vec<(P, P)>) {
    let picked: HashSet<&Id> = s.selection.iter().collect();
    let Some((x0, y0, x1, y1)) = s
        .canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Shape(o) if picked.contains(&o.id) => Some(o),
            _ => None,
        })
        .map(|e| (e.x, e.y, e.x + e.w, e.y + e.h))
        .reduce(|a, r| (a.0.min(r.0), a.1.min(r.1), a.2.max(r.2), a.3.max(r.3)))
    else {
        return (dx, dy, vec![]);
    };
    let (mx, my, mw, mh) = (x0 + dx, y0 + dy, x1 - x0, y1 - y0);
    let (mut bx, mut by) = (None, None);
    // shortcut: linear scan, add a spatial index past ~50k shapes
    for o in s.canvas.elements.iter().filter_map(|e| match e {
        Element::Shape(o) if !picked.contains(&o.id) => Some(o),
        _ => None,
    }) {
        closer(&mut bx, nearest(marks(mx, mw), marks(o.x, o.w)), o);
        closer(&mut by, nearest(marks(my, mh), marks(o.y, o.h)), o);
    }
    let (cx, cy) = (bx.map_or(0, |b| b.0), by.map_or(0, |b| b.0));
    let (mx, my) = (mx + cx, my + cy);
    let mut guides = vec![];
    if let Some((_, at, (_, oy, _, oh))) = bx {
        guides.push(((at, my.min(oy)), (at, (my + mh - 1).max(oy + oh - 1))));
    }
    if let Some((_, at, (ox, _, ow, _))) = by {
        guides.push(((mx.min(ox), at), ((mx + mw - 1).max(ox + ow - 1), at)));
    }
    (dx + cx, dy + cy, guides)
}

/// Resizes `id` between `fixed` and `cursor`, then snaps ratio-bound kinds around `fixed`.
fn set_rect(canvas: &Canvas, id: &Id, fixed: P, cursor: P) -> Command {
    let cursor = match canvas.shape(id) {
        Some(sh) => {
            let (w, h) = (span(fixed.0, cursor.0).1, span(fixed.1, cursor.1).1);
            let (w, h) = Shape::snap_size(sh.kind, w, h);
            let sign = |f: i32, c: i32| if c < f { -1 } else { 1 };
            (
                fixed.0 + sign(fixed.0, cursor.0) * (w - 1),
                fixed.1 + sign(fixed.1, cursor.1) * (h - 1),
            )
        }
        None => cursor,
    };
    let ((x, w), (y, h)) = (span(fixed.0, cursor.0), span(fixed.1, cursor.1));
    Command::SetRect {
        id: id.clone(),
        x,
        y,
        w,
        h,
    }
}

pub(super) fn drag(s: &mut Session, screen: P, world: P) {
    let mut inv = vec![];
    let mut action = match std::mem::replace(&mut s.gesture, Gesture::Idle) {
        Gesture::Pressed {
            origin,
            world: w0,
            target,
        } => {
            if (screen.0 - origin.0).abs().max((screen.1 - origin.1).abs()) < DRAG_THRESHOLD {
                s.gesture = Gesture::Pressed {
                    origin,
                    world: w0,
                    target,
                };
                return;
            }
            let action = begin(s, &mut inv, target, origin, w0);
            unpin(s, &mut inv, &action);
            action
        }
        Gesture::Dragging { action, inv: old } => {
            inv = old;
            action
        }
        Gesture::Idle => return,
    };
    step(s, &mut inv, &mut action, screen, world);
    s.gesture = Gesture::Dragging { action, inv };
}

fn begin(s: &mut Session, inv: &mut Vec<Command>, target: Target, origin: P, w0: P) -> Action {
    match target {
        Target::Pan => Action::Pan(origin),
        Target::Element(_) => Action::Move {
            start: w0,
            last: w0,
        },
        Target::Label(id, edge) => Action::LabelDrag(id, edge),
        Target::Route(id) => {
            let Some(Element::Line(l)) = s.canvas.get(&id) else {
                return Action::Band(w0);
            };
            let at = s.canvas.bend_slot(l, w0);
            let mut bends = l.bends.clone();
            bends.insert(at, snap_p(s.grid, w0));
            run(
                s,
                inv,
                Command::SetBends {
                    id: id.clone(),
                    bends,
                },
            );
            Action::BendDrag { id, at }
        }
        Target::Handle(Handle::Bend(at), id) => Action::BendDrag { id, at },
        Target::Empty if s.shift || s.tool == Tool::Select => Action::Band(w0),
        Target::Empty => Action::Pan(origin),
        Target::Handle(Handle::End(side), id) => Action::EndDrag { id, side },
        Target::Handle(Handle::Resize, id) => {
            let fixed = s.canvas.shape(&id).map_or((0, 0), |s| (s.x, s.y));
            Action::Resize { id, fixed }
        }
        Target::Draw(tool) => {
            let id = Id::random();
            let w0 = if tool == Tool::Arrow {
                w0
            } else {
                snap_p(s.grid, w0)
            };
            let at = s.canvas.elements.len();
            let kind = match tool {
                Tool::Rect => Some(ShapeKind::Rect),
                Tool::Diamond => Some(ShapeKind::Diamond),
                Tool::Cylinder => Some(ShapeKind::Cylinder),
                Tool::Parallelogram => Some(ShapeKind::Parallelogram),
                Tool::Hexagon => Some(ShapeKind::Hexagon),
                Tool::Frame => Some(ShapeKind::Frame),
                _ => None,
            };
            let el = match kind {
                Some(kind) => Element::Shape(Shape {
                    id: id.clone(),
                    ..Shape::new(kind, w0.0, w0.1, MIN_SIZE, MIN_SIZE, "")
                }),
                None => Element::Line(Line {
                    id: id.clone(),
                    ..Line::new(
                        LineKind::Arrow,
                        bound_at(&s.canvas, w0).unwrap_or(End::Free(w0.0, w0.1)),
                        End::Free(w0.0, w0.1),
                    )
                }),
            };
            run(s, inv, Command::Insert(at, el));
            s.selection = vec![id.clone()];
            if kind.is_some() {
                Action::Create { id, origin: w0 }
            } else {
                Action::CreateLine(id)
            }
        }
    }
}

/// The ids whose lines a drag reroutes.
fn rerouted(s: &Session, action: &Action) -> Vec<Id> {
    match action {
        Action::Move { .. } => s.selection.clone(),
        Action::Resize { id, .. } | Action::EndDrag { id, .. } | Action::CreateLine(id) => {
            vec![id.clone()]
        }
        _ => vec![],
    }
}

/// Clears the bends of the lines the drag reroutes, so they follow the pointer.
fn unpin(s: &mut Session, inv: &mut Vec<Command>, action: &Action) {
    let pinned: Vec<Id> = s
        .canvas
        .touching(&rerouted(s, action))
        .into_iter()
        .filter(|l| !l.bends.is_empty())
        .map(|l| l.id.clone())
        .collect();
    for id in pinned {
        run(s, inv, Command::SetBends { id, bends: vec![] });
    }
}

fn step(s: &mut Session, inv: &mut Vec<Command>, action: &mut Action, screen: P, world: P) {
    match action {
        Action::Pan(last) => {
            s.glide = None;
            let used = s.view.pan_screen(last.0 - screen.0, last.1 - screen.1);
            *last = (last.0 - used.0, last.1 - used.1);
        }
        Action::Move { start, last } => {
            s.guides.clear();
            let world = if s.shift {
                axis_lock(*start, world)
            } else {
                world
            };
            let (mut dx, mut dy) = (world.0 - last.0, world.1 - last.1);
            let first = s.selection.iter().find_map(|id| s.canvas.shape(id));
            if let Some(sh) = first {
                // Snap the first shape's corner; the rest of the selection keeps its offsets.
                let (x0, y0) = (sh.x - (last.0 - start.0), sh.y - (last.1 - start.1));
                let want = snap_p(s.grid, (x0 + world.0 - start.0, y0 + world.1 - start.1));
                (dx, dy) = (want.0 - sh.x, want.1 - sh.y);
            }
            if !s.shift {
                (dx, dy, s.guides) = align(s, dx, dy);
            }
            if (dx, dy) != (0, 0) {
                let cmds = s
                    .selection
                    .iter()
                    .map(|id| Command::Move {
                        id: id.clone(),
                        dx,
                        dy,
                    })
                    .collect();
                run(s, inv, Command::Batch(cmds));
                *last = (last.0 + dx, last.1 + dy);
            }
            s.snap = None;
            s.snap_at = None;
            if let Some((id, side, at)) = dropped_ends(s).into_iter().next() {
                snap_to_shape(s, &id, side, at);
            }
        }
        Action::Resize { id, fixed } => {
            let world = snap_p(s.grid, world);
            let world = if s.shift {
                square(*fixed, world)
            } else {
                world
            };
            run(s, inv, set_rect(&s.canvas, id, *fixed, world))
        }
        Action::Create { id, origin } => {
            let world = snap_p(s.grid, world);
            let world = if s.shift {
                square(*origin, world)
            } else {
                world
            };
            run(s, inv, set_rect(&s.canvas, id, *origin, world))
        }
        Action::EndDrag { id, side } => {
            let end = bound_at(&s.canvas, world).unwrap_or(End::Free(world.0, world.1));
            let cmd = Command::SetEnd {
                id: id.clone(),
                side: *side,
                end,
            };
            run(s, inv, cmd);
            snap_to_shape(s, id, *side, world);
        }
        Action::BendDrag { id, at } => {
            let Some(Element::Line(l)) = s.canvas.get(id) else {
                return;
            };
            let mut bends = l.bends.clone();
            bends[*at] = snap_p(s.grid, world);
            run(
                s,
                inv,
                Command::SetBends {
                    id: id.clone(),
                    bends,
                },
            );
        }
        Action::LabelDrag(id, Some(edge)) => {
            let Some(Element::Shape(sh)) = s.canvas.get(id) else {
                return;
            };
            let Some(old) = sh.side_label(*edge) else {
                return;
            };
            let (len, offset) = match edge {
                Edge::Top | Edge::Bottom => (sh.w, world.0 - sh.x),
                Edge::Left | Edge::Right => (sh.h, world.1 - sh.y),
            };
            let label = SideLabel {
                at: Some(side_at(len - 3, offset - 1)),
                ..old.clone()
            };
            let cmd = Command::SetSide {
                id: id.clone(),
                edge: *edge,
                label: Some(label),
            };
            run(s, inv, cmd);
        }
        Action::LabelDrag(id, None) => {
            let Some(Element::Line(l)) = s.canvas.get(id) else {
                return;
            };
            let at = label_at_nearest(&s.canvas.route(l), world);
            let cmd = Command::SetLabelAt { id: id.clone(), at };
            run(s, inv, cmd);
        }
        Action::CreateLine(id) => {
            let end = bound_at(&s.canvas, world).unwrap_or(End::Free(world.0, world.1));
            let cmd = Command::SetEnd {
                id: id.clone(),
                side: Side::To,
                end,
            };
            run(s, inv, cmd);
            snap_to_shape(s, id, Side::To, world);
        }
        Action::Band(origin) => {
            s.band = Some((*origin, world));
            let canvas = &s.canvas;
            let find = canvas.shape_lookup();
            let mut hit: Vec<Id> = canvas
                .elements
                .iter()
                .filter(|e| swept(canvas, &find, e, *origin, world))
                .map(|e| e.id().clone())
                .collect();
            canvas.expand_groups(&mut hit);
            s.selection = hit;
        }
    }
}

/// Sets the shape under `world` and the cell where the dragged `side` of line `id` will attach to it.
fn snap_to_shape(s: &mut Session, id: &Id, side: Side, world: P) {
    s.snap = hit::shape_at(&s.canvas, world);
    s.snap_at = s.snap.as_ref().and_then(|sid| {
        let shape = s.canvas.shape(sid)?;
        let Some(Element::Line(l)) = s.canvas.get(id) else {
            return None;
        };
        let (from, to) = s.canvas.line_ends(l);
        let other = if side == Side::From { to } else { from };
        Some(side_mid(shape, edge_anchor(shape, world), other).0)
    });
}

pub(super) fn pan_step(s: &mut Session) {
    if let Gesture::Dragging {
        mut action,
        mut inv,
    } = std::mem::replace(&mut s.gesture, Gesture::Idle)
    {
        let world = s.view.to_world(s.cursor.0, s.cursor.1);
        step(s, &mut inv, &mut action, s.cursor, world);
        s.gesture = Gesture::Dragging { action, inv };
    }
}

pub(super) fn up(s: &mut Session, world: P) {
    s.last_pan = None;
    s.band = None;
    s.guides.clear();
    s.snap = None;
    s.snap_at = None;
    match std::mem::replace(&mut s.gesture, Gesture::Idle) {
        Gesture::Pressed {
            target, world: w0, ..
        } => click(s, target, w0),
        Gesture::Dragging { action, mut inv } => {
            match &action {
                Action::EndDrag { id, side } => bind(s, &mut inv, id, *side, world),
                Action::Move { .. } => {
                    for (id, side, at) in dropped_ends(s) {
                        bind(s, &mut inv, &id, side, at);
                    }
                }
                Action::CreateLine(id) => {
                    bind(s, &mut inv, id, Side::To, world);
                    s.tool = Tool::Pointer;
                }
                Action::Create { .. } => s.tool = Tool::Pointer,
                Action::BendDrag { id, .. } => drop_spikes(s, &mut inv, id),
                _ => {}
            }
            for cmd in s.canvas.pin_cmds(&rerouted(s, &action)) {
                run(s, &mut inv, cmd);
            }
            if !inv.is_empty() {
                inv.reverse();
                s.history.record(Command::Batch(inv));
            }
        }
        Gesture::Idle => {}
    }
}

/// A bend whose route cell sits between two equal cells makes the line double back on itself.
fn drop_spikes(s: &mut Session, inv: &mut Vec<Command>, id: &Id) {
    let Some(Element::Line(l)) = s.canvas.get(id) else {
        return;
    };
    let cells = s.canvas.route(l);
    let spike = |b: &P| cells.windows(3).any(|w| w[1] == *b && w[0] == w[2]);
    let bends: Vec<_> = l.bends.iter().copied().filter(|b| !spike(b)).collect();
    if bends.len() != l.bends.len() {
        run(
            s,
            inv,
            Command::SetBends {
                id: id.clone(),
                bends,
            },
        );
    }
}

fn bind(s: &mut Session, inv: &mut Vec<Command>, id: &Id, side: Side, world: P) {
    if let Some(end) = bound_at(&s.canvas, world) {
        run(
            s,
            inv,
            Command::SetEnd {
                id: id.clone(),
                side,
                end,
            },
        );
    }
}

/// Free ends of lines outside the selection that sit under a selected shape.
fn dropped_ends(s: &Session) -> Vec<(Id, Side, P)> {
    let selected: HashSet<&Id> = s.selection.iter().collect();
    // One pass over the elements: `canvas.shape` per selected id scans them all.
    let boxes: Vec<_> = s
        .canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Shape(sh) if selected.contains(&sh.id) => Some((sh.x, sh.y, sh.w, sh.h)),
            _ => None,
        })
        .collect();
    let inside = |&(x, y): &(i32, i32)| {
        boxes
            .iter()
            .any(|&(bx, by, w, h)| x >= bx && x < bx + w && y >= by && y < by + h)
    };
    let mut found = vec![];
    for e in &s.canvas.elements {
        let Element::Line(l) = e else { continue };
        if selected.contains(&l.id) {
            continue;
        }
        for (side, end) in [(Side::From, &l.from), (Side::To, &l.to)] {
            let End::Free(x, y) = end else { continue };
            if inside(&(*x, *y))
                && hit::shape_at(&s.canvas, (*x, *y)).is_some_and(|sid| selected.contains(&sid))
            {
                found.push((l.id.clone(), side, (*x, *y)));
            }
        }
    }
    found
}

fn bound_at(canvas: &Canvas, p: P) -> Option<End> {
    let id = hit::shape_at(canvas, p)?;
    let anchor = canvas
        .shape(&id)
        .map_or(Anchor::Auto, |s| edge_anchor(s, p));
    Some(End::Bound(id, anchor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::handle_mouse;
    use crate::model::Canvas;
    use crate::view::View;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use std::time::Instant;

    const STEPS: u16 = 200;

    fn mouse(s: &mut Session, kind: MouseEventKind, col: u16, row: u16) {
        let ev = MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        };
        handle_mouse(s, ev, Instant::now());
    }

    fn with_shape() -> Session {
        let mut s = Session::new(Canvas::default(), View::default(), (400, 40));
        s.canvas.elements.push(Element::Shape(Shape {
            id: Id::from("aaaaaaaa"),
            ..Shape::new(ShapeKind::Rect, 30, 15, 8, 4, "")
        }));
        s
    }

    fn press_and_drag(s: &mut Session, from: (u16, u16), step: impl Fn(u16) -> (u16, u16)) {
        mouse(s, MouseEventKind::Down(MouseButton::Left), from.0, from.1);
        for i in 1..=STEPS {
            let (c, r) = step(i);
            mouse(s, MouseEventKind::Drag(MouseButton::Left), c, r);
        }
    }

    fn inv(s: &Session) -> &[Command] {
        match &s.gesture {
            Gesture::Dragging { inv, .. } => inv,
            _ => panic!("not dragging"),
        }
    }

    fn jitter(s: &mut Session, from: (u16, u16)) {
        mouse(s, MouseEventKind::Down(MouseButton::Left), from.0, from.1);
        mouse(
            s,
            MouseEventKind::Drag(MouseButton::Left),
            from.0 + 1,
            from.1,
        );
        mouse(s, MouseEventKind::Up(MouseButton::Left), from.0 + 1, from.1);
    }

    #[test]
    fn a_down_after_a_lost_up_records_the_drag() {
        let mut s = Session::new(Canvas::default(), View::default(), (400, 40));
        s.tool = Tool::Rect;
        mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 20, 5);
        mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 30, 9);
        mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 60, 20);
        mouse(&mut s, MouseEventKind::Up(MouseButton::Left), 60, 20);
        assert_eq!(s.canvas.elements.len(), 1);
        assert!(s.undo().is_some());
        assert!(s.canvas.elements.is_empty());
    }

    #[test]
    fn middle_drag_pans_over_an_element_in_any_tool() {
        let mut s = with_shape();
        s.tool = Tool::Rect;
        let before = s.view;
        mouse(&mut s, MouseEventKind::Down(MouseButton::Middle), 32, 16);
        mouse(&mut s, MouseEventKind::Drag(MouseButton::Middle), 42, 20);
        mouse(&mut s, MouseEventKind::Up(MouseButton::Middle), 42, 20);
        assert_eq!(
            s.view.to_world(0, 0),
            (before.to_world(0, 0).0 - 10, before.to_world(0, 0).1 - 4)
        );
        assert_eq!(s.canvas.elements.len(), 1);
        assert_eq!(s.rev, 0);
        assert!(s.selection.is_empty());
    }

    #[test]
    fn one_cell_jitter_does_not_move() {
        let mut s = with_shape();
        jitter(&mut s, (30, 16));
        assert_eq!(s.canvas.shape(&Id::from("aaaaaaaa")).map(|e| e.x), Some(30));
        assert_eq!(s.rev, 0);
        assert_eq!(s.selection, [Id::from("aaaaaaaa")]);
    }

    #[test]
    fn one_cell_jitter_does_not_draw() {
        let mut s = with_shape();
        s.tool = Tool::Rect;
        jitter(&mut s, (100, 30));
        assert_eq!(s.canvas.elements.len(), 1);
    }

    #[test]
    fn move_drag_keeps_one_summed_batch() {
        let mut s = with_shape();
        press_and_drag(&mut s, (30, 16), |i| (30 + i, 16));
        let id = Id::from("aaaaaaaa");
        let want = Command::Batch(vec![Command::Move {
            id,
            dx: -i32::from(STEPS),
            dy: 0,
        }]);
        assert_eq!(inv(&s), [want]);
    }

    #[test]
    fn label_drag_keeps_the_first_label_at() {
        let mut s = Session::new(Canvas::default(), View::default(), (400, 40));
        let id = Id::from("cccccccc");
        s.canvas.elements.push(Element::Line(Line {
            id: id.clone(),
            label: "hi".into(),
            ..Line::new(LineKind::Arrow, End::Free(10, 16), End::Free(110, 16))
        }));
        s.selection = vec![id.clone()];
        press_and_drag(&mut s, (60, 16), |i| (60 + i % 20, 16));
        assert_eq!(inv(&s), [Command::SetLabelAt { id, at: None }]);
    }

    #[test]
    fn resize_drag_keeps_the_first_rect() {
        let mut s = with_shape();
        s.selection = vec![Id::from("aaaaaaaa")];
        press_and_drag(&mut s, (37, 18), |i| (37 + i, 18));
        let want = Command::SetRect {
            id: Id::from("aaaaaaaa"),
            x: 30,
            y: 15,
            w: 8,
            h: 4,
        };
        assert_eq!(inv(&s), [want]);
    }

    fn drag_to(s: &mut Session, from: (u16, u16), to: (u16, u16)) {
        mouse(s, MouseEventKind::Down(MouseButton::Left), from.0, from.1);
        mouse(s, MouseEventKind::Drag(MouseButton::Left), to.0, to.1);
        mouse(s, MouseEventKind::Up(MouseButton::Left), to.0, to.1);
    }

    fn rect(s: &Session, id: &str) -> (i32, i32, i32, i32) {
        let e = s.canvas.shape(&Id::from(id)).unwrap();
        (e.x, e.y, e.w, e.h)
    }

    #[test]
    fn grid_snaps_a_drawn_shape() {
        let mut s = with_shape();
        s.grid = 4;
        s.tool = Tool::Rect;
        drag_to(&mut s, (101, 31), (111, 39));
        let (x, y, w, h) = rect(&s, &s.canvas.elements[1].id().to_string());
        assert_eq!((x, y, w, h), (100, 32, 13, 9));
    }

    #[test]
    fn grid_snaps_a_moved_shape() {
        let mut s = with_shape();
        s.grid = 4;
        drag_to(&mut s, (30, 16), (38, 22));
        assert_eq!(rect(&s, "aaaaaaaa"), (40, 20, 8, 4));
    }

    #[test]
    fn grid_snaps_a_resized_shape() {
        let mut s = with_shape();
        s.grid = 4;
        s.selection = vec![Id::from("aaaaaaaa")];
        drag_to(&mut s, (37, 18), (46, 25));
        assert_eq!(rect(&s, "aaaaaaaa"), (30, 15, 19, 10));
    }

    #[test]
    fn line_draw_keeps_remove_and_the_first_end() {
        let mut s = with_shape();
        s.tool = Tool::Arrow;
        press_and_drag(&mut s, (100, 30), |i| (100 + i, 30));
        let [Command::Remove(_), Command::SetEnd {
            side: Side::To,
            end,
            ..
        }] = inv(&s)
        else {
            panic!("unexpected inverse: {:?}", inv(&s));
        };
        assert_eq!(*end, End::Free(100, 30));
    }

    #[test]
    fn line_draw_binds_the_end_over_a_shape_while_dragging() {
        let mut s = with_shape();
        s.tool = Tool::Arrow;
        press_and_drag(&mut s, (100, 30), |_| (32, 16));
        let Some(Element::Line(l)) = s.canvas.elements.last() else {
            panic!("no line");
        };
        assert!(
            matches!(&l.to, End::Bound(id, _) if *id == Id::from("aaaaaaaa")),
            "{:?}",
            l.to
        );
    }

    #[test]
    fn moving_a_shape_onto_a_free_line_end_binds_it() {
        let mut s = with_shape();
        s.canvas.elements.push(Element::Line(Line {
            id: Id::from("cccccccc"),
            ..Line::new(LineKind::Arrow, End::Free(10, 16), End::Free(60, 16))
        }));
        mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 30, 16);
        mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 60, 16);
        assert_eq!(s.snap, Some(Id::from("aaaaaaaa")));
        assert!(s.snap_at.is_some());
        mouse(&mut s, MouseEventKind::Up(MouseButton::Left), 60, 16);
        let Some(Element::Line(l)) = s.canvas.get(&Id::from("cccccccc")) else {
            panic!("no line");
        };
        assert!(
            matches!(&l.to, End::Bound(id, _) if *id == Id::from("aaaaaaaa")),
            "{:?}",
            l.to
        );
        assert_eq!(l.from, End::Free(10, 16));
    }

    fn with_neighbour() -> Session {
        let mut s = with_shape();
        s.canvas.elements.push(Element::Shape(Shape {
            id: Id::from("bbbbbbbb"),
            ..Shape::new(ShapeKind::Rect, 60, 30, 8, 4, "")
        }));
        s
    }

    fn hold(s: &mut Session, from: (u16, u16), to: (u16, u16), modifiers: KeyModifiers) {
        for (kind, (column, row)) in [
            (MouseEventKind::Down(MouseButton::Left), from),
            (MouseEventKind::Drag(MouseButton::Left), to),
        ] {
            let ev = MouseEvent {
                kind,
                column,
                row,
                modifiers,
            };
            handle_mouse(s, ev, Instant::now());
        }
    }

    #[test]
    fn guide_snaps_a_moved_shape_to_a_neighbour_edge() {
        let mut s = with_neighbour();
        hold(&mut s, (30, 16), (59, 30), KeyModifiers::NONE);
        assert_eq!(rect(&s, "aaaaaaaa"), (60, 29, 8, 4));
        assert_eq!(s.guides.len(), 2);
    }

    #[test]
    fn guide_ignores_a_neighbour_beyond_one_cell() {
        let mut s = with_neighbour();
        hold(&mut s, (30, 16), (49, 30), KeyModifiers::NONE);
        assert_eq!(rect(&s, "aaaaaaaa").0, 49);
    }

    #[test]
    fn guide_is_off_with_shift() {
        let mut s = with_neighbour();
        hold(&mut s, (30, 16), (59, 30), KeyModifiers::SHIFT);
        assert_eq!(rect(&s, "aaaaaaaa").0, 59);
        assert!(s.guides.is_empty());
    }

    #[test]
    fn guides_clear_on_release_and_cancel() {
        let mut s = with_neighbour();
        drag_to(&mut s, (30, 16), (59, 30));
        assert!(s.guides.is_empty());
        let mut s = with_neighbour();
        hold(&mut s, (30, 16), (59, 30), KeyModifiers::NONE);
        assert!(!s.guides.is_empty());
        assert!(s.cancel_gesture());
        assert!(s.guides.is_empty());
    }
}
