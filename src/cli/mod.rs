use crate::app::template;
use crate::config::Config;
use crate::diagram::{self, import_failure};
use crate::model::{Canvas, Command, Element, History, Id};
use crate::store::Store;
use crate::ui::scope::ExportFormat;
use anyhow::{bail, Result};
use std::io::{Read, Write};

pub mod docs;
mod ops;

pub const COMMANDS: [&str; 12] = [
    "list",
    "new",
    "show",
    "edit",
    "import",
    "export",
    "rename",
    "undo",
    "redo",
    "delete",
    "templates",
    "template",
];

/// (name, usage, description) for each entry of `COMMANDS`, in the same order.
pub const COMMAND_DOCS: [(&str, &str, &str); 12] = [
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
        "undo",
        "clidraw undo <name>",
        "Reverts the last edit or import on a canvas. Prints {\"undone\":label}.",
    ),
    (
        "redo",
        "clidraw redo <name>",
        "Re-applies the last undone step. Prints {\"redone\":label}.",
    ),
    (
        "delete",
        "clidraw delete <name>",
        "Deletes a canvas. Prints {\"deleted\":name}.",
    ),
    (
        "templates",
        "clidraw templates",
        "Prints the names of the built-in templates as a JSON array.",
    ),
    (
        "template",
        "clidraw template <name> <template>",
        "Adds a built-in template inside a frame, below the existing content. Creates the canvas if it does not exist. Prints {\"shapes\":N,\"lines\":N}.",
    ),
];

fn load_history(name: &str, store: &Store) -> History {
    let mut history = History::with_limit(Config::load(store.dir()).0.history_limit);
    let (undo, redo) = store.load_history(name);
    history.restore(undo, redo);
    history
}

fn save_history(name: &str, history: &History, store: &Store) -> Result<()> {
    Ok(store.save_history(name, history.commands(), history.redo_commands())?)
}

fn step(name: &str, store: &Store, redo: bool, out: &mut impl Write) -> Result<()> {
    let mut canvas = store.load(name)?;
    let mut history = load_history(name, store);
    let (done, key) = if redo {
        (history.redo(&mut canvas)?, "redone")
    } else {
        (history.undo(&mut canvas)?, "undone")
    };
    let Some(label) = done else {
        bail!("nothing to {}", if redo { "redo" } else { "undo" });
    };
    store.save(name, &canvas)?;
    save_history(name, &history, store)?;
    writeln!(out, "{}", serde_json::json!({ key: label }))?;
    Ok(())
}

fn edit(name: &str, store: &Store, stdin: &mut impl Read, out: &mut impl Write) -> Result<()> {
    let mut canvas = store.load(name)?;
    let mut history = load_history(name, store);
    let mut text = String::new();
    stdin.read_to_string(&mut text)?;
    let ops: Vec<ops::Op> = serde_json::from_str(&text)?;
    let mut created = vec![];
    let mut inverses = vec![];
    let mut moved = vec![];
    let applied = ops.len();
    for op in ops {
        let batch = Command::Batch(ops::commands(op, &canvas, &mut created)?);
        moved.extend(batch.rerouting());
        inverses.push(canvas.apply(batch)?);
    }
    inverses.push(canvas.apply(Command::Batch(canvas.pin_cmds(&moved)))?);
    if applied > 0 {
        inverses.reverse();
        history.record(Command::Batch(inverses));
    }
    store.save(name, &canvas)?;
    save_history(name, &history, store)?;
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
    let existed = store.exists(name);
    let mut canvas = if existed {
        store.load(name)?
    } else {
        Canvas::default()
    };
    let mut history = load_history(name, store);
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
    let undo_inserts = canvas.apply(Command::Batch(inserts))?;
    let groups = [canvas.groups.clone(), fresh.groups].concat();
    let undo_groups = canvas.apply(Command::SetGroups(groups))?;
    if existed {
        history.record(Command::Batch(vec![undo_groups, undo_inserts]));
    }
    store.save(name, &canvas)?;
    save_history(name, &history, store)?;
    writeln!(
        out,
        "{}",
        serde_json::json!({ "shapes": shapes, "lines": lines, "skipped": input.skipped })
    )?;
    Ok(())
}

fn template(name: &str, tpl: &str, store: &Store, out: &mut impl Write) -> Result<()> {
    let built = template::build(tpl)
        .map_err(|_| anyhow::anyhow!("no template {tpl:?}; run clidraw templates"))?;
    let mut canvas = if store.exists(name) {
        store.load(name)?
    } else {
        Canvas::default()
    };
    let (ox, oy) = canvas
        .all_bounds()
        .map_or((0, 0), |(x, y, _, h)| (x, y + h + 1));
    let fresh = template::framed(template::pinned(built), tpl, ox, oy);
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
        serde_json::json!({ "shapes": shapes, "lines": lines })
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
        ("templates", []) => writeln!(out, "{}", serde_json::to_string(&template::names())?)?,
        ("template", [name, tpl]) => template(name, tpl, store, out)?,
        ("import", [name, path]) => import(name, path, store, stdin, out)?,
        ("undo", [name]) => step(name, store, false, out)?,
        ("redo", [name]) => step(name, store, true, out)?,
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
