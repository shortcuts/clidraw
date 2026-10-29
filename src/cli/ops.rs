use crate::model::{
    kind_cmds, Align, Anchor, Border, Canvas, Command, Dir, Element, End, Id, Line, LineKind, Meta,
    ModelError, Paint, Shape, ShapeKind, Stroke, Text, TextAlign, Z,
};
use anyhow::{bail, Result};
use serde::Deserialize;
use unicode_width::UnicodeWidthStr;

/// An endpoint is a shape id, or a `[x, y]` cell for a free end.
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum Endpoint {
    Id(Id),
    Cell(i32, i32),
}

impl From<Endpoint> for End {
    fn from(e: Endpoint) -> End {
        match e {
            Endpoint::Id(id) => End::Bound(id, Anchor::Auto),
            Endpoint::Cell(x, y) => End::Free(x, y),
        }
    }
}

/// One edit. `set` takes a color name, a #rrggbb hex, or "none" to clear it.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Op {
    AddShape {
        id: Option<Id>,
        kind: Option<ShapeKind>,
        x: i32,
        y: i32,
        w: Option<i32>,
        h: Option<i32>,
        label: Option<String>,
        color: Option<Paint>,
        fill: Option<Paint>,
        meta: Option<Meta>,
    },
    AddText {
        id: Option<Id>,
        x: i32,
        y: i32,
        text: String,
        color: Option<Paint>,
        meta: Option<Meta>,
    },
    Connect {
        id: Option<Id>,
        from: Endpoint,
        to: Endpoint,
        kind: Option<LineKind>,
        label: Option<String>,
        stroke: Option<Stroke>,
        meta: Option<Meta>,
    },
    Set {
        id: Id,
        label: Option<String>,
        color: Option<String>,
        fill: Option<String>,
        stroke: Option<Stroke>,
        border: Option<Border>,
        align: Option<TextAlign>,
        bold: Option<bool>,
        link: Option<String>,
        kind: Option<ShapeKind>,
        head: Option<LineKind>,
        meta: Option<Meta>,
    },
    Resize {
        id: Id,
        w: i32,
        h: i32,
    },
    Group {
        ids: Vec<Id>,
    },
    Ungroup {
        ids: Vec<Id>,
    },
    Order {
        ids: Vec<Id>,
        z: Z,
    },
    Layout {
        ids: Option<Vec<Id>>,
        dir: Option<Dir>,
    },
    Align {
        ids: Vec<Id>,
        mode: Align,
    },
    Move {
        id: Id,
        dx: i32,
        dy: i32,
    },
    Delete {
        id: Id,
    },
}

fn paint(name: &str) -> Result<Option<Paint>> {
    if name == "none" {
        return Ok(None);
    }
    Paint::parse(name)
        .map(Some)
        .ok_or_else(|| anyhow::anyhow!("unknown color {name:?}"))
}

/// Sizes a shape to its label: one padding cell and a border on each side.
fn fit(kind: ShapeKind, label: &str) -> (i32, i32) {
    let lines = label.split('\n');
    let h = lines.clone().count() as i32 + 2;
    let w = lines.map(|l| l.width()).max().unwrap_or(0) as i32 + 4;
    Shape::snap_size(kind, w.max(6), h)
}

/// Rejects what the renderers cannot draw: an empty id.
fn ids_ok<'a>(op: &str, ids: impl IntoIterator<Item = &'a Id>) -> Result<()> {
    if ids.into_iter().any(|id| id.0.is_empty()) {
        bail!("{op}: empty id");
    }
    Ok(())
}

/// Rejects a line break in a one-line field.
fn one_line(op: &str, field: &str, value: &str) -> Result<()> {
    if value.contains('\n') {
        bail!("{op}: {field} must be one line");
    }
    Ok(())
}

/// Rejects an empty key and a line break in a key or value.
fn check_meta(op: &str, meta: &Option<Meta>) -> Result<()> {
    for (k, v) in meta.iter().flatten() {
        if k.is_empty() {
            bail!("{op}: meta key is empty");
        }
        one_line(op, "meta key", k)?;
        one_line(op, "meta value", v)?;
    }
    Ok(())
}

/// The pairs worth storing: an empty value means no entry.
fn kept(meta: Option<Meta>) -> Meta {
    let mut meta = meta.unwrap_or_default();
    meta.retain(|_, v| !v.is_empty());
    meta
}

fn exist(canvas: &Canvas, ids: &[Id]) -> Result<()> {
    match ids.iter().find(|id| canvas.get(id).is_none()) {
        Some(id) => Err(ModelError::NotFound(id.clone()).into()),
        None => Ok(()),
    }
}

