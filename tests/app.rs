use clidraw::app::App;
use clidraw::config::Config;
use clidraw::model::{
    Anchor, Border, Command, Element, End, Id, Line, LineKind, Paint, Shape, ShapeKind, Stroke,
    Text, TextAlign,
};
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

/// Two shapes near the origin and one far away; opens the palette's Export SVG, Area.
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
                side_labels: vec![],
            }),
        ));
    }
    let t = Instant::now();
    open_export(&mut app, t);
    click(&mut app, 34, 11, t);
    (app, t)
}

fn type_path(app: &mut App, path: &str, t: Instant) {
    for _ in 0..32 {
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
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 0, 2), now);
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
    app.session.selection = vec![Id::from("bbbbbbbb")];
    open_export(&mut app, now);
    click(&mut app, 34, 12, now);
    let out = t.path().join("sel.svg");
    type_path(&mut app, out.to_str().unwrap(), now);
    let svg = std::fs::read_to_string(out).unwrap();
    assert!(svg.contains("Beta"), "{svg}");
    assert!(!svg.contains("Alpha") && !svg.contains("Gamma"), "{svg}");
}

#[test]
fn exports_the_selection_as_a_loadable_clidraw_file() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(key(KeyCode::Esc), now);
    app.session.selection = vec![Id::from("bbbbbbbb")];
    run(&mut app, "export clidraw");
    click(&mut app, 34, 12, now);
    let out = t.path().join("sel.clidraw.json");
    type_path(&mut app, out.to_str().unwrap(), now);
    let text = std::fs::read_to_string(out).unwrap();
    let canvas: clidraw::model::Canvas = serde_json::from_str(&text).unwrap();
    assert_eq!(canvas.elements.len(), 1);
    assert!(text.contains("Beta") && !text.contains("Alpha"), "{text}");
}

#[test]
fn an_empty_selection_opens_no_prompt() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(key(KeyCode::Esc), now);
    open_export(&mut app, now);
    click(&mut app, 34, 12, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Nothing selected: select a shape first")
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
    open_export(&mut app, now);
    click(&mut app, 34, 10, now);
    let out = t.path().join("pad.svg");
    type_path(&mut app, out.to_str().unwrap(), now);
    let svg = std::fs::read_to_string(out).unwrap();
    assert!(svg.contains("viewBox=\"0 0 "), "{svg}");
}

#[test]
fn chrome_accent_survives_opening_a_canvas() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let cfg = Config {
        chrome_accent: Some(ratatui::style::Color::Rgb(1, 2, 3)),
        ..Config::default()
    };
    let app = App::open_with(store, Some("demo".into()), (80, 24), cfg, vec![]).unwrap();
    assert_eq!(
        app.session.chrome_accent,
        Some(ratatui::style::Color::Rgb(1, 2, 3))
    );
}

fn status_row_reversed(app: &App) -> bool {
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    app.render(&mut buf, area);
    buf[(38, 23)].modifier.contains(Modifier::REVERSED)
}

#[test]
fn notice_status_is_plain_and_error_status_is_reversed() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 0, 2), now);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 40, 8), now);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 40, 8), now);
    type_path(&mut app, t.path().join("area.svg").to_str().unwrap(), now);
    assert!(app
        .status
        .as_ref()
        .map(|s| s.text.as_str())
        .unwrap()
        .starts_with("Exported"));
    assert!(!status_row_reversed(&app), "a notice is not reversed");

    run(&mut app, "nope");
    assert!(status_row_reversed(&app), "an error is reversed");
}

#[test]
fn esc_cancels_the_area_pick() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 0, 2), now);
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
        Some("Nothing inside the area: drag over a shape")
    );
}

#[test]
fn esc_on_an_idle_canvas_opens_nothing() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    app.on_event(key(KeyCode::Esc), Instant::now());
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

fn app_in(dir: &std::path::Path) -> App {
    let store = Store::at(dir.join("c"), dir.join("state"));
    App::open(store, Some("demo".into()), (80, 24)).unwrap()
}

fn help_page(app: &App) -> usize {
    match app.overlay {
        clidraw::ui::Overlay::Help(p, _) => p,
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
    for _ in 0..9 {
        app.on_event(key(KeyCode::Right), now);
    }
    assert_eq!(help_page(&app), 5);
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
    click(&mut app, 33, 2, now);
    assert_eq!(help_page(&app), 2);
    click(&mut app, 17, 2, now);
    assert_eq!(help_page(&app), 0);
    click(&mut app, 17, 12, now);
    assert_eq!(help_page(&app), 0, "a click off the tabs keeps the modal");
}

fn map_app(dir: &std::path::Path) -> (App, Instant) {
    let mut app = app_in(dir);
    for (id, x) in [("aaaaaaaa", 2), ("bbbbbbbb", 1000)] {
        app.session.exec(Command::Insert(
            app.session.canvas.elements.len(),
            Element::Shape(Shape {
                id: Id::from(id),
                kind: ShapeKind::Rect,
                x,
                y: 4,
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
            }),
        ));
    }
    app.session.motion = false;
    let t = Instant::now();
    ctrl_key(&mut app, 'p', t);
    type_str(&mut app, "mini", t);
    app.on_event(key(KeyCode::Enter), t);
    (app, t)
}

#[test]
fn minimap_action_opens_and_a_click_jumps() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = map_app(t.path());
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Map(_)));
    click(&mut app, 78, 5, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert!(
        app.session.view.ox > 900,
        "view stayed at {}",
        app.session.view.ox
    );
    assert!(app.session.selection.is_empty());
}

#[test]
fn clicking_a_shape_in_the_selection_overview_fits_the_view_to_it() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = map_app(t.path());
    app.session.selection.push(Id::from("bbbbbbbb"));
    app.on_event(key(KeyCode::Tab), now);
    click(&mut app, 40, 11, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert!(
        (900..1010).contains(&app.session.view.ox),
        "view at {}",
        app.session.view.ox
    );
}

#[test]
fn tab_cycles_the_overview_target_from_the_canvas() {
    use clidraw::ui::{minimap::Target, Overlay};
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = map_app(t.path());
    let mut seen = vec![];
    for _ in 0..4 {
        match app.overlay {
            Overlay::Map(t) => seen.push(t),
            _ => panic!("overview closed"),
        }
        app.on_event(key(KeyCode::Tab), now);
    }
    assert_eq!(
        seen,
        [
            Target::Canvas,
            Target::Selection,
            Target::Frame,
            Target::Canvas
        ]
    );
}

