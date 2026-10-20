use super::drag::{drag, up};
use super::hit::{self, Handle, Hit};
use super::{Session, Tool};
use crate::model::{Command, Element, Id, ShapeKind, Side};
use crate::ui::palette::{self, Choice, PaletteHit};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use std::time::Instant;

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

/// Screen offset one wheel notch pans the view by.
pub fn scroll_step(kind: MouseEventKind) -> Option<(i32, i32)> {
    match kind {
        MouseEventKind::ScrollUp => Some((0, -2)),
        MouseEventKind::ScrollDown => Some((0, 2)),
        MouseEventKind::ScrollLeft => Some((-4, 0)),
        MouseEventKind::ScrollRight => Some((4, 0)),
        _ => None,
    }
}

pub fn handle_mouse(s: &mut Session, ev: MouseEvent, now: Instant) {
    let (col, row) = (i32::from(ev.column), i32::from(ev.row));
    if let Some((dx, dy)) = scroll_step(ev.kind) {
        return s.view.pan(dx, dy);
    }
    let world = s.view.to_world(col, row);
    match ev.kind {
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
        .is_some_and(|(c, r, t)| (c, r) == screen && now.duration_since(t) <= s.double_click);
    s.last_down = (!double).then_some((screen.0, screen.1, now));
    s.gesture = Gesture::Idle;
    if let Some(h) = palette::hit(s, screen.0, screen.1) {
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
        PaletteHit::Option(Choice::Head(kind)) => {
            let cmds: Vec<Command> = s
                .selection
                .iter()
                .filter(|id| matches!(s.canvas.get(id), Some(Element::Line(_))))
                .map(|id| Command::SetLineKind {
                    id: id.clone(),
                    kind,
                })
                .collect();
            s.exec(Command::Batch(cmds));
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
            match kind {
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
                self.canvas.retain_present(&mut self.selection);
                true
            }
        }
    }
}
