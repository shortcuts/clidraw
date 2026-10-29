use crate::config::Config;
use crate::diagram::{self, import_failure};
use crate::model::{Canvas, Command, Element, Id};
use crate::store::Store;
use crate::ui::scope::ExportFormat;
use anyhow::{bail, Result};
use std::io::{Read, Write};

pub mod docs;
mod ops;

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
        "Adds a Mermaid diagram from a file, or from stdin with -. The importer reads flowchart, sequence and class diagram syntax. Creates the canvas if it does not exist. Prints {\"shapes\":N,\"lines\":N,\"skipped\":N}.",
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

fn edit(name: &str, store: &Store, stdin: &mut impl Read, out: &mut impl Write) -> Result<()> {
    let mut canvas = store.load(name)?;
    let mut text = String::new();
    stdin.read_to_string(&mut text)?;
    let ops: Vec<ops::Op> = serde_json::from_str(&text)?;
    let mut created = vec![];
    let mut moved = vec![];
    let applied = ops.len();
    for op in ops {
        let batch = Command::Batch(ops::commands(op, &canvas, &mut created)?);
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
    let input =
        diagram::import(&text).map_err(|e| anyhow::Error::msg(import_failure(&text, &e)))?;
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

    #[test]
    fn command_docs_match_commands() {
        let names: Vec<&str> = COMMAND_DOCS.iter().map(|c| c.0).collect();
        assert_eq!(names, COMMANDS);
    }
}
