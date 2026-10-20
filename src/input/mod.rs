mod drag;
mod hit;
mod keys;
mod state;

pub use hit::{handles, shape_at, Corner, Handle, Hit};
pub use keys::{handle_key, handle_paste, KeyOutcome};
pub use state::{handle_mouse, scroll_step};

use crate::model::{Canvas, Command, Element, History, Id, Text};
use crate::ui::palette::Dock;
use crate::view::View;
use state::Gesture;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Hand,
    Rect,
    Ellipse,
    Diamond,
    Line,
    Arrow,
    Text,
}

impl Tool {
    pub const ALL: [Tool; 8] = [
        Tool::Select,
        Tool::Hand,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Diamond,
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
            Tool::Line => "Line",
            Tool::Arrow => "Arrow",
            Tool::Text => "Text",
        }
    }
}

pub enum EditTarget {
    Existing(Id),
    New(i32, i32),
}

pub struct Edit {
    pub target: EditTarget,
    pub buf: String,
}

/// Everything the editor knows apart from files and overlays.
pub struct Session {
    pub canvas: Canvas,
    pub history: History,
    pub view: View,
    pub tool: Tool,
    pub selection: Vec<Id>,
    pub dock: Dock,
    pub double_click: Duration,
    pub screen: (u16, u16),
    pub edit: Option<Edit>,
    pub band: Option<((i32, i32), (i32, i32))>,
    /// Shape under a dragged line end; the drop binds it.
    pub snap: Option<Id>,
    /// Counts canvas mutations; autosave compares it with the saved value.
    pub rev: u64,
    gesture: Gesture,
    last_down: Option<(i32, i32, Instant)>,
}

impl Session {
    pub fn new(canvas: Canvas, view: View, screen: (u16, u16)) -> Session {
        Session {
            canvas,
            history: History::default(),
            view,
            tool: Tool::Select,
            selection: vec![],
            dock: Dock::Left,
            double_click: Duration::from_millis(400),
            screen,
            edit: None,
            band: None,
            snap: None,
            rev: 0,
            gesture: Gesture::Idle,
            last_down: None,
        }
    }

    pub fn exec(&mut self, cmd: Command) {
        if self.history.exec(&mut self.canvas, cmd).is_ok() {
            self.rev += 1;
            self.prune_selection();
        }
    }

    pub fn undo(&mut self) {
        if matches!(self.history.undo(&mut self.canvas), Ok(true)) {
            self.rev += 1;
            self.prune_selection();
        }
    }

    pub fn redo(&mut self) {
        if matches!(self.history.redo(&mut self.canvas), Ok(true)) {
            self.rev += 1;
            self.prune_selection();
        }
    }

    fn prune_selection(&mut self) {
        let canvas = &self.canvas;
        self.selection.retain(|id| canvas.get(id).is_some());
    }

    pub fn replace_canvas(&mut self, canvas: Canvas, view: View) {
        *self = Session {
            dock: self.dock,
            double_click: self.double_click,
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
        self.selection = vec![id.clone()];
        self.edit = Some(Edit {
            target: EditTarget::Existing(id),
            buf,
        });
    }

    pub fn start_edit_new(&mut self, p: (i32, i32)) {
        self.edit = Some(Edit {
            target: EditTarget::New(p.0, p.1),
            buf: String::new(),
        });
    }

    pub fn commit_edit(&mut self) {
        let Some(Edit { target, buf }) = self.edit.take() else {
            return;
        };
        match target {
            EditTarget::Existing(id) => {
                let empty_text =
                    matches!(self.canvas.get(&id), Some(Element::Text(_))) && buf.is_empty();
                if empty_text {
                    self.exec(Command::Remove(id));
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
