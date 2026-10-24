use super::list::{List, Row};
use super::menu::{ContextItem, MenuItem};
use super::registry;
use crate::input::{Session, Tool};
use crate::model::{Align, Z};

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
    Prefill(&'static str),
    NewCanvas,
    RenameCanvas,
    DeleteCanvas,
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
            Action::Prefill("link ") => !s.selection.is_empty(),
            _ => true,
        }
    }

    pub fn category(self) -> &'static str {
        match self {
            Action::Tool(_) => "Tool",
            Action::Align(_) | Action::Order(_) | Action::Ex("layout" | "layout lr") => "Arrange",
            Action::Menu(MenuItem::FitAll)
            | Action::Map
            | Action::ZoomIn
            | Action::ZoomOut
            | Action::Present
            | Action::Help
            | Action::Find
            | Action::Ex("perf") => "View",
            Action::Menu(MenuItem::Canvases)
            | Action::NewCanvas
            | Action::RenameCanvas
            | Action::DeleteCanvas => "Canvas",
            Action::Menu(_) | Action::Ex("w") | Action::Prefill("import ") => "File",
            _ => "Edit",
        }
    }
}

pub fn list(s: &Session) -> List<Action> {
    let rows = registry::BINDINGS
        .iter()
        .filter_map(|b| Some((b, b.action?)))
        .filter(|(_, action)| action.applies(s))
        .map(|(b, value)| Row {
            value,
            label: format!("{}: {}", value.category(), b.name()),
            key: b.keys_label(),
        })
        .collect();
    List::new("Actions", rows).with_query().min_width(52)
}
