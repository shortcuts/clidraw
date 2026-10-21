use clidraw::app::App;
use clidraw::config::Config;
use clidraw::model::{Anchor, Command, Element, End, Id, Line, LineKind, Shape, ShapeKind, Text};
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

    app.on_event(key(KeyCode::Char('r')), t0);
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
                align: Default::default(),
                bold: false,
                link: None,
            }),
        ));
    }
    let t = Instant::now();
    app.on_event(key(KeyCode::Esc), t);
    click(&mut app, 31, 11, t);
    click(&mut app, 31, 11, t);
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
fn exports_only_the_selection() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(key(KeyCode::Esc), now);
    app.on_event(key(KeyCode::Esc), now);
    app.session.selection = vec![Id::from("bbbbbbbb")];
    click(&mut app, 31, 11, now);
    click(&mut app, 31, 12, now);
    let out = t.path().join("sel.svg");
    type_path(&mut app, out.to_str().unwrap(), now);
    let svg = std::fs::read_to_string(out).unwrap();
    assert!(svg.contains("Beta"), "{svg}");
    assert!(!svg.contains("Alpha") && !svg.contains("Gamma"), "{svg}");
}

#[test]
fn an_empty_selection_opens_no_prompt() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(key(KeyCode::Esc), now);
    app.on_event(key(KeyCode::Esc), now);
    click(&mut app, 31, 11, now);
    click(&mut app, 31, 12, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("nothing selected")
    );
}

#[test]
fn export_padding_comes_from_the_config() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let cfg = Config {
        export_padding: 0,
        ..Config::default()
    };
    let mut app = App::open_with(store, Some("demo".into()), (80, 24), cfg, vec![]).unwrap();
    app.session.exec(Command::Insert(
        0,
        Element::Text(Text {
            id: Id::from("aaaaaaaa"),
            x: 0,
            y: 0,
            text: "hi".into(),
            color: None,
        }),
    ));
    let now = Instant::now();
    app.on_event(key(KeyCode::Esc), now);
    click(&mut app, 31, 11, now);
    click(&mut app, 31, 10, now);
    let out = t.path().join("pad.svg");
    type_path(&mut app, out.to_str().unwrap(), now);
    let svg = std::fs::read_to_string(out).unwrap();
    assert!(svg.contains("viewBox=\"0 0 "), "{svg}");
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
    app.on_event(key(KeyCode::Char('r')), t0);
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
            align: Default::default(),
            bold: false,
            link: None,
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
            align: Default::default(),
            bold: false,
            link: None,
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

fn labeled(id: &str, x: i32, label: &str) -> Command {
    Command::Insert(
        0,
        Element::Text(Text {
            id: Id::from(id),
            x,
            y: 0,
            text: label.into(),
            color: None,
        }),
    )
}

#[test]
fn slash_command_selects_matches_in_turn_and_centers_the_view() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    app.session.exec(labeled("aaaaaaaa", 500, "Auth service"));
    app.session.exec(labeled("bbbbbbbb", 0, "db"));
    app.session.exec(labeled("cccccccc", 900, "auth DB"));
    run(&mut app, "/auth");
    assert_eq!(app.session.selection, vec![Id::from("cccccccc")]);
    run(&mut app, "/");
    assert_eq!(app.session.selection, vec![Id::from("aaaaaaaa")]);
    let t0 = Instant::now();
    app.tick(t0);
    app.tick(t0 + Duration::from_millis(120));
    let (sx, _) = app.session.view.to_screen(506, 0);
    assert_eq!(sx, i32::from(app.session.screen.0) / 2);
    run(&mut app, "/zzz");
    assert!(app.status.is_some());
}

fn last_row(app: &App) -> String {
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    app.render(&mut buf, area);
    (0..80).map(|x| buf[(x, 23)].symbol().to_string()).collect()
}

#[test]
fn info_bar_shows_unsaved_until_the_save() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    assert!(last_row(&app).ends_with("saved"));
    app.session.exec(Command::Insert(
        0,
        Element::Shape(Shape {
            id: "aaaaaaaa".into(),
            kind: ShapeKind::Rect,
            x: 1,
            y: 1,
            w: 5,
            h: 3,
            label: String::new(),
            stroke: Default::default(),
            border: Default::default(),
            color: None,
            fill: None,
            align: Default::default(),
            bold: false,
            link: None,
        }),
    ));
    assert!(last_row(&app).ends_with("unsaved"), "{}", last_row(&app));
    app.save();
    let r = last_row(&app);
    assert!(r.ends_with("saved") && !r.ends_with("unsaved"), "{r}");
}