#[test]
fn escape_closes_the_minimap_without_moving_the_view() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = map_app(t.path());
    app.on_event(key(KeyCode::Esc), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert_eq!(app.session.view.ox, 0);
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
    open_canvases(&mut app, now);
    click(&mut app, 22, 11, now);
    for _ in 0..40 {
        app.on_event(key(KeyCode::Backspace), now);
    }
    app.on_event(key(KeyCode::Char('?')), now);
    let clidraw::ui::Overlay::Prompt(p) = &app.overlay else {
        panic!("expected a prompt, got another overlay")
    };
    assert_eq!(p.buf(), "?");
}

#[test]
fn new_canvas_prompt_is_prefilled_with_a_generated_name() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_canvases(&mut app, now);
    click(&mut app, 25, 11, now);
    let clidraw::ui::Overlay::Prompt(p) = &app.overlay else {
        panic!("no prompt");
    };
    let name = p.buf().to_string();
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
            side_labels: vec![],
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
fn quitting_during_a_text_edit_saves_the_typed_text() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.start_edit_new((5, 5));
    type_str(&mut app, "hello", now);
    ctrl_key(&mut app, 'c', now);
    assert!(app.quit);
    app.finish();
    let saved = std::fs::read_to_string(t.path().join("c/demo.clidraw.json")).unwrap();
    assert!(saved.contains("hello"), "{saved}");
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
fn undo_and_redo_name_the_action_in_the_status_line() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    insert_shape(&mut app);
    let now = Instant::now();
    let ctrl = |c| Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
    let status = |app: &App| app.status.as_ref().map(|s| s.text.clone());
    app.on_event(key(KeyCode::Char('u')), now);
    assert_eq!(status(&app).as_deref(), Some("Undid: add 1 element"));
    app.on_event(ctrl('r'), now);
    assert_eq!(status(&app).as_deref(), Some("Redid: add 1 element"));
    run(&mut app, "u");
    assert_eq!(status(&app).as_deref(), Some("Undid: add 1 element"));
    run(&mut app, "redo");
    assert_eq!(status(&app).as_deref(), Some("Redid: add 1 element"));
    run(&mut app, "u");
    app.on_event(key(KeyCode::Char('u')), now);
    assert_eq!(status(&app), None, "nothing to undo");
}

#[test]
fn unknown_command_sets_status_and_esc_cancels_the_prompt() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    run(&mut app, "nope");
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Unknown command: nope (Ctrl-p lists commands)")
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

fn col_of(app: &App, needle: &str) -> u16 {
    let r = last_row(app);
    r[..r.find(needle).unwrap()].chars().count() as u16
}

#[test]
fn info_bar_shows_unsaved_until_the_save() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    assert!(last_row(&app).ends_with("demo"), "{}", last_row(&app));
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
            side_labels: vec![],
        }),
    ));
    assert!(last_row(&app).ends_with("demo ●"), "{}", last_row(&app));
    app.save();
    let r = last_row(&app);
    assert!(r.ends_with("demo"), "{r}");
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

fn tooltip_row(choice: clidraw::ui::palette::Choice) -> String {
    use clidraw::ui::palette::{hit, PaletteHit};
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    app.session.exec(shape_at("bbbbbbbb", 40, 8));
    app.session.exec(shape_at("aaaaaaaa", 20, 8));
    app.session.selection = vec![Id::from("aaaaaaaa"), Id::from("bbbbbbbb")];
    let at = |app: &clidraw::app::App| {
        (0..80u16)
            .flat_map(|c| (0..24u16).map(move |r| (c, r)))
            .find(|&(c, r)| {
                hit(&app.session, c.into(), r.into()) == Some(PaletteHit::Option(choice))
            })
    };
    while at(&app).is_none() {
        app.session.option_scroll += 4;
    }
    let (c, r) = at(&app).unwrap();
    let area = Rect::new(0, 0, 80, 24);
    let t0 = Instant::now();
    app.on_events(vec![mouse(MouseEventKind::Moved, c, r)], t0);
    app.tick(t0 + Duration::from_millis(400));
    let mut buf = Buffer::empty(area);
    app.render(&mut buf, area);
    (0..80u16).map(|x| buf[(x, r + 1)].symbol()).collect()
}

#[test]
fn palette_tooltip_names_a_keyed_cell_and_its_key_after_a_rest() {
    use clidraw::model::Align;
    use clidraw::ui::palette::Choice;
    let row = tooltip_row(Choice::Place(Align::Left));
    assert!(row.contains(" Arrange: Align left  < "), "{row}");
}

#[test]
fn palette_tooltip_names_a_keyless_option() {
    use clidraw::model::Stroke;
    use clidraw::ui::palette::Choice;
    let row = tooltip_row(Choice::Stroke(Stroke::Dashed));
    assert!(row.contains(" Stroke: dashed "), "{row}");
}

#[test]
fn top_bar_has_no_stacking_order_cells() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    app.session.exec(shape_at("aaaaaaaa", 20, 8));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    app.render(&mut buf, area);
    let text: String = (0..24u16)
        .flat_map(|r| (0..80u16).map(move |c| (c, r)))
        .map(|p| buf[p].symbol())
        .collect();
    for word in ["raise", "lower", "front", "back"] {
        assert!(!text.contains(word), "{word}\n{text}");
    }
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
            side_labels: vec![],
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
    right_click(&mut app, 20, 6, now);
    assert!(context_open(&app));
    assert_eq!(app.session.selection, vec![Id::from("aaaaaaaa")]);
    click(&mut app, 24, 8, now);
    assert!(!context_open(&app));
    assert_eq!(app.session.canvas.elements.len(), 2);
    app.on_event(key(KeyCode::Char('u')), now);
    assert_eq!(app.session.canvas.elements.len(), 1);

    right_click(&mut app, 20, 6, now);
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
    right_click(&mut app, 20, 6, now);
    click(&mut app, 24, 10, now);
    assert_eq!(shape_ids(&app), ["bbbbbbbb", "aaaaaaaa"]);
}

#[test]
fn right_click_menu_raises_and_lowers_a_shape() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("bbbbbbbb", 40, 5));
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    assert_eq!(shape_ids(&app), ["aaaaaaaa", "bbbbbbbb"]);
    right_click(&mut app, 20, 6, now);
    let clidraw::ui::Overlay::Context(menu) = &app.overlay else {
        panic!("no context menu");
    };
    assert!(menu.labels().contains(&"Raise") && menu.labels().contains(&"Lower"));
    click(&mut app, 24, 11, now);
    assert_eq!(shape_ids(&app), ["bbbbbbbb", "aaaaaaaa"]);
    right_click(&mut app, 20, 6, now);
    click(&mut app, 24, 12, now);
    assert_eq!(shape_ids(&app), ["aaaaaaaa", "bbbbbbbb"]);
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
    right_click(&mut app, 20, 6, now);
    click(&mut app, 70, 20, now);
    assert!(!context_open(&app));
    right_click(&mut app, 20, 6, now);
    right_click(&mut app, 60, 15, now);
    let clidraw::ui::Overlay::Context(c) = &app.overlay else {
        panic!("no menu")
    };
    assert_eq!(c.anchor, clidraw::ui::list::Anchor::At(60, 15));
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
    right_click(&mut app, 20, 6, now);
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

