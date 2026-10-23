use super::drag::{drag, pan_step, up};
use super::hit::{self, Handle, Hit};
use super::{Session, Tool};
use crate::model::{Command, Element, Id, Paint, Shape, ShapeKind, Side};
use crate::ui::minimap;
use crate::ui::palette::{self, paint, Choice, PaletteHit};
use crate::view::View;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::time::{Duration, Instant};

pub(super) type P = (i32, i32);

pub(super) enum Gesture {
    Idle,
    Pressed { origin: P, world: P, target: Target },
    Dragging { action: Action, inv: Vec<Command> },
}

/// A view move in flight; `start` stays unset until the first tick, which owns the clock.
pub(super) struct Glide {
    from: P,
    to: P,
    start: Option<Instant>,
}

pub(super) enum Target {
    Handle(Handle, Id),
    Element(Id),
    Label(Id),
    Route(Id),
    Empty,
    Draw(Tool),
    Pan,
}

pub(super) enum Action {
    Pan(P),
    Move { start: P, last: P },
    Resize { id: Id, fixed: P },
    EndDrag { id: Id, side: Side },
    LabelDrag(Id),
    BendDrag { id: Id, at: usize },
    Band(P),
    Create { id: Id, origin: P },
    CreateLine(Id),
}

const EDGE_ZONE: i32 = 2;
const EDGE_TICK: Duration = Duration::from_millis(30);
const TIP_DELAY: Duration = Duration::from_millis(400);
const GLIDE: Duration = Duration::from_millis(120);
const GLIDE_FRAME: Duration = Duration::from_millis(16);
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
        self.glide = None;
        self.view.pan(
            self.view.world_step(step.0 * k),
            self.view.world_step(step.1 * k),
        );
        self.hover = Hit::Empty;
        self.burst = Some((now, prior + n));
    }

    /// Centers `bounds` on screen, gliding there when `motion` is on.
    pub fn fit(&mut self, bounds: Option<(i32, i32, i32, i32)>) {
        let mut target = self.view;
        target.fit_all(bounds, self.screen);
        if target.zoom == self.view.zoom {
            self.go_to((target.ox, target.oy));
        } else {
            self.commit_edit();
            self.set_view(target);
        }
    }

    // shortcut: the target is not recomputed on resize, fine for a 120 ms glide
    fn go_to(&mut self, to: P) {
        let from = (self.view.ox, self.view.oy);
        if self.motion && from != to {
            self.glide = Some(Glide {
                from,
                to,
                start: None,
            });
        } else {
            self.glide = None;
            self.view = View {
                ox: to.0,
                oy: to.1,
                ..self.view
            };
        }
    }

    /// Restarts the tooltip delay after the palette hover changed.
    pub fn tip_reset(&mut self, now: Instant) {
        self.tip = false;
        self.tip_due = self.palette_hover.map(|_| now + TIP_DELAY);
    }

    pub fn tip_wait(&self, now: Instant) -> Option<Duration> {
        self.tip_due.map(|d| d.saturating_duration_since(now))
    }

    /// Shows the tooltip once its delay passed. Returns true when it appeared.
    pub fn tip_step(&mut self, now: Instant) -> bool {
        if self.tip_wait(now) != Some(Duration::ZERO) {
            return false;
        }
        self.tip_due = None;
        self.tip = true;
        true
    }

    /// Time until the next glide frame, or `None` when no glide runs.
    pub fn glide_wait(&self) -> Option<Duration> {
        self.glide.as_ref().map(|_| GLIDE_FRAME)
    }

    /// Moves the view along the ease-out curve. Returns true when the view moved.
    pub fn glide_step(&mut self, now: Instant) -> bool {
        let Some(g) = &mut self.glide else {
            return false;
        };
        let start = *g.start.get_or_insert(now);
        let t = (now.saturating_duration_since(start).as_secs_f64() / GLIDE.as_secs_f64()).min(1.0);
        let p = 1.0 - (1.0 - t).powi(3);
        let at = |a: i32, b: i32| a + (f64::from(b - a) * p).round() as i32;
        let (to, from) = (g.to, g.from);
        let next = if t >= 1.0 {
            to
        } else {
            (at(from.0, to.0), at(from.1, to.1))
        };
        if t >= 1.0 {
            self.glide = None;
        }
        let moved = next != (self.view.ox, self.view.oy);
        self.view = View {
            ox: next.0,
            oy: next.1,
            ..self.view
        };
        moved
    }

    fn edge_velocity(&self) -> P {
        let Gesture::Dragging { action, .. } = &self.gesture else {
            return (0, 0);
        };
        if matches!(action, Action::Pan(_)) {
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
        self.glide = None;
        self.view
            .pan(self.view.world_step(dx), self.view.world_step(dy));
        self.last_pan = Some(now);
        pan_step(self);
        true
    }
}

