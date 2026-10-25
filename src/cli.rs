use crate::config::Config;
use crate::diagram::{import_failure, DiagramKind, Flowchart};
use crate::model::{
    Anchor, Canvas, Command, Element, End, Id, Line, LineKind, Paint, Shape, ShapeKind, Text,
};
use crate::store::Store;
use crate::ui::scope::ExportFormat;
use anyhow::{bail, Result};
use serde::Deserialize;
use std::io::{Read, Write};
use unicode_width::UnicodeWidthStr;

pub const COMMANDS: [&str; 6] = ["list", "new", "show", "edit", "import", "export"];

/// An endpoint is a shape id, or a `[x, y]` cell for a free end.
#[derive(Deserialize)]
#[serde(untagged)]
enum Endpoint {
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

/// One edit. `set` takes a color name, or "none" to clear it.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Op {
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
    },
    AddText {
        id: Option<Id>,
        x: i32,
        y: i32,
        text: String,
        color: Option<Paint>,
    },
    Connect {
        id: Option<Id>,
        from: Endpoint,
        to: Endpoint,
        kind: Option<LineKind>,
        label: Option<String>,
    },
    Set {
        id: Id,
        label: Option<String>,
        color: Option<String>,
        fill: Option<String>,
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
    serde_json::from_value(name.into())
        .map(Some)
        .map_err(|_| anyhow::anyhow!("unknown color {name:?}"))
}

/// Sizes a shape to its label: one padding cell and a border on each side.
fn fit(kind: ShapeKind, label: &str) -> (i32, i32) {
    let lines = label.split('\n');
    let h = lines.clone().count() as i32 + 2;
    let w = lines.map(|l| l.width()).max().unwrap_or(0) as i32 + 4;
    Shape::snap_size(kind, w.max(6), h)
}

/// Rejects what the renderers cannot draw: an empty id, a set with no field, or a line break in a one-line label.
fn check(op: &Op) -> Result<()> {
    let (name, id, one_line) = match op {
        Op::AddShape { id, .. } => ("add_shape", id.as_ref(), None),
        Op::AddText { id, text, .. } => ("add_text", id.as_ref(), Some(("text", text.as_str()))),
        Op::Connect { id, label, .. } => (
            "connect",
            id.as_ref(),
            label.as_deref().map(|l| ("label", l)),
        ),
        Op::Set {
            label: None,
            color: None,
            fill: None,
            ..
        } => bail!("set: nothing to change"),
        Op::Set { id, .. } => ("set", Some(id), None),
        Op::Move { id, .. } => ("move", Some(id), None),
        Op::Delete { id } => ("delete", Some(id), None),
    };
    if id.is_some_and(|id| id.0.is_empty()) {
        bail!("{name}: empty id");
    }
    if let Some((field, value)) = one_line {
        if value.contains('\n') {
            bail!("{name}: {field} must be one line");
        }
    }
    Ok(())
}

/// Turns ops into one command each. Pushes the id of each created element onto `created`.
fn commands(ops: Vec<Op>, created: &mut Vec<Id>) -> Result<Vec<Command>> {
    let mut cmds = vec![];
    let add = |created: &mut Vec<Id>, id: Option<Id>, build: &dyn Fn(Id) -> Element| {
        let id = id.unwrap_or_else(Id::random);
        created.push(id.clone());
        Command::Insert(usize::MAX, build(id))
    };
    for op in ops {
        check(&op)?;
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
            } => {
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
                        kind,
                        x,
                        y,
                        w,
                        h,
                        label: label.clone(),
                        stroke: Default::default(),
                        border: Default::default(),
                        color,
                        fill,
                        align: Default::default(),
                        bold: false,
                        link: None,
                    })
                }));
            }
            Op::AddText {
                id,
                x,
                y,
                text,
                color,
            } => {
                cmds.push(add(created, id, &|id| {
                    Element::Text(Text {
                        id,
                        x,
                        y,
                        text: text.clone(),
                        color,
                    })
                }));
            }
            Op::Connect {
                id,
                from,
                to,
                kind,
                label,
            } => {
                let (from, to): (End, End) = (from.into(), to.into());
                cmds.push(add(created, id, &|id| {
                    Element::Line(Line {
                        id,
                        kind: kind.unwrap_or(LineKind::Arrow),
                        from: from.clone(),
                        to: to.clone(),
                        label: label.clone().unwrap_or_default(),
                        label_at: None,
                        stroke: Default::default(),
                        bends: vec![],
                        color: None,
                    })
                }));
            }
            Op::Set {
                id,
                label,
                color,
                fill,
            } => {
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
                        id,
                        fill: paint(&f)?,
                    });
                }
            }
            Op::Move { id, dx, dy } => cmds.push(Command::Move { id, dx, dy }),
            Op::Delete { id } => cmds.push(Command::Remove(id)),
        }
    }
    Ok(cmds)
}

