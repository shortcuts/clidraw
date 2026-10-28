use super::list::{List, Row};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Svg,
    Mermaid,
    Clidraw,
    Share,
    Obsidian,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeItem {
    Whole,
    Area,
    Selection,
}

fn row<T>(value: T, label: &str) -> Row<T> {
    Row {
        value,
        label: label.to_string(),
        key: String::new(),
    }
}

pub fn formats() -> List<ExportFormat> {
    List::new(
        "Export",
        vec![
            row(ExportFormat::Svg, "SVG"),
            row(ExportFormat::Mermaid, "Mermaid"),
            row(ExportFormat::Clidraw, "Clidraw"),
        ],
    )
}

pub fn list(format: ExportFormat) -> List<ScopeItem> {
    List::new(
        match format {
            ExportFormat::Svg => "Export SVG",
            ExportFormat::Mermaid => "Export Mermaid",
            ExportFormat::Clidraw => "Export Clidraw",
            ExportFormat::Share => "Copy share link",
            ExportFormat::Obsidian => "Copy for Obsidian",
        },
        vec![
            row(ScopeItem::Whole, "Whole canvas"),
            row(ScopeItem::Area, "Area"),
            row(ScopeItem::Selection, "Selection"),
        ],
    )
}
