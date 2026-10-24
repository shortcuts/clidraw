use super::infobar::fit;
use super::style::{ACTIVE, MUTED};
use super::{boxed, centered, Grid};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::cmp::Reverse;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug)]
pub struct Row<T> {
    pub value: T,
    pub label: String,
    pub key: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Center,
    At(i32, i32),
}

pub enum Outcome<T> {
    Run(T),
    Close,
    Key(char, Option<T>),
    Idle,
}

pub enum Click<T> {
    /// The value of the row and the click column minus the column where its key text starts.
    Row(T, i32),
    Inside,
    Outside,
}

/// Boxed list shared by every overlay that picks a row: context menu, export scope, switcher, palette.
#[derive(Clone, Debug)]
pub struct List<T: Copy> {
    title: &'static str,
    rows: Vec<Row<T>>,
    query: Option<String>,
    selected: usize,
    pub anchor: Anchor,
    min_width: i32,
    max_rows: usize,
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

fn width(s: &str) -> i32 {
    s.width() as i32
}

impl<T: Copy> List<T> {
    pub fn new(title: &'static str, rows: Vec<Row<T>>) -> List<T> {
        List {
            title,
            rows,
            query: None,
            selected: 0,
            anchor: Anchor::Center,
            min_width: 0,
            max_rows: 10,
        }
    }

    pub fn anchor(mut self, anchor: Anchor) -> List<T> {
        self.anchor = anchor;
        self
    }

    pub fn min_width(mut self, w: i32) -> List<T> {
        self.min_width = w;
        self
    }

    pub fn with_query(mut self) -> List<T> {
        self.query = Some(String::new());
        self
    }

    pub fn select(mut self, index: usize) -> List<T> {
        self.selected = index;
        self
    }

    fn view(&self) -> Vec<&Row<T>> {
        let query = self.query.as_deref().unwrap_or("");
        let mut m: Vec<(i32, &Row<T>)> = self
            .rows
            .iter()
            .filter_map(|r| Some((score(query, &r.label)?, r)))
            .collect();
        if !query.trim().is_empty() {
            m.sort_by_key(|(s, r)| (Reverse(*s), r.label.chars().count()));
        }
        m.into_iter().map(|e| e.1).collect()
    }

    pub fn labels(&self) -> Vec<&str> {
        self.view().iter().map(|r| r.label.as_str()).collect()
    }

    pub fn selected(&self) -> Option<T> {
        self.view().get(self.selected).map(|r| r.value)
    }

    pub fn push(&mut self, c: char) {
        if let Some(q) = &mut self.query {
            q.push(c);
            self.selected = 0;
        }
    }

    pub fn pop(&mut self) {
        if let Some(q) = &mut self.query {
            q.pop();
            self.selected = 0;
        }
    }

    fn first_visible(&self) -> usize {
        self.selected.saturating_sub(self.max_rows - 1)
    }

    fn size(&self) -> (i32, i32) {
        let label = self.rows.iter().map(|r| width(&r.label)).max().unwrap_or(0);
        let key = self.rows.iter().map(|r| width(&r.key)).max().unwrap_or(0);
        let gap = if key > 0 { 2 } else { 0 };
        let w = self
            .min_width
            .max(label + gap + key + 4)
            .max(width(self.title) + 6);
        let visible = if self.query.is_some() {
            self.max_rows
        } else {
            self.rows.len().min(self.max_rows)
        };
        (w, visible as i32 + 2 + i32::from(self.query.is_some()))
    }

    fn rect(&self, screen: (u16, u16)) -> (i32, i32, i32, i32) {
        let (w, h) = self.size();
        match self.anchor {
            Anchor::Center => centered(screen, w, h),
            Anchor::At(x, y) => {
                let (sw, sh) = (i32::from(screen.0), i32::from(screen.1));
                let (w, h) = (w.min(sw), h.min(sh));
                (x.min(sw - w).max(0), y.min(sh - h).max(0), w, h)
            }
        }
    }

    pub fn draw(&self, grid: &mut Grid) {
        let r = self.rect((grid.w as u16, grid.h as u16));
        boxed(grid, r, self.title);
        if let Some(q) = &self.query {
            grid.text(r.0 + 2, r.1 + 1, &format!("> {q}_"), 0);
        }
        let top = r.1 + 1 + i32::from(self.query.is_some());
        let view = self.view();
        if view.is_empty() {
            grid.text(r.0 + 2, top, "No match", MUTED);
        }
        let first = self.first_visible();
        for (i, row) in view.iter().skip(first).take(self.max_rows).enumerate() {
            let y = top + i as i32;
            if y >= r.1 + r.3 - 1 {
                break;
            }
            let on = first + i == self.selected;
            let key_w = width(&row.key);
            let gap = i32::from(key_w > 0);
            let label = fit(&row.label, (r.2 - 4 - key_w - gap).max(0));
            if on {
                grid.text(r.0 + 1, y, &" ".repeat((r.2 - 2) as usize), ACTIVE);
            }
            grid.text(r.0 + 2, y, &label, if on { ACTIVE } else { 0 });
            grid.text(
                r.0 + r.2 - 2 - key_w,
                y,
                &row.key,
                if on { ACTIVE } else { MUTED },
            );
        }
    }

    pub fn key(&mut self, k: KeyEvent) -> Outcome<T> {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let plain = !ctrl && !k.modifiers.contains(KeyModifiers::ALT);
        let last = self.view().len().saturating_sub(1);
        match k.code {
            KeyCode::Esc => match &mut self.query {
                Some(q) if !q.is_empty() => {
                    q.clear();
                    self.selected = 0;
                    Outcome::Idle
                }
                _ => Outcome::Close,
            },
            KeyCode::Enter => self.selected().map_or(Outcome::Idle, Outcome::Run),
            KeyCode::Up => self.step(-1, last),
            KeyCode::Down => self.step(1, last),
            KeyCode::Char('p') if ctrl => self.step(-1, last),
            KeyCode::Char('n') if ctrl => self.step(1, last),
            KeyCode::Backspace if self.query.is_some() => {
                self.pop();
                Outcome::Idle
            }
            KeyCode::Char(c) if plain && self.query.is_some() => {
                self.push(c);
                Outcome::Idle
            }
            KeyCode::Char('j') if plain => self.step(1, last),
            KeyCode::Char('k') if plain => self.step(-1, last),
            KeyCode::Char(c) if plain => Outcome::Key(c, self.selected()),
            _ => Outcome::Idle,
        }
    }

    fn step(&mut self, by: isize, last: usize) -> Outcome<T> {
        self.selected = self.selected.saturating_add_signed(by).min(last);
        Outcome::Idle
    }

    pub fn click(&self, screen: (u16, u16), col: i32, row: i32) -> Click<T> {
        let (x, y, w, h) = self.rect(screen);
        if col < x || col >= x + w || row < y || row >= y + h {
            return Click::Outside;
        }
        let top = y + 1 + i32::from(self.query.is_some());
        let Ok(i) = usize::try_from(row - top) else {
            return Click::Inside;
        };
        let inner = col > x && col < x + w - 1 && row < y + h - 1;
        match self.view().get(self.first_visible() + i) {
            Some(r) if inner => Click::Row(r.value, col - (x + w - 2 - width(&r.key))),
            _ => Click::Inside,
        }
    }
}
