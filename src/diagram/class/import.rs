use super::super::{statements, Imported};
use crate::model::{
    Anchor, Border, Canvas, Dir, Element, End, Id, Line, LineKind, Shape, ShapeKind, Stroke,
};
use std::collections::HashMap;
use std::ops::Range;
use unicode_width::UnicodeWidthStr;

pub(super) const MAX_SHAPES: usize = 2000;

const UNSUPPORTED: &[&str] = &[
    "namespace",
    "note",
    "direction",
    "classDef",
    "style",
    "cssClass",
    "click",
    "callback",
    "link",
    "title",
    "accTitle",
    "accDescr",
];

// Longest first, so `<|--` wins over `--`. The last field is set when the marker sits on the left,
// so the line runs right to left and its head lands on the marker class.
// clidraw has one head glyph, so inheritance, composition and aggregation all read as `-->`.
const RELATIONS: [(&str, LineKind, Stroke, bool); 14] = [
    ("<|--", LineKind::Arrow, Stroke::Solid, true),
    ("--|>", LineKind::Arrow, Stroke::Solid, false),
    ("<|..", LineKind::Arrow, Stroke::Dotted, true),
    ("..|>", LineKind::Arrow, Stroke::Dotted, false),
    ("*--", LineKind::Arrow, Stroke::Solid, true),
    ("--*", LineKind::Arrow, Stroke::Solid, false),
    ("o--", LineKind::Arrow, Stroke::Solid, true),
    ("--o", LineKind::Arrow, Stroke::Solid, false),
    ("<--", LineKind::Arrow, Stroke::Solid, true),
    ("-->", LineKind::Arrow, Stroke::Solid, false),
    ("<..", LineKind::Arrow, Stroke::Dotted, true),
    ("..>", LineKind::Arrow, Stroke::Dotted, false),
    ("--", LineKind::Line, Stroke::Solid, false),
    ("..", LineKind::Line, Stroke::Dotted, false),
];

