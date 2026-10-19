use super::drag::{drag, up};
use super::hit::{self, Handle, Hit};
use super::{Session, Tool};
use crate::model::{Command, Element, Id, LineKind, ShapeKind, Side};
use crate::ui::palette::{self, PaletteHit};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use std::time::{Duration, Instant};

const DOUBLE_CLICK: Duration = Duration::from_millis(400);

pub(super) type P = (i32, i32);

pub(super) enum Gesture {
    Idle,
    Pressed { origin: P, world: P, target: Target },
    Dragging { action: Action, inv: Vec<Command> },
}

pub(super) enum Target {
    PaletteTitle,
    Handle(Handle, Id),
    Element(Id),
    Empty,
    Draw(Tool),
    Pan,
}

pub(super) enum Action {
    Palette,
    Pan(P),
    Move(P),
    Resize { id: Id, fixed: P },
    EndDrag { id: Id, side: Side },
    Band(P),
    Create { id: Id, origin: P },
    CreateLine(Id),
}

pub fn handle_mouse(s: &mut Session, ev: MouseEvent, now: Instant) {
    let (col, row) = (i32::from(ev.column), i32::from(ev.row));
    let world = s.view.to_world(col, row);
    match ev.kind {
        MouseEventKind::ScrollUp => s.view.pan(0, -2),
        MouseEventKind::ScrollDown => s.view.pan(0, 2),
        MouseEventKind::ScrollLeft => s.view.pan(-4, 0),
        MouseEventKind::ScrollRight => s.view.pan(4, 0),
        MouseEventKind::Down(MouseButton::Left) => down(s, (col, row), world, now),
        // Some terminals report the wrong button on Drag and Up.
        MouseEventKind::Drag(_) => drag(s, (col, row), world),
        MouseEventKind::Up(_) => up(s, (col, row), world),
        _ => {}
    }
}

fn down(s: &mut Session, screen: P, world: P, now: Instant) {
    s.commit_edit();
    let double = s
        .last_down
        .is_some_and(|(c, r, t)| (c, r) == screen && now.duration_since(t) <= DOUBLE_CLICK);
    s.last_down = (!double).then_some((screen.0, screen.1, now));
    s.gesture = Gesture::Idle;
    if let Some(h) = palette::hit(s.dock, s.screen, screen.0, screen.1) {
        return palette_down(s, h);
    }
    let hit = hit::test(&s.canvas, &s.selection, world);
    if double {
        return match hit {
            Hit::Element(id) | Hit::Handle(_, id) => s.start_edit_existing(id),
            Hit::Empty => s.start_edit_new(world),
        };
    }
    let target = match (hit, s.tool) {
        (Hit::Handle(h, id), _) => Target::Handle(h, id),
        (_, Tool::Hand) => Target::Pan,
        (_, Tool::Line | Tool::Arrow) => Target::Draw(s.tool),
        (Hit::Element(id), _) => {
            if !s.selection.contains(&id) {
                s.selection = vec![id.clone()];
            }
            Target::Element(id)
        }
        (Hit::Empty, Tool::Rect | Tool::Ellipse | Tool::Diamond) => Target::Draw(s.tool),
        (Hit::Empty, _) => Target::Empty,
    };
    s.gesture = Gesture::Pressed {
        origin: screen,
        world,
        target,
    };
}

fn palette_down(s: &mut Session, h: PaletteHit) {
    match h {
        PaletteHit::Title => {
            s.gesture = Gesture::Pressed {
                origin: (0, 0),
                world: (0, 0),
                target: Target::PaletteTitle,
            }
        }
        PaletteHit::Tool(t) => {
            let kind = match t {
                Tool::Rect => Some(ShapeKind::Rect),
                Tool::Ellipse => Some(ShapeKind::Ellipse),
                Tool::Diamond => Some(ShapeKind::Diamond),
                _ => None,
            };
            let shapes: Vec<Id> = s
                .selection
                .iter()
                .filter(|id| s.canvas.shape(id).is_some())
                .cloned()
                .collect();
            let lines: Vec<(Id, LineKind)> = s
                .selection
                .iter()
                .filter_map(|id| match s.canvas.get(id) {
                    Some(Element::Line(l)) => Some((id.clone(), l.kind)),
                    _ => None,
                })
                .collect();
            match kind {
                _ if matches!(t, Tool::Line | Tool::Arrow) && !lines.is_empty() => {
                    let cmds = lines
                        .into_iter()
                        .map(|(id, old)| {
                            let kind = match (t, old) {
                                (Tool::Arrow, LineKind::Arrow) => LineKind::ArrowStart,
                                (Tool::Arrow, _) => LineKind::Arrow,
                                _ => LineKind::Line,
                            };
                            Command::SetLineKind { id, kind }
                        })
                        .collect();
                    s.exec(Command::Batch(cmds));
                }
                Some(kind) if !shapes.is_empty() => {
                    let cmds = shapes
                        .into_iter()
                        .map(|id| Command::SetKind { id, kind })
                        .collect();
                    s.exec(Command::Batch(cmds));
                }
                _ => s.tool = t,
            }
        }
    }
}

pub(super) fn click(s: &mut Session, target: Target, world: P) {
    match target {
        Target::Element(id) => s.selection = vec![id],
        Target::Empty if s.tool == Tool::Text => {
            s.start_edit_new(world);
            s.tool = Tool::Select;
        }
        Target::Draw(_) | Target::Empty => {
            s.selection = match hit::test(&s.canvas, &[], world) {
                Hit::Element(id) => vec![id],
                _ => vec![],
            }
        }
        Target::PaletteTitle | Target::Pan | Target::Handle(..) => {}
    }
}

impl Session {
    /// Undoes a gesture in progress. Returns false when there was none.
    pub(super) fn cancel_gesture(&mut self) -> bool {
        match std::mem::replace(&mut self.gesture, Gesture::Idle) {
            Gesture::Idle => false,
            Gesture::Pressed { .. } => true,
            Gesture::Dragging { inv, .. } => {
                for undo in inv.into_iter().rev() {
                    let _ = self.canvas.apply(undo);
                }
                self.rev += 1;
                self.band = None;
                self.selection.retain(|id| self.canvas.get(id).is_some());
                true
            }
        }
    }
}
