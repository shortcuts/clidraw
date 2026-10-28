use super::common::*;
use clidraw::model::{Element, Id};
use crossterm::event::KeyCode;
use std::time::Instant;

#[test]
fn import_adds_selected_shapes_as_one_undo_step() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let file = t.path().join("d.mmd");
    std::fs::write(&file, "flowchart TD\nA --> B\nB --> C\n").unwrap();
    run(&mut app, &format!("import {}", file.display()));
    assert_eq!(app.session.canvas.elements.len(), 6);
    assert_eq!(app.session.selection.len(), 6);
    let frame = app
        .session
        .canvas
        .elements
        .iter()
        .find_map(|e| match e {
            Element::Shape(s) if s.kind == clidraw::model::ShapeKind::Frame => Some(s.clone()),
            _ => None,
        })
        .unwrap();
    let mut unit = vec![frame.id.clone()];
    app.session.canvas.expand_frames(&mut unit);
    assert_eq!(unit.len(), 6);
    assert_eq!(frame.label, "d.mmd");
    assert_eq!(frame.align, clidraw::model::TextAlign::Center);
    let (wx, wy) = app.session.view.to_world(2, 2);
    assert_eq!((frame.x, frame.y), (wx - 1, wy - 1));
    assert!(app
        .status
        .as_ref()
        .unwrap()
        .text
        .starts_with("Imported 3 shapes, 2 lines"));
    app.on_event(key(KeyCode::Char('u')), Instant::now());
    assert!(app.session.canvas.elements.is_empty());
}

#[test]
fn import_frame_leaves_four_columns_and_two_rows_around_content() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let file = t.path().join("d.mmd");
    std::fs::write(&file, "flowchart TD\nA --> B\nB --> C\n").unwrap();
    run(&mut app, &format!("import {}", file.display()));
    assert_frame_padding(&app);
}

#[test]
fn second_import_frame_lands_below_the_first() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let file = t.path().join("d.mmd");
    std::fs::write(&file, "flowchart TD\nA --> B\n").unwrap();
    run(&mut app, &format!("import {}", file.display()));
    run(&mut app, &format!("import {}", file.display()));
    let frames: Vec<_> = app
        .session
        .canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Shape(s) if s.kind == clidraw::model::ShapeKind::Frame => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(frames.len(), 2);
    assert!(frames[1].y >= frames[0].y + frames[0].h);
}

#[test]
fn lib_save_then_insert_adds_fresh_selected_copies_in_one_undo_step() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let file = t.path().join("d.mmd");
    std::fs::write(&file, "flowchart TD\nA --> B\n").unwrap();
    run(&mut app, &format!("import {}", file.display()));
    assert_eq!(app.session.selection.len(), 4);
    run(&mut app, "lib save pair");
    assert_eq!(status(&app), "Saved pair to the library");
    assert!(t.path().join("c/library/pair.clidraw.json").exists());

    run(&mut app, "e other");
    run(&mut app, "lib insert pair");
    assert_eq!(status(&app), "Inserted pair");
    assert_eq!(app.session.canvas.elements.len(), 4);
    assert_eq!(app.session.selection.len(), 4);
    let (wx, wy) = app.session.view.to_world(2, 2);
    assert_eq!(app.session.canvas.origin(), (wx, wy));
    let bound = app.session.canvas.elements.iter().any(|e| {
        matches!(e, Element::Line(l) if matches!(l.from, clidraw::model::End::Bound(..))
            && matches!(l.to, clidraw::model::End::Bound(..)))
    });
    assert!(bound);

    run(&mut app, "lib insert pair");
    assert_eq!(app.session.canvas.elements.len(), 8);
    run(&mut app, "u");
    assert_eq!(app.session.canvas.elements.len(), 4);
}

#[test]
fn lib_list_delete_and_save_refuses_to_overwrite() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    run(&mut app, "lib list");
    assert_eq!(status(&app), "Library is empty");
    insert_shape(&mut app);
    app.session.selection = vec![Id::from("aaaaaaaa")];
    run(&mut app, "lib save one");
    run(&mut app, "lib save one");
    assert_eq!(status(&app), "one is in the library: :lib delete one first");
    run(&mut app, "lib list");
    assert_eq!(status(&app), "Library: one");
    run(&mut app, "lib delete one");
    assert_eq!(status(&app), "Deleted one from the library");
    assert!(!t.path().join("c/library/one.clidraw.json").exists());
    run(&mut app, "lib delete one");
    assert!(status(&app).starts_with("Library failed"));
    run(&mut app, "lib delete");
    assert_eq!(status(&app), "Usage: :lib list|save|insert|delete [name]");
}

#[test]
fn lib_errors_write_nothing() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    run(&mut app, "lib");
    assert_eq!(status(&app), "Usage: :lib list|save|insert|delete [name]");
    run(&mut app, "lib save");
    assert_eq!(status(&app), "Usage: :lib list|save|insert|delete [name]");
    run(&mut app, "lib save box");
    assert_eq!(status(&app), "Select shapes to save first");
    assert!(!t.path().join("c/library").exists());
    insert_shape(&mut app);
    app.session.selection = vec![Id::from("aaaaaaaa")];
    run(&mut app, "lib save a/b");
    assert!(status(&app).starts_with("Library failed"));
    run(&mut app, "lib insert nope");
    assert!(status(&app).starts_with("Library failed"));
}

#[test]
fn import_of_a_missing_file_shows_an_error() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    run(&mut app, "import /no/such.mmd");
    assert!(app.status.unwrap().text.starts_with("Import failed"));
}

#[test]
fn failed_import_lists_the_live_hint_diagnostics() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let file = t.path().join("d.mmd");
    std::fs::write(&file, "flowchart TD\nsubgraph x\nA -->\n").unwrap();
    run(&mut app, &format!("import {}", file.display()));
    assert_eq!(
        status(&app),
        "Import failed: nothing to import; line 2: `subgraph` is not supported and is skipped on import; line 3: Cannot read this line: expected nodes joined by arrows"
    );
}

#[test]
fn import_drawer_navigates_fills_and_descends() {
    let t = tempfile::tempdir().unwrap();
    let files = t.path().join("files");
    std::fs::create_dir(&files).unwrap();
    let d = files.display().to_string();
    std::fs::write(files.join("a.mmd"), "flowchart TD\nA --> B\n").unwrap();
    std::fs::write(files.join("b.mmd"), "").unwrap();
    std::fs::write(files.join(".hid"), "").unwrap();
    std::fs::create_dir(files.join("sub")).unwrap();
    std::fs::write(files.join("sub").join("c.mmd"), "").unwrap();
    let mut app = vim_app(t.path());
    let now = Instant::now();
    type_str(&mut app, &format!(":import {d}/"), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Up), now);
    ctrl_key(&mut app, 'n', now);
    ctrl_key(&mut app, 'n', now);
    app.on_event(key(KeyCode::Tab), now);
    assert_eq!(command_buf(&app), format!("import {d}/b.mmd"));

    let mut app = vim_app(t.path());
    type_str(&mut app, &format!(":import {d}/"), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(command_buf(&app), format!("import {d}/sub/"));
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(command_buf(&app), format!("import {d}/sub/c.mmd"));

    let mut app = vim_app(t.path());
    type_str(&mut app, &format!(":import {d}/a.mmd"), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(app
        .status
        .as_ref()
        .unwrap()
        .text
        .starts_with("Imported 2 shapes"));
}
