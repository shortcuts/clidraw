use clidraw::model::{
    Anchor, Canvas, Command, Element, End, Id, Line, LineKind, Paint, Shape, ShapeKind,
};
use clidraw::store::{resolve_dir, validate_name, State, Store, StoreError};
use std::path::PathBuf;

fn store() -> (tempfile::TempDir, Store) {
    let t = tempfile::tempdir().unwrap();
    let s = Store::at(t.path().join("c"), t.path().join("state"));
    (t, s)
}

fn shape(id: &str, x: i32) -> Element {
    Element::Shape(Shape {
        id: Id::from(id),
        kind: ShapeKind::Rect,
        x,
        y: 0,
        w: 4,
        h: 3,
        label: "a".into(),
        stroke: Default::default(),
        border: Default::default(),
        color: None,
        fill: None,
        align: Default::default(),
        bold: false,
        link: None,
    })
}

fn sample() -> Canvas {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 0));
    c.elements.push(shape("bbbbbbbb", 10));
    c.elements.push(Element::Line(Line {
        id: Id::from("cccccccc"),
        kind: LineKind::Arrow,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Free(-3, 4),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
    }));
    c
}

#[test]
fn round_trip() {
    let (_t, s) = store();
    assert!(s.save("a", &sample()).unwrap());
    assert_eq!(s.load("a").unwrap(), sample());
    assert_eq!(s.list().unwrap(), vec!["a".to_string()]);
}

#[test]
fn frame_round_trips() {
    let (_t, s) = store();
    let mut c = sample();
    let Element::Shape(sh) = &mut c.elements[0] else {
        panic!("no shape")
    };
    sh.kind = ShapeKind::Frame;
    s.save("a", &c).unwrap();
    assert_eq!(s.load("a").unwrap(), c);
}

#[test]
fn colors_round_trip_and_uncolored_files_keep_their_bytes() {
    let (t, s) = store();
    let path = t.path().join("c/a.clidraw.json");
    s.save("a", &sample()).unwrap();
    let plain = std::fs::read_to_string(&path).unwrap();
    assert!(!plain.contains("color") && !plain.contains("fill"));
    let mut c = sample();
    let Element::Shape(sh) = &mut c.elements[0] else {
        panic!("no shape")
    };
    sh.color = Some(Paint::Red);
    sh.fill = Some(Paint::Blue);
    s.save("a", &c).unwrap();
    let colored = std::fs::read_to_string(&path).unwrap();
    assert!(colored.contains("\"color\": \"red\"") && colored.contains("\"fill\": \"blue\""));
    assert_eq!(s.load("a").unwrap(), c);
}

