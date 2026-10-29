use super::common::*;
use clidraw::app::App;
use clidraw::model::{Command, Element, End, Id, Line, LineKind};
use crossterm::event::KeyCode;
use std::time::Instant;

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
            meta: Default::default(),
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
