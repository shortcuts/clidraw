use clidraw::model::{Canvas, Element, Id, Shape, ShapeKind};
use clidraw::store::Store;
use std::io::Write;
use std::process::{Command, Stdio};

fn run(home: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_clidraw"))
        .args(args)
        .env("CLIDRAW_HOME", home.join("c"))
        .env("XDG_STATE_HOME", home.join("state"))
        .output()
        .unwrap()
}

fn seeded() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    let mut c = Canvas::default();
    c.elements.push(Element::Shape(Shape {
        id: Id::from("aaaaaaaa"),
        kind: ShapeKind::Rect,
        x: 0,
        y: 0,
        w: 6,
        h: 3,
        label: "Hi".into(),
        stroke: Default::default(),
        border: Default::default(),
        color: None,
        fill: None,
        align: Default::default(),
        bold: false,
        link: None,
    }));
    Store::at(t.path().join("c"), t.path().join("state"))
        .save("demo", &c)
        .unwrap();
    t
}

#[test]
fn export_writes_svg_and_mermaid_without_the_ui() {
    let t = seeded();
    let svg = t.path().join("o.svg");
    let mmd = t.path().join("o.mmd");
    for (flag, out) in [("--svg", &svg), ("--mermaid", &mmd)] {
        let r = run(t.path(), &["export", "demo", flag, out.to_str().unwrap()]);
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    }
    assert!(std::fs::read_to_string(svg).unwrap().starts_with("<svg"));
    assert!(std::fs::read_to_string(mmd).unwrap().contains("Hi"));
}

#[test]
fn export_fails_on_a_missing_canvas_or_bad_flag() {
    let t = seeded();
    let out = t.path().join("o.svg");
    let out = out.to_str().unwrap();
    assert!(!run(t.path(), &["export", "nope", "--svg", out])
        .status
        .success());
    assert!(!run(t.path(), &["export", "demo", "--png", out])
        .status
        .success());
    assert!(!run(t.path(), &["export", "demo", "--svg"]).status.success());
    assert!(!std::path::Path::new(out).exists());
}

fn run_stdin(home: &std::path::Path, args: &[&str], stdin: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_clidraw"))
        .args(args)
        .env("CLIDRAW_HOME", home.join("c"))
        .env("XDG_STATE_HOME", home.join("state"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn out(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn show(home: &std::path::Path) -> serde_json::Value {
    let r = run(home, &["show", "demo"]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    serde_json::from_slice(&r.stdout).unwrap()
}

const TWO_AND_AN_ARROW: &str = r#"[
  {"op":"add_shape","id":"a","x":0,"y":0,"label":"Hi"},
  {"op":"add_shape","id":"b","x":20,"y":0,"label":"Yo","color":"red"},
  {"op":"connect","from":"a","to":"b"}
]"#;

#[test]
fn new_then_list_shows_the_canvas() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(out(&run(t.path(), &["list"])).trim(), "[]");
    assert!(run(t.path(), &["new", "demo"]).status.success());
    assert_eq!(out(&run(t.path(), &["list"])).trim(), r#"["demo"]"#);
    assert!(!run(t.path(), &["new", "demo"]).status.success());
}

#[test]
fn edit_adds_connects_and_returns_ids() {
    let t = seeded_empty();
    let r = run_stdin(t.path(), &["edit", "demo"], TWO_AND_AN_ARROW);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let v: serde_json::Value = serde_json::from_str(&out(&r)).unwrap();
    assert_eq!(v["applied"], 3);
    assert_eq!(v["ids"][0], "a");
    assert_eq!(v["ids"][1], "b");
    let c = show(t.path());
    let els = c["elements"].as_array().unwrap();
    assert_eq!(els.len(), 3);
    assert_eq!(els[2]["type"], "line");
    assert_eq!(els[2]["from"], serde_json::json!({"bound": "a"}));
    assert_eq!(els[2]["to"], serde_json::json!({"bound": "b"}));
}

fn seeded_empty() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    assert!(run(t.path(), &["new", "demo"]).status.success());
    t
}

#[test]
fn edit_is_atomic_and_needs_an_existing_canvas() {
    let t = tempfile::tempdir().unwrap();
    let r = run_stdin(t.path(), &["edit", "demo"], TWO_AND_AN_ARROW);
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).starts_with("Error:"));

    let t = seeded_empty();
    let before = show(t.path());
    let bad = r#"[{"op":"add_shape","id":"a","x":0,"y":0},{"op":"delete","id":"zzz"}]"#;
    assert!(!run_stdin(t.path(), &["edit", "demo"], bad).status.success());
    assert_eq!(show(t.path()), before);
}

#[test]
fn edit_set_move_delete() {
    let t = seeded_empty();
    run_stdin(t.path(), &["edit", "demo"], TWO_AND_AN_ARROW);
    let ops = r#"[
      {"op":"set","id":"a","label":"New","color":"green"},
      {"op":"set","id":"b","color":"none"},
      {"op":"move","id":"a","dx":3,"dy":2},
      {"op":"delete","id":"b"}
    ]"#;
    let r = run_stdin(t.path(), &["edit", "demo"], ops);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let c = show(t.path());
    let a = &c["elements"][0];
    assert_eq!(
        (a["label"].as_str(), a["color"].as_str()),
        (Some("New"), Some("green"))
    );
    assert_eq!((a["x"].as_i64(), a["y"].as_i64()), (Some(3), Some(2)));
    assert!(c["elements"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["id"] != "b"));
}