/// Turns one op into commands against the canvas as the earlier ops left it. Pushes the id of each created element onto `created`.
pub(super) fn commands(op: Op, canvas: &Canvas, created: &mut Vec<Id>) -> Result<Vec<Command>> {
    let mut cmds = vec![];
    let add = |created: &mut Vec<Id>, id: Option<Id>, build: &dyn Fn(Id) -> Element| {
        let id = id.unwrap_or_else(Id::random);
        created.push(id.clone());
        Command::Insert(usize::MAX, build(id))
    };
    match op {
        Op::AddShape {
            id,
            kind,
            x,
            y,
            w,
            h,
            label,
            color,
            fill,
            meta,
        } => {
            ids_ok("add_shape", &id)?;
            check_meta("add_shape", &meta)?;
            let meta = kept(meta);
            let kind = kind.unwrap_or(ShapeKind::Rect);
            let label = label.unwrap_or_default();
            let (w, h) = match (w, h) {
                (Some(w), Some(h)) => (w, h),
                (w, h) => {
                    let (fw, fh) = fit(kind, &label);
                    (w.unwrap_or(fw), h.unwrap_or(fh))
                }
            };
            cmds.push(add(created, id, &|id| {
                Element::Shape(Shape {
                    id,
                    color,
                    fill,
                    meta: meta.clone(),
                    ..Shape::new(kind, x, y, w, h, label.clone())
                })
            }));
        }
        Op::AddText {
            id,
            x,
            y,
            text,
            color,
            meta,
        } => {
            ids_ok("add_text", &id)?;
            check_meta("add_text", &meta)?;
            let meta = kept(meta);
            one_line("add_text", "text", &text)?;
            cmds.push(add(created, id, &|id| {
                Element::Text(Text {
                    id,
                    x,
                    y,
                    text: text.clone(),
                    color,
                    meta: meta.clone(),
                })
            }));
        }
        Op::Connect {
            id,
            from,
            to,
            kind,
            label,
            stroke,
            meta,
        } => {
            ids_ok("connect", &id)?;
            check_meta("connect", &meta)?;
            let meta = kept(meta);
            if let Some(l) = &label {
                one_line("connect", "label", l)?;
            }
            let (from, to): (End, End) = (from.into(), to.into());
            cmds.push(add(created, id, &|id| {
                Element::Line(Line {
                    id,
                    label: label.clone().unwrap_or_default(),
                    meta: meta.clone(),
                    stroke: stroke.unwrap_or_default(),
                    ..Line::new(kind.unwrap_or(LineKind::Arrow), from.clone(), to.clone())
                })
            }));
        }
        Op::Set {
            id,
            label,
            color,
            fill,
            stroke,
            border,
            align,
            bold,
            link,
            kind,
            head,
            meta,
        } => {
            ids_ok("set", [&id])?;
            check_meta("set", &meta)?;
            if let Some(l) = &link {
                one_line("set", "link", l)?;
            }
            if let Some(kind) = kind {
                match canvas.get(&id) {
                    Some(Element::Shape(s)) => {
                        cmds.extend(kind_cmds(&id, s.kind, (s.x, s.y, s.w, s.h), kind))
                    }
                    Some(_) => return Err(ModelError::WrongKind(id).into()),
                    None => return Err(ModelError::NotFound(id).into()),
                }
            }
            if let Some(label) = label {
                cmds.push(Command::SetLabel {
                    id: id.clone(),
                    label,
                });
            }
            if let Some(c) = color {
                cmds.push(Command::SetColor {
                    id: id.clone(),
                    color: paint(&c)?,
                });
            }
            if let Some(f) = fill {
                cmds.push(Command::SetFill {
                    id: id.clone(),
                    fill: paint(&f)?,
                });
            }
            if let Some(stroke) = stroke {
                cmds.push(Command::SetStroke {
                    id: id.clone(),
                    stroke,
                });
            }
            if let Some(border) = border {
                cmds.push(Command::SetBorder {
                    id: id.clone(),
                    border,
                });
            }
            if let Some(align) = align {
                cmds.push(Command::SetAlign {
                    id: id.clone(),
                    align,
                });
            }
            if let Some(bold) = bold {
                cmds.push(Command::SetBold {
                    id: id.clone(),
                    bold,
                });
            }
            if let Some(link) = link {
                cmds.push(Command::SetLink {
                    id: id.clone(),
                    link: Some(link).filter(|l| !l.is_empty()),
                });
            }
            if let Some(pairs) = meta.filter(|m| !m.is_empty()) {
                let Some(el) = canvas.get(&id) else {
                    return Err(ModelError::NotFound(id).into());
                };
                let mut merged = el.meta().clone();
                merged.extend(pairs);
                merged.retain(|_, v| !v.is_empty());
                cmds.push(Command::SetMeta {
                    id: id.clone(),
                    meta: merged,
                });
            }
            if let Some(kind) = head {
                cmds.push(Command::SetLineKind { id, kind });
            }
            if cmds.is_empty() {
                bail!("set: nothing to change");
            }
        }
        Op::Resize { id, w, h } => {
            ids_ok("resize", [&id])?;
            match canvas.get(&id) {
                Some(Element::Shape(s)) => cmds.push(Command::SetRect {
                    id,
                    x: s.x,
                    y: s.y,
                    w,
                    h,
                }),
                Some(_) => return Err(ModelError::WrongKind(id).into()),
                None => return Err(ModelError::NotFound(id).into()),
            }
        }
        Op::Group { ids } => {
            ids_ok("group", &ids)?;
            if ids.len() < 2 {
                bail!("group: needs at least two ids");
            }
            exist(canvas, &ids)?;
            cmds.extend(canvas.group_cmd(&ids));
        }
        Op::Ungroup { ids } => {
            ids_ok("ungroup", &ids)?;
            exist(canvas, &ids)?;
            cmds.extend(canvas.ungroup_cmd(&ids));
        }
        Op::Order { ids, z } => {
            ids_ok("order", &ids)?;
            exist(canvas, &ids)?;
            cmds = canvas.z_order(&ids, z);
        }
        Op::Layout { ids, dir } => {
            ids_ok("layout", ids.iter().flatten())?;
            let ids = match ids {
                Some(ids) => {
                    exist(canvas, &ids)?;
                    ids
                }
                None => canvas.elements.iter().map(|e| e.id().clone()).collect(),
            };
            cmds = canvas.layout(&ids, dir.unwrap_or(Dir::TopDown));
        }
        Op::Align { ids, mode } => {
            ids_ok("align", &ids)?;
            exist(canvas, &ids)?;
            cmds = canvas.align(&ids, mode);
        }
        Op::Move { id, dx, dy } => {
            ids_ok("move", [&id])?;
            cmds.push(Command::Move { id, dx, dy });
        }
        Op::Delete { id } => {
            ids_ok("delete", [&id])?;
            cmds.push(Command::Remove(id));
        }
    }
    Ok(cmds)
}

