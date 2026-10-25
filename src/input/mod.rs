mod drag;
mod edit;
mod hit;
mod keys;
mod state;

pub use edit::{Edit, EditBox, EditTarget};
pub use hit::{handles, shape_at, Handle, Hit};
pub use keys::{edit_key, handle_key, handle_paste, KeyOutcome};
pub use state::{handle_mouse, scroll_step, Origin};

use crate::model::{
    wrap, Align, Anchor, Canvas, Command, Dir, Element, End, History, Id, Line, LineKind, Shape,
    Stroke, Text, Z,
};
use crate::ui::palette::{self, PaletteHit};
use crate::ui::theme::Theme;
use crate::ui::Layout;
use crate::view::{View, Zoom};
use ratatui::style::Color;
use state::{Action, Gesture, Glide};
use std::cell::RefCell;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Rect,
    Diamond,
    Cylinder,
    Parallelogram,
    Hexagon,
    Frame,
    Arrow,
    Text,
}

impl Tool {
    pub const ALL: [Tool; 9] = [
        Tool::Select,
        Tool::Rect,
        Tool::Diamond,
        Tool::Cylinder,
        Tool::Parallelogram,
        Tool::Hexagon,
        Tool::Frame,
        Tool::Arrow,
        Tool::Text,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Rect => "Rect",
            Tool::Diamond => "Diamond",
            Tool::Cylinder => "Cylinder",
            Tool::Parallelogram => "Slant",
            Tool::Hexagon => "Hexagon",
            Tool::Frame => "Frame",
            Tool::Arrow => "Arrow",
            Tool::Text => "Text",
        }
    }

    pub fn key(self) -> Option<char> {
        crate::ui::registry::tool_key(self)
    }
}

/// Everything the editor knows apart from files and overlays.
pub struct Session {
    pub canvas: Canvas,
    pub history: History,
    pub view: View,
    pub tool: Tool,
    pub theme: Theme,
    pub chrome_accent: Option<Color>,
    pub selection: Vec<Id>,
    pub clipboard: Canvas,
    pub double_click: Duration,
    /// Cells between snap lines for drawn, moved and resized shapes; 1 means no snapping.
    pub grid: i32,
    /// Whether Fit all and search jumps glide instead of snapping.
    pub motion: bool,
    /// Percent of the default distance a wheel notch pans.
    pub scroll_speed: i32,
    /// Last search term; an empty search repeats it.
    pub search: String,
    pub screen: (u16, u16),
    pub edit: Option<Edit>,
    /// Full-screen read-only canvas for talks; only scrolling and exit work.
    pub present: bool,
    pub band: Option<((i32, i32), (i32, i32))>,
    /// Alignment guide segments (world cells, inclusive) while a move drag snaps to other shapes.
    pub guides: Vec<((i32, i32), (i32, i32))>,
    /// Shape under a dragged line end; the drop binds it.
    pub snap: Option<Id>,
    /// Cell where the dragged line end will attach to `snap`.
    pub snap_at: Option<(i32, i32)>,
    /// What a click at the pointer would hit; only the Select tool at rest sets it.
    pub hover: Hit,
    /// Whether a click at the pointer would grab a label to drag or edit it.
    pub label_hover: bool,
    pub palette_hover: Option<PaletteHit>,
    /// Columns the tool row and the option row are scrolled by.
    pub tool_scroll: i32,
    pub option_scroll: i32,
    /// Whether the pointer rested on `palette_hover` long enough to show its tooltip.
    pub tip: bool,
    tip_due: Option<Instant>,
    /// Counts canvas mutations; autosave compares it with the saved value.
    pub rev: u64,
    /// Frame cache; `rev` and the element count invalidate it.
    pub layout: RefCell<Option<Layout>>,
    gesture: Gesture,
    last_down: Option<(i32, i32, Instant)>,
    /// Time of the last wheel event and the notches in the current burst.
    burst: Option<(Instant, u32)>,
    cursor: (i32, i32),
    shift: bool,
    last_pan: Option<Instant>,
    glide: Option<Glide>,
}

