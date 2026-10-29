use crate::cli;
use crate::store::Store;
use anyhow::Result;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

fn schema(props: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": required, "additionalProperties": false})
}

/// The `tools/list` result. The MCP reference page is generated from it.
pub fn tools() -> Value {
    let name = json!({"type": "string", "description": "Canvas name"});
    json!([
        {"name": "list", "description": "List canvas names as a JSON array.",
         "inputSchema": schema(json!({}), &[])},
        {"name": "new", "description": "Create an empty canvas.",
         "inputSchema": schema(json!({"name": name}), &["name"])},
        {"name": "show", "description": "Print a canvas as JSON.",
         "inputSchema": schema(json!({"name": name}), &["name"])},
        {"name": "edit", "description": format!(
             "Edit an existing canvas. Applies all ops or none and returns {{\"applied\":N,\"ids\":[...]}}. \
              Each item of ops is an object whose op field names the op. Ops: {}",
             cli::docs::op_list()),
         "inputSchema": schema(json!({"name": name, "ops": {
             "type": "array", "items": cli::docs::op_items_schema()}}), &["name", "ops"])},
        {"name": "import",
         "description": "Add a Mermaid diagram below the existing content. The importer reads flowchart, sequence, class and state diagram syntax. Creates the canvas if missing.",
         "inputSchema": schema(json!({"name": name,
             "mermaid": {"type": "string", "description": "Mermaid diagram text"}}), &["name", "mermaid"])},
        {"name": "export", "description": "Render a saved canvas and return the text.",
         "inputSchema": schema(json!({"name": name,
             "format": {"type": "string", "enum": ["svg", "mermaid", "clidraw"]}}), &["name", "format"])},
        {"name": "rename", "description": "Rename a canvas.",
         "inputSchema": schema(json!({"name": name,
             "to": {"type": "string", "description": "New canvas name"}}), &["name", "to"])},
        {"name": "undo", "description": "Revert the last edit or import on a canvas. Returns {\"undone\":label}.",
         "inputSchema": schema(json!({"name": name}), &["name"])},
        {"name": "redo", "description": "Re-apply the last undone step. Returns {\"redone\":label}.",
         "inputSchema": schema(json!({"name": name}), &["name"])},
        {"name": "delete", "description": "Delete a canvas and its undo history.",
         "inputSchema": schema(json!({"name": name}), &["name"])},
        {"name": "templates", "description": "List the built-in template names as a JSON array.",
         "inputSchema": schema(json!({}), &[])},
        {"name": "template",
         "description": "Add a built-in template inside a frame, below the existing content. Creates the canvas if missing.",
         "inputSchema": schema(json!({"name": name,
             "template": {"type": "string", "description": "Template name from the templates tool"}}), &["name", "template"])},
    ])
}

fn string<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing string argument `{key}`"))
}

/// Maps a tool call to CLI arguments and the stdin text for `cli::run`.
fn command(tool: &str, a: &Value) -> Result<(Vec<String>, String)> {
    let mut argv = vec![tool.to_string()];
    let mut stdin = String::new();
    if !matches!(tool, "list" | "templates") {
        argv.push(string(a, "name")?.to_string());
    }
    match tool {
        "edit" => {
            let ops = a
                .get("ops")
                .filter(|o| o.is_array())
                .ok_or_else(|| anyhow::anyhow!("missing array argument `ops`"))?;
            stdin = ops.to_string();
        }
        "import" => {
            argv.push("-".into());
            stdin = string(a, "mermaid")?.to_string();
        }
        "template" => argv.push(string(a, "template")?.to_string()),
        "rename" => argv.push(string(a, "to")?.to_string()),
        "export" => {
            let format = string(a, "format")?;
            argv.extend([format!("--{format}"), "-".into()]);
        }
        _ => {}
    }
    Ok((argv, stdin))
}

fn call(params: &Value, store: &Store) -> Result<Value, String> {
    let tool = params.get("name").and_then(Value::as_str).unwrap_or("");
    if !cli::COMMANDS.contains(&tool) {
        return Err(format!("unknown tool `{tool}`"));
    }
    let empty = json!({});
    let args = params.get("arguments").unwrap_or(&empty);
    let mut out = Vec::new();
    let result = command(tool, args)
        .and_then(|(argv, stdin)| cli::run(&argv, store, &mut stdin.as_bytes(), &mut out));
    let (text, is_error) = match result {
        Ok(()) => (String::from_utf8_lossy(&out).trim_end().to_string(), false),
        Err(e) => (format!("Error: {e}"), true),
    };
    Ok(json!({"content": [{"type": "text", "text": text}], "isError": is_error}))
}

fn reply(id: &Value, outcome: Result<Value, (i32, String)>) -> Value {
    match outcome {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err((code, message)) => {
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
        }
    }
}

fn handle(line: &str, store: &Store) -> Option<Value> {
    let Ok(msg) = serde_json::from_str::<Value>(line) else {
        return Some(reply(&Value::Null, Err((-32700, "parse error".into()))));
    };
    let Some(obj) = msg.as_object() else {
        return Some(reply(&Value::Null, Err((-32600, "invalid request".into()))));
    };
    let id = obj.get("id")?;
    let params = obj.get("params").unwrap_or(&Value::Null);
    let outcome = match obj.get("method").and_then(Value::as_str) {
        Some("initialize") => {
            let asked = params.get("protocolVersion").and_then(Value::as_str);
            Ok(json!({
                "protocolVersion": asked.filter(|v| VERSIONS.contains(v)).unwrap_or(VERSIONS[0]),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "clidraw", "version": env!("CARGO_PKG_VERSION")},
            }))
        }
        Some("ping") => Ok(json!({})),
        Some("tools/list") => Ok(json!({"tools": tools()})),
        Some("tools/call") => call(params, store).map_err(|m| (-32602, m)),
        _ => Err((-32601, "method not found".into())),
    };
    Some(reply(id, outcome))
}

/// Serves MCP over newline-delimited JSON-RPC until `input` ends.
pub fn serve(store: &Store, input: impl BufRead, out: &mut impl Write) -> Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(r) = handle(&line, store) {
            writeln!(out, "{r}")?;
            out.flush()?;
        }
    }
    Ok(())
}
