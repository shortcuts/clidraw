use super::common::*;
use clidraw::app::App;
use clidraw::config::Config;
use clidraw::model::{Element, Id, Shape, ShapeKind};
use clidraw::store::Store;
use crossterm::event::KeyCode;
use std::time::{Duration, Instant};

#[test]
fn mermaid_editor_enter_imports_the_line() {
    let d = tempfile::tempdir().unwrap();
    let mut app = app_in(d.path());
    type_mermaid(&mut app, "A --> B", KeyCode::Enter);
    assert_eq!(app.session.canvas.elements.len(), 4);
    assert_eq!(status(&app), "Imported 2 shapes, 1 lines");
}

#[test]
fn mermaid_editor_esc_imports_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut app = app_in(d.path());
    type_mermaid(&mut app, "A --> B", KeyCode::Esc);
    assert!(app.session.canvas.elements.is_empty());
}

#[test]
fn mermaid_editor_enter_on_empty_buffer_sets_no_status() {
    let d = tempfile::tempdir().unwrap();
    let mut app = app_in(d.path());
    type_mermaid(&mut app, "", KeyCode::Enter);
    assert!(app.session.canvas.elements.is_empty());
    assert!(status(&app).is_empty());
}

#[test]
fn mermaid_editor_tab_applies_the_fix_and_enter_alone_does_not() {
    let d = tempfile::tempdir().unwrap();
    let mut app = app_in(d.path());
    type_mermaid(&mut app, "A -> B", KeyCode::Enter);
    assert!(app.session.canvas.elements.is_empty());

    let mut app = app_in(d.path());
    type_mermaid(&mut app, "A -> B", KeyCode::Tab);
    assert_eq!(buffer(&app), "A --> B");
    assert!(app.session.canvas.elements.is_empty());
    app.on_event(key(KeyCode::Enter), Instant::now());
    assert_eq!(app.session.canvas.elements.len(), 4);
}

#[test]
fn mermaid_editor_tab_does_nothing_when_quick_fixes_are_off() {
    let d = tempfile::tempdir().unwrap();
    let store = Store::at(d.path().join("c"), d.path().join("state"));
    let cfg = Config {
        quick_fixes: false,
        ..Config::default()
    };
    let mut app = App::open_with(store, Some("demo".into()), (80, 24), cfg, vec![]).unwrap();
    type_mermaid(&mut app, "A -> B", KeyCode::Tab);
    assert_eq!(buffer(&app), "A -> B");
}

fn box_state(app: &App) -> (ShapeKind, i32, i32, i32, i32) {
    match app.session.canvas.get(&Id::from("aaaaaaaa")) {
        Some(Element::Shape(s)) => (s.kind, s.x, s.y, s.w, s.h),
        _ => panic!("box is gone"),
    }
}

fn open_change_shape(app: &mut App, now: Instant) {
    insert_box(app);
    app.session.selection = vec![Id::from("aaaaaaaa")];
    ctrl_key(app, 'p', now);
    type_str(app, "change shape", now);
    app.on_event(key(KeyCode::Enter), now);
}

#[test]
fn change_shape_previews_without_history_and_enter_commits() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_change_shape(&mut app, now);
    let depth = app.session.history.commands().len();
    app.on_event(key(KeyCode::Down), now);
    let (w, h) = Shape::snap_size(ShapeKind::Diamond, 8, 3);
    assert_eq!(
        box_state(&app),
        (ShapeKind::Diamond, 30, 8, w, h),
        "preview shows the kind"
    );
    assert_eq!(app.session.history.commands().len(), depth);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert_eq!(box_state(&app), (ShapeKind::Diamond, 30, 8, w, h));
    assert_eq!(app.session.history.commands().len(), depth + 1);
    app.session.undo();
    assert_eq!(box_state(&app), (ShapeKind::Rect, 30, 8, 8, 3));
}

#[test]
fn change_shape_cycling_returns_to_the_exact_original_size() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_change_shape(&mut app, now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Up), now);
    assert_eq!(box_state(&app), (ShapeKind::Rect, 30, 8, 8, 3));
}

#[test]
fn change_shape_esc_restores_the_shape() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_change_shape(&mut app, now);
    let depth = app.session.history.commands().len();
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Esc), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert_eq!(box_state(&app), (ShapeKind::Rect, 30, 8, 8, 3));
    assert_eq!(app.session.history.commands().len(), depth);
}

#[test]
fn change_shape_click_outside_restores_and_click_row_commits() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_change_shape(&mut app, now);
    app.on_event(key(KeyCode::Down), now);
    click(&mut app, 70, 3, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert_eq!(box_state(&app).0, ShapeKind::Rect);

    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "change shape", now);
    app.on_event(key(KeyCode::Enter), now);
    // The box ends above the bottom bar: border at row 15, Rect at 16, Cylinder at 18.
    click(&mut app, 3, 18, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert_eq!(box_state(&app).0, ShapeKind::Cylinder);
}

#[test]
fn change_shape_is_hidden_without_a_shape() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    assert!(!palette_labels(&app).contains(&"Edit: Change shape".to_string()));
    app.on_event(key(KeyCode::Esc), now);
    insert_box(&mut app);
    app.session.selection = vec![Id::from("aaaaaaaa")];
    ctrl_key(&mut app, 'p', now);
    assert!(palette_labels(&app).contains(&"Edit: Change shape".to_string()));
}

#[test]
fn change_shape_preview_is_not_saved() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_change_shape(&mut app, now);
    app.on_event(key(KeyCode::Down), now);
    app.tick(now + Duration::from_secs(2));
    app.save();
    let saved = std::fs::read_to_string(t.path().join("c/demo.clidraw.json")).unwrap();
    assert!(saved.contains("\"kind\": \"Rect\""), "{saved}");
    assert!(!saved.contains("Diamond"), "{saved}");
}
