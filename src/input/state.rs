use super::drag::{drag, pan_step, up};
use super::hit::{self, Handle, Hit};
use super::{quick, Session, Tool};
use crate::model::{self, Command, Edge, Element, Id, Paint, ShapeKind, Side};
use crate::ui::palette::{self, paint, Choice, PaletteHit};
use crate::view::View;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::time::{Duration, Instant};

pub(super) type P = (i32, i32);

/// A selected shape as it stood before a kind preview began.
#[derive(Clone, Debug)]
pub struct Origin {
    pub id: Id,
    pub kind: ShapeKind,
    pub rect: (i32, i32, i32, i32),
}

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
    Label(Id, Option<Edge>),
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
    LabelDrag(Id, Option<Edge>),
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
        let scaled = |d: i32| match d * k * self.scroll_speed / 100 {
            0 => d.signum(),
            v => v,
        };
        self.glide = None;
        self.view.pan(
            self.view.world_step(scaled(step.0)),
            self.view.world_step(scaled(step.1)),
        );
        self.hover = Hit::Empty;
        self.quick = None;
        self.burst = Some((now, prior + n));
    }

    /// Centers `bounds` on screen, gliding there when `motion` is on.
    pub fn fit(&mut self, bounds: Option<(i32, i32, i32, i32)>) {
        let mut target = self.view;
        target.fit_all(bounds, self.viewport());
        if target.zoom == self.view.zoom {
            self.go_to((target.ox, target.oy));
        } else {
            self.commit_edit();
            self.set_view(target);
        }
    }

    // shortcut: the target is not recomputed on resize, fine for a 120 ms glide
    pub fn go_to(&mut self, to: P) {
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
        self.tip_due = match self.palette_hover {
            Some(PaletteHit::Option(_)) => Some(now + TIP_DELAY),
            _ => None,
        };
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
            let speed = |dist: i32| if dist <= 0 { 3 } else { 1 };
            if pos < EDGE_ZONE {
                -speed(pos)
            } else if len - 1 - pos < EDGE_ZONE {
                speed(len - 1 - pos)
            } else {
                0
            }
        };
        let (vx, vy, vw, vh) = self.viewport();
        (axis(self.cursor.0 - vx, vw), axis(self.cursor.1 - vy, vh))
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
        if row < palette::rows(s) && step.0 != 0 {
            return palette::wheel(s, row, step.0.signum());
        }
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
    let idle = s.tool.is_pointer() && s.edit.is_none() && matches!(s.gesture, Gesture::Idle);
    let live =
        idle && s.in_viewport(screen.0, screen.1) && palette::hit(s, screen.0, screen.1).is_none();
    s.quick = if live { quick::at(s, world) } else { None };
    s.hover = if live && s.quick.is_none() {
        hit::at(s, &s.selection, world, 1)
    } else {
        Hit::Empty
    };
    s.label_hover = idle && hit::label_hit(s, &s.selection, world).is_some();
    s.palette_hover = if matches!(s.gesture, Gesture::Idle) {
        palette::hit(s, screen.0, screen.1)
    } else {
        None
    };
}

