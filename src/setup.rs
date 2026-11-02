use crate::conventions::CONVENTIONS;
use anyhow::{anyhow, bail, Result};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const BEGIN: &str = "<!-- clidraw:begin -->";
const END: &str = "<!-- clidraw:end -->";

pub const HELP: (&str, &str, &str) = (
    "setup",
    "clidraw setup --claude|--codex",
    "Wires clidraw into an agent harness. --claude registers the MCP server at Claude Code user scope with `claude mcp add` and writes the canvas conventions between <!-- clidraw:begin --> and <!-- clidraw:end --> in CLAUDE.md. It honors CLAUDE_CONFIG_DIR. --codex registers the MCP server with `codex mcp add` and writes the same block in AGENTS.md under CODEX_HOME (default ~/.codex). A re-run rewrites only that block.",
);

struct Agent {
    name: &'static str,
    bin: &'static str,
    dir_env: &'static str,
    home_subdir: &'static str,
    rules_file: &'static str,
    remove: &'static [&'static str],
    add: &'static [&'static str],
}

const CLAUDE: Agent = Agent {
    name: "claude",
    bin: "claude",
    dir_env: "CLAUDE_CONFIG_DIR",
    home_subdir: ".claude",
    rules_file: "CLAUDE.md",
    remove: &["mcp", "remove", "--scope", "user", "clidraw"],
    add: &[
        "mcp", "add", "--scope", "user", "clidraw", "--", "clidraw", "mcp",
    ],
};

const CODEX: Agent = Agent {
    name: "codex",
    bin: "codex",
    dir_env: "CODEX_HOME",
    home_subdir: ".codex",
    rules_file: "AGENTS.md",
    remove: &["mcp", "remove", "clidraw"],
    add: &["mcp", "add", "clidraw", "--", "clidraw", "mcp"],
};

pub fn run(args: &[String], out: &mut impl Write) -> Result<()> {
    let mut selected: Vec<&Agent> = vec![];
    for arg in &args[1..] {
        let a = match arg.as_str() {
            "--claude" => &CLAUDE,
            "--codex" => &CODEX,
            other => bail!("unknown setup option: {other}"),
        };
        if !selected.iter().any(|s| s.name == a.name) {
            selected.push(a);
        }
    }
    if selected.is_empty() {
        bail!("usage: clidraw setup --claude|--codex");
    }
    for a in selected {
        agent(out, a)?;
    }
    Ok(())
}

fn agent(out: &mut impl Write, a: &Agent) -> Result<()> {
    let dir = match std::env::var(a.dir_env) {
        Ok(d) if !d.is_empty() => PathBuf::from(d),
        _ => std::env::home_dir()
            .ok_or_else(|| anyhow!("cannot find a home directory"))?
            .join(a.home_subdir),
    };
    let path = dir.join(a.rules_file);
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let new = upsert(&old, &block()).map_err(|e| anyhow!("{}: {e}", path.display()))?;
    register(a)?;
    writeln!(out, "{}: registered MCP server clidraw", a.name)?;
    std::fs::create_dir_all(&dir)?;
    // Plain write, not a rename: CLAUDE.md is often a symlink into a dotfile repo.
    std::fs::write(&path, new)?;
    writeln!(out, "{}: wrote {}", a.name, path.display())?;
    Ok(())
}

fn register(a: &Agent) -> Result<()> {
    let (bin, remove, add) = (a.bin, a.remove, a.add);
    let spawn_error = |e| {
        anyhow!(
            "cannot run `{bin}`: {e}. Install it, then run `clidraw setup --{}` again",
            a.name
        )
    };
    // The entry may not exist; `add` fails on an existing name, so a re-run removes first.
    Command::new(bin)
        .args(remove)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(spawn_error)?;
    let output = Command::new(bin).args(add).output().map_err(spawn_error)?;
    if !output.status.success() {
        bail!(
            "`{bin} {}` failed: {}",
            add.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn block() -> String {
    format!("{BEGIN}\n## clidraw\n\n{CONVENTIONS}{END}\n")
}

fn upsert(text: &str, block: &str) -> Result<String> {
    match (text.find(BEGIN), text.find(END)) {
        (Some(b), Some(e)) if b < e => {
            let after = e + END.len();
            let line_end = text[after..]
                .find('\n')
                .map_or(text.len(), |n| after + n + 1);
            Ok(format!("{}{block}{}", &text[..b], &text[line_end..]))
        }
        (None, None) => {
            let mut out = text.to_string();
            if !out.is_empty() {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push('\n');
            }
            out += block;
            Ok(out)
        }
        _ => bail!("expected one {BEGIN} followed by one {END}; fix or remove the markers"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: &str = "<!-- clidraw:begin -->\nnew\n<!-- clidraw:end -->\n";

    #[test]
    fn upsert_appends_to_empty_text() {
        assert_eq!(upsert("", B).unwrap(), B);
    }

    #[test]
    fn upsert_keeps_text_around_the_block() {
        let old = "top\n<!-- clidraw:begin -->\nold\n<!-- clidraw:end -->\nbottom\n";
        assert_eq!(upsert(old, B).unwrap(), format!("top\n{B}bottom\n"));
        assert_eq!(upsert("top", B).unwrap(), format!("top\n\n{B}"));
    }

    #[test]
    fn upsert_replaces_an_existing_block() {
        let old = "<!-- clidraw:begin -->\nold\n<!-- clidraw:end -->\n";
        assert_eq!(upsert(old, B).unwrap(), B);
    }

    #[test]
    fn upsert_is_idempotent() {
        let once = upsert("top\n", B).unwrap();
        assert_eq!(upsert(&once, B).unwrap(), once);
    }

    #[test]
    fn upsert_rejects_a_lone_marker() {
        assert!(upsert("<!-- clidraw:begin -->\nx\n", B).is_err());
        assert!(upsert("<!-- clidraw:end -->\n<!-- clidraw:begin -->\n", B).is_err());
    }
}
