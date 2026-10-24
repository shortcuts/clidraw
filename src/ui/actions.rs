use super::menu::{ContextItem, MenuItem};
use super::registry::{self, Scope};
use super::style::{ACTIVE, MUTED};
use super::{boxed, centered, Grid};
use crate::input::{Session, Tool};
use crate::model::{Align, Z};
use std::cmp::Reverse;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Tool(Tool),
    Menu(MenuItem),
    Context(ContextItem),
    Align(Align),
    Order(Z),
    Group,
    Ungroup,
    Bold,
    Copy,
    Cut,
    ZoomIn,
    ZoomOut,
    Present,
    Help,
    Find,
    Map,
    Ex(&'static str),
}

impl Action {
    fn applies(self, s: &Session) -> bool {
        match self {
            Action::Context(ContextItem::EditLabel) => s.selection.len() == 1,
            Action::Context(ContextItem::Paste) => !s.clipboard.elements.is_empty(),
            Action::Context(ContextItem::Duplicate | ContextItem::Delete)
            | Action::Align(_)
            | Action::Order(_)
            | Action::Group
            | Action::Ungroup
            | Action::Bold
            | Action::Copy
            | Action::Cut => !s.selection.is_empty(),
            _ => true,
        }
    }
}

#[derive(Clone, Debug)]
struct Entry {
    action: Action,
    label: String,
    keys: String,
}

/// Command palette: every action that can act now, filtered by the query.
pub struct Actions {
    query: String,
    entries: Vec<Entry>,
    selected: usize,
}

/// Case-insensitive subsequence score; None when the query does not fit the label.
// shortcut: greedy match can miss the best alignment, upgrade if rankings feel off
fn score(query: &str, label: &str) -> Option<i32> {
    let label: Vec<char> = label.to_lowercase().chars().collect();
    let (mut total, mut at, mut prev) = (0, 0, None::<usize>);
    for q in query.to_lowercase().chars().filter(|c| !c.is_whitespace()) {
        let i = at + label[at..].iter().position(|&c| c == q)?;
        total += 1;
        if prev.is_some_and(|p| p + 1 == i) {
            total += 2;
        }
        if i == 0 || matches!(label[i - 1], ' ' | ':') {
            total += 3;
        }
        (prev, at) = (Some(i), i + 1);
    }
    Some(total)
}

const WIDTH: i32 = 52;
const MAX_ROWS: usize = 10;

fn rect(screen: (u16, u16)) -> (i32, i32, i32, i32) {
    centered(screen, WIDTH, MAX_ROWS as i32 + 3)
}

impl Actions {
    pub fn new(s: &Session) -> Actions {
        let entries = registry::BINDINGS
            .iter()
            .filter_map(|b| Some((b, b.action?)))
            .filter(|(_, action)| action.applies(s))
            .map(|(b, action)| Entry {
                action,
                label: match b.scope {
                    Scope::Tool => format!("Tool: {}", b.name()),
                    _ => b.label.to_string(),
                },
                keys: b.keys_label(),
            })
            .collect();
        Actions {
            query: String::new(),
            entries,
            selected: 0,
        }
    }

    fn matches(&self) -> Vec<&Entry> {
        let mut m: Vec<(i32, &Entry)> = self
            .entries
            .iter()
            .filter_map(|e| Some((score(&self.query, &e.label)?, e)))
            .collect();
        if !self.query.trim().is_empty() {
            m.sort_by_key(|(s, e)| (Reverse(*s), e.label.chars().count()));
        }
        m.into_iter().map(|e| e.1).collect()
    }

    pub fn labels(&self) -> Vec<&str> {
        self.matches().iter().map(|e| e.label.as_str()).collect()
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
        self.selected = 0;
    }

    pub fn pop(&mut self) {
        self.query.pop();
        self.selected = 0;
    }

    pub fn up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn down(&mut self) {
        self.selected = (self.selected + 1).min(self.matches().len().saturating_sub(1));
    }

    pub fn selected_action(&self) -> Option<Action> {
        self.matches().get(self.selected).map(|e| e.action)
    }

    fn first_visible(&self) -> usize {
        self.selected.saturating_sub(MAX_ROWS - 1)
    }

    /// Action under a click on a visible row.
    pub fn row_at(&self, screen: (u16, u16), col: i32, row: i32) -> Option<Action> {
        let (x, y, w, h) = rect(screen);
        let i = usize::try_from(row - y - 2).ok()?;
        let inner = col > x && col < x + w - 1 && row < y + h - 1;
        let hit = inner.then(|| {
            self.matches()
                .get(self.first_visible() + i)
                .map(|e| e.action)
        });
        hit?
    }

    pub fn inside(&self, screen: (u16, u16), col: i32, row: i32) -> bool {
        let (x, y, w, h) = rect(screen);
        col >= x && col < x + w && row >= y && row < y + h
    }

    pub fn draw(&self, grid: &mut Grid) {
        let screen = (grid.w as u16, grid.h as u16);
        let r = rect(screen);
        boxed(grid, r, "Actions");
        grid.text(r.0 + 2, r.1 + 1, &format!("> {}_", self.query), 0);
        let m = self.matches();
        if m.is_empty() {
            grid.text(r.0 + 2, r.1 + 2, "No match", MUTED);
        }
        let first = self.first_visible();
        for (i, e) in m.iter().skip(first).take(MAX_ROWS).enumerate() {
            let y = r.1 + 2 + i as i32;
            if y >= r.1 + r.3 - 1 {
                break;
            }
            let on = first + i == self.selected;
            let keys_len = e.keys.chars().count() as i32;
            let room = (r.2 - 4 - keys_len - 1).max(0) as usize;
            let label: String = e.label.chars().take(room).collect();
            if on {
                grid.text(r.0 + 1, y, &" ".repeat((r.2 - 2) as usize), ACTIVE);
            }
            grid.text(r.0 + 2, y, &label, if on { ACTIVE } else { 0 });
            grid.text(
                r.0 + r.2 - 2 - keys_len,
                y,
                &e.keys,
                if on { ACTIVE } else { MUTED },
            );
        }
    }
}
