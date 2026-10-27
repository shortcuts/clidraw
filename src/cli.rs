use crate::config::Config;
use crate::diagram::{import_failure, DiagramKind, Flowchart};
use crate::model::{
    kind_cmds, Anchor, Border, Canvas, Command, Dir, Element, End, Id, Line, LineKind, ModelError,
    Paint, Shape, ShapeKind, Stroke, Text, TextAlign, Z,
};
use crate::store::Store;
use crate::ui::scope::ExportFormat;
use anyhow::{bail, Result};
use serde::Deserialize;
use std::io::{Read, Write};
use unicode_width::UnicodeWidthStr;

pub const COMMANDS: [&str; 8] = [
    "list", "new", "show", "edit", "import", "export", "rename", "delete",
];

/// (name, usage, description) for each entry of `COMMANDS`, in the same order.
pub const COMMAND_DOCS: [(&str, &str, &str); 8] = [
    (
        "list",
        "clidraw list",
        "Prints the names of all canvases as a JSON array.",
    ),
    (
        "new",
        "clidraw new <name>",
        "Creates an empty canvas. Prints {\"created\":name}.",
    ),
    (
        "show",
        "clidraw show <name>",
        "Prints the canvas as JSON.",
    ),
    (
        "edit",
        "clidraw edit <name>",
        "Reads a JSON array of ops from stdin and applies them in order. Applies all ops or none. Prints {\"applied\":N,\"ids\":[...]}, where ids lists the created elements.",
    ),
    (
        "import",
        "clidraw import <name> <path|->",
        "Adds a Mermaid flowchart from a file, or from stdin with -. Creates the canvas if it does not exist. Prints {\"shapes\":N,\"lines\":N,\"skipped\":N}.",
    ),
    (
        "export",
        "clidraw export <name> --svg|--mermaid|--clidraw <path|->",
        "Writes the canvas in one format to a file, or to stdout with -.",
    ),
    (
        "rename",
        "clidraw rename <from> <to>",
        "Renames a canvas. Prints {\"renamed\":to}.",
    ),
    (
        "delete",
        "clidraw delete <name>",
        "Deletes a canvas. Prints {\"deleted\":name}.",
    ),
];

const COLORS: &[&str] = &["red", "orange", "yellow", "green", "blue", "violet"];
const COLORS_OR_NONE: &[&str] = &["red", "orange", "yellow", "green", "blue", "violet", "none"];
const SHAPE_KINDS: &[&str] = &[
    "Rect",
    "Diamond",
    "Cylinder",
    "Parallelogram",
    "Hexagon",
    "Frame",
];
const LINE_KINDS: &[&str] = &["Line", "Arrow", "ArrowStart", "ArrowBoth"];
const STROKES: &[&str] = &["solid", "dashed", "dotted"];
const BORDERS: &[&str] = &["single", "rounded", "double", "heavy"];
const ALIGNS: &[&str] = &["left", "center", "right"];
const BOOLS: &[&str] = &["true", "false"];
const Z_ORDERS: &[&str] = &["front", "back", "forward", "backward"];
const DIRS: &[&str] = &["td", "lr"];

/// One field of an op, as the reference page shows it.
pub struct FieldDoc {
    pub name: &'static str,
    pub required: bool,
    /// A JSON literal the parser accepts for this field.
    pub example: &'static str,
    /// Accepted values: a bare word is a string, `true` and `false` are booleans. Empty when the field takes any value.
    pub values: &'static [&'static str],
    pub note: &'static str,
}

/// One edit op. Add each new `Op` field here too: the test cannot see an optional field the table lacks.
pub struct OpDoc {
    pub name: &'static str,
    pub summary: &'static str,
    pub fields: &'static [FieldDoc],
}