fn open_canvases(app: &mut App, t: Instant) {
    ctrl_key(app, 'p', t);
    type_str(app, "switch canvas", t);
    app.on_event(key(KeyCode::Enter), t);
}

fn open_export(app: &mut App, t: Instant) {
    ctrl_key(app, 'p', t);
    type_str(app, "export svg", t);
    app.on_event(key(KeyCode::Enter), t);
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
    assert_eq!(l[0], "Edit: Undo");
    assert!(l.contains(&"Tool: Rect".to_string()));
    for gone in [
        "Edit: Duplicate",
        "Arrange: Align left",
        "Edit: Paste",
        "Edit: Edit label",
    ] {
        assert!(!l.contains(&gone.to_string()), "{gone}");
    }
    app.on_event(key(KeyCode::Esc), now);
    assert!(!palette_open(&app));
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    ctrl_key(&mut app, 'p', now);
    let l = palette_labels(&app);
    for here in ["Edit: Duplicate", "Arrange: Align left", "Edit: Edit label"] {
        assert!(l.contains(&here.to_string()), "{here}");
    }
    assert!(!l.contains(&"Edit: Paste".to_string()));
    app.on_event(key(KeyCode::Esc), now);
    app.session.copy();
    ctrl_key(&mut app, 'p', now);
    assert!(palette_labels(&app).contains(&"Edit: Paste".to_string()));
}

#[test]
fn palette_zoom_actions_step_the_zoom_around_the_screen_center() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "zoom in", now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.session.view.zoom, clidraw::view::Zoom::Two);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "zoom out", now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.session.view.zoom, clidraw::view::Zoom::One);
}

#[test]
fn palette_filters_fuzzily_and_backspace_widens() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "duplicate", now);
    assert_eq!(palette_labels(&app)[0], "Edit: Duplicate");
    app.on_event(key(KeyCode::Esc), now);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "exsvg", now);
    assert_eq!(palette_labels(&app)[0], "File: Export SVG");
    type_str(&mut app, "zzz", now);
    assert!(palette_labels(&app).is_empty());
    app.on_event(key(KeyCode::Enter), now);
    assert!(palette_open(&app));
    for _ in 0..3 {
        app.on_event(key(KeyCode::Backspace), now);
    }
    assert_eq!(palette_labels(&app)[0], "File: Export SVG");
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
    type_str(&mut app, "edit: delete", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(app.session.canvas.elements.is_empty());
    app.on_event(key(KeyCode::Char('u')), now);
    assert_eq!(app.session.canvas.elements.len(), 1);

    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "export svg", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Scope(..)));
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
    assert_eq!(third, "Edit: Select all");
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
                side_labels: vec![],
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
                bends: vec![],
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
fn right_click_menu_arranges_the_route_of_a_bent_line() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(Command::Insert(
        0,
        Element::Line(Line {
            id: Id::from("lllllll1"),
            kind: LineKind::Arrow,
            from: End::Free(20, 5),
            to: End::Free(40, 5),
            label: String::new(),
            label_at: None,
            stroke: Default::default(),
            bends: vec![(30, 9)],
            color: None,
        }),
    ));
    right_click(&mut app, 30, 9, now);
    let clidraw::ui::Overlay::Context(c) = &app.overlay else {
        panic!("no menu")
    };
    assert_eq!(c.labels().last(), Some(&"Reset route"));
    click(&mut app, 32, 9 + 1 + 7, now);
    let Some(Element::Line(l)) = app.session.canvas.get(&Id::from("lllllll1")) else {
        panic!("no line")
    };
    assert_eq!(l.bends, vec![(30, 5)]);
}

fn help_query(app: &App) -> String {
    match &app.overlay {
        clidraw::ui::Overlay::Help(_, q) => q.clone(),
        _ => panic!("help is not open"),
    }
}

#[test]
fn help_typing_builds_the_query_and_backspace_pops() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Char('?')), now);
    type_str(&mut app, "d?l", now);
    assert_eq!(help_query(&app), "d?l");
    app.on_event(key(KeyCode::Backspace), now);
    assert_eq!(help_query(&app), "d?");
    app.on_event(key(KeyCode::Right), now);
    assert_eq!(help_page(&app), 0);
}

#[test]
fn help_esc_clears_the_query_then_closes() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Char('?')), now);
    type_str(&mut app, "del", now);
    app.on_event(key(KeyCode::Esc), now);
    assert_eq!(help_query(&app), "");
    app.on_event(key(KeyCode::Esc), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn help_tab_click_clears_the_query() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Char('?')), now);
    type_str(&mut app, "del", now);
    click(&mut app, 33, 2, now);
    assert_eq!(help_query(&app), "");
}

#[test]
fn help_search_matches_substring_fuzzy_and_synonyms() {
    let keys = |q: &str| -> Vec<String> {
        clidraw::ui::help::search(q)
            .into_iter()
            .map(|r| r.0)
            .collect()
    };
    assert_eq!(keys("delete a char"), ["Backspace, Del"]);
    assert!(keys("dlt").contains(&"Del".to_string()));
    assert!(keys("remove").contains(&"Del".to_string()));
    assert!(keys("zzzz").is_empty());
    assert!(clidraw::ui::help::search("e").len() <= 18);
}

#[test]
fn help_synonym_targets_appear_in_some_row() {
    for (_, target) in clidraw::ui::help::SYNONYMS {
        assert!(
            (0..clidraw::ui::help::SCOPES.len())
                .flat_map(clidraw::ui::help::rows)
                .any(|(k, a)| format!("{k} {a}").to_lowercase().contains(target)),
            "no row for {target}"
        );
    }
}

fn shape_count(app: &App) -> usize {
    app.session.canvas.elements.len()
}

fn insert_box(app: &mut App) {
    app.session.exec(Command::Insert(
        0,
        Element::Shape(Shape {
            id: Id::from("aaaaaaaa"),
            kind: ShapeKind::Rect,
            x: 30,
            y: 8,
            w: 8,
            h: 3,
            label: "A".into(),
            stroke: Default::default(),
            border: Default::default(),
            color: None,
            fill: None,
            align: Default::default(),
            bold: false,
            link: None,
            side_labels: vec![],
        }),
    ));
}

