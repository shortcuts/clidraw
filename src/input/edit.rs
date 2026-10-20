use super::Session;
use crate::model::{label_pos, wrap, Element, Id};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub enum EditTarget {
    Existing(Id),
    New(i32, i32),
}

/// A label under edit. `caret` and `anchor` are byte offsets on char boundaries.
pub struct Edit {
    pub target: EditTarget,
    pub buf: String,
    pub caret: usize,
    pub anchor: Option<usize>,
    pub multiline: bool,
}

/// Where the renderer draws `buf[start..end]` and where a click lands.
#[derive(Clone, Copy)]
pub struct EditBox {
    pub x: i32,
    pub y: i32,
    pub start: usize,
    pub end: usize,
}

impl Edit {
    pub fn new(target: EditTarget, buf: String) -> Edit {
        let caret = buf.len();
        Edit {
            target,
            buf,
            caret,
            anchor: None,
            multiline: false,
        }
    }

    /// Sorted selection bounds; an anchor on the caret selects nothing.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let a = self.anchor?;
        (a != self.caret).then(|| (a.min(self.caret), a.max(self.caret)))
    }

    fn remove_selection(&mut self) -> bool {
        let sel = self.selection();
        self.anchor = None;
        let Some((start, end)) = sel else {
            return false;
        };
        self.buf.drain(start..end);
        self.caret = start;
        true
    }

    pub fn insert(&mut self, text: &str) {
        self.remove_selection();
        self.buf.insert_str(self.caret, text);
        self.caret += text.len();
    }

    pub fn backspace(&mut self) {
        if !self.remove_selection() {
            let prev = self.prev();
            self.buf.drain(prev..self.caret);
            self.caret = prev;
        }
    }

    pub fn delete(&mut self) {
        if !self.remove_selection() {
            self.buf.drain(self.caret..self.next());
        }
    }

    pub fn delete_word_left(&mut self) {
        if !self.remove_selection() {
            let to = self.word_left_pos();
            self.buf.drain(to..self.caret);
            self.caret = to;
        }
    }

    pub fn delete_to_start(&mut self) {
        self.buf.drain(..self.caret);
        self.caret = 0;
        self.anchor = None;
    }

    pub fn select_all(&mut self) {
        self.anchor = Some(0);
        self.caret = self.buf.len();
    }

    pub fn move_to(&mut self, pos: usize, select: bool) {
        self.anchor = if select {
            self.anchor.or(Some(self.caret))
        } else {
            None
        };
        self.caret = pos;
    }

    pub fn left(&mut self, select: bool) {
        match self.selection() {
            Some((start, _)) if !select => self.move_to(start, false),
            _ => self.move_to(self.prev(), select),
        }
    }

    pub fn right(&mut self, select: bool) {
        match self.selection() {
            Some((_, end)) if !select => self.move_to(end, false),
            _ => self.move_to(self.next(), select),
        }
    }

    pub fn home(&mut self, select: bool) {
        self.move_to(0, select);
    }

    pub fn end(&mut self, select: bool) {
        self.move_to(self.buf.len(), select);
    }

    pub fn word_left(&mut self, select: bool) {
        self.move_to(self.word_left_pos(), select);
    }

    pub fn word_right(&mut self, select: bool) {
        let rest = &self.buf[self.caret..];
        let word = rest.trim_start();
        let len = word.find(char::is_whitespace).unwrap_or(word.len());
        self.move_to(self.buf.len() - word.len() + len, select);
    }

    fn word_left_pos(&self) -> usize {
        let head = self.buf[..self.caret].trim_end();
        head.trim_end_matches(|c: char| !c.is_whitespace()).len()
    }

    fn prev(&self) -> usize {
        self.buf[..self.caret]
            .chars()
            .next_back()
            .map_or(0, |c| self.caret - c.len_utf8())
    }

    fn next(&self) -> usize {
        self.buf[self.caret..]
            .chars()
            .next()
            .map_or(self.caret, |c| self.caret + c.len_utf8())
    }
}

impl Session {
    /// The one source of label geometry: the renderer draws from it and clicks hit-test against it.
    pub fn edit_boxes(&self) -> Vec<EditBox> {
        let Some(edit) = self.edit.as_ref() else {
            return vec![];
        };
        let whole = |x, y| {
            vec![EditBox {
                x,
                y,
                start: 0,
                end: edit.buf.len(),
            }]
        };
        let id = match &edit.target {
            EditTarget::Existing(id) => id,
            EditTarget::New(x, y) => return whole(*x, *y),
        };
        match self.canvas.get(id) {
            Some(Element::Text(t)) => whole(t.x, t.y),
            Some(Element::Line(l)) => {
                let (lx, ly) = label_pos(&self.canvas.route(l), l.label_at);
                whole(lx - edit.buf.width() as i32 / 2, ly)
            }
            Some(Element::Shape(sh)) => {
                // Keep in step with the label placement in `shape` in ui/canvas_view.rs.
                let room = (sh.w - 2).max(1) as usize;
                let lines = wrap(&edit.buf, room);
                let top = sh.y + sh.h / 2 - (lines.len() as i32 - 1) / 2;
                lines
                    .into_iter()
                    .zip(top..)
                    .map(|(r, y)| {
                        let end = r.start + edit.buf[r.clone()].trim_end_matches([' ', '\n']).len();
                        let used = edit.buf[r.start..end].width();
                        EditBox {
                            x: sh.x + 1 + sh.align.offset(room, used),
                            y,
                            start: r.start,
                            end,
                        }
                    })
                    .collect()
            }
            None => vec![],
        }
    }

    /// Moves the caret to a click on the label; false when the click is elsewhere.
    pub(super) fn place_caret(&mut self, (wx, wy): (i32, i32)) -> bool {
        let Some(b) = self.edit_boxes().into_iter().find(|b| b.y == wy) else {
            return false;
        };
        let Some(edit) = &mut self.edit else {
            return false;
        };
        let shown = &edit.buf[b.start..b.end];
        let col = wx - b.x;
        if col < 0 || col > shown.width() as i32 {
            return false;
        }
        let mut x = 0;
        let mut pos = b.end;
        for (i, c) in shown.char_indices() {
            x += c.width().unwrap_or(0) as i32;
            if col < x {
                pos = b.start + i;
                break;
            }
        }
        edit.move_to(pos, false);
        true
    }
}