pub fn handle_mouse(s: &mut Session, ev: MouseEvent, now: Instant) {
    let (col, row) = (i32::from(ev.column), i32::from(ev.row));
    if ev.modifiers.contains(KeyModifiers::CONTROL) {
        match ev.kind {
            MouseEventKind::ScrollUp => return s.zoom(1, (col, row)),
            MouseEventKind::ScrollDown => return s.zoom(-1, (col, row)),
            _ => {}
        }
    }
    if let Some(step) = scroll_step(&ev) {
        return s.scroll(step, 1, now);
    }
    if s.present {
        return;
    }
    let world = s.view.to_world(col, row);
    if s.zoomed_out() {
        return match ev.kind {
            MouseEventKind::Down(MouseButton::Left | MouseButton::Middle) => {
                middle_down(s, (col, row), world)
            }
            MouseEventKind::Drag(_) => {
                s.cursor = (col, row);
                drag(s, (col, row), world)
            }
            MouseEventKind::Up(_) => up(s, world),
            _ => {}
        };
    }
    s.shift = ev.modifiers.contains(KeyModifiers::SHIFT);
    match ev.kind {
        MouseEventKind::Down(MouseButton::Left) => down(s, (col, row), world, now),
        MouseEventKind::Down(MouseButton::Middle) => middle_down(s, (col, row), world),
        // Some terminals report the wrong button on Drag and Up.
        MouseEventKind::Drag(_) => {
            s.cursor = (col, row);
            drag(s, (col, row), world)
        }
        MouseEventKind::Up(_) => up(s, world),
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
    s.label_hover = idle && hit::label_hit(&s.canvas, &s.selection, world).is_some();
    s.palette_hover = if matches!(s.gesture, Gesture::Idle) {
        palette::hit(s, screen.0, screen.1)
    } else {
        None
    };
}

fn down(s: &mut Session, screen: P, world: P, now: Instant) {
    s.hover = Hit::Empty;
    s.label_hover = false;
    s.palette_hover = None;
    if s.place_caret(screen) {
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
    if let Some(m) = minimap::hit(s, screen.0, screen.1) {
        let (wx, wy) = m.world(screen.0, screen.1);
        let (sw, sh) = s.view.span(s.screen);
        return s.go_to((wx - sw / 2, wy - sh / 2));
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
        (_, Tool::Arrow) => Target::Draw(s.tool),
        (Hit::Element(id), _) => {
            let reshape = !s.shift
                && s.selection == [id.clone()]
                && matches!(s.canvas.get(&id), Some(Element::Line(_)));
            let mut unit = vec![id.clone()];
            s.canvas.expand_frames(&mut unit);
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
            if reshape {
                Target::Route(id)
            } else {
                Target::Element(id)
            }
        }
        (
            Hit::Empty,
            Tool::Rect
            | Tool::Diamond
            | Tool::Cylinder
            | Tool::Parallelogram
            | Tool::Hexagon
            | Tool::Frame,
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
        PaletteHit::Option(Choice::Align(cur)) => {
            let cmds: Vec<Command> = s
                .selection
                .iter()
                .filter(|id| matches!(s.canvas.get(id), Some(Element::Shape(_))))
                .map(|id| Command::SetAlign {
                    id: id.clone(),
                    align: cur.next(),
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
        PaletteHit::Option(Choice::Order(z)) => s.reorder(z),
        PaletteHit::Option(Choice::Place(a)) => s.align(a),
        PaletteHit::Option(Choice::Group) => s.group(true),
        PaletteHit::Option(Choice::Ungroup) => s.group(false),
        PaletteHit::Option(Choice::Routing(routing)) => s.set_routing(routing),
        PaletteHit::Tool(t) => {
            let kind = match t {
                Tool::Rect => Some(ShapeKind::Rect),
                Tool::Diamond => Some(ShapeKind::Diamond),
                Tool::Cylinder => Some(ShapeKind::Cylinder),
                Tool::Parallelogram => Some(ShapeKind::Parallelogram),
                Tool::Hexagon => Some(ShapeKind::Hexagon),
                Tool::Frame => Some(ShapeKind::Frame),
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
                    let mut cmds = vec![];
                    for id in shapes {
                        if let Some(sh) = s.canvas.shape(&id) {
                            let (w, h) = Shape::snap_size(kind, sh.w, sh.h);
                            if (w, h) != (sh.w, sh.h) {
                                cmds.push(Command::SetRect {
                                    id: id.clone(),
                                    x: sh.x,
                                    y: sh.y,
                                    w,
                                    h,
                                });
                            }
                        }
                        cmds.push(Command::SetKind { id, kind });
                    }
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
        | Target::Route(_)
        | Target::Pan
        | Target::Handle(..) => {}
    }
}

impl Session {
    pub(super) fn select_group_of(&mut self, id: Id) {
        self.selection = vec![id];
        self.canvas.expand_frames(&mut self.selection);
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
                self.snap = None;
                self.snap_at = None;
                self.canvas.retain_present(&mut self.selection);
                true
            }
        }
    }
}