#[test]
fn info_bar_keeps_its_buttons_on_narrow_screens() {
    let t = tempfile::tempdir().unwrap();
    let app = app_in(t.path());
    assert!(
        last_row(&app).starts_with("[Undo] [Redo]"),
        "{}",
        last_row(&app)
    );
    let area = Rect::new(0, 0, 40, 24);
    let mut buf = Buffer::empty(area);
    app.render(&mut buf, area);
    let row: String = (0..40).map(|x| buf[(x, 23)].symbol().to_string()).collect();
    assert!(
        row.starts_with("[Undo] [Redo]") && row.contains("[Fit]"),
        "{row}"
    );
}

#[test]
fn zoom_buttons_step_the_zoom() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    let (plus, minus) = (col_of(&app, "[+]"), col_of(&app, "[-]"));
    click(&mut app, plus, 23, now);
    assert_eq!(app.session.view.zoom, clidraw::view::Zoom::Two);
    click(&mut app, minus, 23, now);
    assert_eq!(app.session.view.zoom, clidraw::view::Zoom::One);
    click(&mut app, minus, 23, now);
    assert_eq!(app.session.view.zoom, clidraw::view::Zoom::Half);
}

#[test]
fn undo_and_redo_buttons_run_the_history() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    insert_box(&mut app);
    click(&mut app, 2, 23, now);
    assert_eq!(shape_count(&app), 0);
    assert!(app.status.as_ref().unwrap().text.starts_with("Undid"));
    click(&mut app, 8, 23, now);
    assert_eq!(shape_count(&app), 1);
}

#[test]
fn undo_button_is_inactive_while_zoomed_out() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    insert_box(&mut app);
    let minus = col_of(&app, "[-]");
    click(&mut app, minus, 23, now);
    click(&mut app, 2, 23, now);
    assert_eq!(shape_count(&app), 1);
}

#[test]
fn fit_button_moves_the_view_to_the_content() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    insert_box(&mut app);
    app.session.view.ox = 500;
    let fit = col_of(&app, "[Fit]");
    click(&mut app, fit, 23, now);
    app.tick(now);
    app.tick(now + Duration::from_secs(3));
    assert_ne!(app.session.view.ox, 500);
}

#[test]
fn clicking_the_bar_creates_nothing() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Char('r')), now);
    click(&mut app, 10, 23, now);
    click(&mut app, 40, 23, now);
    assert_eq!(shape_count(&app), 0);
}

#[test]
fn palette_ctrl_n_and_ctrl_p_move_the_selection() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    let selected = |app: &App| match &app.overlay {
        clidraw::ui::Overlay::Actions(a) => a.selected(),
        _ => panic!("palette is not open"),
    };
    ctrl_key(&mut app, 'p', now);
    let first = selected(&app);
    ctrl_key(&mut app, 'n', now);
    let second = selected(&app);
    assert_ne!(first, second);
    ctrl_key(&mut app, 'p', now);
    assert_eq!(selected(&app), first);
    assert!(palette_open(&app));
    app.on_event(key(KeyCode::Down), now);
    assert_eq!(selected(&app), second);
}

fn registry_app(dir: &std::path::Path) -> App {
    let mut app = app_in(dir);
    for (id, x, y) in [
        ("aaaaaaaa", 2, 5),
        ("dddddddd", 70, 2),
        ("bbbbbbbb", 20, 8),
        ("cccccccc", 50, 20),
        ("eeeeeeee", 30, 18),
    ] {
        app.session.exec(shape_at(id, x, y));
    }
    app.session.undo();
    app.session.selection = ["aaaaaaaa", "bbbbbbbb", "cccccccc"].map(Id::from).to_vec();
    app.session.copy();
    app
}

/// Canvas, selection and mode as text, with generated ids masked.
fn world(app: &App) -> String {
    let mut out = format!(
        "{}|{:?}|{:?}|{}|{}|{:?}",
        serde_json::to_string(&app.session.canvas).unwrap(),
        app.session.selection,
        app.session.tool,
        app.quit,
        app.session.present,
        std::mem::discriminant(&app.overlay),
    );
    let fixed = ["aaaaaaaa", "bbbbbbbb", "cccccccc", "dddddddd"];
    for e in &app.session.canvas.elements {
        if !fixed.contains(&e.id().0.as_str()) {
            out = out.replace(&e.id().0, "new");
        }
    }
    out
}

fn press(app: &mut App, k: &clidraw::ui::registry::Key) {
    app.on_event(Event::Key(KeyEvent::new(k.code, k.mods)), Instant::now());
}

#[test]
fn registry_runnable_entries_dispatch_their_action() {
    use clidraw::ui::registry::BINDINGS;
    let t = tempfile::tempdir().unwrap();
    let now = Instant::now();
    let runnable = BINDINGS
        .iter()
        .filter(|b| b.action.is_some() && !b.keys.is_empty());
    for (i, b) in runnable.enumerate() {
        for (n, k) in b.keys.iter().enumerate() {
            let mut by_key = registry_app(&t.path().join(format!("k{i}-{n}")));
            let mut by_palette = registry_app(&t.path().join(format!("p{i}-{n}")));
            let start = world(&by_key);
            press(&mut by_key, k);
            ctrl_key(&mut by_palette, 'p', now);
            let label = format!("{}: {}", b.action.unwrap().category(), b.name());
            type_str(&mut by_palette, &label, now);
            by_palette.on_event(key(KeyCode::Enter), now);
            assert_eq!(world(&by_key), world(&by_palette), "{} {}", b.id, k.label());
            let inert = ["copy", "ungroup", "fit", "tool_select"];
            assert!(
                world(&by_key) != start || inert.contains(&b.id),
                "{} did nothing",
                b.id
            );
        }
    }
}