pub const OPS: [OpDoc; 11] = [
    OpDoc {
        name: "add_shape",
        summary: "Adds a shape. Its size fits the label unless w and h are set.",
        fields: &[
            FieldDoc {
                name: "x",
                required: true,
                example: "0",
                values: &[],
                note: "Column, in cells.",
            },
            FieldDoc {
                name: "y",
                required: true,
                example: "0",
                values: &[],
                note: "Row, in cells.",
            },
            FieldDoc {
                name: "id",
                required: false,
                example: "\"a\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "kind",
                required: false,
                example: "\"Rect\"",
                values: SHAPE_KINDS,
                note: "Default: Rect.",
            },
            FieldDoc {
                name: "w",
                required: false,
                example: "10",
                values: &[],
                note: "Width, in cells. Default fits the label.",
            },
            FieldDoc {
                name: "h",
                required: false,
                example: "3",
                values: &[],
                note: "Height, in cells. Default fits the label.",
            },
            FieldDoc {
                name: "label",
                required: false,
                example: "\"Hi\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "color",
                required: false,
                example: "\"red\"",
                values: COLORS,
                note: "",
            },
            FieldDoc {
                name: "fill",
                required: false,
                example: "\"blue\"",
                values: COLORS,
                note: "",
            },
        ],
    },
    OpDoc {
        name: "add_text",
        summary: "Adds a line of text.",
        fields: &[
            FieldDoc {
                name: "x",
                required: true,
                example: "0",
                values: &[],
                note: "Column, in cells.",
            },
            FieldDoc {
                name: "y",
                required: true,
                example: "0",
                values: &[],
                note: "Row, in cells.",
            },
            FieldDoc {
                name: "text",
                required: true,
                example: "\"Hi\"",
                values: &[],
                note: "One line.",
            },
            FieldDoc {
                name: "id",
                required: false,
                example: "\"a\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "color",
                required: false,
                example: "\"red\"",
                values: COLORS,
                note: "",
            },
        ],
    },
    OpDoc {
        name: "connect",
        summary: "Adds a line from one endpoint to another.",
        fields: &[
            FieldDoc {
                name: "from",
                required: true,
                example: "\"a\"",
                values: &[],
                note: "A shape id, or a [x, y] cell for a free end.",
            },
            FieldDoc {
                name: "to",
                required: true,
                example: "[0, 0]",
                values: &[],
                note: "A shape id, or a [x, y] cell for a free end.",
            },
            FieldDoc {
                name: "id",
                required: false,
                example: "\"a\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "kind",
                required: false,
                example: "\"Arrow\"",
                values: LINE_KINDS,
                note: "Default: Arrow.",
            },
            FieldDoc {
                name: "label",
                required: false,
                example: "\"Yes\"",
                values: &[],
                note: "One line.",
            },
        ],
    },
    OpDoc {
        name: "set",
        summary: "Changes one element. Set at least one field besides id.",
        fields: &[
            FieldDoc {
                name: "id",
                required: true,
                example: "\"a\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "label",
                required: false,
                example: "\"Hi\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "color",
                required: false,
                example: "\"red\"",
                values: COLORS_OR_NONE,
                note: "Use none to clear the color.",
            },
            FieldDoc {
                name: "fill",
                required: false,
                example: "\"blue\"",
                values: COLORS_OR_NONE,
                note: "Use none to clear the fill.",
            },
            FieldDoc {
                name: "stroke",
                required: false,
                example: "\"dashed\"",
                values: STROKES,
                note: "",
            },
            FieldDoc {
                name: "border",
                required: false,
                example: "\"rounded\"",
                values: BORDERS,
                note: "",
            },
            FieldDoc {
                name: "align",
                required: false,
                example: "\"left\"",
                values: ALIGNS,
                note: "",
            },
            FieldDoc {
                name: "bold",
                required: false,
                example: "true",
                values: BOOLS,
                note: "",
            },
            FieldDoc {
                name: "link",
                required: false,
                example: "\"https://example.com\"",
                values: &[],
                note: "One line. An empty string clears the link.",
            },
            FieldDoc {
                name: "kind",
                required: false,
                example: "\"Diamond\"",
                values: SHAPE_KINDS,
                note: "Changes the kind of a shape.",
            },
            FieldDoc {
                name: "head",
                required: false,
                example: "\"ArrowBoth\"",
                values: LINE_KINDS,
                note: "Changes the kind of a line.",
            },
        ],
    },
    OpDoc {
        name: "resize",
        summary: "Sets the size of a shape.",
        fields: &[
            FieldDoc {
                name: "id",
                required: true,
                example: "\"a\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "w",
                required: true,
                example: "10",
                values: &[],
                note: "Width, in cells.",
            },
            FieldDoc {
                name: "h",
                required: true,
                example: "3",
                values: &[],
                note: "Height, in cells.",
            },
        ],
    },
    OpDoc {
        name: "move",
        summary: "Moves one element.",
        fields: &[
            FieldDoc {
                name: "id",
                required: true,
                example: "\"a\"",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "dx",
                required: true,
                example: "1",
                values: &[],
                note: "Columns to move. Negative moves left.",
            },
            FieldDoc {
                name: "dy",
                required: true,
                example: "0",
                values: &[],
                note: "Rows to move. Negative moves up.",
            },
        ],
    },
    OpDoc {
        name: "delete",
        summary: "Deletes one element.",
        fields: &[FieldDoc {
            name: "id",
            required: true,
            example: "\"a\"",
            values: &[],
            note: "",
        }],
    },
    OpDoc {
        name: "group",
        summary: "Groups two or more elements.",
        fields: &[FieldDoc {
            name: "ids",
            required: true,
            example: "[\"a\", \"b\"]",
            values: &[],
            note: "At least two ids.",
        }],
    },
    OpDoc {
        name: "ungroup",
        summary: "Removes the group that holds the elements.",
        fields: &[FieldDoc {
            name: "ids",
            required: true,
            example: "[\"a\", \"b\"]",
            values: &[],
            note: "",
        }],
    },
    OpDoc {
        name: "order",
        summary: "Changes the stacking order of elements.",
        fields: &[
            FieldDoc {
                name: "ids",
                required: true,
                example: "[\"a\", \"b\"]",
                values: &[],
                note: "",
            },
            FieldDoc {
                name: "z",
                required: true,
                example: "\"front\"",
                values: Z_ORDERS,
                note: "",
            },
        ],
    },
    OpDoc {
        name: "layout",
        summary: "Arranges elements by the lines between them.",
        fields: &[
            FieldDoc {
                name: "ids",
                required: false,
                example: "[\"a\", \"b\"]",
                values: &[],
                note: "Without ids, arranges every element.",
            },
            FieldDoc {
                name: "dir",
                required: false,
                example: "\"td\"",
                values: DIRS,
                note: "td runs top-down, lr runs left-right. Default: td.",
            },
        ],
    },
];

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
        stroke: Option<Stroke>,
        border: Option<Border>,
        align: Option<TextAlign>,
        bold: Option<bool>,
        link: Option<String>,
        kind: Option<ShapeKind>,
        head: Option<LineKind>,
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

