use super::hit::{self, intersects, Corner, Handle};
use super::state::{click, Action, Gesture, Target, P};
use super::{Session, Tool};
use crate::model::{
    edge_anchor, Anchor, Canvas, Command, Element, End, Id, Line, LineKind, Shape, ShapeKind, Side,
};
use crate::ui::palette;
use std::collections::HashSet;

const MIN_SIZE: i32 = 3;
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
        Command::SetRect { id, .. } | Command::SetLabelAt { id, .. } => inv
            .iter()
            .any(|c| matches!(c, Command::SetRect { id: i, .. } if i == id)),
        Command::SetEnd { id, side, .. } => inv
            .iter()
            .any(|c| matches!(c, Command::SetEnd { id: i, side: d, .. } if i == id && d == side)),
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
    let Some((x0, y0, x1, y1)) = s
        .selection
        .iter()
        .filter_map(|id| s.canvas.shape(id))
        .map(|e| (e.x, e.y, e.x + e.w, e.y + e.h))
        .reduce(|a, r| (a.0.min(r.0), a.1.min(r.1), a.2.max(r.2), a.3.max(r.3)))
    else {
        return (dx, dy, vec![]);
    };
    let (mx, my, mw, mh) = (x0 + dx, y0 + dy, x1 - x0, y1 - y0);
    let picked: HashSet<&Id> = s.selection.iter().collect();
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

