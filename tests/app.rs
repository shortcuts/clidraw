use clidraw::app::App;
use clidraw::config::Config;
use clidraw::model::{Command, Element, Id, Shape, ShapeKind};
use clidraw::store::Store;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
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
fn sleeps_until_the_autosave_deadline_only_while_an_edit_is_unsaved() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let mut app = App::open(store, Some("demo".into()), (80, 24)).unwrap();
    let t0 = Instant::now();
    assert_eq!(app.next_wake(t0), None, "idle must block");

    click(&mut app, 2, 3, t0);
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
    click(&mut app, 2, 3, t0);
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
                stroke: Default::default(),
                border: Default::default(),
                color: None,
                fill: None,
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

fn status_row_reversed(app: &App) -> bool {
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    app.render(&mut buf, area);
    buf[(0, 23)].modifier.contains(Modifier::REVERSED)
}

#[test]
fn notice_status_is_plain_and_error_status_is_reversed() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 0, 0), now);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 40, 8), now);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 40, 8), now);
    type_path(&mut app, t.path().join("area.svg").to_str().unwrap(), now);
    assert!(app
        .status
        .as_ref()
        .map(|s| s.text.as_str())
        .unwrap()
        .starts_with("exported"));
    assert!(!status_row_reversed(&app), "a notice is not reversed");

    run(&mut app, "nope");
    assert!(status_row_reversed(&app), "an error is reversed");
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
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("nothing inside the area")
    );
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

fn app_in(dir: &std::path::Path) -> App {
    let store = Store::at(dir.join("c"), dir.join("state"));
    App::open(store, Some("demo".into()), (80, 24)).unwrap()
}

fn help_page(app: &App) -> usize {
    match app.overlay {
        clidraw::ui::Overlay::Help(p) => p,
        _ => panic!("help is not open"),
    }
}

#[test]
fn question_mark_opens_help_and_right_left_page_clamped() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Char('?')), now);
    assert_eq!(help_page(&app), 0);
    app.on_event(key(KeyCode::Left), now);
    assert_eq!(help_page(&app), 0);
    for _ in 0..3 {
        app.on_event(key(KeyCode::Right), now);
    }
    assert_eq!(help_page(&app), 3);
    app.on_event(key(KeyCode::Left), now);
    assert_eq!(help_page(&app), 2);
}

#[test]
fn help_closes_on_esc_question_mark_and_outside_click() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    for close in [key(KeyCode::Esc), key(KeyCode::Char('?'))] {
        app.on_event(key(KeyCode::Char('?')), now);
        app.on_event(close, now);
        assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    }
    app.on_event(key(KeyCode::Char('?')), now);
    click(&mut app, 0, 0, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn clicking_a_help_tab_selects_its_page() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Char('?')), now);
    click(&mut app, 38, 2, now);
    assert_eq!(help_page(&app), 2);
    click(&mut app, 22, 2, now);
    assert_eq!(help_page(&app), 0);
    click(&mut app, 22, 12, now);
    assert_eq!(help_page(&app), 0, "a click off the tabs keeps the modal");
}

#[test]
fn question_mark_types_into_a_label_and_a_prompt() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    click(&mut app, 30, 10, now);
    click(&mut app, 30, 10, now + Duration::from_millis(50));
    app.on_event(key(KeyCode::Char('?')), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert_eq!(app.session.edit.as_ref().unwrap().buf, "?");
    app.on_event(key(KeyCode::Esc), now);
    app.on_event(key(KeyCode::Esc), now);
    app.on_event(key(KeyCode::Char('c')), now);
    click(&mut app, 22, 11, now);
    for _ in 0..40 {
        app.on_event(key(KeyCode::Backspace), now);
    }
    app.on_event(key(KeyCode::Char('?')), now);
    let clidraw::ui::Overlay::Prompt(p) = &app.overlay else {
        panic!("expected a prompt, got another overlay")
    };
    assert_eq!(p.buf, "?");
}

#[test]
fn new_canvas_prompt_is_prefilled_with_a_generated_name() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Esc), now);
    app.on_event(key(KeyCode::Char('c')), now);
    click(&mut app, 25, 11, now);
    let clidraw::ui::Overlay::Prompt(p) = &app.overlay else {
        panic!("no prompt");
    };
    let name = p.buf.clone();
    let (adj, noun) = name.split_once('-').expect("adjective-noun");
    assert!(!adj.is_empty() && !noun.is_empty() && name != "demo");
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.name, name);
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
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 2, 3), t0);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 2, 3), t0);
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 20, 5), t0);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 30, 9), t0);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 30, 9), t0);
    app.tick(t0);
    app.tick(t0 + Duration::from_millis(150));
    assert_ne!(std::fs::read(&file).unwrap(), empty);
}

