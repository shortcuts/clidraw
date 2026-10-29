use super::super::Imported;
use crate::model::{
    Anchor, Border, Canvas, Dir, Element, End, Id, Line, LineKind, Paint, Shape, ShapeKind, Stroke,
};
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

pub(super) const MAX_SHAPES: usize = 2000;

const UNSUPPORTED: [&str; 5] = ["classDef", "class", "subgraph", "end", "direction"];

// Longest first, so `<-.->` wins over `-.->`.
// clidraw has one stroke weight and one head glyph, so `==>` reads as `-->` and `--o` as `-->`.
const ARROWS: [(&str, LineKind, Stroke); 11] = [
    ("<-.->", LineKind::ArrowBoth, Stroke::Dotted),
    ("<==>", LineKind::ArrowBoth, Stroke::Solid),
    ("<-->", LineKind::ArrowBoth, Stroke::Solid),
    ("-.->", LineKind::Arrow, Stroke::Dotted),
    ("==>", LineKind::Arrow, Stroke::Solid),
    ("-.-", LineKind::Line, Stroke::Dotted),
    ("-->", LineKind::Arrow, Stroke::Solid),
    ("---", LineKind::Line, Stroke::Solid),
    ("===", LineKind::Line, Stroke::Solid),
    ("--o", LineKind::Arrow, Stroke::Solid),
    ("--x", LineKind::Arrow, Stroke::Solid),
];

// `-- text -->`: (opener, closer, kind, stroke). The first closer in the text wins.
const TEXT_ARROWS: [(&str, &str, LineKind, Stroke); 8] = [
    ("--", "-->", LineKind::Arrow, Stroke::Solid),
    ("--", "---", LineKind::Line, Stroke::Solid),
    ("--", "--o", LineKind::Arrow, Stroke::Solid),
    ("--", "--x", LineKind::Arrow, Stroke::Solid),
    ("-.", ".->", LineKind::Arrow, Stroke::Dotted),
    ("-.", ".-", LineKind::Line, Stroke::Dotted),
    ("==", "==>", LineKind::Arrow, Stroke::Solid),
    ("==", "===", LineKind::Line, Stroke::Solid),
];

// Longer openers come first so `([` is not read as `(`.
const BRACKETS: [(&str, &str, ShapeKind, Border); 11] = [
    ("((", "))", ShapeKind::Rect, Border::Rounded),
    ("[[", "]]", ShapeKind::Rect, Border::Double),
    ("([", "])", ShapeKind::Stadium, Border::Rounded),
    ("[(", ")]", ShapeKind::Cylinder, Border::Single),
    ("[/", "/]", ShapeKind::Parallelogram, Border::Single),
    ("[\\", "/]", ShapeKind::Parallelogram, Border::Single),
    (">", "]", ShapeKind::Rect, Border::Single),
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

/// `--o` and `--x` end in a letter; `A---oB` is a line to node `oB`, so they need a boundary after.
fn token_at(s: &str, i: usize, tok: &str) -> bool {
    let rest = &s[i..];
    rest.starts_with(tok)
        && (!tok.ends_with(|c: char| c.is_ascii_alphabetic())
            || rest[tok.len()..]
                .chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || c == '|'))
}

/// The arrow at `i`: where it ends and its edge. `None` when no well-formed arrow starts there.
fn arrow_at(s: &str, i: usize) -> Option<(usize, Edge)> {
    if let Some(&(tok, kind, stroke)) = ARROWS.iter().find(|a| token_at(s, i, a.0)) {
        let end = i + tok.len();
        let rest = s[end..].trim_start();
        let Some(after) = rest.strip_prefix('|') else {
            return Some((end, (kind, stroke, String::new())));
        };
        let mut q = false;
        let close = after.bytes().position(|c| {
            q ^= c == b'"';
            c == b'|' && !q
        })?;
        let end = s.len() - after.len() + close + 1;
        return Some((end, (kind, stroke, label(&after[..close]))));
    }
    let open = TEXT_ARROWS.iter().find(|t| {
        s[i..]
            .strip_prefix(t.0)
            .is_some_and(|r| r.starts_with(char::is_whitespace))
    })?;
    let from = i + open.0.len();
    let mut q = false;
    for (j, c) in s[from..].char_indices() {
        q ^= c == '"';
        if q {
            continue;
        }
        let at = from + j;
        let closer = TEXT_ARROWS
            .iter()
            .find(|t| t.0 == open.0 && token_at(s, at, t.1));
        if let Some(&(_, tok, kind, stroke)) = closer {
            return Some((at + tok.len(), (kind, stroke, label(&s[from..at]))));
        }
    }
    None
}

/// Splits `A & B --> C -.->|x| D` into node groups and the edge between each pair of groups.
/// Arrows and `&` inside quotes or brackets do not split.
fn chain(s: &str) -> Option<(Vec<Vec<Node>>, Vec<Edge>)> {
    let b = s.as_bytes();
    let (mut groups, mut edges, mut group) = (vec![], vec![], vec![]);
    let (mut start, mut i, mut depth, mut in_quote) = (0, 0, 0i32, false);
    while i < b.len() {
        match b[i] {
            b'"' => in_quote = !in_quote,
            b'[' | b'(' | b'{' if !in_quote => depth += 1,
            // `A>x]` opens with `>`; `-->` never follows an id character.
            b'>' if !in_quote
                && depth == 0
                && i > 0
                && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_') =>
            {
                depth += 1
            }
            b']' | b')' | b'}' if !in_quote => depth -= 1,
            b'&' if !in_quote && depth == 0 => {
                group.push(node(&s[start..i])?);
                start = i + 1;
            }
            _ if !in_quote && depth == 0 => {
                if let Some((end, edge)) = arrow_at(s, i) {
                    group.push(node(&s[start..i])?);
                    groups.push(std::mem::take(&mut group));
                    edges.push(edge);
                    (start, i) = (end, end);
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    group.push(node(&s[start..])?);
    groups.push(group);
    Some((groups, edges))
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
    Chain(Vec<Vec<Node>>, Vec<Edge>),
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
        .map(|(groups, edges)| Stmt::Chain(groups, edges))
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
        let (groups, links) = match parse_line(line) {
            Ok(Stmt::Chain(groups, links)) => (groups, links),
            Ok(stmt) => {
                deferred.push(stmt);
                continue;
            }
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let slots: Vec<Vec<usize>> = groups
            .into_iter()
            .map(|g| {
                g.into_iter()
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
                    .collect()
            })
            .collect();
        for (pair, edge) in slots.windows(2).zip(links) {
            for &a in &pair[0] {
                for &b in &pair[1] {
                    edges.push((a, b, edge.clone()));
                }
            }
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
        let widest = rows.iter().map(|r| r.width()).max().unwrap_or(0) as i32;
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
            border,
            ..Shape::new(kind, x, y, w, h, label)
        }));
    }
    let ids: Vec<Id> = canvas.elements.iter().map(|e| e.id().clone()).collect();
    for (i, (a, b, (kind, stroke, label))) in edges.into_iter().enumerate() {
        canvas.elements.push(Element::Line(Line {
            id: Id(format!("l{i}")),
            label,
            stroke,
            ..Line::new(
                kind,
                End::Bound(ids[a].clone(), Anchor::Auto),
                End::Bound(ids[b].clone(), Anchor::Auto),
            )
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
