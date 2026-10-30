use super::common::*;
use clidraw::app::App;
use clidraw::config::Config;
use clidraw::model::{Command, Element, Id, Shape, ShapeKind};
use clidraw::store::Store;
use crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use std::time::{Duration, Instant};

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

    app.on_event(key(KeyCode::Char('r')), t0);
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 20, 5), t0);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 30, 9), t0);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 30, 9), t0);
    app.tick(t0 + Duration::from_millis(100));
    assert_eq!(std::fs::read(&file).unwrap(), empty, "too early to save");
    app.tick(t0 + Duration::from_millis(700));
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(saved.contains("\"type\":\"shape\""), "{saved}");
    assert!(saved.contains("\"kind\":\"Rect\""));
    assert!(!saved.contains("offset"));
}

#[test]
fn sleeps_until_the_autosave_deadline_only_while_an_edit_is_unsaved() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let mut app = App::open(store, Some("demo".into()), (80, 24)).unwrap();
    let t0 = Instant::now();
    assert_eq!(app.next_wake(t0), None, "idle must block");

    app.on_event(key(KeyCode::Char('r')), t0);
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 20, 5), t0);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 30, 9), t0);
    app.tick(t0);
    assert_eq!(
        app.next_wake(t0),
        None,
        "a drag blocks saves; Up wakes the loop"
    );

    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 30, 9), t0);
    app.tick(t0);
    let wait = Duration::from_millis(500);
    assert_eq!(app.next_wake(t0), Some(wait));
    let later = t0 + Duration::from_millis(200);
    assert_eq!(app.next_wake(later), Some(Duration::from_millis(300)));
    assert_eq!(
        app.next_wake(t0 + Duration::from_secs(2)),
        Some(Duration::ZERO)
    );

    app.tick(t0 + wait);
    assert_eq!(app.next_wake(t0 + wait), None, "saved, so block again");
}

#[test]
fn does_not_autosave_during_a_drag() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let mut app = App::open(store, Some("demo".into()), (80, 24)).unwrap();
    let file = t.path().join("c/demo.clidraw.json");
    let t0 = Instant::now();
    app.on_event(key(KeyCode::Char('r')), t0);
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 20, 5), t0);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 30, 9), t0);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 30, 9), t0);
    app.tick(t0 + Duration::from_secs(1));
    let before = std::fs::read(&file).unwrap();

    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 25, 5), t0);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 35, 12), t0);
    app.tick(t0);
    app.tick(t0 + Duration::from_secs(2));
    assert_eq!(std::fs::read(&file).unwrap(), before, "saved mid-drag");

    app.save();
    assert_ne!(std::fs::read(&file).unwrap(), before, "save() must write");

    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 40, 14), t0);
    app.tick(t0 + Duration::from_secs(3));
    let mid = std::fs::read(&file).unwrap();
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 40, 14), t0);
    app.tick(t0 + Duration::from_secs(4));
    assert_ne!(std::fs::read(&file).unwrap(), mid, "no save after release");
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

#[test]
fn config_sets_autosave_default_canvas_and_status() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let cfg = Config {
        autosave: Duration::from_millis(100),
        default_canvas: "scratch".into(),
        ..Config::default()
    };
    let mut app =
        App::open_with(store, None, (80, 24), cfg, vec!["config.toml: oops".into()]).unwrap();
    assert_eq!(app.name, "scratch");
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("config.toml: oops")
    );
    let file = t.path().join("c/scratch.clidraw.json");
    let empty = std::fs::read(&file).unwrap();
    let t0 = Instant::now();
    app.on_event(key(KeyCode::Char('r')), t0);
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 20, 5), t0);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 30, 9), t0);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 30, 9), t0);
    app.tick(t0);
    app.tick(t0 + Duration::from_millis(150));
    assert_ne!(std::fs::read(&file).unwrap(), empty);
}