#[cfg(test)]
mod tests {
    use super::super::docs::OPS;
    use super::*;
    use serde_json::{json, Map, Value};

    fn parse(obj: &Map<String, Value>) -> Result<Op, serde_json::Error> {
        serde_json::from_value(Value::Object(obj.clone()))
    }

    #[test]
    fn align_op_moves_the_right_shape_to_the_left_edge() {
        let mut canvas = Canvas::default();
        for (id, x) in [("a", 0), ("b", 10)] {
            let shape = Shape {
                id: Id(id.into()),
                ..Shape::new(ShapeKind::Rect, x, 0, 6, 3, "")
            };
            canvas
                .apply(Command::Insert(usize::MAX, Element::Shape(shape)))
                .unwrap();
        }
        let op: Op =
            serde_json::from_value(json!({"op": "align", "ids": ["a", "b"], "mode": "left"}))
                .unwrap();
        let cmds = commands(op, &canvas, &mut vec![]).unwrap();
        assert_eq!(
            cmds,
            vec![Command::Move {
                id: Id("b".into()),
                dx: -10,
                dy: 0
            }]
        );
    }

    #[test]
    fn ops_table_matches_parser() {
        assert_eq!(OPS.len(), 12);
        for op in &OPS {
            let mut obj = Map::new();
            obj.insert("op".into(), json!(op.name));
            for f in op.fields {
                obj.insert(f.name.into(), serde_json::from_str(f.example).unwrap());
            }
            assert!(parse(&obj).is_ok(), "{}: {:?}", op.name, parse(&obj).err());
            for f in op.fields {
                for v in f.values {
                    let value = serde_json::from_str(v).unwrap_or_else(|_| json!(v));
                    obj.insert(f.name.into(), value);
                    assert!(parse(&obj).is_ok(), "{}.{} = {v}", op.name, f.name);
                }
                obj.insert(f.name.into(), serde_json::from_str(f.example).unwrap());
            }
        }
    }
}
