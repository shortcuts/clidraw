use super::hit::{self, intersects, Corner, Handle};
use super::state::{click, Action, Gesture, Target, P};
use super::{Session, Tool};
use crate::model::{
    edge_anchor, Anchor, Canvas, Command, Element, End, Id, Line, LineKind, Shape, ShapeKind, Side,
};
use crate::ui::palette;

const MIN_SIZE: i32 = 3;
/// Cells the pointer must travel from the press before a drag starts.
const DRAG_THRESHOLD: i32 = 2;

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
            let at = s.canvas.elements.len();
            let kind = match tool {
                Tool::Rect => Some(ShapeKind::Rect),
                Tool::Ellipse => Some(ShapeKind::Ellipse),
                Tool::Diamond => Some(ShapeKind::Diamond),
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
            let world = if s.shift {
                axis_lock(*start, world)
            } else {
                world
            };
            let (dx, dy) = (world.0 - last.0, world.1 - last.1);
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
                *last = world;
            }
        }
        Action::Resize { id, fixed } => {
            let world = if s.shift {
                square(*fixed, world)
            } else {
                world
            };
            run(s, inv, set_rect(id, *fixed, world))
        }
        Action::Create { id, origin } => {
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
            s.selection = canvas
                .elements
                .iter()
                .filter(|e| intersects(canvas, &find, e, *origin, world))
                .map(|e| e.id().clone())
                .collect();
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
}
