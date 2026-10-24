use super::list::{List, Row};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Svg,
    Mermaid,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeItem {
    Whole,
    Area,
    Selection,
}

pub fn list() -> List<ScopeItem> {
    let row = |value, label: &str| Row {
        value,
        label: label.to_string(),
        key: String::new(),
    };
    List::new(
        "Export",
        vec![
            row(ScopeItem::Whole, "Whole canvas"),
            row(ScopeItem::Area, "Area"),
            row(ScopeItem::Selection, "Selection"),
        ],
    )
}
