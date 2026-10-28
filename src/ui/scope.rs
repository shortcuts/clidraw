use super::list::{List, Row};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Svg,
    Mermaid,
    Clidraw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sink {
    File(ExportFormat),
    ShareLink,
    ObsidianCopy,
    ObsidianNote,
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

pub fn list(sink: Sink) -> List<ScopeItem> {
    List::new(
        match sink {
            Sink::File(ExportFormat::Svg) => "Export SVG",
            Sink::File(ExportFormat::Mermaid) => "Export Mermaid",
            Sink::File(ExportFormat::Clidraw) => "Export Clidraw",
            Sink::ShareLink => "Copy share link",
            Sink::ObsidianCopy => "Copy for Obsidian",
            Sink::ObsidianNote => "Save to Obsidian note",
        },
        vec![
            row(ScopeItem::Whole, "Whole canvas"),
            row(ScopeItem::Area, "Area"),
            row(ScopeItem::Selection, "Selection"),
        ],
    )
}