#[test]
fn add_shape_sizes_itself_from_the_label() {
    let t = seeded_empty();
    run_stdin(
        t.path(),
        &["edit", "demo"],
        r#"[{"op":"add_shape","x":1,"y":1,"label":"Hi"},{"op":"add_shape","x":1,"y":9,"w":9,"h":4,"kind":"Diamond"}]"#,
    );
    let c = show(t.path());
    assert_eq!(
        (
            c["elements"][0]["w"].as_i64(),
            c["elements"][0]["h"].as_i64()
        ),
        (Some(6), Some(3))
    );
    assert_eq!(
        (
            c["elements"][1]["w"].as_i64(),
            c["elements"][1]["h"].as_i64()
        ),
        (Some(9), Some(4))
    );
}

#[test]
fn import_appends_below_existing() {
    let t = seeded_empty();
    run_stdin(
        t.path(),
        &["edit", "demo"],
        r#"[{"op":"add_shape","x":0,"y":0,"w":6,"h":3}]"#,
    );
    let mmd = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/sample.mmd");
    let r = run(t.path(), &["import", "demo", mmd]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let v: serde_json::Value = serde_json::from_str(&out(&r)).unwrap();
    assert!(v["shapes"].as_u64().unwrap() > 0);
    let c = show(t.path());
    let shapes: Vec<_> = c["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "shape")
        .collect();
    assert_eq!(shapes.len() as u64, v["shapes"].as_u64().unwrap() + 1);
    assert!(shapes[1..].iter().all(|s| s["y"].as_i64().unwrap() > 3));
}

#[test]
fn failed_import_lists_diagnostics() {
    let t = seeded_empty();
    let r = run_stdin(
        t.path(),
        &["import", "demo", "-"],
        "sequenceDiagram\nA->>B: hi",
    );
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr)
        .contains("line 1: `sequenceDiagram` diagrams are not supported"));
}

#[test]
fn export_clidraw_to_stdout_is_the_saved_canvas() {
    let t = seeded();
    let r = run(t.path(), &["export", "demo", "--clidraw", "-"]);
    assert!(r.status.success());
    let exported: Canvas = serde_json::from_str(&out(&r)).unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    assert_eq!(exported, store.load("demo").unwrap());
}