impl Session {
    pub fn new(canvas: Canvas, view: View, screen: (u16, u16)) -> Session {
        Session {
            canvas,
            history: History::default(),
            view,
            tool: Tool::Select,
            theme: Theme::Dark,
            chrome_accent: None,
            selection: vec![],
            clipboard: Canvas::default(),
            double_click: Duration::from_millis(400),
            grid: 1,
            motion: true,
            scroll_speed: 100,
            search: String::new(),
            screen,
            edit: None,
            present: false,
            band: None,
            guides: vec![],
            snap: None,
            snap_at: None,
            hover: Hit::Empty,
            label_hover: false,
            palette_hover: None,
            tool_scroll: 0,
            option_scroll: 0,
            tip: false,
            tip_due: None,
            rev: 0,
            layout: RefCell::new(None),
            gesture: Gesture::Idle,
            last_down: None,
            burst: None,
            cursor: (0, 0),
            shift: false,
            last_pan: None,
            glide: None,
        }
    }

    /// OSC 22 name of the mouse pointer shape for the current state.
    pub fn pointer(&self) -> &'static str {
        if self.edit.is_some() {
            return "text";
        }
        if let Gesture::Dragging { action, .. } = &self.gesture {
            return match action {
                Action::Pan(_) | Action::Move { .. } | Action::LabelDrag(_) => "grabbing",
                Action::Resize { .. }
                | Action::EndDrag { .. }
                | Action::BendDrag { .. }
                | Action::Create { .. }
                | Action::CreateLine(_) => "crosshair",
                Action::Band(_) => "default",
            };
        }
        if self.palette_hover.is_some() {
            return "pointer";
        }
        match (self.tool, &self.hover) {
            (Tool::Text, _) => "text",
            (Tool::Select, Hit::Handle(Handle::Resize, _)) => "nwse-resize",
            (Tool::Select, Hit::Handle(Handle::End(_) | Handle::Bend(_), _)) => "crosshair",
            (Tool::Select, _) if self.label_hover => "text",
            (Tool::Select, Hit::Element(_)) => "grab",
            (Tool::Select, Hit::Empty) => "default",
            _ => "crosshair",
        }
    }

    /// Autosave must not stall a drag step.
    pub fn dragging(&self) -> bool {
        matches!(self.gesture, Gesture::Dragging { .. })
    }

    /// Size in cells of the shape or line a drag is drawing or resizing.
    pub fn readout(&self) -> Option<String> {
        let Gesture::Dragging { action, .. } = &self.gesture else {
            return None;
        };
        match action {
            Action::Create { id, .. } | Action::Resize { id, .. } => {
                let sh = self.canvas.shape(id)?;
                Some(format!("{}x{}", sh.w, sh.h))
            }
            Action::CreateLine(id) | Action::EndDrag { id, .. } => {
                let Some(Element::Line(l)) = self.canvas.get(id) else {
                    return None;
                };
                Some(format!("{} cells", self.canvas.route(l).len()))
            }
            _ => None,
        }
    }

    /// Applies `cmd`, then pins the automatic route of the lines it moved or reconnected, in one undo step.
    pub fn exec(&mut self, cmd: Command) {
        let ids = cmd.rerouting();
        let Ok(mut undo) = self.canvas.apply(cmd) else {
            return;
        };
        let pins = self.canvas.pin_cmds(&ids);
        if !pins.is_empty() {
            if let Ok(pin_undo) = self.canvas.apply(Command::Batch(pins)) {
                undo = Command::Batch(vec![pin_undo, undo]);
            }
        }
        self.history.record(undo);
        self.rev += 1;
        self.prune_selection();
    }

    pub fn undo(&mut self) -> Option<String> {
        if !matches!(self.gesture, Gesture::Idle) {
            return None;
        }
        let label = self.history.undo(&mut self.canvas).ok()??;
        self.rev += 1;
        self.prune_selection();
        Some(format!("Undid: {label}"))
    }

    pub fn redo(&mut self) -> Option<String> {
        if !matches!(self.gesture, Gesture::Idle) {
            return None;
        }
        let label = self.history.redo(&mut self.canvas).ok()??;
        self.rev += 1;
        self.prune_selection();
        Some(format!("Redid: {label}"))
    }

    fn idle(&self) -> bool {
        self.edit.is_none() && matches!(self.gesture, Gesture::Idle)
    }

    pub fn can_edit(&self) -> bool {
        self.idle() && !self.zoomed_out()
    }

    /// At 25% and 50% the canvas is a read-only block map that only pans and zooms.
    pub fn zoomed_out(&self) -> bool {
        self.view.zoom < Zoom::One
    }

    /// The screen rect (x, y, w, h) between the tool rows and the info bar; all of it in present mode.
    pub fn viewport(&self) -> (i32, i32, i32, i32) {
        let (w, h) = (i32::from(self.screen.0), i32::from(self.screen.1));
        if self.present {
            return (0, 0, w, h);
        }
        let top = palette::rows(self);
        (0, top, w, (h - 1 - top).max(0))
    }

    pub fn in_viewport(&self, col: i32, row: i32) -> bool {
        let (x, y, w, h) = self.viewport();
        col >= x && col < x + w && row >= y && row < y + h
    }

    pub fn viewport_center(&self) -> (i32, i32) {
        let (x, y, w, h) = self.viewport();
        (x + w / 2, y + h / 2)
    }

    /// The world rect (x, y, w, h) the viewport shows.
    pub fn visible_world(&self) -> (i32, i32, i32, i32) {
        let (x, y, w, h) = self.viewport();
        let (wx, wy) = self.view.to_world(x, y);
        let (sw, sh) = self.view.span((w as u16, h as u16));
        (wx, wy, sw, sh)
    }

    /// Moves `n` steps along the zoom scale around screen cell `at`; ignored while editing or dragging.
    pub fn zoom(&mut self, n: i32, at: (i32, i32)) {
        self.zoom_to(self.view.zoom.step(n), at);
    }

    pub fn zoom_to(&mut self, zoom: Zoom, at: (i32, i32)) {
        if !self.idle() || zoom == self.view.zoom {
            return;
        }
        let mut view = self.view;
        view.zoom_at(zoom, at.0, at.1);
        self.set_view(view);
    }

    /// Swaps the view at once; a zoomed-out view clears everything that needs the mouse.
    pub(super) fn set_view(&mut self, view: View) {
        self.glide = None;
        self.view = view;
        self.hover = Hit::Empty;
        if self.zoomed_out() {
            self.selection.clear();
            self.label_hover = false;
            self.palette_hover = None;
        }
    }

    pub fn copy(&mut self) {
        if self.idle() && !self.selection.is_empty() {
            self.clipboard = self.canvas.clone_canvas(&self.selection, 0, 0);
        }
    }

    pub fn cut(&mut self) {
        if self.idle() && !self.selection.is_empty() {
            self.copy();
            let cmds = self
                .selection
                .iter()
                .cloned()
                .map(Command::Remove)
                .collect();
            self.exec(Command::Batch(cmds));
        }
    }

    /// Each paste lands one step further along than the last.
    pub fn paste(&mut self) {
        if !self.idle() {
            return;
        }
        let ids: Vec<Id> = self
            .clipboard
            .elements
            .iter()
            .map(|e| e.id().clone())
            .collect();
        let new = self.clipboard.clone_canvas(&ids, 2, 1);
        self.clipboard = new.clone();
        self.insert(new);
    }

    /// Adds a copy of the selected shape one gap away in `(dx, dy)`, joins them with an arrow, then edits its label.
    pub fn connect(&mut self, (dx, dy): (i32, i32)) {
        let [id] = self.selection.as_slice() else {
            return;
        };
        let Some(from) = self.canvas.shape(id).cloned() else {
            return;
        };
        let (new_id, line_id) = (Id::random(), Id::random());
        let to = Shape {
            id: new_id.clone(),
            x: from.x + dx * (from.w + 4),
            y: from.y + dy * (from.h + 2),
            label: String::new(),
            link: None,
            ..from.clone()
        };
        let line = Line {
            id: line_id,
            kind: LineKind::Arrow,
            from: End::Bound(from.id, Anchor::Auto),
            to: End::Bound(new_id.clone(), Anchor::Auto),
            label: String::new(),
            label_at: None,
            stroke: Stroke::Solid,
            bends: vec![],
            color: None,
        };
        let at = self.canvas.elements.len();
        self.exec(Command::Batch(vec![
            Command::Insert(at, Element::Shape(to)),
            Command::Insert(at + 1, Element::Line(line)),
        ]));
        self.start_edit_existing(new_id);
    }

    /// Bolds the labels of the selected shapes; unbolds them when all are bold already.
    pub fn toggle_bold(&mut self) {
        let shapes: Vec<(&Id, bool)> = self
            .selection
            .iter()
            .filter_map(|id| match self.canvas.get(id)? {
                Element::Shape(sh) => Some((id, sh.bold)),
                _ => None,
            })
            .collect();
        let bold = !shapes.iter().all(|&(_, b)| b);
        let cmds: Vec<Command> = shapes
            .into_iter()
            .map(|(id, _)| Command::SetBold {
                id: id.clone(),
                bold,
            })
            .collect();
        self.exec(Command::Batch(cmds));
    }

    /// Links the selected shapes to `url`, or clears their links when `url` is empty.
    /// Returns false when `url` is not an http, https or mailto address.
    pub fn set_link(&mut self, url: &str) -> bool {
        let ok = ["http://", "https://", "mailto:"]
            .iter()
            .any(|p| url.starts_with(p))
            && !url.contains(|c: char| c.is_whitespace() || c == '"');
        if !url.is_empty() && !ok {
            return false;
        }
        let cmds = self
            .selection
            .iter()
            .filter(|id| self.canvas.shape(id).is_some())
            .map(|id| Command::SetLink {
                id: id.clone(),
                link: (!url.is_empty()).then(|| url.to_string()),
            })
            .collect();
        self.exec(Command::Batch(cmds));
        true
    }

    pub fn select_all(&mut self) {
        if !matches!(self.gesture, Gesture::Idle) {
            return;
        }
        self.selection = self
            .canvas
            .elements
            .iter()
            .map(|e| e.id().clone())
            .collect();
    }

    pub fn delete_selection(&mut self) {
        if !matches!(self.gesture, Gesture::Idle) {
            return;
        }
        let cmds = self
            .selection
            .iter()
            .cloned()
            .map(Command::Remove)
            .collect();
        self.exec(Command::Batch(cmds));
    }

    pub fn align(&mut self, a: Align) {
        let cmds = self.canvas.align(&self.selection, a);
        if !cmds.is_empty() {
            self.exec(Command::Batch(cmds));
        }
    }

    pub fn layout(&mut self, dir: Dir) {
        let cmds = self.canvas.layout(&self.selection, dir);
        if !cmds.is_empty() {
            self.exec(Command::Batch(cmds));
        }
    }

    /// Groups the selection, or ungroups it when `group` is false.
    pub fn group(&mut self, group: bool) {
        let cmd = if group {
            self.canvas.group_cmd(&self.selection)
        } else {
            self.canvas.ungroup_cmd(&self.selection)
        };
        if let Some(cmd) = cmd {
            self.exec(cmd);
        }
    }

    pub fn line_has_bends(&self) -> bool {
        self.selection
            .iter()
            .any(|id| matches!(self.canvas.get(id), Some(Element::Line(l)) if !l.bends.is_empty()))
    }

    pub fn reset_route(&mut self) {
        let cmds = self.canvas.pin_cmds(&self.selection);
        if !cmds.is_empty() {
            self.exec(Command::Batch(cmds));
        }
    }

    pub fn reorder(&mut self, z: Z) {
        let cmds = self.canvas.z_order(&self.selection, z);
        if !cmds.is_empty() {
            self.exec(Command::Batch(cmds));
        }
    }

    /// Selects what a right click lands on. None when no menu may open; Some(true) on an element.
    pub fn right_down(&mut self, col: i32, row: i32) -> Option<bool> {
        if self.zoomed_out()
            || self.present
            || !self.idle()
            || !self.in_viewport(col, row)
            || palette::hit(self, col, row).is_some()
        {
            return None;
        }
        let world = self.view.to_world(col, row);
        let id = match hit::test(self, &self.selection, world) {
            Hit::Element(id) | Hit::Handle(_, id) => id,
            Hit::Empty => return Some(false),
        };
        if !self.selection.contains(&id) {
            self.select_group_of(id);
        }
        Some(true)
    }

    pub fn duplicate(&mut self) {
        if self.idle() && !self.selection.is_empty() {
            let new = self.canvas.clone_canvas(&self.selection, 2, 1);
            self.insert(new);
        }
    }

    pub(crate) fn insert(&mut self, new: Canvas) {
        if new.elements.is_empty() {
            return;
        }
        let ids: Vec<Id> = new.elements.iter().map(|e| e.id().clone()).collect();
        let at = self.canvas.elements.len();
        let mut cmds: Vec<Command> = new
            .elements
            .into_iter()
            .enumerate()
            .map(|(i, e)| Command::Insert(at + i, e))
            .collect();
        if !new.groups.is_empty() {
            let mut groups = self.canvas.groups.clone();
            groups.extend(new.groups);
            cmds.push(Command::SetGroups(groups));
        }
        self.exec(Command::Batch(cmds));
        self.selection = ids;
    }

    fn prune_selection(&mut self) {
        self.canvas.retain_present(&mut self.selection);
    }

    pub fn replace_canvas(&mut self, canvas: Canvas, view: View) {
        *self = Session {
            double_click: self.double_click,
            grid: self.grid,
            chrome_accent: self.chrome_accent,
            motion: self.motion,
            scroll_speed: self.scroll_speed,
            theme: self.theme,
            history: History::with_limit(self.history.limit),
            ..Session::new(canvas, view, self.screen)
        };
    }

    /// Selects the next element after the current selection whose label holds `term`, wrapping, and centers it.
    pub fn find(&mut self, term: &str) -> bool {
        if !term.is_empty() {
            self.search = term.to_lowercase();
        }
        let els = &self.canvas.elements;
        let from = match self.selection.as_slice() {
            [id] => els.iter().position(|e| e.id() == id).map_or(0, |i| i + 1),
            _ => 0,
        };
        let hit = (0..els.len()).map(|i| (from + i) % els.len()).find(|&i| {
            let label = match &els[i] {
                Element::Shape(s) => &s.label,
                Element::Line(l) => &l.label,
                Element::Text(t) => &t.text,
            };
            !self.search.is_empty() && label.to_lowercase().contains(&self.search)
        });
        let Some(i) = hit else { return false };
        let b = self.canvas.bounds(&els[i]);
        self.selection = vec![els[i].id().clone()];
        self.fit(Some(b));
        true
    }

    pub fn start_edit_existing(&mut self, id: Id) {
        let buf = match self.canvas.get(&id) {
            Some(Element::Shape(s)) => s.label.clone(),
            Some(Element::Line(l)) => l.label.clone(),
            Some(Element::Text(t)) => t.text.clone(),
            None => return,
        };
        let multiline = matches!(self.canvas.get(&id), Some(Element::Shape(_)));
        self.selection = vec![id.clone()];
        self.edit = Some(Edit {
            multiline,
            ..Edit::new(EditTarget::Existing(id), buf)
        });
    }

    pub fn start_edit_new(&mut self, p: (i32, i32)) {
        self.edit = Some(Edit::new(EditTarget::New(p.0, p.1), String::new()));
    }

    pub fn commit_edit(&mut self) {
        let Some(Edit { target, buf, .. }) = self.edit.take() else {
            return;
        };
        match target {
            EditTarget::Existing(id) => {
                let empty_text =
                    matches!(self.canvas.get(&id), Some(Element::Text(_))) && buf.is_empty();
                if empty_text {
                    self.exec(Command::Remove(id));
                } else if let Some(Element::Shape(sh)) = self.canvas.get(&id) {
                    let need = wrap(&buf, (sh.w - 2).max(1) as usize).len() as i32 + 2;
                    let (x, y, w, h) = (sh.x, sh.y, sh.w, sh.h);
                    let relabel = Command::SetLabel {
                        id: id.clone(),
                        label: buf,
                    };
                    self.exec(if need > h {
                        Command::Batch(vec![
                            relabel,
                            Command::SetRect {
                                id,
                                x,
                                y,
                                w: w.max(Shape::ratio_width(sh.kind, need).unwrap_or(0)),
                                h: need,
                            },
                        ])
                    } else {
                        relabel
                    });
                } else {
                    self.exec(Command::SetLabel { id, label: buf });
                }
            }
            EditTarget::New(x, y) if !buf.is_empty() => {
                let id = Id::random();
                let at = self.canvas.elements.len();
                self.exec(Command::Insert(
                    at,
                    Element::Text(Text {
                        id: id.clone(),
                        x,
                        y,
                        text: buf,
                        color: None,
                    }),
                ));
                self.selection = vec![id];
            }
            EditTarget::New(..) => {}
        }
    }
}