type Case = (&'static str, fn(&mut App), fn(&App) -> bool);

fn palette_typed(app: &mut App, text: &str) {
    let now = Instant::now();
    ctrl_key(app, 'p', now);
    type_str(app, text, now);
}

fn editing(app: &mut App) {
    app.session.start_edit_existing(Id::from("aaaaaaaa"));
    let e = app.session.edit.as_mut().unwrap();
    e.buf = "hello world".into();
    e.caret = e.buf.len();
}

fn label_is(app: &App, want: &str) -> bool {
    app.session.edit.as_ref().is_some_and(|e| e.buf == want)
}

fn palette_open_at_second(app: &mut App) {
    palette_typed(app, "");
    app.on_event(key(KeyCode::Down), Instant::now());
}

fn zoomed(app: &mut App) {
    app.session.zoom_to(clidraw::view::Zoom::Half, (40, 12));
}

#[test]
fn registry_display_only_entries_are_covered() {
    use clidraw::ui::actions::Action;
    use clidraw::ui::{registry::BINDINGS, Overlay};
    let cases: Vec<Case> = vec![
        ("cancel", |_| {}, |a| a.session.selection.is_empty()),
        ("move", |_| {}, |a| world(a).contains("\"x\":1")),
        (
            "connect",
            |a| a.session.selection = vec![Id::from("bbbbbbbb")],
            |a| a.session.canvas.elements.len() == 6,
        ),
        (
            "palette",
            |_| {},
            |a| matches!(a.overlay, Overlay::Actions(_)),
        ),
        (
            "command",
            |_| {},
            |a| matches!(a.overlay, Overlay::Prompt(_)),
        ),
        ("reset_zoom", zoomed, |a| !a.session.zoomed_out()),
        ("confirm", |a| type_str(a, ":q", Instant::now()), |a| a.quit),
        (
            "cancel_prompt",
            |a| type_str(a, ":", Instant::now()),
            |a| matches!(a.overlay, Overlay::None),
        ),
        (
            "apply_fix",
            |a| type_mermaid(a, "A -> B", KeyCode::Tab),
            |a| buffer(a) == "A --> B",
        ),
        (
            "overview_target",
            |a| {
                palette_typed(a, "mini");
                a.on_event(key(KeyCode::Enter), Instant::now());
            },
            |a| {
                matches!(
                    a.overlay,
                    Overlay::Map(clidraw::ui::minimap::Target::Selection)
                )
            },
        ),
        (
            "run",
            |a| palette_typed(a, "present"),
            |a| a.session.present,
        ),
        (
            "select",
            palette_open_at_second,
            |a| matches!(&a.overlay, Overlay::Actions(x) if x.selected() == Some(Action::Ex("u"))),
        ),
        (
            "close",
            |a| palette_typed(a, ""),
            |a| matches!(a.overlay, Overlay::None),
        ),
        ("caret", editing, |a| {
            a.session.edit.as_ref().is_some_and(|e| e.caret == 10)
        }),
        ("caret_ends", editing, |a| {
            a.session.edit.as_ref().is_some_and(|e| e.caret == 0)
        }),
        ("word", editing, |a| {
            a.session.edit.as_ref().is_some_and(|e| e.caret == 6)
        }),
        ("select_text", editing, |a| {
            a.session.edit.as_ref().is_some_and(|e| e.anchor.is_some())
        }),
        ("delete_char", editing, |a| label_is(a, "hello worl")),
        ("delete_word", editing, |a| label_is(a, "hello ")),
        ("delete_to_start", editing, |a| label_is(a, "")),
        ("newline", editing, |a| {
            a.session
                .edit
                .as_ref()
                .is_some_and(|e| e.buf.ends_with('\n'))
        }),
        ("commit_edit", editing, |a| {
            a.session.edit.is_none() && world(a).contains("hello world")
        }),
        ("discard_edit", editing, |a| {
            a.session.edit.is_none() && !world(a).contains("hello")
        }),
    ];
    let t = tempfile::tempdir().unwrap();
    for b in BINDINGS
        .iter()
        .filter(|b| b.action.is_none() && !b.keys.is_empty())
    {
        let Some((i, (_, setup, check))) = cases.iter().enumerate().find(|(_, c)| c.0 == b.id)
        else {
            panic!("no case for {}", b.id);
        };
        let mut app = registry_app(&t.path().join(format!("d{i}")));
        setup(&mut app);
        press(&mut app, &b.keys[0]);
        assert!(check(&app), "{} {}", b.id, b.keys[0].label());
    }
}

fn prompt_kind(app: &App) -> Option<clidraw::ui::prompt::PromptKind> {
    match &app.overlay {
        clidraw::ui::Overlay::Prompt(p) => Some(p.kind.clone()),
        _ => None,
    }
}

#[test]
fn context_menu_runs_the_highlighted_row_from_the_keyboard() {
    use clidraw::ui::Overlay;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    right_click(&mut app, 20, 6, now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, Overlay::None));
    assert_eq!(app.session.canvas.elements.len(), 2);

    right_click(&mut app, 20, 6, now);
    app.on_event(key(KeyCode::Esc), now);
    assert!(!context_open(&app));
}

#[test]
fn export_scope_runs_the_highlighted_row_from_the_keyboard() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_export(&mut app, now);
    ctrl_key(&mut app, 'n', now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Nothing selected: select a shape first")
    );
}

#[test]
fn switcher_renames_and_deletes_the_highlighted_canvas_from_the_keyboard() {
    use clidraw::ui::prompt::PromptKind;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_canvases(&mut app, now);
    app.on_event(key(KeyCode::Char('x')), now);
    assert!(matches!(prompt_kind(&app), Some(PromptKind::ConfirmDelete(n)) if n == app.name));
    app.on_event(key(KeyCode::Esc), now);
    open_canvases(&mut app, now);
    app.on_event(key(KeyCode::Char('r')), now);
    assert!(matches!(prompt_kind(&app), Some(PromptKind::Rename(n)) if n == app.name));
}

#[test]
fn rename_and_delete_move_and_drop_the_saved_viewport() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let mut seed = clidraw::store::State::default();
    seed.offsets.insert("demo".into(), (7, 9));
    store.save_state(&seed).unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_canvases(&mut app, now);
    app.on_event(key(KeyCode::Char('r')), now);
    type_str(&mut app, "2", now);
    app.on_event(key(KeyCode::Enter), now);
    let state = store.load_state();
    assert_eq!(state.offsets.get("demo2"), Some(&(7, 9)));
    assert!(!state.offsets.contains_key("demo"));
    app.on_event(key(KeyCode::Char('x')), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(!store.load_state().offsets.contains_key("demo2"));
}

