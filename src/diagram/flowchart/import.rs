use super::super::Imported;
use crate::model::{
    Anchor, Border, Canvas, Dir, Element, End, Id, Line, LineKind, Paint, Shape, ShapeKind, Stroke,
};
use std::collections::HashMap;

pub(super) const MAX_SHAPES: usize = 2000;

const UNSUPPORTED: [&str; 5] = ["classDef", "class", "subgraph", "end", "direction"];

// Longest first, so `<-.->` wins over `-.->`.
const ARROWS: [(&str, LineKind, Stroke); 6] = [
    ("<-.->", LineKind::ArrowBoth, Stroke::Dotted),
    ("<-->", LineKind::ArrowBoth, Stroke::Solid),
    ("-.->", LineKind::Arrow, Stroke::Dotted),
    ("-.-", LineKind::Line, Stroke::Dotted),
    ("-->", LineKind::Arrow, Stroke::Solid),
    ("---", LineKind::Line, Stroke::Solid),
];

// Two-character openers come first so `([` is not read as `(`.
const BRACKETS: [(&str, &str, ShapeKind, Border); 7] = [
    ("([", "])", ShapeKind::Rect, Border::Rounded),
    ("[(", ")]", ShapeKind::Cylinder, Border::Single),
    ("[/", "/]", ShapeKind::Parallelogram, Border::Single),
    ("{{", "}}", ShapeKind::Hexagon, Border::Single),
    ("[", "]", ShapeKind::Rect, Border::Single),
    ("(", ")", ShapeKind::Rect, Border::Rounded),
    ("{", "}", ShapeKind::Diamond, Border::Single),
];

pub(super) struct Node {
    pub(super) id: String,
    def: Option<(ShapeKind, Border, String)>,
}

fn label(s: &str) -> String {
    let s = s.trim();
    let inner = match (s.strip_prefix('"'), s.rfind('"')) {
        (Some(_), Some(end)) if end > 0 => &s[1..end],
        _ => s,
    };
    inner.replace("#quot;", "\"").replace("<br/>", "\n")
}

fn node(tok: &str) -> Option<Node> {
    let tok = tok.trim();
    let end = tok
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(tok.len());
    let (id, rest) = tok.split_at(end);
    if id.is_empty() {
        return None;
    }
    let def = if rest.is_empty() {
        None
    } else {
        let (_, inner, kind, border) =
            BRACKETS.iter().find_map(|&(open, close, kind, border)| {
                let inner = rest.strip_prefix(open)?.strip_suffix(close)?;
                Some((open, inner, kind, border))
            })?;
        Some((kind, border, label(inner)))
    };
    Some(Node {
        id: id.to_string(),
        def,
    })
}

type Edge = (LineKind, Stroke, String);

