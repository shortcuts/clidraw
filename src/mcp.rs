use crate::cli;
use crate::store::Store;
use anyhow::Result;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const OPS: &str = "Applies all ops or none and returns {\"applied\":N,\"ids\":[...]}. Ops: \
add_shape (x, y, optional kind, w, h, label, color, fill, id), \
add_text (x, y, text, optional color, id), \
connect (from, to, optional kind, label, id), \
set (id, optional label, color, fill), move (id, dx, dy), delete (id). \
Shape kind: Rect, Diamond, Cylinder, Parallelogram, Hexagon, Frame. \
Line kind: Line, Arrow, ArrowStart, ArrowBoth. \
Colors: red, orange, yellow, green, blue, violet, or none in set. \
A connect end is a shape id or an [x, y] cell.";

fn schema(props: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": required, "additionalProperties": false})
}

fn tools() -> Value {
    let name = json!({"type": "string", "description": "Canvas name"});
    json!([
        {"name": "list", "description": "List canvas names as a JSON array.",
         "inputSchema": schema(json!({}), &[])},
        {"name": "new", "description": "Create an empty canvas.",
         "inputSchema": schema(json!({"name": name}), &["name"])},
        {"name": "show", "description": "Print a canvas as JSON.",
         "inputSchema": schema(json!({"name": name}), &["name"])},
        {"name": "edit", "description": format!("Edit an existing canvas. {OPS}"),
         "inputSchema": schema(json!({"name": name, "ops": {
             "type": "array", "items": {"type": "object", "required": ["op"]}}}), &["name", "ops"])},
        {"name": "import",
         "description": "Add a Mermaid flowchart below the existing content. Creates the canvas if missing.",
         "inputSchema": schema(json!({"name": name,
             "mermaid": {"type": "string", "description": "Mermaid flowchart text"}}), &["name", "mermaid"])},
        {"name": "export", "description": "Render a saved canvas and return the text.",
         "inputSchema": schema(json!({"name": name,
             "format": {"type": "string", "enum": ["svg", "mermaid", "text"]}}), &["name", "format"])},
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
    if tool != "list" {
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
