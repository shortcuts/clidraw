mod drag;
mod edit;
mod hit;
mod keys;
mod state;

pub use edit::{Edit, EditBox, EditTarget};
pub use hit::{handles, shape_at, Corner, Handle, Hit};
pub use keys::{handle_key, handle_paste, KeyOutcome};
pub use state::{handle_mouse, scroll_step};

use crate::model::{wrap, Canvas, Command, Element, History, Id, Text};
use crate::ui::palette::Dock;
use crate::ui::Layout;
use crate::view::View;
use state::Gesture;
use std::cell::RefCell;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Hand,
    Rect,
    Ellipse,
    Diamond,
    Cylinder,
    Parallelogram,
    Hexagon,
    Line,
    Arrow,
    Text,
}

impl Tool {
    pub const ALL: [Tool; 11] = [
        Tool::Select,
        Tool::Hand,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Diamond,
        Tool::Cylinder,
        Tool::Parallelogram,
        Tool::Hexagon,
        Tool::Line,
        Tool::Arrow,
        Tool::Text,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Hand => "Hand",
            Tool::Rect => "Rect",
            Tool::Ellipse => "Ellipse",
            Tool::Diamond => "Diamond",
            Tool::Cylinder => "Cylinder",
            Tool::Parallelogram => "Slant",
            Tool::Hexagon => "Hexagon",
            Tool::Line => "Line",
            Tool::Arrow => "Arrow",
            Tool::Text => "Text",
        }
    }

    pub fn key(self) -> char {
        match self {
            Tool::Select => 'v',
            Tool::Hand => 'h',
            Tool::Rect => 'r',
            Tool::Ellipse => 'o',
            Tool::Diamond => 'd',
            Tool::Cylinder => 'y',
            Tool::Parallelogram => 'p',
            Tool::Hexagon => 'x',
            Tool::Line => 'l',
            Tool::Arrow => 'a',
            Tool::Text => 't',
        }
    }
}

/// Everything the editor knows apart from files and overlays.
pub struct Session {
    pub canvas: Canvas,
    pub history: History,
    pub view: View,
    pub tool: Tool,
    pub selection: Vec<Id>,
    pub clipboard: Canvas,
    pub dock: Dock,
    pub double_click: Duration,
    /// Cells between snap lines for drawn, moved and resized shapes; 1 means no snapping.
    pub grid: i32,
    pub screen: (u16, u16),
    pub edit: Option<Edit>,
    /// Compact read-only view of the whole diagram; `view` stays untouched.
    pub overview: bool,
    pub band: Option<((i32, i32), (i32, i32))>,
    /// Alignment guide segments (world cells, inclusive) while a move drag snaps to other shapes.
    pub guides: Vec<((i32, i32), (i32, i32))>,
    /// Shape under a dragged line end; the drop binds it.
    pub snap: Option<Id>,
    /// What a click at the pointer would hit; only the Select tool at rest sets it.
    pub hover: Hit,
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
}

impl Session {
    pub fn new(canvas: Canvas, view: View, screen: (u16, u16)) -> Session {
        Session {
            canvas,
            history: History::default(),
            view,
            tool: Tool::Select,
            selection: vec![],
            clipboard: Canvas::default(),
            dock: Dock::Left,
            double_click: Duration::from_millis(400),
            grid: 1,
            screen,
            edit: None,
            overview: false,
            band: None,
            guides: vec![],
            snap: None,
            hover: Hit::Empty,
            rev: 0,
            layout: RefCell::new(None),
            gesture: Gesture::Idle,
            last_down: None,
            burst: None,
            cursor: (0, 0),
            shift: false,
            last_pan: None,
        }
    }

    /// Autosave must not stall a drag step.
    pub fn dragging(&self) -> bool {
        matches!(self.gesture, Gesture::Dragging { .. })
    }

    pub fn exec(&mut self, cmd: Command) {
        if self.history.exec(&mut self.canvas, cmd).is_ok() {
            self.rev += 1;
            self.prune_selection();
        }
    }

    pub fn undo(&mut self) -> Option<String> {
        let label = self.history.undo(&mut self.canvas).ok()??;
        self.rev += 1;
        self.prune_selection();
        Some(format!("Undid: {label}"))
    }

    pub fn redo(&mut self) -> Option<String> {
        let label = self.history.redo(&mut self.canvas).ok()??;
        self.rev += 1;
        self.prune_selection();
        Some(format!("Redid: {label}"))
    }

    fn idle(&self) -> bool {
        self.edit.is_none() && matches!(self.gesture, Gesture::Idle)
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

    pub fn duplicate(&mut self) {
        if self.idle() && !self.selection.is_empty() {
            let new = self.canvas.clone_canvas(&self.selection, 2, 1);
            self.insert(new);
        }
    }

    fn insert(&mut self, new: Canvas) {
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
            dock: self.dock,
            double_click: self.double_click,
            grid: self.grid,
            history: History::with_limit(self.history.limit),
            ..Session::new(canvas, view, self.screen)
        };
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
                                w,
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
                    }),
                ));
                self.selection = vec![id];
            }
            EditTarget::New(..) => {}
        }
    }
}