#[test]
fn switcher_moves_with_j_k_and_enter_on_the_current_canvas_closes() {
    use clidraw::ui::prompt::PromptKind;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_canvases(&mut app, now);
    app.on_event(key(KeyCode::Char('k')), now);
    app.on_event(key(KeyCode::Char('x')), now);
    assert!(prompt_kind(&app).is_none(), "x on the New row does nothing");
    app.on_event(key(KeyCode::Char('j')), now);
    app.on_event(key(KeyCode::Char('x')), now);
    assert!(matches!(
        prompt_kind(&app),
        Some(PromptKind::ConfirmDelete(_))
    ));
    app.on_event(key(KeyCode::Esc), now);
    open_canvases(&mut app, now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn palette_types_j_k_into_the_query_and_esc_clears_it_before_closing() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    let all = palette_labels(&app).len();
    type_str(&mut app, "jk", now);
    assert!(palette_labels(&app).len() < all);
    app.on_event(key(KeyCode::Esc), now);
    assert!(palette_open(&app));
    assert_eq!(palette_labels(&app).len(), all);
    app.on_event(key(KeyCode::Esc), now);
    assert!(!palette_open(&app));
}

#[test]
fn palette_reaches_canvas_and_file_commands() {
    use clidraw::ui::prompt::PromptKind;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    for (query, kind) in [
        ("create canvas", "new"),
        ("rename canvas", "rename"),
        ("delete canvas", "delete"),
        ("import file", "import"),
        ("use library", "lib"),
    ] {
        ctrl_key(&mut app, 'p', now);
        type_str(&mut app, query, now);
        app.on_event(key(KeyCode::Enter), now);
        let got = match prompt_kind(&app) {
            Some(PromptKind::NewCanvas) => "new",
            Some(PromptKind::Rename(n)) if n == app.name => "rename",
            Some(PromptKind::ConfirmDelete(n)) if n == app.name => "delete",
            Some(PromptKind::Command) => kind,
            _ => "none",
        };
        assert_eq!(got, kind, "{query}");
        app.on_event(key(KeyCode::Esc), now);
    }
    ctrl_key(&mut app, 'p', now);
    assert!(!palette_labels(&app).contains(&"Edit: Link selection to URL".to_string()));
}

fn run_palette(app: &mut App, query: &str, now: Instant) {
    ctrl_key(app, 'p', now);
    type_str(app, query, now);
    app.on_event(key(KeyCode::Enter), now);
}

fn rect(app: &App) -> Shape {
    match app.session.canvas.get(&Id::from("aaaaaaaa")) {
        Some(Element::Shape(s)) => s.clone(),
        _ => panic!("no shape"),
    }
}

#[test]
fn palette_applies_the_selection_style_options_and_lists_them_only_for_a_selection() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    ctrl_key(&mut app, 'p', now);
    assert!(!palette_labels(&app).contains(&"Stroke: dashed".to_string()));
    app.on_event(key(KeyCode::Esc), now);
    app.session.selection = vec![Id::from("aaaaaaaa")];
    ctrl_key(&mut app, 'p', now);
    let l = palette_labels(&app);
    for here in [
        "Stroke: dashed",
        "Border: rounded",
        "Text: right",
        "Ink: color",
        "Fill: color",
    ] {
        assert!(l.contains(&here.to_string()), "{here}");
    }
    assert!(!l.contains(&"Head: both".to_string()));
    app.on_event(key(KeyCode::Esc), now);
    run_palette(&mut app, "stroke dashed", now);
    assert_eq!(rect(&app).stroke, Stroke::Dashed);
    run_palette(&mut app, "border rounded", now);
    assert_eq!(rect(&app).border, Border::Rounded);
    run_palette(&mut app, "text right", now);
    assert_eq!(rect(&app).align, TextAlign::Right);
    app.session.undo();
    assert_eq!(rect(&app).align, TextAlign::Center);
}

#[test]
fn palette_head_both_sets_the_selected_line() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(Command::Insert(
        0,
        Element::Line(Line {
            id: Id::from("llllllll"),
            kind: LineKind::Line,
            from: End::Free(5, 5),
            to: End::Free(25, 5),
            label: String::new(),
            label_at: None,
            stroke: Default::default(),
            bends: vec![],
            color: None,
        }),
    ));
    app.session.selection = vec![Id::from("llllllll")];
    run_palette(&mut app, "head both", now);
    match app.session.canvas.get(&Id::from("llllllll")) {
        Some(Element::Line(l)) => assert_eq!(l.kind, LineKind::ArrowBoth),
        _ => panic!("no line"),
    }
}

#[test]
fn palette_ink_opens_a_color_list_that_enter_applies_and_esc_leaves() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    run_palette(&mut app, "ink color", now);
    app.on_event(key(KeyCode::Esc), now);
    assert_eq!(rect(&app).color, None);
    run_palette(&mut app, "ink color", now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(rect(&app).color, Some(Paint::Orange));
    run_palette(&mut app, "fill color", now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(rect(&app).fill, Some(Paint::Red));
}

#[test]
fn export_command_takes_a_format_or_lists_them() {
    use clidraw::ui::prompt::PromptKind;
    use clidraw::ui::scope::ExportFormat;
    use clidraw::ui::Overlay;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    run(&mut app, "export");
    assert!(matches!(app.overlay, Overlay::Formats(_)));
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(
        app.overlay,
        Overlay::Scope(ExportFormat::Mermaid, _)
    ));
    app.on_event(key(KeyCode::Esc), now);
    for (arg, format) in [
        ("svg", ExportFormat::Svg),
        ("mmd", ExportFormat::Mermaid),
        ("clidraw", ExportFormat::Clidraw),
    ] {
        run(&mut app, &format!("export {arg}"));
        assert!(matches!(app.overlay, Overlay::Scope(f, _) if f == format));
        app.on_event(key(KeyCode::Esc), now);
    }
    run(&mut app, "export svg");
    app.on_event(key(KeyCode::Enter), now);
    assert!(
        matches!(&app.overlay, Overlay::Prompt(p) if matches!(p.kind, PromptKind::ExportSvg(_)))
    );
    app.on_event(key(KeyCode::Esc), now);
    run(&mut app, "export png");
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Usage: :export [svg|mmd|clidraw]")
    );
}

fn buffer(app: &App) -> String {
    let clidraw::ui::Overlay::Prompt(p) = &app.overlay else {
        panic!("no prompt")
    };
    p.buf().to_string()
}

fn command_line(app: &mut App, typed: &str, t: Instant) {
    app.on_event(key(KeyCode::Char(':')), t);
    type_str(app, typed, t);
}

