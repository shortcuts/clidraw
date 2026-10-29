use super::super::{statements, Imported};
use crate::model::{Anchor, Canvas, Element, End, Id, Line, LineKind, Shape, ShapeKind, Stroke};
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

pub(super) const MAX_SHAPES: usize = 2000;

const UNSUPPORTED: &[&str] = &[
    "autonumber",
    "activate",
    "deactivate",
    "Note",
    "note",
    "loop",
    "alt",
    "else",
    "opt",
    "par",
    "and",
    "critical",
    "break",
    "rect",
    "end",
    "box",
    "title",
    "link",
    "links",
    "create",
    "destroy",
];

// Longest first, so `-->>` wins over `-->`.
// clidraw has one head glyph, so `-x` and `-)` read as `->>`.
const ARROWS: [(&str, LineKind, Stroke); 8] = [
    ("-->>", LineKind::Arrow, Stroke::Dashed),
    ("--x", LineKind::Arrow, Stroke::Dashed),
    ("--)", LineKind::Arrow, Stroke::Dashed),
    ("->>", LineKind::Arrow, Stroke::Solid),
    ("-x", LineKind::Arrow, Stroke::Solid),
    ("-)", LineKind::Arrow, Stroke::Solid),
    ("-->", LineKind::Line, Stroke::Dashed),
    ("->", LineKind::Line, Stroke::Solid),
];

const BOX_H: i32 = 3;
const GAP: i32 = 10;
const ROW_GAP: i32 = 4;

pub(super) enum Rejected {
    Unsupported(&'static str),
    Unparsable,
}

pub(super) enum Stmt {
    Participant {
        id: String,
        label: String,
    },
    Message {
        from: String,
        to: String,
        kind: LineKind,
        stroke: Stroke,
        text: String,
    },
}

fn id(token: &str) -> Result<&str, Rejected> {
    let token = token.trim();
    if token.is_empty() || token.contains(char::is_whitespace) {
        return Err(Rejected::Unparsable);
    }
    Ok(token)
}

/// Reads one trimmed statement line. `import` and `validate` share it so they never disagree.
pub(super) fn parse_line(line: &str) -> Result<Stmt, Rejected> {
    let (first, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    if let Some(&word) = UNSUPPORTED.iter().find(|&&w| w == first) {
        return Err(Rejected::Unsupported(word));
    }
    if matches!(first, "participant" | "actor") {
        let (name, label) = rest.split_once(" as ").unwrap_or((rest, rest));
        let label = label.trim();
        if label.is_empty() {
            return Err(Rejected::Unparsable);
        }
        return Ok(Stmt::Participant {
            id: id(name)?.into(),
            label: label.into(),
        });
    }
    let (head, text) = line.split_once(':').unwrap_or((line, ""));
    let (at, &(arrow, kind, stroke)) = ARROWS
        .iter()
        .filter_map(|a| head.find(a.0).map(|at| (at, a)))
        .min_by_key(|&(at, _)| at)
        .ok_or(Rejected::Unparsable)?;
    let from = id(&head[..at])?;
    let to = id(head[at + arrow.len()..]
        .trim()
        .trim_start_matches(['+', '-']))?;
    if from == to {
        return Err(Rejected::Unsupported("self message"));
    }
    Ok(Stmt::Message {
        from: from.into(),
        to: to.into(),
        kind,
        stroke,
        text: text.trim().into(),
    })
}

fn slot(
    labels: &mut Vec<String>,
    index: &mut HashMap<String, usize>,
    id: String,
    label: Option<String>,
) -> usize {
    let at = *index.entry(id.clone()).or_insert_with(|| {
        labels.push(id);
        labels.len() - 1
    });
    if let Some(label) = label {
        labels[at] = label;
    }
    at
}

pub(super) fn import(text: &str) -> Result<Imported, String> {
    let mut lines = statements(text).map(|(line, _)| line);
    let head = lines.next().ok_or("nothing to import")?;
    if head.trim_end_matches(';').split_whitespace().next() != Some("sequenceDiagram") {
        return Err("only sequence diagrams can be imported".into());
    }

    let mut labels = vec![];
    let mut index = HashMap::new();
    let mut messages = vec![];
    let mut skipped = 0;
    for line in lines {
        match parse_line(line) {
            Ok(Stmt::Participant { id, label }) => {
                slot(&mut labels, &mut index, id, Some(label));
            }
            Ok(Stmt::Message {
                from,
                to,
                kind,
                stroke,
                text,
            }) => {
                let from = slot(&mut labels, &mut index, from, None);
                let to = slot(&mut labels, &mut index, to, None);
                messages.push((from, to, kind, stroke, text));
            }
            Err(_) => skipped += 1,
        }
    }
    if labels.is_empty() {
        return Err("nothing to import".into());
    }
    if labels.len() + messages.len() > MAX_SHAPES {
        return Err(format!(
            "too many participants and messages (max {MAX_SHAPES})"
        ));
    }

    let bottom = BOX_H + 2 + ROW_GAP * messages.len() as i32;
    let mut canvas = Canvas::default();
    let mut centers = vec![];
    let mut x = 0;
    for (i, label) in labels.into_iter().enumerate() {
        let w = (label.width() as i32 + 6).max(14);
        centers.push(x + w / 2);
        canvas.elements.push(Element::Shape(Shape {
            id: Id(format!("n{i}")),
            ..Shape::new(ShapeKind::Rect, x, 0, w, BOX_H, label)
        }));
        x += w + GAP;
    }
    for (i, &center) in centers.iter().enumerate() {
        canvas.elements.push(Element::Line(Line {
            id: Id(format!("l{i}")),
            stroke: Stroke::Dotted,
            ..Line::new(
                LineKind::Line,
                End::Bound(Id(format!("n{i}")), Anchor::Auto),
                End::Free(center, bottom),
            )
        }));
    }
    for (k, (from, to, kind, stroke, label)) in messages.into_iter().enumerate() {
        let y = BOX_H + 2 + ROW_GAP * k as i32;
        canvas.elements.push(Element::Line(Line {
            id: Id(format!("l{}", centers.len() + k)),
            label,
            stroke,
            ..Line::new(kind, End::Free(centers[from], y), End::Free(centers[to], y))
        }));
    }
    Ok(Imported { canvas, skipped })
}
