use super::actions::Action;
use super::list::{Anchor, List, Row};
use super::registry::{self, Binding};
use crate::model::Z;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    Canvases,
    FitAll,
    ExportSvg,
    ExportMermaid,
    ExportClidraw,
    CopyClidraw,
    CopyShareLink,
    CopyForObsidian,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextItem {
    EditLabel,
    Duplicate,
    Delete,
    Front,
    Forward,
    Backward,
    Back,
    ResetRoute,
    Paste,
    SelectAll,
    FitAll,
}

impl ContextItem {
    fn action(self) -> Action {
        match self {
            ContextItem::Front => Action::Order(Z::Front),
            ContextItem::Forward => Action::Order(Z::Forward),
            ContextItem::Backward => Action::Order(Z::Backward),
            ContextItem::Back => Action::Order(Z::Back),
            ContextItem::FitAll => Action::Menu(MenuItem::FitAll),
            _ => Action::Context(self),
        }
    }
}

fn list(
    title: &'static str,
    at: (i32, i32),
    items: Vec<(ContextItem, &'static str)>,
) -> List<ContextItem> {
    let rows = items
        .into_iter()
        .map(|(value, label)| Row {
            value,
            label: label.to_string(),
            key: registry::for_action(value.action())
                .map(Binding::first_label)
                .unwrap_or_default(),
        })
        .collect();
    List::new(title, rows).anchor(Anchor::At(at.0, at.1))
}

pub fn element(at: (i32, i32), single: bool, bent: bool) -> List<ContextItem> {
    let edit = single.then_some((ContextItem::EditLabel, "Edit label"));
    let items = edit.into_iter().chain([
        (ContextItem::Duplicate, "Duplicate"),
        (ContextItem::Delete, "Delete"),
        (ContextItem::Front, "Bring to front"),
        (ContextItem::Forward, "Raise"),
        (ContextItem::Backward, "Lower"),
        (ContextItem::Back, "Send to back"),
    ]);
    let reset = bent.then_some((ContextItem::ResetRoute, "Reset route"));
    list("Element", at, items.chain(reset).collect())
}

pub fn canvas(at: (i32, i32), can_paste: bool) -> List<ContextItem> {
    let paste = can_paste.then_some((ContextItem::Paste, "Paste"));
    let items = paste.into_iter().chain([
        (ContextItem::SelectAll, "Select all"),
        (ContextItem::FitAll, "Fit all"),
    ]);
    list("Canvas", at, items.collect())
}