fn edit(name: &str, store: &Store, stdin: &mut impl Read, out: &mut impl Write) -> Result<()> {
    let mut canvas = store.load(name)?;
    let mut text = String::new();
    stdin.read_to_string(&mut text)?;
    let ops: Vec<Op> = serde_json::from_str(&text)?;
    let mut created = vec![];
    let applied = ops.len();
    let cmds = commands(ops, &mut created)?;
    let batch = Command::Batch(cmds);
    let moved = batch.rerouting();
    canvas.apply(batch)?;
    canvas.apply(Command::Batch(canvas.pin_cmds(&moved)))?;
    store.save(name, &canvas)?;
    let ids: Vec<&str> = created.iter().map(|i| i.0.as_str()).collect();
    writeln!(
        out,
        "{}",
        serde_json::json!({ "applied": applied, "ids": ids })
    )?;
    Ok(())
}

fn import(
    name: &str,
    path: &str,
    store: &Store,
    stdin: &mut impl Read,
    out: &mut impl Write,
) -> Result<()> {
    let mut canvas = if store.exists(name) {
        store.load(name)?
    } else {
        Canvas::default()
    };
    let mut text = String::new();
    if path == "-" {
        stdin.read_to_string(&mut text)?;
    } else {
        text = std::fs::read_to_string(path)?;
    }
    let input = Flowchart
        .import(&text)
        .map_err(|e| anyhow::Error::msg(import_failure(&text, &e)))?;
    let parsed = input.canvas;
    let ids: Vec<Id> = parsed.elements.iter().map(|e| e.id().clone()).collect();
    let (px, py, _, _) = parsed.all_bounds().unwrap_or_default();
    let (ox, oy) = canvas
        .all_bounds()
        .map_or((0, 0), |(x, y, _, h)| (x, y + h + 1));
    let fresh = parsed.clone_canvas(&ids, ox - px, oy - py);
    let shapes = fresh
        .elements
        .iter()
        .filter(|e| matches!(e, Element::Shape(_)))
        .count();
    let lines = fresh.elements.len() - shapes;
    let inserts = fresh
        .elements
        .into_iter()
        .map(|e| Command::Insert(usize::MAX, e))
        .collect();
    canvas.apply(Command::Batch(inserts))?;
    canvas.groups.extend(fresh.groups);
    store.save(name, &canvas)?;
    writeln!(
        out,
        "{}",
        serde_json::json!({ "shapes": shapes, "lines": lines, "skipped": input.skipped })
    )?;
    Ok(())
}

fn export(args: &[String], store: &Store, out: &mut impl Write) -> Result<()> {
    let (name, flag, path) = match args {
        [name, flag, path] => (name, flag.as_str(), path),
        _ => bail!("usage: clidraw export <name> --svg|--mermaid|--clidraw <path|->"),
    };
    let format = match flag {
        "--svg" => ExportFormat::Svg,
        "--mermaid" => ExportFormat::Mermaid,
        "--clidraw" => ExportFormat::Clidraw,
        _ => bail!("unknown format {flag:?}, use --svg, --mermaid or --clidraw"),
    };
    let canvas = store.load(name)?;
    let (config, _) = Config::load(store.dir());
    let (text, skipped) = crate::export::render(&canvas, format, config.export_padding);
    if path == "-" {
        write!(out, "{text}")?;
    } else {
        std::fs::write(path, text)?;
    }
    if skipped > 0 {
        eprintln!("skipped {skipped} element(s) with no Mermaid form");
    }
    Ok(())
}

/// Runs one command; `args[0]` is a member of `COMMANDS`.
pub fn run(
    args: &[String],
    store: &Store,
    stdin: &mut impl Read,
    out: &mut impl Write,
) -> Result<()> {
    let rest: Vec<&str> = args[1..].iter().map(String::as_str).collect();
    match (args[0].as_str(), rest.as_slice()) {
        ("list", []) => writeln!(out, "{}", serde_json::to_string(&store.list()?)?)?,
        ("new", [name]) => {
            store.create(name)?;
            writeln!(out, "{}", serde_json::json!({ "created": name }))?;
        }
        ("show", [name]) => writeln!(out, "{}", serde_json::to_string_pretty(&store.load(name)?)?)?,
        ("edit", [name]) => edit(name, store, stdin, out)?,
        ("import", [name, path]) => import(name, path, store, stdin, out)?,
        ("export", _) => export(&args[1..], store, out)?,
        (cmd, _) => bail!("usage: clidraw {cmd} (see README)"),
    }
    Ok(())
}
