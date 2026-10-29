use super::super::{statements, Imported};
use crate::model::{
    Anchor, Border, Canvas, Dir, Element, End, Id, Line, LineKind, Shape, ShapeKind,
};
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

pub(super) const MAX_SHAPES: usize = 2000;

const UNSUPPORTED: &[&str] = &[
    "direction",
    "classDef",
    "class",
    "style",
    "click",
    "title",
    "accTitle",
    "accDescr",
    "scale",
    "hide",
];

pub(super) enum Rejected {
    /// Fails the whole import.
    Syntax(&'static str),
    Unsupported(&'static str),
    Unparsable,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum Node {
    Start,
    End,
    State(String),
}

pub(super) enum Stmt {
    State { id: String, label: Option<String> },
    Transition { from: Node, to: Node, text: String },
}

fn token(s: &str) -> Option<&str> {
    let s = s.trim();
    (!s.is_empty() && !s.contains(char::is_whitespace)).then_some(s)
}

/// Reads one trimmed statement line. `import` and `validate` share it so they never disagree.
pub(super) fn parse_line(line: &str) -> Result<Stmt, Rejected> {
    let (first, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    match first {
        "note" => return Err(Rejected::Syntax("note")),
        "--" if rest.is_empty() => return Err(Rejected::Syntax("concurrency")),
        "}" if rest.is_empty() => return Err(Rejected::Syntax("composite state")),
        "state" => return parse_state(rest.trim()),
        _ => {}
    }
    if let Some(&word) = UNSUPPORTED.iter().find(|&&w| w == first) {
        return Err(Rejected::Unsupported(word));
    }
    let (head, text) = line.split_once(':').unwrap_or((line, ""));
    let Some((left, right)) = head.split_once("-->") else {
        return match (token(head), line.contains(':')) {
            (Some(id), true) => Ok(Stmt::State {
                id: id.into(),
                label: Some(text.trim().into()),
            }),
            _ => Err(Rejected::Unparsable),
        };
    };
    let (Some(left), Some(right)) = (token(left), token(right)) else {
        return Err(Rejected::Unparsable);
    };
    let node = |t: &str, marker| {
        if t == "[*]" {
            marker
        } else {
            Node::State(t.into())
        }
    };
    let (from, to) = (node(left, Node::Start), node(right, Node::End));
    if left == "[*]" && right == "[*]" {
        return Err(Rejected::Unparsable);
    }
    if from == to {
        return Err(Rejected::Unsupported("self transition"));
    }
    Ok(Stmt::Transition {
        from,
        to,
        text: text.trim().into(),
    })
}

/// `state id`, `state "Long name" as id`; a `{` opens a composite state.
fn parse_state(rest: &str) -> Result<Stmt, Rejected> {
    if rest.ends_with('{') {
        return Err(Rejected::Syntax("composite state"));
    }
    if rest.contains("<<") {
        return Err(Rejected::Unsupported("<<"));
    }
    if let Some(quoted) = rest.strip_prefix('"') {
        let (label, id) = quoted.split_once("\" as ").ok_or(Rejected::Unparsable)?;
        return token(id).map_or(Err(Rejected::Unparsable), |id| {
            Ok(Stmt::State {
                id: id.into(),
                label: Some(label.into()),
            })
        });
    }
    token(rest).map_or(Err(Rejected::Unparsable), |id| {
        Ok(Stmt::State {
            id: id.into(),
            label: None,
        })
    })
}

fn slot(labels: &mut Vec<String>, index: &mut HashMap<Node, usize>, node: Node) -> usize {
    *index.entry(node.clone()).or_insert_with(|| {
        labels.push(match node {
            Node::Start => "●".into(),
            Node::End => "◉".into(),
            Node::State(id) => id,
        });
        labels.len() - 1
    })
}

pub(super) fn import(text: &str) -> Result<Imported, String> {
    let (head, _) = statements(text).next().ok_or("nothing to import")?;
    let word = head.trim_end_matches(';').split_whitespace().next();
    if !matches!(word, Some("stateDiagram" | "stateDiagram-v2")) {
        return Err("only state diagrams can be imported".into());
    }

    let mut labels = vec![];
    let mut index = HashMap::new();
    let mut transitions = vec![];
    let mut skipped = 0;
    for (line, _) in statements(text).skip(1) {
        match parse_line(line) {
            Ok(Stmt::State { id, label }) => {
                let at = slot(&mut labels, &mut index, Node::State(id));
                if let Some(label) = label {
                    labels[at] = label;
                }
            }
            Ok(Stmt::Transition { from, to, text }) => {
                let from = slot(&mut labels, &mut index, from);
                let to = slot(&mut labels, &mut index, to);
                transitions.push((from, to, text));
            }
            Err(Rejected::Syntax(_)) => {
                return Err("composite states, notes and concurrency cannot be imported".into())
            }
            Err(_) => skipped += 1,
        }
    }
    if labels.is_empty() {
        return Err("nothing to import".into());
    }
    if labels.len() > MAX_SHAPES {
        return Err(format!("too many states (max {MAX_SHAPES})"));
    }

    // Distinct start cells in appearance order make `layout`'s sort deterministic.
    let mut canvas = Canvas::default();
    let mut x = 0;
    for (i, label) in labels.into_iter().enumerate() {
        let w = if label == "●" || label == "◉" {
            5
        } else {
            (label.width() as i32 + 6).max(8)
        };
        canvas.elements.push(Element::Shape(Shape {
            id: Id(format!("n{i}")),
            border: Border::Rounded,
            ..Shape::new(ShapeKind::Rect, x, 0, w, 3, label)
        }));
        x += w + 2;
    }
    let ids: Vec<Id> = canvas.elements.iter().map(|e| e.id().clone()).collect();
    for (i, (from, to, label)) in transitions.into_iter().enumerate() {
        canvas.elements.push(Element::Line(Line {
            id: Id(format!("l{i}")),
            label,
            ..Line::new(
                LineKind::Arrow,
                End::Bound(ids[from].clone(), Anchor::Auto),
                End::Bound(ids[to].clone(), Anchor::Auto),
            )
        }));
    }
    // shortcut: `layout` finds each endpoint by linear scan, so the state cap bounds its cost.
    for cmd in canvas.layout(&ids, Dir::TopDown) {
        let _ = canvas.apply(cmd);
    }
    Ok(Imported { canvas, skipped })
}
