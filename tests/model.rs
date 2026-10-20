use clidraw::model::{
    corners, label_pos, route, Axis, Canvas, Command, Element, End, History, Id, Line, LineKind,
    Shape, ShapeKind, Side, Z,
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

fn ids_of(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("shape{i:03}")).collect()
}

fn row(n: usize) -> Canvas {
    let elements = ids_of(n).iter().map(|id| shape(id, 0)).collect();
    Canvas {
        elements,
        ..Canvas::default()
    }
}

fn moves(ids: &[String]) -> Vec<Command> {
    ids.iter()
        .enumerate()
        .map(|(i, id)| Command::Move {
            id: Id::from(id.as_str()),
            dx: i as i32 + 1,
            dy: 0,
        })
        .collect()
}

fn x_of(c: &Canvas, id: &str) -> i32 {
    c.shape(&Id::from(id)).unwrap().x
}

#[test]
fn large_batch_matches_one_by_one_application() {
    let mut batched = row(30);
    let original = batched.clone();
    let mut single = batched.clone();
    let cmds = moves(&ids_of(30)[..20]);
    let undo = batched.apply(Command::Batch(cmds.clone())).unwrap();
    for c in cmds {
        single.apply(c).unwrap();
    }
    assert_eq!(batched, single);
    assert_eq!(x_of(&batched, "shape019"), 20);
    batched.apply(undo).unwrap();
    assert_eq!(batched, original);
}

#[test]
fn large_batch_survives_an_insert_in_the_middle() {
    let mut c = row(12);
    let mut cmds = moves(&ids_of(12)[..9]);
    cmds.push(Command::Insert(0, shape("newshape", 0)));
    cmds.push(Command::Move {
        id: Id::from("shape000"),
        dx: 100,
        dy: 0,
    });
    c.apply(Command::Batch(cmds)).unwrap();
    assert_eq!(x_of(&c, "shape000"), 101);
    assert_eq!(x_of(&c, "newshape"), 0);
}

#[test]
fn large_batch_survives_a_remove_in_the_middle() {
    let mut c = row(12);
    let mut cmds = moves(&ids_of(12)[..9]);
    cmds.push(Command::Remove(Id::from("shape000")));
    cmds.push(Command::Move {
        id: Id::from("shape011"),
        dx: 100,
        dy: 0,
    });
    c.apply(Command::Batch(cmds)).unwrap();
    assert!(c.get(&Id::from("shape000")).is_none());
    assert_eq!(x_of(&c, "shape011"), 100);
    assert_eq!(x_of(&c, "shape001"), 2);
}

#[test]
fn retain_present_drops_missing_ids_in_both_branches() {
    let c = row(20);
    for n in [3, 12] {
        let mut ids: Vec<Id> = ids_of(n).iter().map(|s| Id::from(s.as_str())).collect();
        ids.insert(1, Id::from("ghost000"));
        c.retain_present(&mut ids);
        let want: Vec<Id> = ids_of(n).iter().map(|s| Id::from(s.as_str())).collect();
        assert_eq!(ids, want);
    }
}

#[test]
fn history_keeps_only_the_newest_entries_up_to_its_limit() {
    let mut c = Canvas::default();
    let mut h = History::with_limit(3);
    for i in 0..5 {
        h.exec(
            &mut c,
            Command::Insert(i, shape(&format!("shape{i:03}"), 0)),
        )
        .unwrap();
    }
    let mut undone = 0;
    while h.undo(&mut c).unwrap() {
        undone += 1;
    }
    assert_eq!(undone, 3);
    assert_eq!(c.elements.len(), 2);
}

fn order(c: &Canvas) -> Vec<String> {
    c.elements.iter().map(|e| e.id().to_string()).collect()
}

fn five() -> Canvas {
    let mut c = Canvas::default();
    for id in ["aaaaaaaa", "bbbbbbbb", "cccccccc", "dddddddd", "eeeeeeee"] {
        c.push(shape(id, 0)).unwrap();
    }
    c
}

fn z(c: &mut Canvas, ids: &[&str], z: Z) -> Vec<String> {
    let sel: Vec<Id> = ids.iter().map(|i| Id::from(*i)).collect();
    c.apply(Command::Batch(c.z_order(&sel, z))).unwrap();
    order(c)
}

#[test]
fn z_order_moves_a_selection_as_a_block() {
    let want = |s: &[&str]| s.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let sel = ["bbbbbbbb", "cccccccc"];
    assert_eq!(
        z(&mut five(), &sel, Z::Forward),
        want(&["aaaaaaaa", "dddddddd", "bbbbbbbb", "cccccccc", "eeeeeeee"])
    );
    assert_eq!(
        z(&mut five(), &["cccccccc", "dddddddd"], Z::Backward),
        want(&["aaaaaaaa", "cccccccc", "dddddddd", "bbbbbbbb", "eeeeeeee"])
    );
    assert_eq!(
        z(&mut five(), &["dddddddd", "bbbbbbbb"], Z::Front),
        want(&["aaaaaaaa", "cccccccc", "eeeeeeee", "bbbbbbbb", "dddddddd"])
    );
    assert_eq!(
        z(&mut five(), &["dddddddd", "bbbbbbbb"], Z::Back),
        want(&["bbbbbbbb", "dddddddd", "aaaaaaaa", "cccccccc", "eeeeeeee"])
    );
}

#[test]
fn z_order_at_the_limit_or_unknown_ids_emits_nothing() {
    let c = five();
    let top = [Id::from("eeeeeeee")];
    let bottom = [Id::from("aaaaaaaa")];
    assert!(c.z_order(&top, Z::Forward).is_empty());
    assert!(c.z_order(&top, Z::Front).is_empty());
    assert!(c.z_order(&bottom, Z::Backward).is_empty());
    assert!(c.z_order(&bottom, Z::Back).is_empty());
    assert!(c.z_order(&[Id::from("zzzzzzzz")], Z::Front).is_empty());
}

#[test]
fn move_to_undoes_and_keeps_arrows_bound() {
    let (mut c, _) = setup();
    let before = c.clone();
    let undo = c
        .apply(Command::MoveTo {
            id: Id::from("aaaaaaaa"),
            to: 2,
        })
        .unwrap();
    assert_eq!(order(&c), ["bbbbbbbb", "line0000", "aaaaaaaa"]);
    let Element::Line(l) = &c.elements[1] else {
        panic!("no line")
    };
    assert_eq!(l.from, bound("aaaaaaaa"));
    c.apply(undo).unwrap();
    assert_eq!(c, before);
    assert!(c
        .apply(Command::MoveTo {
            id: Id::from("zzzzzzzz"),
            to: 0
        })
        .is_err());
}
