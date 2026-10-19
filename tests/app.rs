use clidraw::app::App;
use clidraw::model::{Command, Element, Id, Shape, ShapeKind};
use clidraw::store::Store;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::time::{Duration, Instant};

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

#[test]
fn autosaves_edits_after_500ms_and_pan_leaves_the_file_alone() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let mut app = App::open(store, Some("demo".into()), (80, 24)).unwrap();
    let file = t.path().join("c/demo.clidraw.json");
    let empty = std::fs::read(&file).unwrap();
    let t0 = Instant::now();

    app.on_event(mouse(MouseEventKind::ScrollDown, 30, 10), t0);
    app.tick(t0 + Duration::from_secs(2));
    assert_eq!(std::fs::read(&file).unwrap(), empty);

    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 2, 3), t0);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 2, 3), t0);
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 20, 5), t0);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 30, 9), t0);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 30, 9), t0);
    app.tick(t0 + Duration::from_millis(100));
    assert_eq!(std::fs::read(&file).unwrap(), empty, "too early to save");
    app.tick(t0 + Duration::from_millis(700));
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(saved.contains("\"type\": \"shape\""), "{saved}");
    assert!(saved.contains("\"kind\": \"Rect\""));
    assert!(!saved.contains("offset"));
}

#[test]
fn reopens_the_last_canvas() {
    let t = tempfile::tempdir().unwrap();
    let mk = || Store::at(t.path().join("c"), t.path().join("state"));
    let app = App::open(mk(), Some("one".into()), (80, 24)).unwrap();
    drop(app);
    let app = App::open(mk(), None, (80, 24)).unwrap();
    assert_eq!(app.name, "one");
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn click(app: &mut App, col: u16, row: u16, t: Instant) {
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), col, row), t);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), col, row), t);
}

/// Two shapes near the origin and one far away; opens Esc menu, Export SVG, Area.
fn area_app(dir: &std::path::Path) -> (App, Instant) {
    let store = Store::at(dir.join("c"), dir.join("state"));
    let mut app = App::open(store, Some("demo".into()), (80, 24)).unwrap();
    for (id, x, y, label) in [
        ("aaaaaaaa", 2, 2, "Alpha"),
        ("bbbbbbbb", 20, 2, "Beta"),
        ("cccccccc", 60, 15, "Gamma"),
    ] {
        app.session.exec(Command::Insert(
            app.session.canvas.elements.len(),
            Element::Shape(Shape {
                id: Id::from(id),
                kind: ShapeKind::Rect,
                x,
                y,
                w: 8,
                h: 3,
                label: label.into(),
            }),
        ));
    }
    let t = Instant::now();
    app.on_event(key(KeyCode::Esc), t);
    click(&mut app, 31, 11, t);
    click(&mut app, 31, 12, t);
    (app, t)
}

fn type_path(app: &mut App, path: &str, t: Instant) {
    for _ in 0..16 {
        app.on_event(key(KeyCode::Backspace), t);
    }
    for c in path.chars() {
        app.on_event(key(KeyCode::Char(c)), t);
    }
    app.on_event(key(KeyCode::Enter), t);
}

#[test]
fn exports_only_the_dragged_area() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 0, 0), now);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 40, 8), now);
    assert!(app.session.band.is_some());
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 40, 8), now);
    assert!(app.session.band.is_none());
    let out = t.path().join("area.svg");
    type_path(&mut app, out.to_str().unwrap(), now);
    let svg = std::fs::read_to_string(out).unwrap();
    assert!(svg.contains("Alpha") && svg.contains("Beta"), "{svg}");
    assert!(!svg.contains("Gamma"), "{svg}");
}

#[test]
fn esc_cancels_the_area_pick() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 0, 0), now);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 40, 8), now);
    app.on_event(key(KeyCode::Esc), now);
    assert!(app.session.band.is_none());
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 40, 8), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert_eq!(app.status, None);
}

#[test]
fn an_empty_area_opens_no_prompt() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    click(&mut app, 70, 2, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert_eq!(app.status.as_deref(), Some("nothing inside the area"));
}

#[test]
fn menu_hotkeys_pick_entries() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let mut app = App::open(store, Some("demo".into()), (80, 24)).unwrap();
    let now = Instant::now();
    app.on_event(key(KeyCode::Esc), now);
    app.on_event(key(KeyCode::Char('m')), now);
    assert!(matches!(
        app.overlay,
        clidraw::ui::Overlay::Scope(clidraw::ui::scope::ExportFormat::Mermaid)
    ));
    app.on_event(key(KeyCode::Esc), now);
    app.on_event(key(KeyCode::Esc), now);
    app.on_event(key(KeyCode::Char('z')), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Menu));
    app.on_event(key(KeyCode::Char('q')), now);
    assert!(app.quit);
}
