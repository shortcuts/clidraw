use clidraw::model::{
    corners, label_pos, route, Axis, Canvas, Command, Element, End, History, Id, Line, LineKind,
    Shape, ShapeKind, Side,
};

fn shape(id: &str, x: i32) -> Element {
    Element::Shape(Shape {
        id: Id::from(id),
        kind: ShapeKind::Rect,
        x,
        y: 0,
        w: 6,
        h: 3,
        label: String::new(),
    })
}

fn arrow(from: End, to: End) -> Element {
    Element::Line(Line {
        id: Id::from("line0000"),
        kind: LineKind::Arrow,
        from,
        to,
        label: String::new(),
    })
}

fn bound(id: &str) -> End {
    End::Bound(Id::from(id))
}

fn setup() -> (Canvas, History) {
    let mut c = Canvas::default();
    let mut h = History::default();
    for e in [
        shape("aaaaaaaa", 0),
        shape("bbbbbbbb", 20),
        arrow(bound("aaaaaaaa"), bound("bbbbbbbb")),
    ] {
        let at = c.elements.len();
        h.exec(&mut c, Command::Insert(at, e)).unwrap();
    }
    (c, h)
}

#[test]
fn undo_and_redo_replay_every_command() {
    let (mut c, mut h) = setup();
    let full = c.clone();
    while h.undo(&mut c).unwrap() {}
    assert!(c.elements.is_empty());
    while h.redo(&mut c).unwrap() {}
    assert_eq!(c, full);
}

#[test]
fn a_new_edit_clears_redo() {
    let (mut c, mut h) = setup();
    h.undo(&mut c).unwrap();
    h.exec(
        &mut c,
        Command::Move {
            id: Id::from("aaaaaaaa"),
            dx: 1,
            dy: 0,
        },
    )
    .unwrap();
    assert!(!h.redo(&mut c).unwrap());
}

#[test]
fn removing_a_shape_frees_bound_ends_at_their_last_position_and_undo_rebinds() {
    let (mut c, mut h) = setup();
    h.exec(&mut c, Command::Remove(Id::from("bbbbbbbb")))
        .unwrap();
    let Element::Line(l) = &c.elements[1] else {
        panic!("no line")
    };
    assert_eq!(l.from, bound("aaaaaaaa"));
    assert_eq!(l.to, End::Free(20, 1));
    h.undo(&mut c).unwrap();
    let Element::Line(l) = c.elements.last().unwrap() else {
        panic!("no line")
    };
    assert_eq!(l.to, bound("bbbbbbbb"));
    assert_eq!(c.elements.len(), 3);
}

#[test]
fn binding_and_unbinding_a_line_end_round_trips() {
    let (mut c, mut h) = setup();
    let id = Id::from("line0000");
    h.exec(
        &mut c,
        Command::SetEnd {
            id: id.clone(),
            side: Side::To,
            end: End::Free(9, 9),
        },
    )
    .unwrap();
    h.exec(
        &mut c,
        Command::SetEnd {
            id: id.clone(),
            side: Side::To,
            end: bound("aaaaaaaa"),
        },
    )
    .unwrap();
    h.undo(&mut c).unwrap();
    let Element::Line(l) = c.get(&id).unwrap() else {
        panic!("no line")
    };
    assert_eq!(l.to, End::Free(9, 9));
}

#[test]
fn moving_a_shape_moves_the_bound_line_end_but_free_ends_stay() {
    let (mut c, mut h) = setup();
    let line = Id::from("line0000");
    h.exec(
        &mut c,
        Command::Move {
            id: Id::from("bbbbbbbb"),
            dx: 0,
            dy: 10,
        },
    )
    .unwrap();
    let Element::Line(l) = c.get(&line).unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(c.line_ends(&l).1, (20, 11));
    h.exec(
        &mut c,
        Command::SetEnd {
            id: line.clone(),
            side: Side::From,
            end: End::Free(1, 1),
        },
    )
    .unwrap();
    h.exec(
        &mut c,
        Command::Move {
            id: line.clone(),
            dx: 2,
            dy: 2,
        },
    )
    .unwrap();
    let Element::Line(l) = c.get(&line).unwrap() else {
        panic!("no line")
    };
    assert_eq!(l.from, End::Free(3, 3));
    assert_eq!(l.to, bound("bbbbbbbb"));
}

