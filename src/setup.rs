use crate::conventions::CONVENTIONS;
use anyhow::{anyhow, bail, Result};
use std::io::Write;
use std::ops::Range;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const BEGIN: &str = "<!-- clidraw:begin -->";
const END: &str = "<!-- clidraw:end -->";

pub const HELP: (&str, &str, &str) = (
    "setup",
    "clidraw setup --claude|--codex|--cursor|--all [--uninstall]",
    "Wires clidraw into an agent harness. --claude registers the MCP server at Claude Code user scope with `claude mcp add` and writes the canvas conventions between <!-- clidraw:begin --> and <!-- clidraw:end --> in CLAUDE.md. It honors CLAUDE_CONFIG_DIR. --codex registers the MCP server with `codex mcp add` and writes the same block in AGENTS.md under CODEX_HOME (default ~/.codex). --cursor merges the server into ~/.cursor/mcp.json and keeps its other servers; Cursor has no rules file, so the conventions come from the MCP instructions. --all runs Claude, Codex and Cursor and skips a harness whose command is not installed. A re-run rewrites only that block. --uninstall removes the block and the MCP server for the chosen harnesses and keeps the rest of each file.",
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

#[derive(Clone, Copy)]
enum Target {
    Agent(&'static Agent),
    Cursor,
}

impl Target {
    fn name(self) -> &'static str {
        match self {
            Target::Agent(a) => a.name,
            Target::Cursor => "cursor",
        }
    }
}

pub fn run(args: &[String], out: &mut impl Write) -> Result<()> {
    let mut selected: Vec<Target> = vec![];
    let mut uninstall = false;
    let mut all = false;
    for arg in &args[1..] {
        let targets = match arg.as_str() {
            "--uninstall" => {
                uninstall = true;
                continue;
            }
            "--claude" => vec![Target::Agent(&CLAUDE)],
            "--codex" => vec![Target::Agent(&CODEX)],
            "--cursor" => vec![Target::Cursor],
            "--all" => {
                all = true;
                vec![
                    Target::Agent(&CLAUDE),
                    Target::Agent(&CODEX),
                    Target::Cursor,
                ]
            }
            other => bail!("unknown setup option: {other}"),
        };
        for t in targets {
            if !selected.iter().any(|s| s.name() == t.name()) {
                selected.push(t);
            }
        }
    }
    if selected.is_empty() {
        bail!("usage: clidraw setup --claude|--codex|--cursor|--all [--uninstall]");
    }
    for t in selected {
        let result = match (t, uninstall) {
            (Target::Agent(a), false) => agent(out, a),
            (Target::Agent(a), true) => unagent(out, a),
            (Target::Cursor, false) => cursor(out),
            (Target::Cursor, true) => uncursor(out),
        };
        match result {
            Err(e) if all && e.is::<Missing>() => {
                writeln!(out, "{}: not found, skipped", t.name())?
            }
            result => result?,
        }
    }
    Ok(())
}

fn cursor_dir() -> Result<PathBuf> {
    Ok(std::env::home_dir()
        .ok_or_else(|| anyhow!("cannot find a home directory"))?
        .join(".cursor"))
}

fn cursor(out: &mut impl Write) -> Result<()> {
    let dir = cursor_dir()?;
    let path = dir.join("mcp.json");
    let old = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => bail!("{}: {e}", path.display()),
    };
    let new = merge_cursor(&old).map_err(|e| anyhow!("{}: {e}", path.display()))?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&path, new)?;
    writeln!(out, "cursor: registered MCP server clidraw")?;
    writeln!(out, "cursor: wrote {}", path.display())?;
    writeln!(
        out,
        "cursor: no rules file; conventions come from the MCP instructions"
    )?;
    Ok(())
}

fn uncursor(out: &mut impl Write) -> Result<()> {
    let path = cursor_dir()?.join("mcp.json");
    let old = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => bail!("{}: {e}", path.display()),
    };
    let new = unmerge_cursor(&old).map_err(|e| anyhow!("{}: {e}", path.display()))?;
    std::fs::write(&path, new)?;
    writeln!(
        out,
        "cursor: removed MCP server clidraw from {}",
        path.display()
    )?;
    Ok(())
}

fn agent_dir(a: &Agent) -> Result<PathBuf> {
    match std::env::var(a.dir_env) {
        Ok(d) if !d.is_empty() => Ok(PathBuf::from(d)),
        _ => Ok(std::env::home_dir()
            .ok_or_else(|| anyhow!("cannot find a home directory"))?
            .join(a.home_subdir)),
    }
}