#[test]
fn unchanged_save_keeps_bytes_and_skips_write() {
    let (t, s) = store();
    s.save("a", &sample()).unwrap();
    let path = t.path().join("c/a.clidraw.json");
    let before = std::fs::read(&path).unwrap();
    assert!(!s.save("a", &sample()).unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(before.ends_with(b"}\n"));
}

#[test]
fn moving_one_shape_changes_only_its_lines() {
    let (t, s) = store();
    s.save("a", &sample()).unwrap();
    let path = t.path().join("c/a.clidraw.json");
    let before = std::fs::read_to_string(&path).unwrap();
    let mut c = sample();
    c.elements[1] = shape("bbbbbbbb", 11);
    s.save("a", &c).unwrap();
    let after = std::fs::read_to_string(&path).unwrap();
    let diff: Vec<_> = before
        .lines()
        .zip(after.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(diff.len(), 1);
    assert!(diff[0].1.contains("\"x\": 11"));
}

#[test]
fn leaves_no_temp_file() {
    let (t, s) = store();
    s.save("a", &sample()).unwrap();
    let names: Vec<_> = std::fs::read_dir(t.path().join("c"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, vec!["a.clidraw.json".to_string()]);
}

#[test]
fn refuses_higher_version_and_keeps_the_file() {
    let (t, s) = store();
    std::fs::create_dir_all(t.path().join("c")).unwrap();
    let path = t.path().join("c/new.clidraw.json");
    std::fs::write(&path, "{\"version\": 2, \"layers\": []}\n").unwrap();
    assert!(matches!(s.load("new"), Err(StoreError::TooNew(2))));
    assert!(matches!(
        s.save("new", &sample()),
        Err(StoreError::TooNew(2))
    ));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "{\"version\": 2, \"layers\": []}\n"
    );
}

#[test]
fn rejects_bad_names() {
    for n in [
        "", ".hidden", "a/b", "a\\b", "-x", "--help", "a:b", "a*", "a?", "a<", "a>", "a|", "a\"",
        "a\tb", "x.", "x ", "CON", "con", "nul.txt", "Com1", "LPT9.a.b",
    ] {
        assert!(validate_name(n).is_err(), "{n:?}");
    }
    for n in ["flow-v2", "console", "com10", "a.b"] {
        assert!(validate_name(n).is_ok(), "{n:?}");
    }
    let (_t, s) = store();
    assert!(matches!(
        s.save("../x", &sample()),
        Err(StoreError::BadName(_))
    ));
}

#[test]
fn rename_delete_and_stale_temp_cleanup() {
    let (t, s) = store();
    s.create("a").unwrap();
    assert!(matches!(s.create("a"), Err(StoreError::Exists(_))));
    s.rename("a", "b").unwrap();
    std::fs::write(t.path().join("c/.b.clidraw.json.tmp"), "junk").unwrap();
    assert_eq!(s.list().unwrap(), vec!["b".to_string()]);
    assert!(!t.path().join("c/.b.clidraw.json.tmp").exists());
    s.delete("b").unwrap();
    assert!(s.list().unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn follows_a_symlinked_directory_and_keeps_it() {
    let t = tempfile::tempdir().unwrap();
    let real = t.path().join("dotfiles");
    std::fs::create_dir(&real).unwrap();
    let link = t.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let s = Store::at(link.clone(), t.path().join("state"));
    s.save("a", &sample()).unwrap();
    assert!(real.join("a.clidraw.json").exists());
    assert!(std::fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn directory_resolution_order() {
    let home = Some(PathBuf::from("/h"));
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |k: &str| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        }
    };
    let both = env(&[("CLIDRAW_HOME", "/x"), ("XDG_CONFIG_HOME", "/y")]);
    assert_eq!(resolve_dir(both, home.clone()), Some("/x".into()));
    let xdg = env(&[("XDG_CONFIG_HOME", "/y")]);
    assert_eq!(resolve_dir(xdg, home.clone()), Some("/y/clidraw".into()));
    if cfg!(unix) {
        assert_eq!(
            resolve_dir(env(&[]), home),
            Some("/h/.config/clidraw".into())
        );
    }
}

#[test]
fn a_pinned_end_round_trips_and_auto_ends_write_no_anchor() {
    let (t, s) = store();
    let mut c = sample();
    let Element::Line(l) = &mut c.elements[2] else {
        panic!()
    };
    l.from = End::Bound(Id::from("aaaaaaaa"), Anchor::Left(2));
    s.save("a", &c).unwrap();
    assert_eq!(s.load("a").unwrap(), c);
    let text = std::fs::read_to_string(t.path().join("c/a.clidraw.json")).unwrap();
    assert!(text.contains("\"left\": 2"));
    s.save("b", &sample()).unwrap();
    let text = std::fs::read_to_string(t.path().join("c/b.clidraw.json")).unwrap();
    assert!(text.contains("\"bound\"") && !text.contains("anchored"));
    assert_eq!(s.load("b").unwrap(), sample());
}

#[test]
fn groups_round_trip_and_an_ungrouped_canvas_writes_no_key() {
    let (t, s) = store();
    let mut c = sample();
    s.save("a", &c).unwrap();
    let text = std::fs::read_to_string(t.path().join("c/a.clidraw.json")).unwrap();
    assert!(!text.contains("groups"));
    assert!(s.load("a").unwrap().groups.is_empty());
    c.groups = vec![vec![Id::from("aaaaaaaa"), Id::from("bbbbbbbb")]];
    s.save("a", &c).unwrap();
    assert_eq!(s.load("a").unwrap(), c);
}

#[test]
fn library_lives_in_a_subdirectory_that_list_ignores() {
    let (t, s) = store();
    s.save("library", &sample()).unwrap();
    s.library().save("pair", &sample()).unwrap();
    assert!(t.path().join("c/library/pair.clidraw.json").exists());
    assert_eq!(s.list().unwrap(), vec!["library".to_string()]);
    assert_eq!(s.library().load("pair").unwrap().elements.len(), 3);
}

#[test]
fn bends_round_trip_and_an_empty_list_writes_no_key() {
    let (_t, s) = store();
    let mut c = sample();
    let Some(Element::Line(l)) = c.elements.last_mut() else {
        unreachable!()
    };
    l.bends = vec![(1, 2), (3, 4)];
    s.save("a", &c).unwrap();
    assert_eq!(s.load("a").unwrap(), c);
    assert!(!serde_json::to_string(&sample()).unwrap().contains("bends"));
    assert!(serde_json::to_string(&c).unwrap().contains("bends"));
}

#[test]
fn load_rejects_invalid_elements_and_names_them() {
    let (t, s) = store();
    let mut small = sample();
    let Element::Shape(sh) = &mut small.elements[0] else {
        unreachable!()
    };
    sh.w = -2;
    let mut dangling = sample();
    dangling.elements.remove(0);
    for (bad, id) in [(small, "aaaaaaaa"), (dangling, "aaaaaaaa")] {
        std::fs::create_dir_all(t.path().join("c")).unwrap();
        std::fs::write(
            t.path().join("c/bad.clidraw.json"),
            serde_json::to_string(&bad).unwrap(),
        )
        .unwrap();
        let err = s.load("bad").unwrap_err();
        assert!(matches!(err, StoreError::Invalid(_)), "{err}");
        assert!(err.to_string().contains(id), "{err}");
    }
}

#[test]
fn rename_moves_the_history_and_delete_removes_it() {
    let (t, s) = store();
    s.save("a", &sample()).unwrap();
    s.save_history("a", vec![Command::Remove(Id::from("x"))])
        .unwrap();
    s.rename("a", "b").unwrap();
    assert_eq!(s.load_history("b").len(), 1);
    assert!(!t.path().join("state/history/a.json").exists());
    s.delete("b").unwrap();
    assert!(!t.path().join("state/history/b.json").exists());
}

#[test]
fn a_failed_state_write_keeps_the_previous_state() {
    let (t, s) = store();
    let old = State {
        last: Some("a".into()),
        ..Default::default()
    };
    s.save_state(&old).unwrap();
    std::fs::create_dir(t.path().join("state/.state.json.tmp")).unwrap();
    let new = State {
        last: Some("b".into()),
        ..Default::default()
    };
    assert!(s.save_state(&new).is_err());
    assert_eq!(s.load_state().last.as_deref(), Some("a"));
}

#[test]
fn a_failed_history_write_keeps_the_previous_history() {
    let (t, s) = store();
    s.save("a", &sample()).unwrap();
    s.save_history("a", vec![Command::Remove(Id::from("x"))])
        .unwrap();
    std::fs::create_dir(t.path().join("state/history/.a.json.tmp")).unwrap();
    let undo = vec![
        Command::Remove(Id::from("y")),
        Command::Remove(Id::from("z")),
    ];
    assert!(s.save_history("a", undo).is_err());
    assert_eq!(s.load_history("a").len(), 1);
}

#[test]
fn canvas_hash_is_fnv1a_so_saved_history_survives_toolchain_upgrades() {
    let (t, s) = store();
    std::fs::create_dir_all(t.path().join("c")).unwrap();
    std::fs::write(t.path().join("c/h.clidraw.json"), "a").unwrap();
    assert_eq!(s.canvas_hash("h"), Some(0xaf63dc4c8601ec8c));
}
