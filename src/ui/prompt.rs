use super::style::{ACTIVE, MUTED};
use super::{boxed, centered, Grid};

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
    ExportText(Extent),
    NewCanvas,
    Rename(String),
    ConfirmDelete(String),
    Command,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub kind: PromptKind,
    pub buf: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptHit {
    Ok,
    Cancel,
}

const OK: &str = "[ OK ]";
const CANCEL: &str = "[ Cancel ]";

fn rect(screen: (u16, u16)) -> (i32, i32, i32, i32) {
    centered(screen, 52, 5)
}

impl Prompt {
    pub fn new(kind: PromptKind, buf: String) -> Prompt {
        Prompt { kind, buf }
    }

    fn title(&self) -> &'static str {
        match self.kind {
            PromptKind::ExportSvg(_) => "Export SVG to path",
            PromptKind::ExportMermaid(_) => "Export Mermaid to path",
            PromptKind::ExportText(_) => "Export Text to path",
            PromptKind::NewCanvas => "New canvas name",
            PromptKind::Rename(_) => "Rename canvas to",
            PromptKind::ConfirmDelete(_) => "Delete canvas",
            PromptKind::Command => "Command",
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
    let line = match &p.kind {
        PromptKind::ConfirmDelete(name) => format!("Delete \"{name}\"?"),
        _ => format!("{}_", p.buf),
    };
    grid.text(r.0 + 2, r.1 + 1, &line, 0);
    grid.text(r.0 + 2, r.1 + 2, "Enter confirms, Esc cancels", MUTED);
    grid.text(r.0 + 2, r.1 + 3, OK, ACTIVE);
    grid.text(r.0 + 4 + OK.len() as i32, r.1 + 3, CANCEL, 0);
}
