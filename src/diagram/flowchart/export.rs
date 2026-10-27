use super::super::Exported;
use crate::model::{
    Border, Canvas, Element, End, Id, LineKind, Paint, ShapeKind, Stroke, FILL_ALPHA,
};
use std::fmt::Write;

fn label(s: &str) -> String {
    let s = if s.is_empty() { " " } else { s };
    format!("\"{}\"", s.replace('"', "#quot;").replace('\n', "<br/>"))
}

fn hex(p: Paint) -> String {
    let (r, g, b) = p.rgb();
    format!("#{r:02x}{g:02x}{b:02x}")
}

pub(super) fn export(canvas: &Canvas) -> Exported {
    let shapes: Vec<&Id> = canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Shape(s) if s.kind != ShapeKind::Frame => Some(&s.id),
            _ => None,
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
    let mut styles = String::new();
    let mut clicks = String::new();
    let mut links = 0;
    let mut skipped = 0;
    let mut n = 0;
    for e in &canvas.elements {
        match e {
            Element::Shape(s) if s.kind == ShapeKind::Frame => skipped += 1,
            Element::Shape(s) => {
                skipped += s.side_labels.len();
                let l = label(&s.label);
                let _ = match s.kind {
                    ShapeKind::Rect if s.border == Border::Rounded => writeln!(text, "  n{n}({l})"),
                    ShapeKind::Rect => writeln!(text, "  n{n}[{l}]"),
                    ShapeKind::Diamond => writeln!(text, "  n{n}{{{l}}}"),
                    ShapeKind::Cylinder => writeln!(text, "  n{n}[({l})]"),
                    ShapeKind::Parallelogram => writeln!(text, "  n{n}[/{l}/]"),
                    ShapeKind::Hexagon => writeln!(text, "  n{n}{{{{{l}}}}}"),
                    ShapeKind::Frame => unreachable!(),
                };
                let keys: Vec<String> = [
                    s.fill.map(|p| {
                        format!("fill:{}{:02x}", hex(p), u16::from(FILL_ALPHA) * 255 / 100)
                    }),
                    s.color.map(|p| format!("stroke:{}", hex(p))),
                    s.color.map(|p| format!("color:{}", hex(p))),
                ]
                .into_iter()
                .flatten()
                .collect();
                if !keys.is_empty() {
                    let _ = writeln!(styles, "  style n{n} {}", keys.join(","));
                }
                if let Some(url) = &s.link {
                    let url = url.replace('"', "%22").replace(['\n', '\r'], "");
                    let _ = writeln!(clicks, "  click n{n} \"{url}\" _blank");
                }
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
                    if let Some(p) = l.color {
                        let _ =
                            writeln!(styles, "  linkStyle {links} stroke:{0},color:{0}", hex(p));
                    }
                    links += 1;
                }
                _ => skipped += 1,
            },
            Element::Text(_) => skipped += 1,
        }
    }
    text.push_str(&edges);
    text.push_str(&styles);
    text.push_str(&clicks);
    Exported { text, skipped }
}