#[test]
fn palette_hover_redraws_once_and_underlines_the_cell() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let (c, r) = (0..80u16)
        .flat_map(|c| (0..24u16).map(move |r| (c, r)))
        .find(|&(c, r)| {
            clidraw::ui::palette::hit(&app.session, c.into(), r.into())
                == Some(clidraw::ui::palette::PaletteHit::Tool(
                    clidraw::input::Tool::Rect,
                ))
        })
        .unwrap();
    let t0 = Instant::now();
    app.dirty = false;
    app.on_events(vec![mouse(MouseEventKind::Moved, c, r)], t0);
    assert!(app.dirty, "entering a cell redraws");
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    app.render(&mut buf, area);
    assert!(buf[(c, r)].modifier.contains(Modifier::UNDERLINED));
    assert!(!buf[(c, r + 1)].modifier.contains(Modifier::UNDERLINED));
    app.dirty = false;
    app.on_events(vec![mouse(MouseEventKind::Moved, c, r)], t0);
    assert!(!app.dirty, "same cell stays clean");
}

#[test]
fn palette_tooltip_names_the_tool_and_key_after_a_rest() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let (c, r) = (0..80u16)
        .flat_map(|c| (0..24u16).map(move |r| (c, r)))
        .find(|&(c, r)| {
            clidraw::ui::palette::hit(&app.session, c.into(), r.into())
                == Some(clidraw::ui::palette::PaletteHit::Tool(
                    clidraw::input::Tool::Rect,
                ))
        })
        .unwrap();
    let area = Rect::new(0, 0, 80, 24);
    let row = |app: &App| {
        let mut buf = Buffer::empty(area);
        app.render(&mut buf, area);
        (0..80u16)
            .map(|x| buf[(x, r + 1)].symbol())
            .collect::<String>()
    };
    let t0 = Instant::now();
    app.on_events(vec![mouse(MouseEventKind::Moved, c, r)], t0);
    assert_eq!(app.next_wake(t0), Some(Duration::from_millis(400)));
    app.dirty = false;
    app.tick(t0 + Duration::from_millis(399));
    assert!(!app.dirty);
    assert!(!row(&app).contains("Rect  r"));
    app.tick(t0 + Duration::from_millis(400));
    assert!(app.dirty, "the tooltip appears");
    assert!(row(&app).contains(" Rect  r "));
    app.on_events(vec![mouse(MouseEventKind::Moved, 70, 20)], t0);
    assert!(!row(&app).contains("Rect  r"));
}

#[test]
fn palette_tooltip_names_an_order_cell_and_its_key_after_a_rest() {
    use clidraw::ui::palette::{hit, Choice, PaletteHit};
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    app.session.exec(shape_at("aaaaaaaa", 20, 8));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    let (c, r) = (0..80u16)
        .flat_map(|c| (0..24u16).map(move |r| (c, r)))
        .find(|&(c, r)| {
            hit(&app.session, c.into(), r.into())
                == Some(PaletteHit::Option(Choice::Order(clidraw::model::Z::Front)))
        })
        .unwrap();
    let area = Rect::new(0, 0, 80, 24);
    let t0 = Instant::now();
    app.on_events(vec![mouse(MouseEventKind::Moved, c, r)], t0);
    app.tick(t0 + Duration::from_millis(400));
    let mut buf = Buffer::empty(area);
    app.render(&mut buf, area);
    let row: String = (0..80u16).map(|x| buf[(x, r + 1)].symbol()).collect();
    assert!(row.contains(" front  } "), "{row}");
}

fn shape_at(id: &str, x: i32, y: i32) -> Command {
    Command::Insert(
        0,
        Element::Shape(Shape {
            id: Id::from(id),
            kind: ShapeKind::Rect,
            x,
            y,
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
        }),
    )
}

fn right_click(app: &mut App, col: u16, row: u16, t: Instant) {
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Right), col, row), t);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Right), col, row), t);
}

fn context_open(app: &App) -> bool {
    matches!(app.overlay, clidraw::ui::Overlay::Context(_))
}

fn shape_ids(app: &App) -> Vec<String> {
    app.session
        .canvas
        .elements
        .iter()
        .map(|e| e.id().to_string())
        .collect()
}