#[test]
fn bad_commands_fail_and_leave_the_canvas_alone() {
    let (mut c, _) = setup();
    let before = c.clone();
    assert!(c.apply(Command::Remove(Id::from("nothing0"))).is_err());
    assert!(c.apply(Command::Insert(0, shape("aaaaaaaa", 0))).is_err());
    let bad = Command::SetEnd {
        id: Id::from("line0000"),
        side: Side::To,
        end: bound("ghost000"),
    };
    assert!(c
        .apply(Command::Batch(vec![
            Command::Move {
                id: Id::from("aaaaaaaa"),
                dx: 5,
                dy: 0
            },
            bad
        ]))
        .is_err());
    assert_eq!(c, before);
}

#[test]
fn random_ids_are_eight_characters() {
    assert_eq!(Id::random().0.len(), 8);
}

#[test]
fn set_line_kind_undoes_and_rejects_shapes() {
    let mut c = Canvas::default();
    c.push(shape("aaaaaaaa", 0)).unwrap();
    c.push(arrow(bound("aaaaaaaa"), End::Free(9, 9))).unwrap();
    let set = |kind| Command::SetLineKind {
        id: Id::from("line0000"),
        kind,
    };
    let undo = c.apply(set(LineKind::ArrowStart)).unwrap();
    assert!(matches!(&c.elements[1], Element::Line(l) if l.kind == LineKind::ArrowStart));
    c.apply(undo).unwrap();
    assert!(matches!(&c.elements[1], Element::Line(l) if l.kind == LineKind::Arrow));
    let on_shape = Command::SetLineKind {
        id: Id::from("aaaaaaaa"),
        kind: LineKind::Line,
    };
    assert!(c.apply(on_shape).is_err());
}

#[test]
fn bound_route_is_a_z_through_the_middle_column() {
    let cells = route(
        (0, 0),
        Some(Axis::Horizontal),
        (10, 4),
        Some(Axis::Horizontal),
    );
    assert_eq!(corners(&cells), [(0, 0), (5, 0), (5, 4), (10, 4)]);
    assert_eq!(cells.len(), 6 + 4 + 5);
    assert_eq!(label_pos(&cells), (2, 0));
}

#[test]
fn route_between_aligned_anchors_is_straight() {
    let cells = route(
        (2, 3),
        Some(Axis::Horizontal),
        (6, 3),
        Some(Axis::Horizontal),
    );
    assert_eq!(corners(&cells), [(2, 3), (6, 3)]);
}

#[test]
fn within_keeps_only_elements_fully_inside_the_area() {
    let c = Canvas {
        elements: vec![
            shape("aaaaaaaa", 0),
            shape("bbbbbbbb", 20),
            shape("cccccccc", 8),
            arrow(bound("aaaaaaaa"), bound("bbbbbbbb")),
            Element::Line(Line {
                id: Id::from("free0000"),
                kind: LineKind::Arrow,
                from: End::Free(1, 1),
                to: End::Free(30, 1),
                label: String::new(),
            }),
        ],
        ..Canvas::default()
    };
    let ids =
        |c: &Canvas| -> Vec<String> { c.elements.iter().map(|e| e.id().to_string()).collect() };
    // bbbbbbbb (x 20..26) straddles the right edge, cccccccc (x 8..14) is cut at x 12.
    let part = c.within((0, 0, 12, 3));
    assert_eq!(ids(&part), ["aaaaaaaa"]);
    let all = c.within((0, 0, 40, 3));
    assert_eq!(all.elements.len(), 5);
    let no_b = c.within((0, 0, 26, 3));
    assert_eq!(ids(&no_b), ["aaaaaaaa", "bbbbbbbb", "cccccccc", "line0000"]);
    assert!(c.within((100, 100, 5, 5)).elements.is_empty());
}

#[test]
fn within_drops_a_line_whose_bound_shape_is_outside() {
    let c = Canvas {
        elements: vec![
            shape("aaaaaaaa", 0),
            shape("bbbbbbbb", 20),
            arrow(bound("aaaaaaaa"), bound("bbbbbbbb")),
        ],
        ..Canvas::default()
    };
    // The line ends sit on the shape edges, so both ends fit in this area.
    let area = (0, 0, 21, 3);
    let kept = c.within(area);
    assert_eq!(kept.elements.len(), 1);
}
