use super::common::*;
use clidraw::app::App;
use clidraw::model::{Command, Element, Id, Shape, ShapeKind};
use crossterm::event::KeyCode;
use std::time::Instant;

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
                meta: Default::default(),
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
fn fit_button_opens_the_minimap() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    insert_box(&mut app);
    app.session.view.ox = 500;
    let fit = col_of(&app, "[Fit]");
    click(&mut app, fit, 23, now);
    assert!(matches!(
        app.overlay,
        clidraw::ui::Overlay::Map(clidraw::ui::minimap::Target::Canvas)
    ));
    assert_eq!(app.session.view.ox, 500);
}

#[test]
fn z_opens_the_minimap() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    insert_box(&mut app);
    app.session.view.ox = 500;
    app.on_event(key(KeyCode::Char('z')), now);
    assert!(matches!(
        app.overlay,
        clidraw::ui::Overlay::Map(clidraw::ui::minimap::Target::Canvas)
    ));
    assert_eq!(app.session.view.ox, 500);
}

#[test]
fn fit_all_from_the_palette_and_the_canvas_menu_opens_the_minimap() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    insert_box(&mut app);
    palette_typed(&mut app, "Fit all");
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(
        app.overlay,
        clidraw::ui::Overlay::Map(clidraw::ui::minimap::Target::Canvas)
    ));

    app.overlay = clidraw::ui::Overlay::None;
    right_click(&mut app, 60, 15, now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(
        app.overlay,
        clidraw::ui::Overlay::Map(clidraw::ui::minimap::Target::Canvas)
    ));
}
