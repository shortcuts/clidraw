use clidraw::mcp::serve;
use clidraw::store::Store;
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

fn rpc(store: &Store, lines: &[Value]) -> Vec<Value> {
    let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
    rpc_raw(store, &input)
}

fn rpc_raw(store: &Store, input: &str) -> Vec<Value> {
    let mut out = Vec::new();
    serve(store, input.as_bytes(), &mut out).unwrap();
    String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn store() -> (tempfile::TempDir, Store) {
    let t = tempfile::tempdir().unwrap();
    let s = Store::at(t.path().join("c"), t.path().join("state"));
    (t, s)
}

fn req(id: u32, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

fn tool(id: u32, name: &str, arguments: Value) -> Value {
    req(
        id,
        "tools/call",
        json!({"name": name, "arguments": arguments}),
    )
}

fn text(reply: &Value) -> &str {
    reply["result"]["content"][0]["text"].as_str().unwrap()
}

#[test]
fn initialize_echoes_a_known_version_and_falls_back() {
    let (_t, s) = store();
    let r = rpc(
        &s,
        &[
            req(1, "initialize", json!({"protocolVersion": "2025-06-18"})),
            req(2, "initialize", json!({"protocolVersion": "1999-01-01"})),
        ],
    );
    assert_eq!(r[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(r[1]["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(r[0]["result"]["capabilities"], json!({"tools": {}}));
    assert_eq!(r[0]["result"]["serverInfo"]["name"], "clidraw");
}

#[test]
fn notifications_get_no_reply_and_unknown_methods_error() {
    let (_t, s) = store();
    let note = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    let r = rpc(
        &s,
        &[
            note,
            req(1, "foo/bar", json!({})),
            req(2, "ping", json!({})),
        ],
    );
    assert_eq!(r.len(), 2);
    assert_eq!(r[0]["error"]["code"], -32601);
    assert_eq!(r[1]["result"], json!({}));
}

#[test]
fn bad_input_lines_get_protocol_errors() {
    let (_t, s) = store();
    let ping = req(1, "ping", json!({})).to_string();
    let r = rpc_raw(&s, &format!("not json\n[]\n{ping}\n"));
    assert_eq!(r[0]["error"]["code"], -32700);
    assert_eq!(r[0]["id"], Value::Null);
    assert_eq!(r[1]["error"]["code"], -32600);
    assert_eq!(r[2]["result"], json!({}));
}

#[test]
fn tools_list_names_the_six_cli_commands() {
    let (_t, s) = store();
    let r = rpc(&s, &[req(1, "tools/list", json!({}))]);
    let tools = r[0]["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["list", "new", "show", "edit", "import", "export"]);
    assert!(tools.iter().all(|t| t["inputSchema"]["type"] == "object"));
}

#[test]
fn new_edit_show_export_round_trip() {
    let (_t, s) = store();
    let ops = json!([
        {"op": "add_shape", "id": "a", "x": 0, "y": 0, "label": "Alpha"},
        {"op": "add_shape", "id": "b", "x": 20, "y": 0, "label": "Beta"},
        {"op": "connect", "from": "a", "to": "b"},
    ]);
    let r = rpc(
        &s,
        &[
            tool(1, "new", json!({"name": "demo"})),
            tool(2, "edit", json!({"name": "demo", "ops": ops})),
            tool(3, "show", json!({"name": "demo"})),
            tool(4, "export", json!({"name": "demo", "format": "text"})),
            tool(5, "list", json!({})),
        ],
    );
    assert!(r.iter().all(|x| x["result"]["isError"] == false));
    let canvas: Value = serde_json::from_str(text(&r[2])).unwrap();
    assert_eq!(canvas["elements"].as_array().unwrap().len(), 3);
    assert!(text(&r[3]).contains("Alpha") && text(&r[3]).contains("Beta"));
    assert_eq!(text(&r[4]), r#"["demo"]"#);
}

#[test]
fn import_takes_mermaid_text() {
    let (_t, s) = store();
    let r = rpc(
        &s,
        &[tool(
            1,
            "import",
            json!({"name": "m", "mermaid": "graph TD\n  A-->B"}),
        )],
    );
    let got: Value = serde_json::from_str(text(&r[0])).unwrap();
    assert_eq!((&got["shapes"], &got["lines"]), (&json!(2), &json!(1)));
}

#[test]
fn tool_failures_are_results_not_protocol_errors() {
    let (_t, s) = store();
    let r = rpc(
        &s,
        &[
            tool(1, "show", json!({"name": "missing"})),
            tool(2, "new", json!({"name": "x"})),
            tool(3, "edit", json!({"name": "x", "ops": [{"op": "bogus"}]})),
            tool(4, "new", json!({})),
            tool(5, "nope", json!({})),
        ],
    );
    for i in [0, 2, 3] {
        assert_eq!(r[i]["result"]["isError"], true, "{i}");
        assert!(text(&r[i]).starts_with("Error:"), "{i}");
    }
    assert_eq!(r[1]["result"]["isError"], false);
    assert_eq!(r[4]["error"]["code"], -32602);
}

#[test]
fn binary_serves_over_stdio() {
    let t = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_clidraw"))
        .arg("mcp")
        .env("CLIDRAW_HOME", t.path().join("c"))
        .env("XDG_STATE_HOME", t.path().join("state"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, "{}", req(1, "initialize", json!({}))).unwrap();
    writeln!(stdin, "{}", req(2, "tools/list", json!({}))).unwrap();
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8(out.stdout).unwrap().lines().count(), 2);
}