fn drag_app(dir: &std::path::Path) -> App {
    let store = Store::at(dir.join("c"), dir.join("state"));
    let mut app = App::open(store, Some("demo".into()), (80, 24)).unwrap();
    app.session.exec(Command::Insert(
        0,
        Element::Shape(Shape {
            id: Id::from("aaaaaaaa"),
            kind: ShapeKind::Rect,
            x: 10,
            y: 5,
            w: 8,
            h: 3,
            label: String::new(),
            stroke: Default::default(),
            border: Default::default(),
            color: None,
            fill: None,
            align: Default::default(),
            bold: false,
            link: None,
            side_labels: vec![],
            turn: 0,
            meta: Default::default(),
        }),
    ));
    app
}

#[test]
fn queued_drags_run_the_handler_once_with_the_last_position() {
    let t = tempfile::tempdir().unwrap();
    let now = Instant::now();
    let down = mouse(MouseEventKind::Down(MouseButton::Left), 12, 6);
    let up = mouse(MouseEventKind::Up(MouseButton::Left), 40, 20);
    let drag = |i: u16| {
        mouse(
            MouseEventKind::Drag(MouseButton::Left),
            12 + i / 4,
            6 + i / 8,
        )
    };

    let mut one = drag_app(&t.path().join("one"));
    let one_before = one.session.rev;
    one.on_event(down.clone(), now);
    one.on_event(drag(99), now);
    let one_rev = one.session.rev;
    one.on_event(up.clone(), now);

    let mut many = drag_app(&t.path().join("many"));
    let before = many.session.rev;
    let mut events = vec![down];
    events.extend((0..100).map(drag));
    many.on_events(events, now);
    assert_eq!(many.session.rev - before, one_rev - one_before);
    many.on_events(vec![up], now);
    assert_eq!(many.session.canvas, one.session.canvas);
}

#[test]
fn queued_scrolls_add_up_and_keep_order_around_other_events() {
    let t = tempfile::tempdir().unwrap();
    let mut app = drag_app(t.path());
    let now = Instant::now();
    let scroll = |k| mouse(k, 30, 10);
    let mut evs = vec![scroll(MouseEventKind::ScrollDown); 3];
    evs.push(scroll(MouseEventKind::ScrollRight));
    evs.extend(vec![scroll(MouseEventKind::ScrollDown); 2]);
    app.on_events(evs, now);
    assert_eq!((app.session.view.ox, app.session.view.oy), (8, 18));
}

#[test]
fn moved_events_leave_the_screen_clean_but_input_dirties_it() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    assert!(app.dirty);
    app.dirty = false;
    let t0 = Instant::now();
    app.on_events(vec![mouse(MouseEventKind::Moved, 60, 15)], t0);
    assert!(!app.dirty);
    app.on_events(vec![mouse(MouseEventKind::ScrollDown, 5, 5)], t0);
    assert!(app.dirty);
    app.dirty = false;
    app.on_event(Event::Resize(100, 30), t0);
    assert!(app.dirty);
    app.dirty = false;
    app.on_event(key(KeyCode::Char('?')), t0);
    assert!(app.dirty);
}

#[test]
fn history_limit_from_config_survives_opening_another_canvas() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let cfg = Config {
        history_limit: 2,
        ..Config::default()
    };
    let mut app = App::open_with(store, None, (80, 24), cfg, vec![]).unwrap();
    run(&mut app, "e other");
    for i in 0..3 {
        let shape = Shape {
            id: Id::from(format!("shape{i:03}").as_str()),
            kind: ShapeKind::Rect,
            x: 0,
            y: 0,
            w: 3,
            h: 3,
            label: String::new(),
            stroke: Default::default(),
            border: Default::default(),
            color: None,
            fill: None,
            align: Default::default(),
            bold: false,
            link: None,
            side_labels: vec![],
            turn: 0,
            meta: Default::default(),
        };
        app.session.exec(Command::Insert(i, Element::Shape(shape)));
    }
    for _ in 0..3 {
        app.session.undo();
    }
    assert_eq!(app.session.canvas.elements.len(), 1);
}

