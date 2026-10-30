use super::common::*;
use clidraw::app::App;
use clidraw::config::Config;
use clidraw::model::{Command, Element, Id, Shape, ShapeKind};
use clidraw::store::Store;
use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use std::time::{Duration, Instant};

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
            turn: 0,
            meta: Default::default(),
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
fn palette_tool_cell_shows_no_tooltip() {
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
    assert_eq!(app.next_wake(t0), None);
    app.dirty = false;
    app.tick(t0 + Duration::from_millis(400));
    assert!(!app.dirty);
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

fn shape_count(app: &App) -> usize {
    app.session.canvas.elements.len()
}

#[test]
fn info_bar_keeps_its_buttons_on_narrow_screens() {
    let t = tempfile::tempdir().unwrap();
    let app = app_in(t.path());
    assert!(
        last_row(&app).starts_with(" Undo   Redo "),
        "{}",
        last_row(&app)
    );
    let area = Rect::new(0, 0, 40, 24);
    let mut buf = Buffer::empty(area);
    app.render(&mut buf, area);
    let row: String = (0..40).map(|x| buf[(x, 23)].symbol().to_string()).collect();
    assert!(
        row.starts_with(" Undo   Redo ") && row.contains(" Fit "),
        "{row}"
    );
}

#[test]
fn zoom_buttons_step_the_zoom() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    let (plus, minus) = (col_of(&app, " + "), col_of(&app, " - "));
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
    let minus = col_of(&app, " - ");
    click(&mut app, minus, 23, now);
    click(&mut app, 2, 23, now);
    assert_eq!(shape_count(&app), 1);
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
