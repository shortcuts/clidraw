use super::hit::{self, intersects, Corner, Handle};
use super::state::{click, Action, Gesture, Target, P};
use super::{Session, Tool};
use crate::model::{Command, Element, End, Id, Line, LineKind, Shape, ShapeKind, Side};
use crate::ui::palette;

const MIN_SIZE: i32 = 3;

fn run(s: &mut Session, inv: &mut Vec<Command>, cmd: Command) {
    if let Ok(undo) = s.canvas.apply(cmd) {
        inv.push(undo);
        s.rev += 1;
    }
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
            if origin == screen {
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
        Target::Element(_) => Action::Move(w0),
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
                    from: hit::shape_at(&s.canvas, w0).map_or(End::Free(w0.0, w0.1), End::Bound),
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
        Action::Move(last) => {
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
        Action::Resize { id, fixed } => run(s, inv, set_rect(id, *fixed, world)),
        Action::Create { id, origin } => run(s, inv, set_rect(id, *origin, world)),
        Action::EndDrag { id, side } => run(
            s,
            inv,
            Command::SetEnd {
                id: id.clone(),
                side: *side,
                end: End::Free(world.0, world.1),
            },
        ),
        Action::CreateLine(id) => run(
            s,
            inv,
            Command::SetEnd {
                id: id.clone(),
                side: Side::To,
                end: End::Free(world.0, world.1),
            },
        ),
        Action::Band(origin) => {
            s.band = Some((*origin, world));
            let canvas = &s.canvas;
            s.selection = canvas
                .elements
                .iter()
                .filter(|e| intersects(canvas, e, *origin, world))
                .map(|e| e.id().clone())
                .collect();
        }
    }
}

pub(super) fn up(s: &mut Session, screen: P, world: P) {
    s.band = None;
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
    if let Some(shape) = hit::shape_at(&s.canvas, world) {
        run(
            s,
            inv,
            Command::SetEnd {
                id: id.clone(),
                side,
                end: End::Bound(shape),
            },
        );
    }
}
