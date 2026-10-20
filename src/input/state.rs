use super::drag::{drag, pan_step, up};
use super::hit::{self, Handle, Hit};
use super::{Session, Tool};
use crate::model::{Command, Element, Id, Paint, ShapeKind, Side};
use crate::ui::palette::{self, paint, Choice, PaletteHit};
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::time::{Duration, Instant};

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
    Label(Id),
    Empty,
    Draw(Tool),
    Pan,
}

pub(super) enum Action {
    Palette,
    Pan(P),
    Move { start: P, last: P },
    Resize { id: Id, fixed: P },
    EndDrag { id: Id, side: Side },
    LabelDrag(Id),
    Band(P),
    Create { id: Id, origin: P },
    CreateLine(Id),
}

const EDGE_ZONE: i32 = 2;
const EDGE_TICK: Duration = Duration::from_millis(30);
const SCROLL_BURST: Duration = Duration::from_millis(100);
// A flick stays under about 8 rows per notch.
const SCROLL_MAX: u32 = 4;

/// Screen offset one wheel notch pans the view by. Shift turns vertical into
/// horizontal; Ctrl is reserved for zoom.
pub fn scroll_step(ev: &MouseEvent) -> Option<(i32, i32)> {
    if ev.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    let shift = ev.modifiers.contains(KeyModifiers::SHIFT);
    match ev.kind {
        MouseEventKind::ScrollUp if shift => Some((-4, 0)),
        MouseEventKind::ScrollDown if shift => Some((4, 0)),
        MouseEventKind::ScrollUp => Some((0, -2)),
        MouseEventKind::ScrollDown => Some((0, 2)),
        MouseEventKind::ScrollLeft => Some((-4, 0)),
        MouseEventKind::ScrollRight => Some((4, 0)),
        _ => None,
    }
}

impl Session {
    /// Pans `n` notches of `step`, scaled up while notches keep arriving.
    pub fn scroll(&mut self, step: P, n: u32, now: Instant) {
        let prior = match self.burst {
            Some((last, count)) if now.duration_since(last) <= SCROLL_BURST => count,
            _ => 0,
        };
        let k = (n * (1 + prior / 2).min(SCROLL_MAX)) as i32;
        self.view.pan(step.0 * k, step.1 * k);
        self.hover = Hit::Empty;
        self.burst = Some((now, prior + n));
    }

    fn edge_velocity(&self) -> P {
        let Gesture::Dragging { action, .. } = &self.gesture else {
            return (0, 0);
        };
        if matches!(action, Action::Palette | Action::Pan(_)) {
            return (0, 0);
        }
        let axis = |pos: i32, len: i32| {
            let speed = |dist: i32| if dist == 0 { 3 } else { 1 };
            if pos < EDGE_ZONE {
                -speed(pos)
            } else if len - 1 - pos < EDGE_ZONE {
                speed(len - 1 - pos)
            } else {
                0
            }
        };
        let (w, h) = self.screen;
        (
            axis(self.cursor.0, i32::from(w)),
            axis(self.cursor.1, i32::from(h)),
        )
    }

    /// Time until the next edge pan, or `None` when the drag is not at an edge.
    pub fn edge_wait(&self, now: Instant) -> Option<Duration> {
        if self.edge_velocity() == (0, 0) {
            return None;
        }
        let due = self.last_pan.map_or(now, |t| t + EDGE_TICK);
        Some(due.saturating_duration_since(now))
    }

    /// Pans the view and re-runs the drag step when a tick is due.
    pub fn edge_pan(&mut self, now: Instant) -> bool {
        if self.edge_wait(now) != Some(Duration::ZERO) {
            return false;
        }
        let (dx, dy) = self.edge_velocity();
        self.view.pan(dx, dy);
        self.last_pan = Some(now);
        pan_step(self);
        true
    }
}

pub fn handle_mouse(s: &mut Session, ev: MouseEvent, now: Instant) {
    if s.overview {
        return;
    }
    let (col, row) = (i32::from(ev.column), i32::from(ev.row));
    if let Some(step) = scroll_step(&ev) {
        return s.scroll(step, 1, now);
    }
    let world = s.view.to_world(col, row);
    s.shift = ev.modifiers.contains(KeyModifiers::SHIFT);
    match ev.kind {
        MouseEventKind::Down(MouseButton::Left) => down(s, (col, row), world, now),
        MouseEventKind::Down(MouseButton::Middle) => middle_down(s, (col, row), world),
        // Some terminals report the wrong button on Drag and Up.
        MouseEventKind::Drag(_) => {
            s.cursor = (col, row);
            drag(s, (col, row), world)
        }
        MouseEventKind::Up(_) => up(s, (col, row), world),
        MouseEventKind::Moved => hover(s, (col, row), world),
        _ => {}
    }
}

fn hover(s: &mut Session, screen: P, world: P) {
    let idle = s.tool == Tool::Select && s.edit.is_none() && matches!(s.gesture, Gesture::Idle);
    s.hover = if idle && palette::hit(s, screen.0, screen.1).is_none() {
        hit::test(&s.canvas, &s.selection, world)
    } else {
        Hit::Empty
    };
}