#[test]
fn right_click_menu_runs_element_entries() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    right_click(&mut app, 22, 6, now);
    assert!(context_open(&app));
    assert_eq!(app.session.selection, vec![Id::from("aaaaaaaa")]);
    click(&mut app, 24, 8, now);
    assert!(!context_open(&app));
    assert_eq!(app.session.canvas.elements.len(), 2);
    app.on_event(
        Event::Key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL)),
        now,
    );
    assert_eq!(app.session.canvas.elements.len(), 1);

    right_click(&mut app, 22, 6, now);
    click(&mut app, 24, 9, now);
    assert!(app.session.canvas.elements.is_empty());
}

#[test]
fn right_click_menu_brings_a_shape_to_front() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("bbbbbbbb", 40, 5));
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    assert_eq!(shape_ids(&app), ["aaaaaaaa", "bbbbbbbb"]);
    right_click(&mut app, 22, 6, now);
    click(&mut app, 24, 10, now);
    assert_eq!(shape_ids(&app), ["bbbbbbbb", "aaaaaaaa"]);
}

#[test]
fn right_click_on_canvas_offers_paste_only_with_a_clipboard() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    right_click(&mut app, 60, 15, now);
    let clidraw::ui::Overlay::Context(c) = &app.overlay else {
        panic!("no menu")
    };
    assert_eq!(c.labels(), ["Select all", "Fit all"]);

    app.on_event(key(KeyCode::Esc), now);
    assert!(!context_open(&app));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    app.session.copy();
    right_click(&mut app, 60, 15, now);
    let clidraw::ui::Overlay::Context(c) = &app.overlay else {
        panic!("no menu")
    };
    assert_eq!(c.labels(), ["Paste", "Select all", "Fit all"]);
    click(&mut app, 62, 16, now);
    assert_eq!(app.session.canvas.elements.len(), 2);
}

#[test]
fn right_click_menu_selects_all() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.exec(shape_at("bbbbbbbb", 40, 5));
    right_click(&mut app, 60, 15, now);
    click(&mut app, 62, 15 + 1, now);
    assert_eq!(app.session.selection.len(), 2);
}

#[test]
fn right_click_menu_closes_and_moves() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    right_click(&mut app, 22, 6, now);
    click(&mut app, 70, 20, now);
    assert!(!context_open(&app));
    right_click(&mut app, 22, 6, now);
    right_click(&mut app, 60, 15, now);
    let clidraw::ui::Overlay::Context(c) = &app.overlay else {
        panic!("no menu")
    };
    assert_eq!(c.at, (60, 15));
}

#[test]
fn right_click_while_editing_opens_nothing() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    click(&mut app, 22, 6, now);
    click(&mut app, 22, 6, now);
    assert!(app.session.edit.is_some());
    right_click(&mut app, 22, 6, now);
    assert!(!context_open(&app));
}

fn ctrl_key(app: &mut App, c: char, t: Instant) {
    app.on_event(
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)),
        t,
    );
}

fn type_str(app: &mut App, s: &str, t: Instant) {
    for c in s.chars() {
        app.on_event(key(KeyCode::Char(c)), t);
    }
}

fn palette_labels(app: &App) -> Vec<String> {
    match &app.overlay {
        clidraw::ui::Overlay::Actions(a) => a.labels().iter().map(|l| l.to_string()).collect(),
        _ => panic!("palette is not open"),
    }
}

fn palette_open(app: &App) -> bool {
    matches!(app.overlay, clidraw::ui::Overlay::Actions(_))
}

#[test]
fn palette_lists_only_actions_that_can_act() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    let l = palette_labels(&app);
    assert_eq!(l[0], "Undo");
    assert!(l.contains(&"Tool: Rect".to_string()));
    for gone in ["Duplicate", "Align left", "Paste", "Edit label"] {
        assert!(!l.contains(&gone.to_string()), "{gone}");
    }
    app.on_event(key(KeyCode::Esc), now);
    assert!(!palette_open(&app));
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    ctrl_key(&mut app, 'p', now);
    let l = palette_labels(&app);
    for here in ["Duplicate", "Align left", "Edit label"] {
        assert!(l.contains(&here.to_string()), "{here}");
    }
    assert!(!l.contains(&"Paste".to_string()));
    app.on_event(key(KeyCode::Esc), now);
    app.session.copy();
    ctrl_key(&mut app, 'p', now);
    assert!(palette_labels(&app).contains(&"Paste".to_string()));
}

