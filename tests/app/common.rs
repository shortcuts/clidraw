use clidraw::app::App;
use clidraw::config::CommandUi;
use clidraw::model::{Command, Element, Id, Shape, ShapeKind, Text};
use clidraw::store::Store;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::time::Instant;

pub fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

pub fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

pub fn click(app: &mut App, col: u16, row: u16, t: Instant) {
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), col, row), t);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), col, row), t);
}

/// Two shapes near the origin and one far away; opens the palette's Export SVG, Area.
pub fn area_app(dir: &std::path::Path) -> (App, Instant) {
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
                meta: Default::default(),
            }),
        ));
    }
    let t = Instant::now();
    open_export(&mut app, t);
    click(&mut app, 34, 11, t);
    (app, t)
}

pub fn type_path(app: &mut App, path: &str, t: Instant) {
    for _ in 0..32 {
        app.on_event(key(KeyCode::Backspace), t);
    }
    for c in path.chars() {
        app.on_event(key(KeyCode::Char(c)), t);
    }
    app.on_event(key(KeyCode::Enter), t);
}

pub fn app_in(dir: &std::path::Path) -> App {
    let store = Store::at(dir.join("c"), dir.join("state"));
    App::open(store, Some("demo".into()), (80, 24)).unwrap()
}

pub fn vim_app(dir: &std::path::Path) -> App {
    let mut app = app_in(dir);
    app.session.command_ui = CommandUi::Vim;
    app
}

pub fn run(app: &mut App, line: &str) {
    app.session.command_ui = CommandUi::Vim;
    let t = Instant::now();
    app.on_event(key(KeyCode::Char(':')), t);
    for c in line.chars() {
        app.on_event(key(KeyCode::Char(c)), t);
    }
    app.on_event(key(KeyCode::Enter), t);
}

pub fn insert_shape(app: &mut App) {
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
            meta: Default::default(),
        }),
    ));
}

pub fn labeled(id: &str, x: i32, label: &str) -> Command {
    Command::Insert(
        0,
        Element::Text(Text {
            id: Id::from(id),
            x,
            y: 0,
            text: label.into(),
            color: None,
            meta: Default::default(),
        }),
    )
}

pub fn last_row(app: &App) -> String {
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    app.render(&mut buf, area);
    (0..80).map(|x| buf[(x, 23)].symbol().to_string()).collect()
}

pub fn col_of(app: &App, needle: &str) -> u16 {
    let r = last_row(app);
    r[..r.find(needle).unwrap()].chars().count() as u16
}

pub fn shape_at(id: &str, x: i32, y: i32) -> Command {
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
            meta: Default::default(),
        }),
    )
}

pub fn right_click(app: &mut App, col: u16, row: u16, t: Instant) {
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Right), col, row), t);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Right), col, row), t);
}

pub fn ctrl_key(app: &mut App, c: char, t: Instant) {
    app.on_event(
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)),
        t,
    );
}

pub fn type_str(app: &mut App, s: &str, t: Instant) {
    for c in s.chars() {
        app.on_event(key(KeyCode::Char(c)), t);
    }
}

pub fn open_canvases(app: &mut App, t: Instant) {
    ctrl_key(app, 'p', t);
    type_str(app, "switch canvas", t);
    app.on_event(key(KeyCode::Enter), t);
}

pub fn open_export(app: &mut App, t: Instant) {
    ctrl_key(app, 'p', t);
    type_str(app, "export svg", t);
    app.on_event(key(KeyCode::Enter), t);
}

pub fn palette_labels(app: &App) -> Vec<String> {
    match &app.overlay {
        clidraw::ui::Overlay::Actions(a) => a.labels().iter().map(|l| l.to_string()).collect(),
        _ => panic!("palette is not open"),
    }
}

pub fn palette_open(app: &App) -> bool {
    matches!(app.overlay, clidraw::ui::Overlay::Actions(_))
}

pub fn status(app: &App) -> &str {
    app.status.as_ref().map_or("", |s| s.text.as_str())
}

pub fn insert_box(app: &mut App) {
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
            meta: Default::default(),
        }),
    ));
}

pub fn palette_typed(app: &mut App, text: &str) {
    let now = Instant::now();
    ctrl_key(app, 'p', now);
    type_str(app, text, now);
}

pub fn prompt_kind(app: &App) -> Option<clidraw::ui::prompt::PromptKind> {
    match &app.overlay {
        clidraw::ui::Overlay::Prompt(p) => Some(p.kind.clone()),
        _ => None,
    }
}

pub fn run_palette(app: &mut App, query: &str, now: Instant) {
    ctrl_key(app, 'p', now);
    type_str(app, query, now);
    app.on_event(key(KeyCode::Enter), now);
}

pub fn buffer(app: &App) -> String {
    let clidraw::ui::Overlay::Prompt(p) = &app.overlay else {
        panic!("no prompt")
    };
    p.buf().to_string()
}

pub fn type_mermaid(app: &mut App, line: &str, end: KeyCode) {
    app.session.command_ui = CommandUi::Vim;
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

pub fn command_buf(app: &App) -> &str {
    match &app.overlay {
        clidraw::ui::Overlay::Prompt(p) => p.buf(),
        _ => panic!("no prompt"),
    }
}

pub fn assert_frame_padding(app: &App) {
    let canvas = &app.session.canvas;
    let frame = canvas
        .elements
        .iter()
        .find_map(|e| match e {
            Element::Shape(s) if s.kind == clidraw::model::ShapeKind::Frame => Some(s.clone()),
            _ => None,
        })
        .unwrap();
    let boxes: Vec<_> = canvas
        .elements
        .iter()
        .filter(|e| e.id() != &frame.id)
        .map(|e| canvas.bounds(e))
        .collect();
    let x0 = boxes.iter().map(|b| b.0).min().unwrap();
    let y0 = boxes.iter().map(|b| b.1).min().unwrap();
    let x1 = boxes.iter().map(|b| b.0 + b.2).max().unwrap();
    let y1 = boxes.iter().map(|b| b.1 + b.3).max().unwrap();
    assert_eq!(x0 - frame.x - 1, 4, "empty columns on the left");
    assert_eq!(frame.x + frame.w - 1 - x1, 4, "empty columns on the right");
    assert_eq!(y0 - frame.y - 1, 2, "empty rows on the top");
    assert_eq!(frame.y + frame.h - 1 - y1, 2, "empty rows on the bottom");
}