pub(super) enum Rejected {
    Unsupported(&'static str),
    Unparsable,
}

#[derive(Clone)]
pub(super) struct Name {
    pub(super) id: String,
    label: String,
}

pub(super) enum Stmt {
    /// `open` is set when a `{` member block follows.
    Class(Name, bool),
    Member(Name, String),
    Relation(Name, Name, LineKind, Stroke, String),
}

/// `Name`, `Name~T~` or `Name["Label"]`; the id is the leading word.
fn name(token: &str) -> Option<Name> {
    let end = token
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(token.len());
    let (id, rest) = token.split_at(end);
    let label = match rest {
        "" => id,
        _ if rest.starts_with('~') => token,
        _ => rest.strip_prefix("[\"")?.strip_suffix("\"]")?,
    };
    (!id.is_empty()).then(|| Name {
        id: id.into(),
        label: label.into(),
    })
}

/// The marker `o` shares a letter with class names, so `Foo--Bar` must not read as `Fo` `o--` `Bar`.
fn fits(head: &str, at: usize, op: &str) -> bool {
    let before = head[..at].chars().next_back();
    let after = head[at + op.len()..].chars().next();
    let boundary = |c: Option<char>| c.is_none_or(|c| c.is_whitespace() || c == '"');
    (!op.starts_with('o') || boundary(before)) && (!op.ends_with('o') || boundary(after))
}

/// Drops the quoted cardinality next to a class name.
fn operand(side: &str, left: bool) -> Option<Name> {
    let side = side.trim();
    let bare = if left {
        side.strip_suffix('"')
            .and_then(|s| s.rfind('"').map(|i| &s[..i]))
    } else {
        side.strip_prefix('"')
            .and_then(|s| s.find('"').map(|i| &s[i + 1..]))
    };
    name(bare.unwrap_or(side).trim())
}

/// Reads one trimmed statement line outside a member block.
/// `import` and `validate` share it so they never disagree.
pub(super) fn parse_line(line: &str) -> Result<Stmt, Rejected> {
    let (first, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    if let Some(&word) = UNSUPPORTED.iter().find(|&&w| w == first) {
        return Err(Rejected::Unsupported(word));
    }
    if line.starts_with("<<") {
        return Err(Rejected::Unsupported("<<"));
    }
    if first == "class" {
        let rest = rest.trim();
        let open = rest.ends_with('{');
        let token = rest.trim_end_matches('{').trim();
        return match name(token) {
            Some(class) if !token.contains(char::is_whitespace) => Ok(Stmt::Class(class, open)),
            _ => Err(Rejected::Unparsable),
        };
    }
    let (head, label) = line.split_once(':').unwrap_or((line, ""));
    if let Some(class) = name(head.trim()) {
        return match label.trim() {
            "" => Err(Rejected::Unparsable),
            member => Ok(Stmt::Member(class, member.into())),
        };
    }
    let (at, &(op, kind, stroke, reverse)) = RELATIONS
        .iter()
        .filter_map(|r| {
            head.match_indices(r.0)
                .find(|&(at, _)| fits(head, at, r.0))
                .map(|(at, _)| (at, r))
        })
        .min_by_key(|&(at, _)| at)
        .ok_or(Rejected::Unparsable)?;
    let (left, right) = match (
        operand(&head[..at], true),
        operand(&head[at + op.len()..], false),
    ) {
        (Some(l), Some(r)) => (l, r),
        _ => return Err(Rejected::Unparsable),
    };
    let (from, to) = if reverse {
        (right, left)
    } else {
        (left, right)
    };
    Ok(Stmt::Relation(from, to, kind, stroke, label.trim().into()))
}

/// Parses every statement after the header, tracking `{ ... }` member blocks.
pub(super) fn parse(
    text: &str,
) -> impl Iterator<Item = (Result<Stmt, Rejected>, Range<usize>)> + '_ {
    let mut open: Option<Name> = None;
    statements(text).skip(1).filter_map(move |(line, range)| {
        let stmt = match &open {
            Some(_) if line == "}" => {
                open = None;
                return None;
            }
            Some(_) if line.starts_with("<<") => Err(Rejected::Unsupported("<<")),
            Some(class) => Ok(Stmt::Member(class.clone(), line.into())),
            None => {
                let stmt = parse_line(line);
                if let Ok(Stmt::Class(class, true)) = &stmt {
                    open = Some(class.clone());
                }
                stmt
            }
        };
        Some((stmt, range))
    })
}

fn slot(
    classes: &mut Vec<(String, Vec<String>)>,
    index: &mut HashMap<String, usize>,
    class: Name,
    named: bool,
) -> usize {
    let at = *index.entry(class.id).or_insert_with(|| {
        classes.push((class.label.clone(), vec![]));
        classes.len() - 1
    });
    if named {
        classes[at].0 = class.label;
    }
    at
}

pub(super) fn import(text: &str) -> Result<Imported, String> {
    let (head, _) = statements(text).next().ok_or("nothing to import")?;
    if head.trim_end_matches(';').split_whitespace().next() != Some("classDiagram") {
        return Err("only class diagrams can be imported".into());
    }

    let mut classes = vec![];
    let mut index = HashMap::new();
    let mut relations = vec![];
    let mut skipped = 0;
    for (stmt, _) in parse(text) {
        match stmt {
            Ok(Stmt::Class(class, _)) => {
                slot(&mut classes, &mut index, class, true);
            }
            Ok(Stmt::Member(class, member)) => {
                let at = slot(&mut classes, &mut index, class, false);
                classes[at].1.push(member);
            }
            Ok(Stmt::Relation(from, to, kind, stroke, label)) => {
                let from = slot(&mut classes, &mut index, from, false);
                let to = slot(&mut classes, &mut index, to, false);
                relations.push((from, to, kind, stroke, label));
            }
            Err(_) => skipped += 1,
        }
    }
    if classes.is_empty() {
        return Err("nothing to import".into());
    }
    if classes.len() > MAX_SHAPES {
        return Err(format!("too many classes (max {MAX_SHAPES})"));
    }

    // Distinct start cells in declaration order make `layout`'s sort deterministic.
    let mut canvas = Canvas::default();
    let mut x = 0;
    for (i, (name, members)) in classes.into_iter().enumerate() {
        let widest = members
            .iter()
            .chain([&name])
            .map(|r| r.width())
            .max()
            .unwrap_or(0);
        let mut rows = vec![name];
        if !members.is_empty() {
            rows.push("─".repeat(widest));
            rows.extend(members);
        }
        let w = (widest as i32 + 6).max(8);
        let h = (rows.len() as i32 + 2).max(3);
        canvas.elements.push(Element::Shape(Shape {
            id: Id(format!("n{i}")),
            border: Border::Single,
            ..Shape::new(ShapeKind::Rect, x, 0, w, h, rows.join("\n"))
        }));
        x += w + 2;
    }
    let ids: Vec<Id> = canvas.elements.iter().map(|e| e.id().clone()).collect();
    for (i, (from, to, kind, stroke, label)) in relations.into_iter().enumerate() {
        canvas.elements.push(Element::Line(Line {
            id: Id(format!("l{i}")),
            label,
            stroke,
            ..Line::new(
                kind,
                End::Bound(ids[from].clone(), Anchor::Auto),
                End::Bound(ids[to].clone(), Anchor::Auto),
            )
        }));
    }
    // shortcut: `layout` finds each endpoint by linear scan, so the class cap bounds its cost.
    for cmd in canvas.layout(&ids, Dir::TopDown) {
        let _ = canvas.apply(cmd);
    }
    Ok(Imported { canvas, skipped })
}
