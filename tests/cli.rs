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
        side_labels: vec![],
        meta: Default::default(),
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
fn unknown_command_usage_points_to_the_cli_page() {
    let t = seeded();
    let r = run(t.path(), &["list", "extra"]);
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr)
        .contains("usage: clidraw list (see https://clidraw.shrtcts.fr/cli.html)"));
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
fn import_reads_a_sequence_diagram_from_stdin() {
    let t = seeded_empty();
    let r = run_stdin(
        t.path(),
        &["import", "demo", "-"],
        "sequenceDiagram\nA->>B: hi",
    );
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let v: serde_json::Value = serde_json::from_str(&out(&r)).unwrap();
    assert_eq!(
        v,
        serde_json::json!({"shapes": 2, "lines": 3, "skipped": 0})
    );
}

#[test]
fn failed_import_lists_diagnostics() {
    let t = seeded_empty();
    let r = run_stdin(t.path(), &["import", "demo", "-"], "gantt\ntitle t");
    assert!(!r.status.success());
    assert!(
        String::from_utf8_lossy(&r.stderr).contains("line 1: `gantt` diagrams are not supported")
    );
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

#[test]
fn edit_rejects_a_shape_below_min_size() {
    let t = seeded_empty();
    let before = show(t.path());
    let bad = r#"[{"op":"add_shape","id":"a","x":0,"y":0,"w":-5,"h":4,"fill":"red"}]"#;
    let r = run_stdin(t.path(), &["edit", "demo"], bad);
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).contains("element a is smaller than 3x3"));
    assert_eq!(show(t.path()), before);
}