fn agent(out: &mut impl Write, a: &Agent) -> Result<()> {
    let dir = agent_dir(a)?;
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

fn unagent(out: &mut impl Write, a: &Agent) -> Result<()> {
    let path = agent_dir(a)?.join(a.rules_file);
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let new = strip(&old).map_err(|e| anyhow!("{}: {e}", path.display()))?;
    unregister(a)?;
    writeln!(out, "{}: removed MCP server clidraw", a.name)?;
    if new != old {
        std::fs::write(&path, new)?;
        writeln!(out, "{}: removed block from {}", a.name, path.display())?;
    }
    Ok(())
}

#[derive(Debug)]
struct Missing(String);

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Missing {}

fn spawn_failed(a: &Agent, e: std::io::Error) -> anyhow::Error {
    let msg = format!(
        "cannot run `{}`: {e}. Install it, then run the command again",
        a.bin
    );
    if e.kind() == std::io::ErrorKind::NotFound {
        anyhow::Error::new(Missing(msg))
    } else {
        anyhow!(msg)
    }
}

fn register(a: &Agent) -> Result<()> {
    // The entry may not exist; `add` fails on an existing name, so a re-run removes first.
    unregister(a)?;
    let output = Command::new(a.bin)
        .args(a.add)
        .output()
        .map_err(|e| spawn_failed(a, e))?;
    if !output.status.success() {
        bail!(
            "`{} {}` failed: {}",
            a.bin,
            a.add.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn unregister(a: &Agent) -> Result<()> {
    Command::new(a.bin)
        .args(a.remove)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| spawn_failed(a, e))?;
    Ok(())
}

fn block() -> String {
    format!("{BEGIN}\n## clidraw\n\n{CONVENTIONS}{END}\n")
}

fn span(text: &str) -> Result<Option<Range<usize>>> {
    match (text.find(BEGIN), text.find(END)) {
        (Some(b), Some(e)) if b < e => {
            let after = e + END.len();
            let line_end = text[after..]
                .find('\n')
                .map_or(text.len(), |n| after + n + 1);
            Ok(Some(b..line_end))
        }
        (None, None) => Ok(None),
        _ => bail!("expected one {BEGIN} followed by one {END}; fix or remove the markers"),
    }
}

fn upsert(text: &str, block: &str) -> Result<String> {
    match span(text)? {
        Some(r) => Ok(format!("{}{block}{}", &text[..r.start], &text[r.end..])),
        None => {
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
    }
}

fn strip(text: &str) -> Result<String> {
    Ok(match span(text)? {
        Some(r) => {
            // upsert adds one blank line before a new block; take it back so install then uninstall is exact.
            let head = &text[..r.start];
            let head = head
                .strip_suffix("\n\n")
                .map_or(head, |h| &text[..h.len() + 1]);
            format!("{head}{}", &text[r.end..])
        }
        None => text.to_string(),
    })
}

fn merge_cursor(text: &str) -> Result<String> {
    let mut root: serde_json::Value = if text.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(text)?
    };
    let servers = root
        .as_object_mut()
        .ok_or_else(|| anyhow!("expected a JSON object"))?
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));
    servers
        .as_object_mut()
        .ok_or_else(|| anyhow!("mcpServers is not a JSON object"))?
        .insert(
            "clidraw".to_string(),
            serde_json::json!({"command": "clidraw", "args": ["mcp"]}),
        );
    Ok(serde_json::to_string_pretty(&root)? + "\n")
}

fn unmerge_cursor(text: &str) -> Result<String> {
    if text.trim().is_empty() {
        return Ok(text.to_string());
    }
    let mut root: serde_json::Value = serde_json::from_str(text)?;
    let root_map = root
        .as_object_mut()
        .ok_or_else(|| anyhow!("expected a JSON object"))?;
    if let Some(servers) = root_map.get_mut("mcpServers") {
        servers
            .as_object_mut()
            .ok_or_else(|| anyhow!("mcpServers is not a JSON object"))?
            .remove("clidraw");
    }
    Ok(serde_json::to_string_pretty(&root)? + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIDRAW_ENTRY: &str = "{\"command\":\"clidraw\",\"args\":[\"mcp\"]}";

    fn keys(v: &serde_json::Value) -> Vec<&str> {
        v.as_object().unwrap().keys().map(String::as_str).collect()
    }

    #[test]
    fn merge_cursor_creates_the_server_in_empty_text() {
        let want = "{\n  \"mcpServers\": {\n    \"clidraw\": {\n      \"command\": \"clidraw\",\n      \"args\": [\n        \"mcp\"\n      ]\n    }\n  }\n}\n";
        assert_eq!(merge_cursor("").unwrap(), want);
    }

    #[test]
    fn merge_cursor_keeps_other_keys_and_servers_in_order() {
        let old = r#"{"theme":"dark","mcpServers":{"b":{"command":"b"},"a":{"command":"a"}}}"#;
        let v: serde_json::Value = serde_json::from_str(&merge_cursor(old).unwrap()).unwrap();
        assert_eq!(keys(&v), ["theme", "mcpServers"]);
        assert_eq!(keys(&v["mcpServers"]), ["b", "a", "clidraw"]);
        assert_eq!(v["mcpServers"]["a"], serde_json::json!({"command": "a"}));
    }

    #[test]
    fn merge_cursor_overwrites_clidraw_in_place() {
        let old = r#"{"mcpServers":{"clidraw":{"command":"old"},"a":{"command":"a"}}}"#;
        let v: serde_json::Value = serde_json::from_str(&merge_cursor(old).unwrap()).unwrap();
        assert_eq!(keys(&v["mcpServers"]), ["clidraw", "a"]);
        assert_eq!(
            serde_json::to_string(&v["mcpServers"]["clidraw"]).unwrap(),
            CLIDRAW_ENTRY
        );
    }

    #[test]
    fn merge_cursor_is_idempotent() {
        let once = merge_cursor("{\"mcpServers\":{}}").unwrap();
        assert!(once.contains("\"clidraw\""));
        assert_eq!(merge_cursor(&once).unwrap(), once);
    }

    #[test]
    fn merge_cursor_rejects_bad_shapes() {
        assert!(merge_cursor("{ nope").is_err());
        assert!(merge_cursor("[]").is_err());
        assert!(merge_cursor(r#"{"mcpServers":[]}"#).is_err());
    }

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