#[test]
fn perf_command_toggles_the_overlay() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let mut app = App::open(store, None, (80, 24)).unwrap();
    assert!(app.perf.is_none());
    run(&mut app, "perf");
    assert!(app.perf.is_some());
    run(&mut app, "perf");
    assert!(app.perf.is_none());
}

#[test]
fn wakes_to_pan_while_a_drag_holds_the_edge() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let mut app = App::open(store, Some("demo".into()), (80, 24)).unwrap();
    let t0 = Instant::now();
    app.on_event(key(KeyCode::Char('r')), t0);
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 20, 5), t0);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 40, 9), t0);
    assert_eq!(app.next_wake(t0), None, "mid-screen drag blocks");

    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 79, 9), t0);
    assert_eq!(app.next_wake(t0), Some(Duration::ZERO));
    app.dirty = false;
    app.tick(t0);
    assert!(app.dirty, "pan must redraw");
    assert_eq!(app.session.view.ox, 3);
    assert_eq!(app.next_wake(t0), Some(Duration::from_millis(30)));
}

#[test]
fn moved_redraws_only_when_the_hovered_target_changes() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    app.session
        .canvas
        .elements
        .push(clidraw::model::Element::Shape(clidraw::model::Shape {
            id: clidraw::model::Id::from("aaaaaaaa"),
            kind: clidraw::model::ShapeKind::Rect,
            x: 20,
            y: 4,
            w: 8,
            h: 4,
            label: String::new(),
            stroke: Default::default(),
            border: Default::default(),
            color: None,
            fill: None,
            align: Default::default(),
            bold: false,
            link: None,
            side_labels: vec![],
            turn: 0,
            meta: Default::default(),
        }));
    let t0 = Instant::now();
    app.dirty = false;
    app.on_events(vec![mouse(MouseEventKind::Moved, 22, 4)], t0);
    assert!(app.dirty, "entering the shape");
    app.dirty = false;
    app.on_events(vec![mouse(MouseEventKind::Moved, 23, 4)], t0);
    assert!(!app.dirty, "same target");
    app.on_events(
        vec![
            mouse(MouseEventKind::Moved, 22, 4),
            mouse(MouseEventKind::Moved, 60, 15),
        ],
        t0,
    );
    assert!(app.dirty, "burst ends off the shape");
    assert_eq!(app.session.hover, clidraw::input::Hit::Empty);
}

#[test]
fn undo_survives_a_reopen_only_while_the_file_is_unchanged() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    insert_shape(&mut app);
    app.save();
    drop(app);

    let mut app = app_in(t.path());
    assert_eq!(app.session.canvas.elements.len(), 1);
    run(&mut app, "u");
    assert_eq!(app.session.canvas.elements.len(), 0);
    app.save();
    drop(app);

    let file = t.path().join("c/demo.clidraw.json");
    let mut app = app_in(t.path());
    run(&mut app, "u");
    assert_eq!(app.session.canvas.elements.len(), 0, "stack was drained");

    insert_shape(&mut app);
    app.save();
    drop(app);
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, text.replace("\"x\":2", "\"x\":9")).unwrap();
    let mut app = app_in(t.path());
    run(&mut app, "u");
    assert_eq!(
        app.session.canvas.elements.len(),
        1,
        "edited file drops history"
    );
}

#[test]
fn ticks_only_while_the_view_glides() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let mut app = App::open(store, Some("demo".into()), (80, 24)).unwrap();
    let t0 = Instant::now();
    assert_eq!(app.next_wake(t0), None);
    app.session.fit(Some((100, 50, 10, 4)));
    assert_eq!(app.next_wake(t0), Some(Duration::from_millis(16)));
    app.dirty = false;
    app.tick(t0);
    app.tick(t0 + Duration::from_millis(60));
    assert!(app.dirty, "a moving glide frame redraws");
    app.tick(t0 + Duration::from_millis(120));
    assert_eq!((app.session.view.ox, app.session.view.oy), (65, 40));
    assert_eq!(app.next_wake(t0 + Duration::from_millis(120)), None);
}