#[test]
fn command_line_edits_at_the_caret() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    command_line(&mut app, "layoutx", now);
    app.on_event(key(KeyCode::Left), now);
    app.on_event(key(KeyCode::Backspace), now);
    assert_eq!(buffer(&app), "layoux");
    app.on_event(key(KeyCode::Char('t')), now);
    assert_eq!(buffer(&app), "layoutx");
    app.on_event(key(KeyCode::Delete), now);
    assert_eq!(buffer(&app), "layout");
    app.on_event(key(KeyCode::Home), now);
    app.on_event(key(KeyCode::Char('x')), now);
    assert_eq!(buffer(&app), "xlayout");
    app.on_event(key(KeyCode::End), now);
    ctrl_key(&mut app, 'w', now);
    assert_eq!(buffer(&app), "");
    type_str(&mut app, "abc", now);
    app.on_event(key(KeyCode::Left), now);
    ctrl_key(&mut app, 'u', now);
    assert_eq!(buffer(&app), "c");
    app.on_event(key(KeyCode::Esc), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn tab_completes_the_verb_and_adds_a_space_only_before_an_argument() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    command_line(&mut app, "e", now);
    app.on_event(key(KeyCode::Tab), now);
    assert_eq!(buffer(&app), "e ");
    app.on_event(key(KeyCode::Esc), now);

    command_line(&mut app, "wq", now);
    app.on_event(key(KeyCode::Tab), now);
    assert_eq!(buffer(&app), "wq");
    app.on_event(key(KeyCode::Esc), now);

    command_line(&mut app, "e", now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Tab), now);
    assert_eq!(buffer(&app), "export ");
}

#[test]
fn find_mode_has_no_completion() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Char('/')), now);
    type_str(&mut app, "e", now);
    app.on_event(key(KeyCode::Tab), now);
    assert_eq!(buffer(&app), "/e");
}

#[test]
fn enter_runs_the_typed_text_not_the_highlighted_row() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    command_line(&mut app, "e", now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Usage: :e <name>")
    );
}

#[test]
fn clicking_a_completion_fills_it_and_clicking_away_closes() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    command_line(&mut app, "wq", now);
    click(&mut app, 5, 21, now);
    assert_eq!(buffer(&app), "wq");
    type_str(&mut app, "x", now);
    click(&mut app, 5, 23, now);
    assert_eq!(buffer(&app), "wqx");
    click(&mut app, 5, 2, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn clicking_a_completion_replaces_the_typed_prefix() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    command_line(&mut app, "lay", now);
    click(&mut app, 5, 21, now);
    assert_eq!(buffer(&app), "layout ");
}

#[test]
fn dialog_prompts_edit_at_the_caret_too() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_canvases(&mut app, now);
    click(&mut app, 25, 11, now);
    for _ in 0..40 {
        app.on_event(key(KeyCode::Backspace), now);
    }
    type_str(&mut app, "ac", now);
    app.on_event(key(KeyCode::Left), now);
    app.on_event(key(KeyCode::Char('b')), now);
    assert_eq!(buffer(&app), "abc");
}

fn type_mermaid(app: &mut App, line: &str, end: KeyCode) {
    let t = Instant::now();
    app.on_event(key(KeyCode::Char(':')), t);
    for c in "mermaid".chars() {
        app.on_event(key(KeyCode::Char(c)), t);
    }
    app.on_event(key(KeyCode::Enter), t);
    for c in line.chars() {
        app.on_event(key(KeyCode::Char(c)), t);
    }
    app.on_event(key(end), t);
}

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

#[test]
fn export_asks_before_overwriting_an_existing_file() {
    use clidraw::ui::prompt::PromptKind;
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 0, 2), now);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 40, 8), now);
    let out = t.path().join("keep.svg");
    std::fs::write(&out, "old").unwrap();
    type_path(&mut app, out.to_str().unwrap(), now);
    assert!(matches!(
        prompt_kind(&app),
        Some(PromptKind::ConfirmOverwrite(..))
    ));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "old");
    app.on_event(key(KeyCode::Enter), now);
    assert!(std::fs::read_to_string(&out).unwrap().contains("Alpha"));
}

#[test]
fn export_cancelled_overwrite_keeps_the_file() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 0, 2), now);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 40, 8), now);
    let out = t.path().join("keep.svg");
    std::fs::write(&out, "old").unwrap();
    type_path(&mut app, out.to_str().unwrap(), now);
    app.on_event(key(KeyCode::Esc), now);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "old");
}

// `home_dir` reads USERPROFILE on Windows, not HOME. One test: parallel tests would race on HOME.
#[cfg(unix)]
#[test]
fn a_leading_tilde_expands_in_export_and_import_paths() {
    let t = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    std::fs::write(home.path().join("x.mmd"), "flowchart TD\nA --> B\n").unwrap();
    std::env::set_var("HOME", home.path());
    let (mut app, now) = area_app(t.path());
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 0, 2), now);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 40, 8), now);
    type_path(&mut app, "~/tilde.svg", now);
    assert!(home.path().join("tilde.svg").exists());

    let mut app = app_in(t.path());
    run(&mut app, "import ~/x.mmd");
    assert!(app
        .status
        .as_ref()
        .unwrap()
        .text
        .starts_with("Imported 2 shapes"));
}

#[test]
fn palette_copy_clidraw_queues_the_canvas_as_osc52() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "copy clidraw", now);
    assert_eq!(palette_labels(&app)[0], "File: Copy clidraw");
    app.on_event(key(KeyCode::Enter), now);
    let seq = app.take_copy().expect("copy queued");
    assert!(seq.starts_with("\x1b]52;c;ewog") && seq.ends_with("\x1b\\"));
    assert!(app.take_copy().is_none());
}

#[test]
fn palette_copy_share_link_queues_the_mermaid_url_and_shows_it() {
    use clidraw::ui::scope::ExportFormat;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "copy share", now);
    assert_eq!(palette_labels(&app)[0], "File: Copy share link");
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Enter), now);
    let text = app.status.as_ref().unwrap().text.clone();
    let url = text
        .strip_prefix("Copied share link: ")
        .expect("status shows the url");
    let fragment = url
        .strip_prefix("https://clidraw.shrtcts.fr/view.html#")
        .expect("url targets the viewer");
    assert!(!fragment.contains(['+', '/', '=']));
    let (mermaid, _) = clidraw::export::render(
        &app.session.canvas,
        ExportFormat::Mermaid,
        Config::default().export_padding,
    );
    assert_eq!(decode_url_safe(fragment), mermaid);
    let seq = app.take_copy().expect("copy queued");
    assert!(seq.starts_with("\x1b]52;c;") && seq.ends_with("\x1b\\"));
}

#[test]
fn palette_copy_share_link_with_no_selection_queues_nothing() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "copy share", now);
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(
        app.status.as_ref().unwrap().text,
        "Nothing selected: select a shape first"
    );
    assert!(app.take_copy().is_none());
}

