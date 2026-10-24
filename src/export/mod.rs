mod svg;

pub use crate::ui::text;

pub use svg::{svg, svg_padded};

use crate::diagram::{DiagramKind, Flowchart};
use crate::model::Canvas;
use crate::ui::scope::ExportFormat;

/// Returns the file text and the count of elements the format could not carry.
pub fn render(canvas: &Canvas, format: ExportFormat, padding: i32) -> (String, usize) {
    match format {
        ExportFormat::Svg => (svg_padded(canvas, padding), 0),
        ExportFormat::Text => (text(canvas), 0),
        ExportFormat::Mermaid => {
            let out = Flowchart.export(canvas);
            (out.text, out.skipped)
        }
    }
}
