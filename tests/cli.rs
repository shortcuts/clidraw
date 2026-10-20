use clidraw::model::{Canvas, Element, Id, Shape, ShapeKind};
use clidraw::store::Store;
use std::process::Command;

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