/// Splits `A --> B -.->|x| C` into nodes and edges. Arrows inside quotes or brackets do not split.
fn chain(s: &str) -> Option<(Vec<Node>, Vec<Edge>)> {
    let b = s.as_bytes();
    let (mut nodes, mut edges) = (vec![], vec![]);
    let (mut start, mut i, mut depth, mut in_quote) = (0, 0, 0i32, false);
    while i < b.len() {
        match b[i] {
            b'"' => in_quote = !in_quote,
            b'[' | b'(' | b'{' if !in_quote => depth += 1,
            b']' | b')' | b'}' if !in_quote => depth -= 1,
            _ if !in_quote && depth == 0 => {
                if let Some(&(arrow, kind, stroke)) =
                    ARROWS.iter().find(|a| s[i..].starts_with(a.0))
                {
                    nodes.push(node(&s[start..i])?);
                    i += arrow.len();
                    let mut text = String::new();
                    let rest = s[i..].trim_start();
                    if let Some(after) = rest.strip_prefix('|') {
                        let mut q = false;
                        let close = after.bytes().position(|c| {
                            q ^= c == b'"';
                            c == b'|' && !q
                        })?;
                        text = label(&after[..close]);
                        i = s.len() - after.len() + close + 1;
                    }
                    edges.push((kind, stroke, text));
                    start = i;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    nodes.push(node(&s[start..])?);
    Some((nodes, edges))
}

pub(super) fn header(line: &str) -> Result<Dir, String> {
    let mut words = line.trim_end_matches(';').split_whitespace();
    match (words.next(), words.next()) {
        (Some("flowchart" | "graph"), Some("LR" | "RL")) => Ok(Dir::LeftRight),
        (Some("flowchart" | "graph"), _) => Ok(Dir::TopDown),
        _ => Err("only flowchart diagrams can be imported".into()),
    }
}

pub(super) enum Rejected {
    Unsupported(&'static str),
    Unparsable,
}

pub(super) enum Stmt {
    Chain(Vec<Node>, Vec<Edge>),
    /// `unmatched` is set when a key or color has no clidraw equivalent.
    Style {
        id: String,
        fill: Option<Paint>,
        color: Option<Paint>,
        unmatched: bool,
    },
    LinkStyle {
        index: usize,
        color: Option<Paint>,
        unmatched: bool,
    },
    Click {
        id: String,
        url: String,
    },
}

/// The palette color whose RGB equals `#rrggbb`, ignoring an alpha suffix.
fn paint(value: &str) -> Option<Paint> {
    let hex = value.strip_prefix('#')?;
    let rgb = u32::from_str_radix(hex.get(..6)?, 16).ok()?;
    let rgb = ((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    Paint::ALL.into_iter().find(|p| p.rgb() == rgb)
}

/// Reads `fill:..,stroke:..,color:..`. Stroke and color both feed the one clidraw color.
fn style_keys(keys: &str) -> (Option<Paint>, Option<Paint>, bool) {
    let (mut fill, mut color, mut unmatched) = (None, None, false);
    for (key, value) in keys.split(',').filter_map(|kv| kv.trim().split_once(':')) {
        let found = paint(value.trim());
        match key.trim() {
            "fill" if found.is_some() => fill = found,
            "stroke" | "color" if found.is_some() => color = found,
            _ => unmatched = true,
        }
    }
    (fill, color, unmatched)
}

fn style_stmt(word: &str, rest: &str) -> Option<Stmt> {
    let (target, keys) = rest.split_once(char::is_whitespace)?;
    let (fill, color, unmatched) = style_keys(keys);
    if word == "style" {
        return Some(Stmt::Style {
            id: target.into(),
            fill,
            color,
            unmatched,
        });
    }
    Some(Stmt::LinkStyle {
        index: target.parse().ok()?,
        color,
        unmatched,
    })
}

fn click_stmt(rest: &str) -> Option<Stmt> {
    let (id, rest) = rest.split_once(char::is_whitespace)?;
    let url = rest.trim_start().strip_prefix('"')?;
    Some(Stmt::Click {
        id: id.into(),
        url: url[..url.find('"')?].into(),
    })
}

/// Reads one trimmed statement line. `import` and `validate` share it so they never disagree.
pub(super) fn parse_line(line: &str) -> Result<Stmt, Rejected> {
    let line = line.strip_suffix(';').unwrap_or(line);
    let (first, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let rest = rest.trim();
    let read = match first {
        "style" | "linkStyle" => style_stmt(first, rest),
        "click" => click_stmt(rest),
        _ => None,
    };
    if let Some(stmt) = read {
        return Ok(stmt);
    }
    if let Some(word) = ["style", "linkStyle", "click"]
        .iter()
        .chain(&UNSUPPORTED)
        .find(|&&w| w == first)
    {
        return Err(Rejected::Unsupported(word));
    }
    chain(line)
        .map(|(nodes, edges)| Stmt::Chain(nodes, edges))
        .ok_or(Rejected::Unparsable)
}

pub(super) fn import(text: &str) -> Result<Imported, String> {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("%%"));
    let dir = header(lines.next().ok_or("nothing to import")?)?;

    let mut shapes: Vec<(String, ShapeKind, Border, String)> = vec![];
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut edges: Vec<(usize, usize, Edge)> = vec![];
    let mut skipped = 0;
    let mut deferred = vec![];
    for line in lines {
        let (nodes, links) = match parse_line(line) {
            Ok(Stmt::Chain(nodes, links)) => (nodes, links),
            Ok(stmt) => {
                deferred.push(stmt);
                continue;
            }
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let slots: Vec<usize> = nodes
            .into_iter()
            .map(|n| {
                let at = *index.entry(n.id.clone()).or_insert_with(|| {
                    shapes.push((n.id.clone(), ShapeKind::Rect, Border::Single, n.id));
                    shapes.len() - 1
                });
                if let Some((kind, border, label)) = n.def {
                    shapes[at] = (shapes[at].0.clone(), kind, border, label);
                }
                at
            })
            .collect();
        for (pair, edge) in slots.windows(2).zip(links) {
            edges.push((pair[0], pair[1], edge));
        }
    }
    if shapes.is_empty() {
        return Err("nothing to import".into());
    }
    if shapes.len() > MAX_SHAPES {
        return Err(format!("too many nodes (max {MAX_SHAPES})"));
    }

    // Distinct start cells in declaration order make `layout`'s sort deterministic.
    let mut canvas = Canvas::default();
    let mut pos = 0;
    for (i, (_, kind, border, label)) in shapes.into_iter().enumerate() {
        let rows: Vec<&str> = label.split('\n').collect();
        let widest = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
        let w = (widest + 6).max(8);
        let h = (rows.len() as i32 + 2).max(3);
        let (x, y) = if dir == Dir::LeftRight {
            (0, pos)
        } else {
            (pos, 0)
        };
        pos += if dir == Dir::LeftRight { h + 1 } else { w + 2 };
        canvas.elements.push(Element::Shape(Shape {
            id: Id(format!("n{i}")),
            kind,
            x,
            y,
            w,
            h,
            label,
            stroke: Stroke::Solid,
            border,
            color: None,
            fill: None,
            align: Default::default(),
            bold: false,
            link: None,
        }));
    }
    let ids: Vec<Id> = canvas.elements.iter().map(|e| e.id().clone()).collect();
    for (i, (a, b, (kind, stroke, label))) in edges.into_iter().enumerate() {
        canvas.elements.push(Element::Line(Line {
            id: Id(format!("l{i}")),
            kind,
            from: End::Bound(ids[a].clone(), Anchor::Auto),
            to: End::Bound(ids[b].clone(), Anchor::Auto),
            label,
            label_at: None,
            stroke,
            bends: vec![],
            color: None,
        }));
    }
    let shape_count = index.len();
    for stmt in deferred {
        match stmt {
            Stmt::Style {
                id,
                fill,
                color,
                unmatched,
            } => {
                skipped += usize::from(unmatched);
                match index.get(&id).map(|&i| &mut canvas.elements[i]) {
                    Some(Element::Shape(s)) => {
                        s.fill = fill.or(s.fill);
                        s.color = color.or(s.color);
                    }
                    _ => skipped += 1,
                }
            }
            Stmt::LinkStyle {
                index: n,
                color,
                unmatched,
            } => {
                skipped += usize::from(unmatched);
                match canvas.elements.get_mut(shape_count + n) {
                    Some(Element::Line(l)) => l.color = color.or(l.color),
                    _ => skipped += 1,
                }
            }
            Stmt::Click { id, url } => match index.get(&id).map(|&i| &mut canvas.elements[i]) {
                Some(Element::Shape(s)) => s.link = Some(url),
                _ => skipped += 1,
            },
            Stmt::Chain(..) => unreachable!(),
        }
    }
    // shortcut: `layout` finds each endpoint by linear scan, so the node cap bounds its cost.
    for cmd in canvas.layout(&ids, dir) {
        let _ = canvas.apply(cmd);
    }
    Ok(Imported { canvas, skipped })
}
