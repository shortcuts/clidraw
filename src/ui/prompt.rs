use super::grid::REVERSE;
use super::list::{Anchor, List, Row};
use super::registry::{self, Binding};
use super::style::{ACTIVE, ERROR, MUTED};
use super::{boxed, centered, Grid};
use crate::diagram::{self, Severity};
use crate::input::{Edit, EditTarget};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// The part of the canvas an export writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extent {
    Whole,
    Area((i32, i32, i32, i32)),
    Selection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptKind {
    ExportSvg(Extent),
    ExportMermaid(Extent),
    ExportClidraw(Extent),
    NewCanvas,
    Rename(String),
    ConfirmDelete(String),
    Command,
    Mermaid(String),
}

pub struct Prompt {
    pub kind: PromptKind,
    pub edit: Edit,
    /// Highlighted completion of the command line.
    pub sel: usize,
    /// Offer and apply the first diagnostic's fix.
    pub fixes: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptHit {
    Ok,
    Cancel,
}

const HINT_GAP: i32 = 2;
const MIN_HINT: i32 = 4;
const OK: &str = "[ OK ]";
const CANCEL: &str = "[ Cancel ]";

fn rect(screen: (u16, u16)) -> (i32, i32, i32, i32) {
    centered(screen, 52, 5)
}

impl Prompt {
    pub fn new(kind: PromptKind, buf: String) -> Prompt {
        // target is unused for prompts
        let edit = Edit::new(EditTarget::New(0, 0), buf);
        Prompt {
            kind,
            edit,
            sel: 0,
            fixes: false,
        }
    }

    pub fn buf(&self) -> &str {
        &self.edit.buf
    }

    /// Find mode starts with `/` and has no completions.
    pub fn completing(&self) -> bool {
        self.kind == PromptKind::Command && !self.buf().starts_with('/')
    }

    /// The first diagnostic of the session header plus the typed line.
    fn diagnostic(&self) -> Option<diagram::Diagnostic> {
        let PromptKind::Mermaid(header) = &self.kind else {
            return None;
        };
        if self.buf().trim().is_empty() {
            return None;
        }
        diagram::validate(&format!("{header}\n{}", self.buf()))
            .into_iter()
            .next()
    }

    /// Replaces the typed line with the first diagnostic's fix.
    pub fn apply_fix(&mut self) {
        let (
            PromptKind::Mermaid(header),
            Some(diagram::Diagnostic {
                range,
                fix: Some(fix),
                ..
            }),
        ) = (&self.kind, self.diagnostic())
        else {
            return;
        };
        let mut text = format!("{header}\n{}", self.buf());
        text.replace_range(range, &fix.replacement);
        let buf = text[header.len() + 1..].to_string();
        self.edit = Edit::new(EditTarget::New(0, 0), buf);
    }

    fn title(&self) -> &'static str {
        match self.kind {
            PromptKind::ExportSvg(_) => "Export SVG to path",
            PromptKind::ExportMermaid(_) => "Export Mermaid to path",
            PromptKind::ExportClidraw(_) => "Export Clidraw to path",
            PromptKind::NewCanvas => "New canvas name",
            PromptKind::Rename(_) => "Rename canvas to",
            PromptKind::ConfirmDelete(_) => "Delete canvas",
            PromptKind::Command | PromptKind::Mermaid(_) => "",
        }
    }
}

pub fn hit(screen: (u16, u16), col: i32, row: i32) -> Option<PromptHit> {
    let (x, y, ..) = rect(screen);
    if row != y + 3 {
        return None;
    }
    let ok = x + 2..x + 2 + OK.len() as i32;
    let cancel = ok.end + 2..ok.end + 2 + CANCEL.len() as i32;
    if ok.contains(&col) {
        Some(PromptHit::Ok)
    } else if cancel.contains(&col) {
        Some(PromptHit::Cancel)
    } else {
        None
    }
}

pub fn draw(grid: &mut Grid, p: &Prompt) {
    let r = rect((grid.w as u16, grid.h as u16));
    boxed(grid, r, p.title());
    match &p.kind {
        PromptKind::ConfirmDelete(name) => {
            grid.text(r.0 + 2, r.1 + 1, &format!("Delete \"{name}\"?"), 0)
        }
        _ => caret_line(grid, r.0 + 2, r.1 + 1, r.2 - 4, "", p),
    }
    grid.text(r.0 + 2, r.1 + 3, OK, ACTIVE);
    grid.text(r.0 + 4 + OK.len() as i32, r.1 + 3, CANCEL, 0);
}

/// Draws `prefix + buffer` with a reversed caret cell, dropping the head when the caret would leave `w` columns.
fn caret_line(grid: &mut Grid, x: i32, y: i32, w: i32, prefix: &str, p: &Prompt) {
    let (buf, caret) = (p.buf(), p.edit.caret);
    let mut line = format!("{prefix}{buf} ");
    let mut at = (prefix.width() + buf[..caret].width()) as i32;
    // shortcut: drops the head, no ellipsis
    while at >= w && !line.is_empty() {
        let c = line.remove(0);
        at -= c.width().unwrap_or(0) as i32;
    }
    grid.text(x, y, &line, 0);
    let under = buf[caret..].chars().next().unwrap_or(' ');
    grid.set(x + at, y, under, REVERSE);
}

pub fn command_list(p: &Prompt, bar: i32) -> List<&'static Binding> {
    let rows: Vec<_> = registry::commands()
        .map(|b| Row {
            value: b,
            label: b.shown.unwrap_or("").into(),
            key: b.name().into(),
        })
        .collect();
    let all = rows.len();
    let mut list = List::new("Commands", rows)
        .headless()
        .min_width(44)
        .max_rows(all);
    list.set_query(p.buf().split_whitespace().next().unwrap_or(""));
    list.anchor(Anchor::Bottom(bar)).select(p.sel)
}

/// Replaces the bottom row with the command line and draws the completions above it.
pub fn draw_command(grid: &mut Grid, p: &Prompt) {
    let row = grid.h - 1;
    grid.fill(0, row, grid.w, 1);
    if p.completing() {
        command_list(p, row).draw(grid);
    }
    let prefix = match &p.kind {
        PromptKind::Mermaid(_) => "mmd> ",
        _ if p.buf().starts_with('/') => "",
        _ => ":",
    };
    caret_line(grid, 0, row, grid.w, prefix, p);
    if let Some(d) = p.diagnostic() {
        let (label, style) = match d.severity {
            Severity::Error => ("error", ERROR),
            Severity::Warning => ("warning", MUTED),
        };
        let x = (prefix.width() + p.buf().width()) as i32 + HINT_GAP;
        let room = grid.w - x;
        if room >= MIN_HINT {
            let mut text = String::new();
            let mut shown = format!("{label}: {}", d.message);
            if let Some(fix) = d.fix.filter(|_| p.fixes) {
                let key = registry::by_id("apply_fix").first_label();
                shown.push_str(&format!("  {key}: {}", fix.label));
            }
            for c in shown.chars() {
                if (text.width() + c.width().unwrap_or(0)) as i32 > room {
                    break;
                }
                text.push(c);
            }
            grid.text(x, row, &text, style);
        }
    }
}