#[test]
fn palette_filters_fuzzily_and_backspace_widens() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "dup", now);
    assert_eq!(palette_labels(&app)[0], "Duplicate");
    app.on_event(key(KeyCode::Esc), now);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "exsvg", now);
    assert_eq!(palette_labels(&app)[0], "Export SVG");
    type_str(&mut app, "zzz", now);
    assert!(palette_labels(&app).is_empty());
    app.on_event(key(KeyCode::Enter), now);
    assert!(palette_open(&app));
    for _ in 0..3 {
        app.on_event(key(KeyCode::Backspace), now);
    }
    assert_eq!(palette_labels(&app)[0], "Export SVG");
}

#[test]
fn palette_enter_runs_the_entry() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "rect", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(!palette_open(&app));
    assert_eq!(app.session.tool, clidraw::input::Tool::Rect);

    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "delete", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(app.session.canvas.elements.is_empty());
    ctrl_key(&mut app, 'z', now);
    assert_eq!(app.session.canvas.elements.len(), 1);

    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "export svg", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Scope(_)));
    app.on_event(key(KeyCode::Esc), now);

    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "perf", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(app.perf.is_some());

    ctrl_key(&mut app, 'p', now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(!palette_open(&app));
    assert!(app.session.canvas.elements.is_empty());
}

#[test]
fn palette_clicks_run_rows_and_outside_closes() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    let third = palette_labels(&app)[2].clone();
    assert_eq!(third, "Select all");
    // 80x24 box is 13 rows high at y=5; rows start at y+2.
    click(&mut app, 40, 5 + 2 + 2, now);
    assert!(!palette_open(&app));
    ctrl_key(&mut app, 'p', now);
    click(&mut app, 1, 1, now);
    assert!(!palette_open(&app));
}

#[test]
fn palette_does_not_open_during_an_edit_and_ctrl_c_quits() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.start_edit_existing(Id::from("aaaaaaaa"));
    ctrl_key(&mut app, 'p', now);
    assert!(!palette_open(&app));
    assert!(app.session.edit.is_some());
    app.on_event(key(KeyCode::Esc), now);
    ctrl_key(&mut app, 'p', now);
    ctrl_key(&mut app, 'c', now);
    assert!(app.quit);
}

fn layout_app(dir: &std::path::Path, edges: &[(&str, &str)]) -> App {
    let mut app = app_in(dir);
    for (i, (id, x, y)) in [("a", 30, 20), ("b", 5, 2), ("c", 50, 9)]
        .into_iter()
        .enumerate()
    {
        app.session.exec(Command::Insert(
            i,
            Element::Shape(Shape {
                id: Id::from(id),
                kind: ShapeKind::Rect,
                x,
                y,
                w: 6,
                h: 3,
                label: String::new(),
                stroke: Default::default(),
                border: Default::default(),
                color: None,
                fill: None,
                align: Default::default(),
                bold: false,
                link: None,
            }),
        ));
    }
    for (i, (from, to)) in edges.iter().enumerate() {
        app.session.exec(Command::Insert(
            3 + i,
            Element::Line(Line {
                id: Id::from(format!("l{i}").as_str()),
                kind: LineKind::Arrow,
                from: End::Bound(Id::from(*from), Anchor::Auto),
                to: End::Bound(Id::from(*to), Anchor::Auto),
                label: String::new(),
                label_at: None,
                stroke: Default::default(),
                routing: Default::default(),
                color: None,
            }),
        ));
    }
    app.session.selection = ["a", "b", "c"].map(Id::from).to_vec();
    app
}

fn at(app: &App, id: &str) -> (i32, i32) {
    let s = app.session.canvas.shape(&Id::from(id)).unwrap();
    (s.x, s.y)
}

#[test]
fn layout_puts_children_below_their_parent() {
    let t = tempfile::tempdir().unwrap();
    let mut app = layout_app(t.path(), &[("a", "b"), ("a", "c")]);
    run(&mut app, "layout");
    let (a, b, c) = (at(&app, "a"), at(&app, "b"), at(&app, "c"));
    assert!(a.1 < b.1 && b.1 == c.1 && b.0 < c.0, "{a:?} {b:?} {c:?}");
}