fn down(s: &mut Session, screen: P, world: P, now: Instant) {
    s.hover = Hit::Empty;
    let quick = s.quick.take();
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
    if s.dragging() {
        up(s, s.view.to_world(s.cursor.0, s.cursor.1));
    }
    s.gesture = Gesture::Idle;
    if let Some(h) = palette::hit(s, screen.0, screen.1) {
        return palette_down(s, h);
    }
    if !s.in_viewport(screen.0, screen.1) {
        return;
    }
    if !double {
        if let Some((cmd, id)) = quick
            .filter(|_| !s.shift)
            .and_then(|q| quick::insert_cmd(s, &q))
        {
            s.exec(cmd);
            s.selection = vec![id];
            return;
        }
    }
    let hit = hit::at(s, &s.selection, world, i32::from(s.tool.is_pointer()));
    if double {
        let id = match hit {
            Hit::Element(id) | Hit::Handle(_, id) => Some(id),
            Hit::Empty => hit::shape_at(&s.canvas, world),
        };
        return match id {
            Some(id) if s.start_edit_side(&id, world) => {}
            Some(id) => {
                s.place_line_label(&id, world);
                s.start_edit_existing(id)
            }
            None => s.start_edit_new(world),
        };
    }
    if let (Some((id, edge)), true, false) = (
        hit::label_hit(s, &s.selection, world),
        s.tool.is_pointer(),
        matches!(hit, Hit::Handle(..)),
    ) {
        s.gesture = Gesture::Pressed {
            origin: screen,
            world,
            target: Target::Label(id, edge),
        };
        return;
    }
    let target = match (hit, s.tool) {
        (Hit::Handle(h, id), _) => Target::Handle(h, id),
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
    s.quick = None;
    if !s.in_viewport(screen.0, screen.1) {
        return;
    }
    s.commit_edit();
    s.gesture = Gesture::Pressed {
        origin: screen,
        world,
        target: Target::Pan,
    };
}

impl Session {
    /// Applies a head, stroke, border or text align value to the selected elements that carry it.
    pub fn set_choice(&mut self, c: Choice) {
        let cmds: Vec<Command> = self
            .selection
            .iter()
            .filter_map(|id| {
                let id = id.clone();
                Some(match (c, self.canvas.get(&id)?) {
                    (Choice::Head(kind), Element::Line(_)) => Command::SetLineKind { id, kind },
                    (Choice::Stroke(stroke), Element::Line(_) | Element::Shape(_)) => {
                        Command::SetStroke { id, stroke }
                    }
                    (Choice::Border(border), Element::Shape(sh)) if sh.has_border() => {
                        Command::SetBorder { id, border }
                    }
                    (Choice::Align(align), Element::Shape(_)) => Command::SetAlign { id, align },
                    _ => return None,
                })
            })
            .collect();
        self.exec(Command::Batch(cmds));
    }

    pub fn origins(&self) -> Vec<Origin> {
        self.selection
            .iter()
            .filter_map(|id| match self.canvas.get(id)? {
                Element::Shape(s) => Some(Origin {
                    id: id.clone(),
                    kind: s.kind,
                    rect: (s.x, s.y, s.w, s.h),
                }),
                _ => None,
            })
            .collect()
    }

    /// Shows `kind` on the origin shapes without a history entry.
    pub fn preview_kind(&mut self, from: &[Origin], kind: ShapeKind) {
        self.restore_kinds(from);
        let _ = self.canvas.apply(Command::Batch(kind_cmds(from, kind)));
    }

    pub fn restore_kinds(&mut self, from: &[Origin]) {
        let cmds = from.iter().flat_map(|o| {
            let (x, y, w, h) = o.rect;
            [
                Command::SetKind {
                    id: o.id.clone(),
                    kind: o.kind,
                },
                Command::SetRect {
                    id: o.id.clone(),
                    x,
                    y,
                    w,
                    h,
                },
            ]
        });
        let _ = self.canvas.apply(Command::Batch(cmds.collect()));
        self.rev += 1;
    }

    /// Restores the origins, then applies `kind` as one undoable edit relative to them.
    pub fn set_kind(&mut self, from: &[Origin], kind: ShapeKind) {
        self.restore_kinds(from);
        self.exec(Command::Batch(kind_cmds(from, kind)));
    }

    /// Sets the ink or fill of the selected elements that carry that color.
    pub fn set_paint(&mut self, c: Choice, color: Option<Paint>) {
        let cmds: Vec<Command> = self
            .selection
            .iter()
            .filter(|id| self.canvas.get(id).and_then(|e| paint(e, c)).is_some())
            .map(|id| match c {
                Choice::Ink => Command::SetColor {
                    id: id.clone(),
                    color,
                },
                _ => Command::SetFill {
                    id: id.clone(),
                    fill: color,
                },
            })
            .collect();
        self.exec(Command::Batch(cmds));
    }
}

fn kind_cmds(from: &[Origin], kind: ShapeKind) -> Vec<Command> {
    from.iter()
        .flat_map(|o| model::kind_cmds(&o.id, o.kind, o.rect, kind))
        .collect()
}

fn palette_down(s: &mut Session, h: PaletteHit) {
    match h {
        PaletteHit::Scroll(strip, dir) => palette::jump(s, strip, dir),
        PaletteHit::Option(Choice::Align(cur)) => s.set_choice(Choice::Align(cur.next())),
        // The app opens the swatch window before the click reaches the session.
        PaletteHit::Option(Choice::Ink | Choice::Fill) => {}
        PaletteHit::Option(c @ (Choice::Head(_) | Choice::Stroke(_) | Choice::Border(_))) => {
            s.set_choice(c)
        }
        PaletteHit::Option(Choice::Place(a)) => s.align(a),
        PaletteHit::Option(Choice::Arrange) => s.reset_route(),
        PaletteHit::Option(Choice::Group) => s.group(true),
        PaletteHit::Option(Choice::Ungroup) => s.group(false),
        PaletteHit::Tool(t) => {
            if !matches!(t, Tool::Select | Tool::Arrow | Tool::Text) {
                s.selection.clear();
            }
            s.tool = t;
        }
    }
}

pub(super) fn click(s: &mut Session, target: Target, world: P) {
    match target {
        Target::Element(id) if !s.shift => s.select_group_of(id),
        Target::Empty if s.tool == Tool::Text => {
            s.start_edit_new(world);
            s.tool = Tool::Pointer;
        }
        Target::Draw(_) | Target::Empty => match hit::at(s, &[], world, 0) {
            Hit::Element(id) => s.select_group_of(id),
            _ => s.selection.clear(),
        },
        Target::Element(_)
        | Target::Label(..)
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
