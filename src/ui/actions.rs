use super::list::{List, Row};
use super::menu::{ContextItem, MenuItem};
use super::registry::{self, Scope};
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

pub fn list(s: &Session) -> List<Action> {
    let rows = registry::BINDINGS
        .iter()
        .filter_map(|b| Some((b, b.action?)))
        .filter(|(_, action)| action.applies(s))
        .map(|(b, value)| Row {
            value,
            label: match b.scope {
                Scope::Tool => format!("Tool: {}", b.name()),
                _ => b.label.to_string(),
            },
            key: b.keys_label(),
        })
        .collect();
    List::new("Actions", rows).with_query().min_width(52)
}
