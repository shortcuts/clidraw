use super::common::*;
use clidraw::app::App;
use clidraw::model::{Anchor, Command, Element, End, Id, Line, LineKind, Shape, ShapeKind};

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
                meta: Default::default(),
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
                meta: Default::default(),
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
fn layout_lr_sets_the_canvas_direction_and_undo_restores_it() {
    let t = tempfile::tempdir().unwrap();
    let mut app = layout_app(t.path(), &[("a", "b")]);
    app.session.selection = vec![];
    run(&mut app, "layout lr");
    assert_eq!(app.session.canvas.dir, clidraw::model::Dir::LeftRight);
    run(&mut app, "u");
    assert_eq!(app.session.canvas.dir, clidraw::model::Dir::TopDown);
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