#[test]
fn palette_copy_for_obsidian_queues_a_fenced_mermaid_block() {
    use clidraw::ui::scope::ExportFormat;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "copy for obsidian", now);
    assert_eq!(palette_labels(&app)[0], "File: Copy for Obsidian");
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.status.as_ref().unwrap().text, "Copied for Obsidian");
    let (mermaid, _) = clidraw::export::render(
        &app.session.canvas,
        ExportFormat::Mermaid,
        Config::default().export_padding,
    );
    let seq = app.take_copy().expect("copy queued");
    let payload = seq
        .strip_prefix("\x1b]52;c;")
        .and_then(|s| s.strip_suffix("\x1b\\"))
        .expect("osc 52 sequence");
    assert_eq!(
        decode_base64(payload),
        format!("```mermaid\n{}\n```\n", mermaid.trim_end())
    );
}

#[test]
fn palette_copy_for_obsidian_fences_past_a_backtick_run_in_a_label() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    let Command::Insert(at, Element::Shape(mut shape)) = shape_at("aaaaaaaa", 20, 5) else {
        unreachable!()
    };
    shape.label = "a ``` b".into();
    app.session.exec(Command::Insert(at, Element::Shape(shape)));
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "copy for obsidian", now);
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Enter), now);
    let seq = app.take_copy().expect("copy queued");
    let payload = seq
        .strip_prefix("\x1b]52;c;")
        .and_then(|s| s.strip_suffix("\x1b\\"))
        .expect("osc 52 sequence");
    let text = decode_base64(payload);
    assert!(text.starts_with("````mermaid\n"), "{text}");
    assert!(text.ends_with("\n````\n"), "{text}");
}

#[test]
fn palette_copy_for_obsidian_with_no_selection_queues_nothing() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "copy for obsidian", now);
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(
        app.status.as_ref().unwrap().text,
        "Nothing selected: select a shape first"
    );
    assert!(app.take_copy().is_none());
}

fn decode_base64(payload: &str) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let (mut bits, mut n, mut out) = (0u32, 0, Vec::new());
    for c in payload.trim_end_matches('=').bytes() {
        let v = ALPHABET
            .iter()
            .position(|&a| a == c)
            .expect("standard base64") as u32;
        bits = bits << 6 | v;
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((bits >> n) as u8);
        }
    }
    String::from_utf8(out).unwrap()
}

fn decode_url_safe(fragment: &str) -> String {
    let (mut bits, mut n, mut out) = (0u32, 0, Vec::new());
    for c in fragment.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => panic!("not base64url: {c}"),
        };
        bits = bits << 6 | u32::from(v);
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((bits >> n) as u8);
        }
    }
    String::from_utf8(out).unwrap()
}

fn command_buf(app: &App) -> &str {
    match &app.overlay {
        clidraw::ui::Overlay::Prompt(p) => p.buf(),
        _ => panic!("no prompt"),
    }
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
    let mut app = app_in(t.path());
    let now = Instant::now();
    type_str(&mut app, &format!(":import {d}/"), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Up), now);
    ctrl_key(&mut app, 'n', now);
    ctrl_key(&mut app, 'n', now);
    app.on_event(key(KeyCode::Tab), now);
    assert_eq!(command_buf(&app), format!("import {d}/b.mmd"));

    let mut app = app_in(t.path());
    type_str(&mut app, &format!(":import {d}/"), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(command_buf(&app), format!("import {d}/sub/"));
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(command_buf(&app), format!("import {d}/sub/c.mmd"));

    let mut app = app_in(t.path());
    type_str(&mut app, &format!(":import {d}/a.mmd"), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(app
        .status
        .as_ref()
        .unwrap()
        .text
        .starts_with("Imported 2 shapes"));
}

/// Finds `text` on the rendered 80x24 screen, from the bottom row up, as (column, row).
fn text_at(app: &App, text: &str) -> (u16, u16) {
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    app.render(&mut buf, area);
    (0..24)
        .rev()
        .find_map(|y| {
            let row: String = (0..80).map(|x| buf[(x, y)].symbol().to_string()).collect();
            row.find(text)
                .map(|at| (row[..at].chars().count() as u16, y))
        })
        .unwrap_or_else(|| panic!("{text} is not on screen"))
}

fn select_one_shape(app: &mut App) {
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.selection = vec![Id::from("aaaaaaaa")];
}

fn shape_kind(app: &App) -> ShapeKind {
    match app.session.canvas.get(&Id::from("aaaaaaaa")) {
        Some(Element::Shape(s)) => s.kind,
        _ => panic!("no shape"),
    }
}

#[test]
fn clicking_an_ink_swatch_paints_the_selection_and_closes_the_list() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    select_one_shape(&mut app);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "ink", now);
    app.on_event(key(KeyCode::Enter), now);
    let (x, y) = text_at(&app, "red");
    click(&mut app, x + 1, y, now);
    assert!(matches!(
        app.session.canvas.get(&Id::from("aaaaaaaa")),
        Some(Element::Shape(s)) if s.color == Some(Paint::Red)
    ));
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn clicking_a_shape_kind_applies_it_and_clicking_off_the_list_restores_the_kind() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    select_one_shape(&mut app);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "change shape", now);
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Down), now);
    assert_eq!(
        shape_kind(&app),
        ShapeKind::Diamond,
        "arrow previews the kind"
    );
    click(&mut app, 0, 0, now);
    assert_eq!(shape_kind(&app), ShapeKind::Rect);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));

    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "change shape", now);
    app.on_event(key(KeyCode::Enter), now);
    let (x, y) = text_at(&app, "Hexagon");
    click(&mut app, x + 1, y, now);
    assert_eq!(shape_kind(&app), ShapeKind::Hexagon);
}

#[test]
fn clicking_the_rename_or_delete_text_of_a_canvas_row_starts_that_prompt() {
    use clidraw::ui::prompt::PromptKind;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_canvases(&mut app, now);
    let (x, y) = text_at(&app, "r rename");
    click(&mut app, x, y, now);
    assert!(matches!(prompt_kind(&app), Some(PromptKind::Rename(_))));
    app.on_event(key(KeyCode::Esc), now);

    open_canvases(&mut app, now);
    let (x, y) = text_at(&app, "x delete");
    click(&mut app, x, y, now);
    assert!(matches!(
        prompt_kind(&app),
        Some(PromptKind::ConfirmDelete(_))
    ));
    app.on_event(key(KeyCode::Esc), now);

    open_canvases(&mut app, now);
    app.on_event(key(KeyCode::Esc), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn clicking_off_the_colors_scope_or_canvas_list_closes_it() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    select_one_shape(&mut app);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "ink", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Colors(..)));
    click(&mut app, 0, 0, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));

    open_export(&mut app, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Scope(..)));
    click(&mut app, 0, 0, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));

    open_canvases(&mut app, now);
    click(&mut app, 0, 0, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}