#[test]
fn pointer_shape_follows_tool_hover_and_drag() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let mut app = App::open(store, Some("demo".into()), (80, 24)).unwrap();
    let t0 = Instant::now();
    assert_eq!(app.take_pointer(), None);

    app.on_event(key(KeyCode::Char('r')), t0);
    assert_eq!(app.take_pointer(), Some("crosshair"));
    app.on_event(key(KeyCode::Char('v')), t0);
    assert_eq!(app.take_pointer(), None);
    app.on_event(key(KeyCode::Esc), t0);
    assert_eq!(app.take_pointer(), Some("default"));
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 30, 10), t0);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 34, 10), t0);
    assert_eq!(app.take_pointer(), Some("grabbing"));
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 34, 10), t0);
    ctrl_key(&mut app, 'p', t0);
    assert_eq!(app.take_pointer(), Some("default"));
}

#[test]
fn failed_save_keeps_the_canvas_unsaved_and_retries() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let dir = t.path().join("c");
    std::fs::remove_dir_all(&dir).unwrap();
    std::fs::write(&dir, "").unwrap();
    let t0 = Instant::now();
    app.on_event(key(KeyCode::Char('r')), t0);
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 20, 5), t0);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 30, 9), t0);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 30, 9), t0);

    app.save();
    assert!(last_row(&app).ends_with("demo ●"), "{}", last_row(&app));

    std::fs::remove_file(&dir).unwrap();
    app.tick(t0);
    app.tick(t0 + Duration::from_secs(10));
    assert!(last_row(&app).ends_with("demo"), "{}", last_row(&app));
}

#[test]
fn reloads_an_external_edit_instead_of_overwriting_it() {
    let t = tempfile::tempdir().unwrap();
    let dirs = (t.path().join("c"), t.path().join("state"));
    let mut app = App::open(
        Store::at(dirs.0.clone(), dirs.1.clone()),
        Some("demo".into()),
        (80, 24),
    )
    .unwrap();
    let file = t.path().join("c/demo.clidraw.json");
    let t0 = Instant::now();

    app.on_event(key(KeyCode::Char('r')), t0);
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 20, 5), t0);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 30, 9), t0);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 30, 9), t0);

    let cli = Store::at(dirs.0.clone(), dirs.1.clone());
    let mut canvas = cli.load("demo").unwrap();
    canvas.elements.push(Element::Shape(Shape {
        id: Id::from("eeeeeeee"),
        kind: ShapeKind::Rect,
        x: 1,
        y: 1,
        w: 8,
        h: 3,
        label: "Cli".into(),
        stroke: Default::default(),
        border: Default::default(),
        color: None,
        fill: None,
        align: Default::default(),
        bold: false,
        link: None,
        side_labels: vec![],
        turn: 0,
        meta: Default::default(),
    }));
    cli.save("demo", &canvas).unwrap();
    let external = std::fs::read(&file).unwrap();

    app.tick(t0);
    app.tick(t0 + Duration::from_millis(700));

    assert_eq!(std::fs::read(&file).unwrap(), external);
    let ids: Vec<_> = app.session.canvas.elements.iter().map(|e| e.id()).collect();
    assert_eq!(ids, [&Id::from("eeeeeeee")]);
    let status = app.status.as_ref().unwrap();
    assert!(
        status.text.contains("Reloaded the canvas from disk"),
        "{}",
        status.text
    );
    app.on_event(key(KeyCode::Char('u')), t0);
    assert_eq!(app.session.canvas.elements.len(), 1, "history is cleared");
}
