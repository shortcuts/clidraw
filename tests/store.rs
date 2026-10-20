use clidraw::model::{Anchor, Canvas, Element, End, Id, Line, LineKind, Shape, ShapeKind};
use clidraw::store::{resolve_dir, validate_name, Store, StoreError};
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
    for n in ["", ".hidden", "a/b", "a\\b"] {
        assert!(validate_name(n).is_err(), "{n:?}");
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
    l.from = End::Bound(Id::from("aaaaaaaa"), Anchor::Left);
    s.save("a", &c).unwrap();
    assert_eq!(s.load("a").unwrap(), c);
    s.save("b", &sample()).unwrap();
    let text = std::fs::read_to_string(t.path().join("c/b.clidraw.json")).unwrap();
    assert!(text.contains("\"bound\"") && !text.contains("anchored"));
    assert_eq!(s.load("b").unwrap(), sample());
}