#[test]
fn layout_lr_puts_children_right_of_their_parent() {
    let t = tempfile::tempdir().unwrap();
    let mut app = layout_app(t.path(), &[("a", "b"), ("a", "c")]);
    run(&mut app, "layout lr");
    let (a, b, c) = (at(&app, "a"), at(&app, "b"), at(&app, "c"));
    assert!(a.0 < b.0 && b.0 == c.0, "{a:?} {b:?} {c:?}");
}

#[test]
fn layout_survives_a_cycle() {
    let t = tempfile::tempdir().unwrap();
    let mut app = layout_app(t.path(), &[("a", "b"), ("b", "a")]);
    app.session.selection = ["a", "b"].map(Id::from).to_vec();
    app.session.exec(Command::Move {
        id: Id::from("b"),
        dx: 25,
        dy: 18,
    });
    assert_eq!(at(&app, "a").1, at(&app, "b").1);
    run(&mut app, "layout");
    assert_ne!(at(&app, "a").1, at(&app, "b").1);
}

#[test]
fn layout_undoes_in_one_step_and_rejects_bad_input() {
    let t = tempfile::tempdir().unwrap();
    let mut app = layout_app(t.path(), &[("a", "b"), ("a", "c")]);
    let before = [at(&app, "a"), at(&app, "b"), at(&app, "c")];
    run(&mut app, "layout x");
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Usage: :layout [td|lr]")
    );
    assert_eq!([at(&app, "a"), at(&app, "b"), at(&app, "c")], before);
    app.session.selection = vec![Id::from("a")];
    run(&mut app, "layout");
    assert_eq!([at(&app, "a"), at(&app, "b"), at(&app, "c")], before);
    app.session.selection = ["a", "b", "c"].map(Id::from).to_vec();
    run(&mut app, "layout");
    assert_ne!(at(&app, "b"), before[1]);
    run(&mut app, "u");
    assert_eq!([at(&app, "a"), at(&app, "b"), at(&app, "c")], before);
}

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
    let (wx, wy) = app.session.view.to_world(2, 1);
    assert_eq!((frame.x, frame.y), (wx - 1, wy - 1));
    assert!(app
        .status
        .as_ref()
        .unwrap()
        .text
        .starts_with("imported 3 shapes, 2 lines"));
    app.on_event(
        Event::Key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL)),
        Instant::now(),
    );
    assert!(app.session.canvas.elements.is_empty());
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

fn status(app: &App) -> &str {
    app.status.as_ref().map_or("", |s| s.text.as_str())
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
    assert_eq!(status(&app), "saved pair to the library");
    assert!(t.path().join("c/library/pair.clidraw.json").exists());

    run(&mut app, "e other");
    run(&mut app, "lib insert pair");
    assert_eq!(status(&app), "inserted pair");
    assert_eq!(app.session.canvas.elements.len(), 4);
    assert_eq!(app.session.selection.len(), 4);
    let (wx, wy) = app.session.view.to_world(2, 1);
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
fn lib_errors_write_nothing() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    run(&mut app, "lib");
    assert_eq!(status(&app), "Usage: :lib save|insert <name>");
    run(&mut app, "lib save");
    assert_eq!(status(&app), "Usage: :lib save|insert <name>");
    run(&mut app, "lib save box");
    assert_eq!(status(&app), "Select shapes to save first");
    assert!(!t.path().join("c/library").exists());
    insert_shape(&mut app);
    app.session.selection = vec![Id::from("aaaaaaaa")];
    run(&mut app, "lib save a/b");
    assert!(status(&app).starts_with("lib failed"));
    run(&mut app, "lib insert nope");
    assert!(status(&app).starts_with("lib failed"));
}

#[test]
fn import_of_a_missing_file_shows_an_error() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    run(&mut app, "import /no/such.mmd");
    assert!(app.status.unwrap().text.starts_with("import failed"));
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
    std::fs::write(&file, text.replace("\"x\": 2", "\"x\": 9")).unwrap();
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
    click(&mut app, 9, 0, t0);
    assert_eq!(app.take_pointer(), Some("grab"));
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 30, 10), t0);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 34, 10), t0);
    assert_eq!(app.take_pointer(), Some("grabbing"));
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 34, 10), t0);
    app.on_event(key(KeyCode::Esc), t0);
    assert_eq!(app.take_pointer(), Some("default"));
}
