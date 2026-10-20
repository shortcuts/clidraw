use crate::model::{Canvas, Element, End, Id, LineKind, ShapeKind, Stroke};
use std::fmt::Write;

pub struct MermaidOutput {
    pub text: String,
    pub skipped: usize,
}

fn label(s: &str) -> String {
    let s = if s.is_empty() { " " } else { s };
    format!("\"{}\"", s.replace('"', "#quot;").replace('\n', "<br/>"))
}

pub fn mermaid(canvas: &Canvas) -> MermaidOutput {
    let shapes: Vec<&Id> = canvas
        .elements
        .iter()
        .filter_map(|e| {
            if let Element::Shape(s) = e {
                Some(&s.id)
            } else {
                None
            }
        })
        .collect();
    let node = |end: &End| match end {
        End::Bound(id, _) => shapes
            .iter()
            .position(|s| *s == id)
            .map(|i| format!("n{i}")),
        End::Free(..) => None,
    };
    let mut text = String::from("flowchart TD\n");
    let mut edges = String::new();
    let mut skipped = 0;
    let mut n = 0;
    for e in &canvas.elements {
        match e {
            Element::Shape(s) => {
                let l = label(&s.label);
                let _ = match s.kind {
                    ShapeKind::Rect => writeln!(text, "  n{n}[{l}]"),
                    ShapeKind::Ellipse => writeln!(text, "  n{n}([{l}])"),
                    ShapeKind::Diamond => writeln!(text, "  n{n}{{{l}}}"),
                };
                n += 1;
            }
            Element::Line(l) => match (node(&l.from), node(&l.to)) {
                (Some(a), Some(b)) => {
                    // Mermaid has no dashed edge, so dashed and dotted both export dotted.
                    let dotted = l.stroke != Stroke::Solid;
                    let arrow = match (l.kind, dotted) {
                        (LineKind::Line, false) => "---",
                        (LineKind::Line, true) => "-.-",
                        (LineKind::ArrowBoth, false) => "<-->",
                        (LineKind::ArrowBoth, true) => "<-.->",
                        (_, false) => "-->",
                        (_, true) => "-.->",
                    };
                    let (a, b) = if l.kind == LineKind::ArrowStart {
                        (b, a)
                    } else {
                        (a, b)
                    };
                    let pipe = if l.label.is_empty() {
                        String::new()
                    } else {
                        format!("|{}|", label(&l.label))
                    };
                    let _ = writeln!(edges, "  {a} {arrow}{pipe} {b}");
                }
                _ => skipped += 1,
            },
            Element::Text(_) => skipped += 1,
        }
    }
    text.push_str(&edges);
    MermaidOutput { text, skipped }
}