/// Rejects what the renderers cannot draw: an empty id or a line break in a one-line field.
fn check(op: &Op) -> Result<()> {
    let (name, ids, one_line): (_, Vec<&Id>, _) = match op {
        Op::AddShape { id, .. } => ("add_shape", id.iter().collect(), None),
        Op::AddText { id, text, .. } => (
            "add_text",
            id.iter().collect(),
            Some(("text", text.as_str())),
        ),
        Op::Connect { id, label, .. } => (
            "connect",
            id.iter().collect(),
            label.as_deref().map(|l| ("label", l)),
        ),
        Op::Set { id, link, .. } => ("set", vec![id], link.as_deref().map(|l| ("link", l))),
        Op::Resize { id, .. } => ("resize", vec![id], None),
        Op::Group { ids } => ("group", ids.iter().collect(), None),
        Op::Ungroup { ids } => ("ungroup", ids.iter().collect(), None),
        Op::Order { ids, .. } => ("order", ids.iter().collect(), None),
        Op::Layout { ids, .. } => ("layout", ids.iter().flatten().collect(), None),
        Op::Move { id, .. } => ("move", vec![id], None),
        Op::Delete { id } => ("delete", vec![id], None),
    };
    if ids.iter().any(|id| id.0.is_empty()) {
        bail!("{name}: empty id");
    }
    if let Some((field, value)) = one_line {
        if value.contains('\n') {
            bail!("{name}: {field} must be one line");
        }
    }
    Ok(())
}