#[test]
fn edit_rejects_empty_ids_and_multiline_line_and_text_labels() {
    let t = seeded_empty();
    let before = show(t.path());
    for (bad, want) in [
        (
            r#"[{"op":"add_shape","id":"","x":0,"y":0}]"#,
            "add_shape: empty id",
        ),
        (
            r#"[{"op":"add_text","id":"","x":0,"y":0,"text":"a"}]"#,
            "add_text: empty id",
        ),
        (
            r#"[{"op":"connect","id":"","from":[0,0],"to":[5,5]}]"#,
            "connect: empty id",
        ),
        (r#"[{"op":"move","id":"","dx":1,"dy":0}]"#, "move: empty id"),
        (r#"[{"op":"set","id":"","color":"red"}]"#, "set: empty id"),
        (r#"[{"op":"delete","id":""}]"#, "delete: empty id"),
        (
            r#"[{"op":"add_text","x":0,"y":0,"text":"a\nb"}]"#,
            "add_text: text must be one line",
        ),
        (
            r#"[{"op":"connect","from":[0,0],"to":[5,5],"label":"a\nb"}]"#,
            "connect: label must be one line",
        ),
    ] {
        let r = run_stdin(t.path(), &["edit", "demo"], bad);
        assert!(!r.status.success(), "{bad}");
        let err = String::from_utf8_lossy(&r.stderr);
        assert!(err.contains(want), "{bad}: {err}");
        assert_eq!(show(t.path()), before);
    }
}

#[test]
fn edit_counts_ops_and_rejects_a_set_that_changes_nothing() {
    let t = seeded_empty();
    let seed = r#"[{"op":"add_shape","id":"a","x":0,"y":0}]"#;
    assert!(run_stdin(t.path(), &["edit", "demo"], seed)
        .status
        .success());
    let ok = r#"[{"op":"set","id":"a","label":"x","color":"red","fill":"blue"},{"op":"move","id":"a","dx":1,"dy":0}]"#;
    let r = run_stdin(t.path(), &["edit", "demo"], ok);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let v: serde_json::Value = serde_json::from_str(&out(&r)).unwrap();
    assert_eq!(v["applied"], 2);
    let before = show(t.path());
    let r = run_stdin(t.path(), &["edit", "demo"], r#"[{"op":"set","id":"a"}]"#);
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).contains("set: nothing to change"));
    assert_eq!(show(t.path()), before);
}

#[test]
fn edit_move_repins_the_bends_of_lines_bound_to_the_shape() {
    let t = seeded_empty();
    assert!(run_stdin(t.path(), &["edit", "demo"], TWO_AND_AN_ARROW)
        .status
        .success());
    let before = show(t.path())["elements"][2]["bends"].clone();
    let r = run_stdin(
        t.path(),
        &["edit", "demo"],
        r#"[{"op":"move","id":"b","dx":0,"dy":10}]"#,
    );
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let after = show(t.path())["elements"][2]["bends"].clone();
    assert_ne!(after, before);
    assert_eq!(
        after,
        serde_json::json!([[12, 1], [12, 11]]),
        "before: {before}"
    );
}

fn edit_ok(t: &tempfile::TempDir, ops: &str) -> serde_json::Value {
    let r = run_stdin(t.path(), &["edit", "demo"], ops);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    show(t.path())
}

const AB: &str = r#"{"op":"add_shape","id":"a","x":0,"y":0,"w":8,"h":4},{"op":"add_shape","id":"b","x":30,"y":9,"w":8,"h":4},{"op":"connect","id":"l","from":"a","to":"b"}"#;

#[test]
fn edit_resizes_and_restyles() {
    let t = seeded_empty();
    let c = edit_ok(
        &t,
        &format!(
            r#"[{AB},{{"op":"resize","id":"a","w":12,"h":5}},
            {{"op":"set","id":"b","kind":"Diamond","bold":true,"stroke":"dashed","border":"double","align":"right","link":"http://x"}},
            {{"op":"set","id":"l","head":"ArrowBoth"}}]"#
        ),
    );
    let els = c["elements"].as_array().unwrap();
    assert_eq!(
        (els[0]["w"].clone(), els[0]["h"].clone()),
        (12.into(), 5.into())
    );
    assert_eq!(els[1]["kind"], "Diamond");
    assert_eq!(els[1]["bold"], true);
    assert_eq!(els[1]["stroke"], "dashed");
    assert_eq!(els[1]["border"], "double");
    assert_eq!(els[1]["align"], "right");
    assert_eq!(els[1]["link"], "http://x");
    assert_eq!(els[2]["kind"], "ArrowBoth");
}

#[test]
fn edit_groups_orders_and_lays_out() {
    let t = seeded_empty();
    let c = edit_ok(
        &t,
        &format!(
            r#"[{AB},{{"op":"group","ids":["a","b"]}},{{"op":"order","ids":["a"],"z":"front"}},
            {{"op":"layout","dir":"lr"}}]"#
        ),
    );
    assert_eq!(c["groups"], serde_json::json!([["a", "b"]]));
    let els = c["elements"].as_array().unwrap();
    assert_eq!(els.last().unwrap()["id"], "a");
    assert_eq!(
        els.iter().find(|e| e["id"] == "a").unwrap()["y"],
        els.iter().find(|e| e["id"] == "b").unwrap()["y"]
    );
    let c = edit_ok(&t, r#"[{"op":"ungroup","ids":["a"]}]"#);
    assert!(c["groups"].is_null());
}

#[test]
fn new_ops_reject_bad_input_and_leave_the_file_alone() {
    let t = seeded_empty();
    edit_ok(&t, &format!("[{AB}]"));
    let before = show(t.path());
    for bad in [
        r#"{"op":"resize","id":"a","w":0,"h":4}"#,
        r#"{"op":"resize","id":"nope","w":9,"h":4}"#,
        r#"{"op":"resize","id":"l","w":9,"h":4}"#,
        r#"{"op":"set","id":"a","head":"Line"}"#,
        r#"{"op":"set","id":"l","kind":"Diamond"}"#,
        r#"{"op":"set","id":"a","link":"a\nb"}"#,
        r#"{"op":"set","id":"a"}"#,
        r#"{"op":"group","ids":["a"]}"#,
        r#"{"op":"group","ids":["a","nope"]}"#,
        r#"{"op":"order","ids":[""],"z":"back"}"#,
        r#"{"op":"layout","ids":["nope"]}"#,
    ] {
        let ops = format!(r#"[{{"op":"move","id":"a","dx":1,"dy":0}},{bad}]"#);
        assert!(
            !run_stdin(t.path(), &["edit", "demo"], &ops)
                .status
                .success(),
            "{bad}"
        );
        assert_eq!(show(t.path()), before, "{bad}");
    }
}

#[test]
fn rename_and_delete_manage_canvases() {
    let t = seeded_empty();
    let r = run(t.path(), &["rename", "demo", "other"]);
    assert_eq!(out(&r).trim(), r#"{"renamed":"other"}"#);
    assert!(!run(t.path(), &["show", "demo"]).status.success());
    let r = run(t.path(), &["delete", "other"]);
    assert_eq!(out(&r).trim(), r#"{"deleted":"other"}"#);
    assert!(!run(t.path(), &["delete", "other"]).status.success());
}

#[test]
fn edit_meta_adds_merges_clears_and_rejects_bad_pairs() {
    let t = seeded_empty();
    let c = edit_ok(
        &t,
        &format!(
            r#"[{AB},{{"op":"add_text","id":"t","x":0,"y":20,"text":"hi","meta":{{"dir":"src"}}}},
            {{"op":"set","id":"a","meta":{{"file":"src/a.rs","empty":""}}}},
            {{"op":"set","id":"l","meta":{{"service":"api"}}}}]"#
        ),
    );
    let get = |c: &serde_json::Value, id: &str| {
        c["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(
        get(&c, "a")["meta"],
        serde_json::json!({"file": "src/a.rs"})
    );
    assert_eq!(get(&c, "l")["meta"], serde_json::json!({"service": "api"}));
    assert_eq!(get(&c, "t")["meta"], serde_json::json!({"dir": "src"}));
    assert!(get(&c, "b").get("meta").is_none());
    let c = edit_ok(
        &t,
        r#"[{"op":"set","id":"a","meta":{"url":"https://x.test","file":"b.rs"}},
        {"op":"add_shape","id":"n","x":0,"y":30,"meta":{"k":"v"}},
        {"op":"connect","id":"m","from":"a","to":"n","meta":{"k":"v"}}]"#,
    );
    assert_eq!(
        get(&c, "a")["meta"],
        serde_json::json!({"file": "b.rs", "url": "https://x.test"})
    );
    assert_eq!(get(&c, "n")["meta"], serde_json::json!({"k": "v"}));
    assert_eq!(get(&c, "m")["meta"], serde_json::json!({"k": "v"}));
    let c = edit_ok(&t, r#"[{"op":"set","id":"a","meta":{"file":""}}]"#);
    assert_eq!(
        get(&c, "a")["meta"],
        serde_json::json!({"url": "https://x.test"})
    );

    let before = show(t.path());
    for (bad, want) in [
        (
            r#"[{"op":"set","id":"a","meta":{"file":"a\nb"}}]"#,
            "set: meta value must be one line",
        ),
        (
            r#"[{"op":"set","id":"a","meta":{"":"x"}}]"#,
            "set: meta key is empty",
        ),
        (
            r#"[{"op":"add_shape","x":0,"y":0,"meta":{"a\nb":"x"}}]"#,
            "add_shape: meta key must be one line",
        ),
        (
            r#"[{"op":"set","id":"a","meta":{}}]"#,
            "set: nothing to change",
        ),
        (r#"[{"op":"set","id":"zz","meta":{"a":"b"}}]"#, "zz"),
    ] {
        let r = run_stdin(t.path(), &["edit", "demo"], bad);
        assert!(!r.status.success(), "{bad}");
        let err = String::from_utf8_lossy(&r.stderr);
        assert!(err.contains(want), "{bad}: {err}");
        assert_eq!(show(t.path()), before);
    }
}

#[test]
fn template_creates_a_framed_canvas() {
    let t = tempfile::tempdir().unwrap();
    let r = run(t.path(), &["template", "demo", "Sequence diagram"]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let c = show(t.path());
    assert!(c["elements"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["kind"] == "Frame" && e["label"] == "Sequence diagram"));
}

const ONE_SHAPE: &str = r#"[{"op":"add_shape","x":0,"y":0,"w":6,"h":3}]"#;

#[test]
fn undo_reverts_an_edit_and_redo_survives_the_process() {
    let t = seeded_empty();
    let before = show(t.path());
    run_stdin(t.path(), &["edit", "demo"], ONE_SHAPE);
    let after = show(t.path());
    let r = run(t.path(), &["undo", "demo"]);
    assert!(out(&r).starts_with(r#"{"undone":""#), "{}", out(&r));
    assert_eq!(show(t.path()), before);
    let r = run(t.path(), &["redo", "demo"]);
    assert!(out(&r).starts_with(r#"{"redone":""#), "{}", out(&r));
    assert_eq!(show(t.path()), after);
}

#[test]
fn undo_reverts_an_import_with_its_groups() {
    let t = seeded_empty();
    run_stdin(t.path(), &["edit", "demo"], ONE_SHAPE);
    let before = show(t.path());
    let mmd = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/sample.mmd");
    run(t.path(), &["import", "demo", mmd]);
    assert_ne!(show(t.path()), before);
    assert!(run(t.path(), &["undo", "demo"]).status.success());
    assert_eq!(show(t.path()), before);
}

#[test]
fn a_new_edit_clears_redo_and_empty_stacks_fail() {
    let t = seeded_empty();
    let r = run(t.path(), &["undo", "demo"]);
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).contains("nothing to undo"));
    run_stdin(t.path(), &["edit", "demo"], ONE_SHAPE);
    run(t.path(), &["undo", "demo"]);
    run_stdin(t.path(), &["edit", "demo"], ONE_SHAPE);
    let r = run(t.path(), &["redo", "demo"]);
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).contains("nothing to redo"));
}

#[test]
fn undo_after_an_outside_write_reports_nothing_to_undo() {
    let t = seeded_empty();
    run_stdin(t.path(), &["edit", "demo"], ONE_SHAPE);
    let path = t.path().join("c/demo.clidraw.json");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push('\n');
    std::fs::write(&path, text).unwrap();
    let r = run(t.path(), &["undo", "demo"]);
    assert!(String::from_utf8_lossy(&r.stderr).contains("nothing to undo"));
}