fn down(s: &mut Session, screen: P, world: P, now: Instant) {
    s.hover = Hit::Empty;
    if s.place_caret(world) {
        return;
    }
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
    if let (Some(id), Tool::Select, false) = (
        hit::label_hit(&s.canvas, &s.selection, world),
        s.tool,
        matches!(hit, Hit::Handle(..)),
    ) {
        s.gesture = Gesture::Pressed {
            origin: screen,
            world,
            target: Target::Label(id),
        };
        return;
    }
    let target = match (hit, s.tool) {
        (Hit::Handle(h, id), _) => Target::Handle(h, id),
        (_, Tool::Hand) => Target::Pan,
        (_, Tool::Line | Tool::Arrow) => Target::Draw(s.tool),
        (Hit::Element(id), _) => {
            let mut unit = vec![id.clone()];
            s.canvas.expand_groups(&mut unit);
            if s.shift {
                if s.selection.contains(&id) {
                    s.selection.retain(|i| !unit.contains(i));
                    return;
                }
                let new = unit.into_iter().filter(|i| !s.selection.contains(i));
                s.selection.extend(new.collect::<Vec<_>>());
            } else if !s.selection.contains(&id) {
                s.selection = unit;
            }
            Target::Element(id)
        }
        (
            Hit::Empty,
            Tool::Rect
            | Tool::Ellipse
            | Tool::Diamond
            | Tool::Cylinder
            | Tool::Parallelogram
            | Tool::Hexagon,
        ) => Target::Draw(s.tool),
        (Hit::Empty, _) => Target::Empty,
    };
    s.gesture = Gesture::Pressed {
        origin: screen,
        world,
        target,
    };
}

fn middle_down(s: &mut Session, screen: P, world: P) {
    s.hover = Hit::Empty;
    s.commit_edit();
    s.gesture = Gesture::Pressed {
        origin: screen,
        world,
        target: Target::Pan,
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
        PaletteHit::Option(Choice::Stroke(stroke)) => {
            let cmds: Vec<Command> = s
                .selection
                .iter()
                .filter(|id| matches!(s.canvas.get(id), Some(Element::Line(_) | Element::Shape(_))))
                .map(|id| Command::SetStroke {
                    id: id.clone(),
                    stroke,
                })
                .collect();
            s.exec(Command::Batch(cmds));
        }
        PaletteHit::Option(Choice::Border(border)) => {
            let cmds: Vec<Command> = s
                .selection
                .iter()
                .filter(
                    |id| matches!(s.canvas.get(id), Some(Element::Shape(sh)) if sh.has_border()),
                )
                .map(|id| Command::SetBorder {
                    id: id.clone(),
                    border,
                })
                .collect();
            s.exec(Command::Batch(cmds));
        }
        PaletteHit::Option(c @ (Choice::Ink | Choice::Fill)) => {
            let cur = s
                .selection
                .iter()
                .find_map(|id| paint(s.canvas.get(id)?, c))
                .flatten();
            let next = Paint::next(cur);
            let cmds: Vec<Command> = s
                .selection
                .iter()
                .filter(|id| s.canvas.get(id).and_then(|e| paint(e, c)).is_some())
                .map(|id| match c {
                    Choice::Ink => Command::SetColor {
                        id: id.clone(),
                        color: next,
                    },
                    _ => Command::SetFill {
                        id: id.clone(),
                        fill: next,
                    },
                })
                .collect();
            s.exec(Command::Batch(cmds));
        }
        PaletteHit::Option(Choice::Routing(routing)) => {
            let cmds: Vec<Command> = s
                .selection
                .iter()
                .filter(|id| matches!(s.canvas.get(id), Some(Element::Line(_))))
                .map(|id| Command::SetRouting {
                    id: id.clone(),
                    routing,
                })
                .collect();
            s.exec(Command::Batch(cmds));
        }
        PaletteHit::Tool(t) => {
            let kind = match t {
                Tool::Rect => Some(ShapeKind::Rect),
                Tool::Ellipse => Some(ShapeKind::Ellipse),
                Tool::Diamond => Some(ShapeKind::Diamond),
                Tool::Cylinder => Some(ShapeKind::Cylinder),
                Tool::Parallelogram => Some(ShapeKind::Parallelogram),
                Tool::Hexagon => Some(ShapeKind::Hexagon),
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
        Target::Element(id) if !s.shift => s.select_group_of(id),
        Target::Empty if s.tool == Tool::Text => {
            s.start_edit_new(world);
            s.tool = Tool::Select;
        }
        Target::Draw(_) | Target::Empty => match hit::test(&s.canvas, &[], world) {
            Hit::Element(id) => s.select_group_of(id),
            _ => s.selection.clear(),
        },
        Target::Element(_)
        | Target::Label(_)
        | Target::PaletteTitle
        | Target::Pan
        | Target::Handle(..) => {}
    }
}

impl Session {
    fn select_group_of(&mut self, id: Id) {
        self.selection = vec![id];
        self.canvas.expand_groups(&mut self.selection);
    }

    /// Undoes a gesture in progress. Returns false when there was none.
    pub(super) fn cancel_gesture(&mut self) -> bool {
        self.last_pan = None;
        match std::mem::replace(&mut self.gesture, Gesture::Idle) {
            Gesture::Idle => false,
            Gesture::Pressed { .. } => true,
            Gesture::Dragging { inv, .. } => {
                for undo in inv.into_iter().rev() {
                    let _ = self.canvas.apply(undo);
                }
                self.rev += 1;
                self.band = None;
                self.guides.clear();
                self.canvas.retain_present(&mut self.selection);
                true
            }
        }
    }
}