fn set_rect(id: &Id, fixed: P, cursor: P) -> Command {
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
            begin(s, &mut inv, target, origin, w0)
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
        Target::PaletteTitle => Action::Palette,
        Target::Pan => Action::Pan(origin),
        Target::Element(_) => Action::Move {
            start: w0,
            last: w0,
        },
        Target::Label(id) => Action::LabelDrag(id),
        Target::Empty => Action::Band(w0),
        Target::Handle(Handle::End(side), id) => Action::EndDrag { id, side },
        Target::Handle(Handle::Corner(c), id) => {
            let (x, y, w, h) = s
                .canvas
                .shape(&id)
                .map_or((0, 0, 1, 1), |s| (s.x, s.y, s.w, s.h));
            let fixed = match c {
                Corner::Nw => (x + w - 1, y + h - 1),
                Corner::Ne => (x, y + h - 1),
                Corner::Sw => (x + w - 1, y),
                Corner::Se => (x, y),
            };
            Action::Resize { id, fixed }
        }
        Target::Draw(tool) => {
            let id = Id::random();
            let w0 = if matches!(tool, Tool::Line | Tool::Arrow) {
                w0
            } else {
                snap_p(s.grid, w0)
            };
            let at = s.canvas.elements.len();
            let kind = match tool {
                Tool::Rect => Some(ShapeKind::Rect),
                Tool::Ellipse => Some(ShapeKind::Ellipse),
                Tool::Diamond => Some(ShapeKind::Diamond),
                Tool::Cylinder => Some(ShapeKind::Cylinder),
                Tool::Parallelogram => Some(ShapeKind::Parallelogram),
                Tool::Hexagon => Some(ShapeKind::Hexagon),
                _ => None,
            };
            let el = match kind {
                Some(kind) => Element::Shape(Shape {
                    id: id.clone(),
                    kind,
                    x: w0.0,
                    y: w0.1,
                    w: MIN_SIZE,
                    h: MIN_SIZE,
                    label: String::new(),
                    stroke: Default::default(),
                }),
                None => Element::Line(Line {
                    id: id.clone(),
                    kind: if tool == Tool::Arrow {
                        LineKind::Arrow
                    } else {
                        LineKind::Line
                    },
                    from: bound_at(&s.canvas, w0).unwrap_or(End::Free(w0.0, w0.1)),
                    to: End::Free(w0.0, w0.1),
                    label: String::new(),
                    label_at: None,
                    stroke: Default::default(),
                    routing: Default::default(),
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

fn step(s: &mut Session, inv: &mut Vec<Command>, action: &mut Action, screen: P, world: P) {
    match action {
        Action::Palette => {}
        Action::Pan(last) => {
            s.view.pan(last.0 - screen.0, last.1 - screen.1);
            *last = screen;
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
        }
        Action::Resize { id, fixed } => {
            let world = snap_p(s.grid, world);
            let world = if s.shift {
                square(*fixed, world)
            } else {
                world
            };
            run(s, inv, set_rect(id, *fixed, world))
        }
        Action::Create { id, origin } => {
            let world = snap_p(s.grid, world);
            let world = if s.shift {
                square(*origin, world)
            } else {
                world
            };
            run(s, inv, set_rect(id, *origin, world))
        }
        Action::EndDrag { id, side } => {
            s.snap = hit::shape_at(&s.canvas, world);
            let end = End::Free(world.0, world.1);
            let cmd = Command::SetEnd {
                id: id.clone(),
                side: *side,
                end,
            };
            run(s, inv, cmd);
        }
        Action::LabelDrag(id) => {
            let Some(Element::Line(l)) = s.canvas.get(id) else {
                return;
            };
            let cells = s.canvas.route(l);
            let near = |c: &P| (c.0 - world.0).abs() + (c.1 - world.1).abs();
            let at = cells
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| near(c))
                .map(|(i, _)| i);
            let last = cells.len().saturating_sub(1).max(1);
            let at = at.map(|i| ((i * 1000 + last / 2) / last) as u16);
            let cmd = Command::SetLabelAt { id: id.clone(), at };
            run(s, inv, cmd);
        }
        Action::CreateLine(id) => {
            s.snap = hit::shape_at(&s.canvas, world);
            let end = End::Free(world.0, world.1);
            let cmd = Command::SetEnd {
                id: id.clone(),
                side: Side::To,
                end,
            };
            run(s, inv, cmd);
        }
        Action::Band(origin) => {
            s.band = Some((*origin, world));
            let canvas = &s.canvas;
            let find = canvas.shape_lookup();
            let mut hit: Vec<Id> = canvas
                .elements
                .iter()
                .filter(|e| intersects(canvas, &find, e, *origin, world))
                .map(|e| e.id().clone())
                .collect();
            canvas.expand_groups(&mut hit);
            s.selection = hit;
        }
    }
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

pub(super) fn up(s: &mut Session, screen: P, world: P) {
    s.last_pan = None;
    s.band = None;
    s.guides.clear();
    s.snap = None;
    match std::mem::replace(&mut s.gesture, Gesture::Idle) {
        Gesture::Pressed {
            target, world: w0, ..
        } => click(s, target, w0),
        Gesture::Dragging { action, mut inv } => {
            match &action {
                Action::Palette => s.dock = palette::nearest_dock(screen.0, screen.1, s.screen),
                Action::EndDrag { id, side } => bind(s, &mut inv, id, *side, world),
                Action::CreateLine(id) => {
                    bind(s, &mut inv, id, Side::To, world);
                    s.tool = Tool::Select;
                }
                Action::Create { .. } => s.tool = Tool::Select,
                _ => {}
            }
            if !inv.is_empty() {
                inv.reverse();
                s.history.record(Command::Batch(inv));
            }
        }
        Gesture::Idle => {}
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
            kind: ShapeKind::Rect,
            x: 30,
            y: 15,
            w: 8,
            h: 4,
            label: String::new(),
            stroke: Default::default(),
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
        jitter(&mut s, (32, 16));
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
        press_and_drag(&mut s, (32, 16), |i| (32 + i, 16));
        let id = Id::from("aaaaaaaa");
        let want = Command::Batch(vec![Command::Move {
            id,
            dx: -i32::from(STEPS),
            dy: 0,
        }]);
        assert_eq!(inv(&s), [want]);
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
        drag_to(&mut s, (32, 16), (40, 22));
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
        s.tool = Tool::Line;
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

    fn with_neighbour() -> Session {
        let mut s = with_shape();
        s.canvas.elements.push(Element::Shape(Shape {
            id: Id::from("bbbbbbbb"),
            kind: ShapeKind::Rect,
            x: 60,
            y: 30,
            w: 8,
            h: 4,
            label: String::new(),
            stroke: Default::default(),
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
        hold(&mut s, (32, 16), (61, 30), KeyModifiers::NONE);
        assert_eq!(rect(&s, "aaaaaaaa"), (60, 29, 8, 4));
        assert_eq!(s.guides.len(), 2);
    }

    #[test]
    fn guide_ignores_a_neighbour_beyond_one_cell() {
        let mut s = with_neighbour();
        hold(&mut s, (32, 16), (51, 30), KeyModifiers::NONE);
        assert_eq!(rect(&s, "aaaaaaaa").0, 49);
    }

    #[test]
    fn guide_is_off_with_shift() {
        let mut s = with_neighbour();
        hold(&mut s, (32, 16), (61, 30), KeyModifiers::SHIFT);
        assert_eq!(rect(&s, "aaaaaaaa").0, 59);
        assert!(s.guides.is_empty());
    }

    #[test]
    fn guides_clear_on_release_and_cancel() {
        let mut s = with_neighbour();
        drag_to(&mut s, (32, 16), (61, 30));
        assert!(s.guides.is_empty());
        let mut s = with_neighbour();
        hold(&mut s, (32, 16), (61, 30), KeyModifiers::NONE);
        assert!(!s.guides.is_empty());
        assert!(s.cancel_gesture());
        assert!(s.guides.is_empty());
    }
}