fn exist(canvas: &Canvas, ids: &[Id]) -> Result<()> {
    match ids.iter().find(|id| canvas.get(id).is_none()) {
        Some(id) => Err(ModelError::NotFound(id.clone()).into()),
        None => Ok(()),
    }
}

/// Turns one op into commands against the canvas as the earlier ops left it. Pushes the id of each created element onto `created`.
fn commands(op: Op, canvas: &Canvas, created: &mut Vec<Id>) -> Result<Vec<Command>> {
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
                    side_labels: vec![],
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
            stroke,
            border,
            align,
            bold,
            link,
            kind,
            head,
        } => {
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
            if let Some(kind) = head {
                cmds.push(Command::SetLineKind { id, kind });
            }
            if cmds.is_empty() {
                bail!("set: nothing to change");
            }
        }
        Op::Resize { id, w, h } => match canvas.get(&id) {
            Some(Element::Shape(s)) => cmds.push(Command::SetRect {
                id,
                x: s.x,
                y: s.y,
                w,
                h,
            }),
            Some(_) => return Err(ModelError::WrongKind(id).into()),
            None => return Err(ModelError::NotFound(id).into()),
        },
        Op::Group { ids } => {
            if ids.len() < 2 {
                bail!("group: needs at least two ids");
            }
            exist(canvas, &ids)?;
            cmds.extend(canvas.group_cmd(&ids));
        }
        Op::Ungroup { ids } => {
            exist(canvas, &ids)?;
            cmds.extend(canvas.ungroup_cmd(&ids));
        }
        Op::Order { ids, z } => {
            exist(canvas, &ids)?;
            cmds = canvas.z_order(&ids, z);
        }
        Op::Layout { ids, dir } => {
            let ids = match ids {
                Some(ids) => {
                    exist(canvas, &ids)?;
                    ids
                }
                None => canvas.elements.iter().map(|e| e.id().clone()).collect(),
            };
            cmds = canvas.layout(&ids, dir.unwrap_or(Dir::TopDown));
        }
        Op::Move { id, dx, dy } => cmds.push(Command::Move { id, dx, dy }),
        Op::Delete { id } => cmds.push(Command::Remove(id)),
    }
    Ok(cmds)
}

fn edit(name: &str, store: &Store, stdin: &mut impl Read, out: &mut impl Write) -> Result<()> {
    let mut canvas = store.load(name)?;
    let mut text = String::new();
    stdin.read_to_string(&mut text)?;
    let ops: Vec<Op> = serde_json::from_str(&text)?;
    let mut created = vec![];
    let mut moved = vec![];
    let applied = ops.len();
    for op in ops {
        check(&op)?;
        let batch = Command::Batch(commands(op, &canvas, &mut created)?);
        moved.extend(batch.rerouting());
        canvas.apply(batch)?;
    }
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
        ("rename", [from, to]) => {
            store.rename(from, to)?;
            writeln!(out, "{}", serde_json::json!({ "renamed": to }))?;
        }
        ("delete", [name]) => {
            store.delete(name)?;
            writeln!(out, "{}", serde_json::json!({ "deleted": name }))?;
        }
        (cmd, _) => bail!("usage: clidraw {cmd} (see https://clidraw.shrtcts.fr/cli.html)"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Map, Value};

    fn parse(obj: &Map<String, Value>) -> Result<Op, serde_json::Error> {
        serde_json::from_value(Value::Object(obj.clone()))
    }

    #[test]
    fn ops_table_matches_parser() {
        assert_eq!(OPS.len(), 11);
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

    #[test]
    fn command_docs_match_commands() {
        let names: Vec<&str> = COMMAND_DOCS.iter().map(|c| c.0).collect();
        assert_eq!(names, COMMANDS);
    }
}