fn run(app: &mut App, line: &str) {
    let t = Instant::now();
    app.on_event(key(KeyCode::Char(':')), t);
    for c in line.chars() {
        app.on_event(key(KeyCode::Char(c)), t);
    }
    app.on_event(key(KeyCode::Enter), t);
}

fn insert_shape(app: &mut App) {
    app.session.exec(Command::Insert(
        0,
        Element::Shape(Shape {
            id: Id::from("aaaaaaaa"),
            kind: ShapeKind::Rect,
            x: 2,
            y: 2,
            w: 8,
            h: 3,
            label: String::new(),
            stroke: Default::default(),
            border: Default::default(),
            color: None,
            fill: None,
        }),
    ));
}

#[test]
fn colon_q_quits_and_w_saves_without_waiting() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let file = t.path().join("c/demo.clidraw.json");
    let empty = std::fs::read(&file).unwrap();
    insert_shape(&mut app);
    run(&mut app, "w");
    assert_ne!(std::fs::read(&file).unwrap(), empty);
    assert!(!app.quit);
    run(&mut app, "q");
    assert!(app.quit);
}

#[test]
fn colon_wq_saves_and_quits() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let file = t.path().join("c/demo.clidraw.json");
    let empty = std::fs::read(&file).unwrap();
    insert_shape(&mut app);
    run(&mut app, "wq");
    assert_ne!(std::fs::read(&file).unwrap(), empty);
    assert!(app.quit);
}

#[test]
fn colon_e_opens_or_creates_a_canvas_and_needs_a_name() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    run(&mut app, "e other");
    assert_eq!(app.name, "other");
    assert!(t.path().join("c/other.clidraw.json").exists());
    run(&mut app, "e");
    assert!(app
        .status
        .as_ref()
        .map(|s| s.text.as_str())
        .unwrap()
        .contains("Usage"));
    assert_eq!(app.name, "other");
}

#[test]
fn colon_u_and_redo_undo_and_redo() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    insert_shape(&mut app);
    run(&mut app, "u");
    assert_eq!(app.session.canvas.elements.len(), 0);
    run(&mut app, "redo");
    assert_eq!(app.session.canvas.elements.len(), 1);
}

#[test]
fn undo_and_redo_name_the_action_in_the_status_line() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    insert_shape(&mut app);
    let now = Instant::now();
    let ctrl = |c| Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
    let status = |app: &App| app.status.as_ref().map(|s| s.text.clone());
    app.on_event(ctrl('z'), now);
    assert_eq!(status(&app).as_deref(), Some("Undid: add 1 element"));
    app.on_event(ctrl('y'), now);
    assert_eq!(status(&app).as_deref(), Some("Redid: add 1 element"));
    run(&mut app, "u");
    assert_eq!(status(&app).as_deref(), Some("Undid: add 1 element"));
    app.on_event(ctrl('z'), now);
    assert_eq!(status(&app), None, "nothing to undo");
}

#[test]
fn unknown_command_sets_status_and_esc_cancels_the_prompt() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    run(&mut app, "nope");
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Unknown command: nope")
    );
    assert!(!app.quit);
    let now = Instant::now();
    app.on_event(key(KeyCode::Char(':')), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Prompt(_)));
    app.on_event(key(KeyCode::Char('q')), now);
    app.on_event(key(KeyCode::Esc), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert!(!app.quit);
}

#[test]
fn help_lists_the_command_line() {
    assert!(clidraw::ui::help::rows(0).iter().any(|(k, _)| k == ":"));
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
    app.on_events(vec![mouse(MouseEventKind::Moved, 5, 5)], t0);
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
    click(&mut app, 2, 3, t0);
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
        }));
    let t0 = Instant::now();
    app.dirty = false;
    app.on_events(vec![mouse(MouseEventKind::Moved, 22, 5)], t0);
    assert!(app.dirty, "entering the shape");
    app.dirty = false;
    app.on_events(vec![mouse(MouseEventKind::Moved, 23, 5)], t0);
    assert!(!app.dirty, "same target");
    app.on_events(
        vec![
            mouse(MouseEventKind::Moved, 22, 5),
            mouse(MouseEventKind::Moved, 60, 15),
        ],
        t0,
    );
    assert!(app.dirty, "burst ends off the shape");
    assert_eq!(app.session.hover, clidraw::input::Hit::Empty);
}
