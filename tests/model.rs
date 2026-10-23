use clidraw::model::{
    corners, edge_anchor, label_pos, route, side_mid, wrap, Anchor, Axis, Canvas, Command, Element,
    End, History, Id, Line, LineKind, Shape, ShapeKind, Side, Stroke, Text, Z,
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
        stroke: Default::default(),
        border: Default::default(),
        color: None,
        fill: None,
        align: Default::default(),
        bold: false,
        link: None,
    })
}

fn arrow(from: End, to: End) -> Element {
    Element::Line(Line {
        id: Id::from("line0000"),
        kind: LineKind::Arrow,
        from,
        to,
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
    })
}

fn bound(id: &str) -> End {
    End::Bound(Id::from(id), Anchor::Auto)
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
    while h.undo(&mut c).unwrap().is_some() {}
    assert!(c.elements.is_empty());
    while h.redo(&mut c).unwrap().is_some() {}
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
    assert_eq!(h.redo(&mut c).unwrap(), None);
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
    assert_eq!(l.to, End::Free(19, 1));
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
    assert_eq!(c.line_ends(&l).1, (19, 11));
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
    assert_eq!(label_pos(&cells, None), (2, 0));
}

#[test]
fn a_free_start_reaches_a_bound_end_along_that_end_axis() {
    let v = route((3, 2), None, (9, 8), Some(Axis::Vertical));
    assert_eq!(corners(&v), [(3, 2), (9, 2), (9, 8)]);
    let h = route((3, 2), None, (9, 8), Some(Axis::Horizontal));
    assert_eq!(corners(&h), [(3, 2), (3, 8), (9, 8)]);
}

#[test]
fn a_bend_on_the_route_leaves_it_unchanged() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 0));
    c.elements.push(shape("bbbbbbbb", 20));
    let Element::Line(mut l) = arrow(
        End::Bound(Id::from("aaaaaaaa"), Anchor::Right(1)),
        End::Bound(Id::from("bbbbbbbb"), Anchor::Left(1)),
    ) else {
        unreachable!()
    };
    let before = c.route(&l);
    l.bends = vec![before[before.len() / 2]];
    assert_eq!(c.route(&l), before);
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
                label_at: None,
                stroke: Default::default(),
                bends: vec![],
                color: None,
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
    while h.undo(&mut c).unwrap().is_some() {
        undone += 1;
    }
    assert_eq!(undone, 3);
    assert_eq!(c.elements.len(), 2);
}

fn framed(id: &str, x: i32, w: i32) -> Element {
    let Element::Shape(s) = shape(id, x) else {
        unreachable!()
    };
    Element::Shape(Shape {
        kind: ShapeKind::Frame,
        y: -1,
        w,
        h: 6,
        ..s
    })
}

#[test]
fn expand_frames_adds_wholly_inside_elements_only() {
    let mut c = Canvas::default();
    c.push(shape("aaaaaaaa", 1)).unwrap();
    c.push(shape("bbbbbbbb", 8)).unwrap();
    c.push(framed("ffffffff", 0, 10)).unwrap();
    let mut ids = vec![Id::from("ffffffff")];
    c.expand_frames(&mut ids);
    assert_eq!(ids, vec![Id::from("ffffffff"), Id::from("aaaaaaaa")]);
    let mut ids = vec![Id::from("bbbbbbbb")];
    c.expand_frames(&mut ids);
    assert_eq!(ids, vec![Id::from("bbbbbbbb")]);
}

#[test]
fn expand_frames_includes_a_nested_frame_and_its_contents() {
    let mut c = Canvas::default();
    c.push(shape("aaaaaaaa", 2)).unwrap();
    c.push(framed("inner000", 1, 8)).unwrap();
    c.push(framed("outer000", 0, 10)).unwrap();
    let mut ids = vec![Id::from("outer000")];
    c.expand_frames(&mut ids);
    assert_eq!(ids.len(), 3);
}

#[test]
fn layout_ignores_a_frame() {
    use clidraw::model::Dir;
    let mut c = Canvas::default();
    c.push(shape("aaaaaaaa", 1)).unwrap();
    c.push(framed("ffffffff", 0, 10)).unwrap();
    let sel = vec![Id::from("aaaaaaaa"), Id::from("ffffffff")];
    assert!(c.layout(&sel, Dir::TopDown).is_empty());
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

#[test]
fn clone_elements_remaps_bound_ends_and_shifts() {
    let (c, _) = setup();
    let ids = [Id::from("aaaaaaaa"), Id::from("bbbbbbbb")];
    let copy = c.clone_elements(&ids, 2, 1);
    assert_eq!(copy.len(), 3);
    let Element::Shape(a) = &copy[0] else {
        panic!()
    };
    let Element::Shape(b) = &copy[1] else {
        panic!()
    };
    assert_eq!((a.x, a.y, b.x), (2, 1, 22));
    assert_ne!(a.id, Id::from("aaaaaaaa"));
    let Element::Line(l) = &copy[2] else { panic!() };
    assert_eq!(
        (&l.from, &l.to),
        (
            &End::Bound(a.id.clone(), Anchor::Auto),
            &End::Bound(b.id.clone(), Anchor::Auto)
        )
    );
}

#[test]
fn clone_elements_frees_a_selected_line_end_whose_shape_stays() {
    let (c, _) = setup();
    let copy = c.clone_elements(&[Id::from("aaaaaaaa"), Id::from("line0000")], 0, 0);
    let Element::Line(l) = &copy[1] else { panic!() };
    assert!(matches!(l.from, End::Bound(..)));
    assert_eq!(l.to, End::Free(19, 1));
}

#[test]
fn clone_elements_skips_a_line_with_one_end_outside() {
    let (c, _) = setup();
    assert_eq!(c.clone_elements(&[Id::from("aaaaaaaa")], 0, 0).len(), 1);
}

#[test]
fn history_labels_name_the_reverted_action() {
    let (mut c, mut h) = setup();
    h.exec(&mut c, Command::Remove(Id::from("aaaaaaaa")))
        .unwrap();
    assert_eq!(h.undo(&mut c).unwrap().as_deref(), Some("delete 1 element"));
    assert_eq!(h.redo(&mut c).unwrap().as_deref(), Some("delete 1 element"));
    h.undo(&mut c).unwrap();
    let moves = ["aaaaaaaa", "bbbbbbbb"].map(|id| Command::Move {
        id: Id::from(id),
        dx: 1,
        dy: 0,
    });
    h.exec(&mut c, Command::Batch(moves.to_vec())).unwrap();
    assert_eq!(h.undo(&mut c).unwrap().as_deref(), Some("move 2 elements"));
}

fn wrapped(s: &str, room: usize) -> Vec<&str> {
    wrap(s, room)
        .into_iter()
        .map(|r| s[r].trim_end_matches([' ', '\n']))
        .collect()
}

#[test]
fn wrap_breaks_at_spaces_newlines_and_inside_long_words() {
    assert_eq!(wrap("ab cd", 3), [0..3, 3..5]);
    assert_eq!(wrapped("ab cd", 3), ["ab", "cd"]);
    assert_eq!(wrapped("abcdef", 4), ["abcd", "ef"]);
    assert_eq!(wrapped("a\nb", 5), ["a", "b"]);
    assert_eq!(wrapped("a\n", 5), ["a", ""]);
    assert_eq!(wrapped("", 5), [""]);
    assert_eq!(wrapped("ab  cd", 3), ["ab", "cd"]);
}

#[test]
fn wrap_ranges_are_contiguous_and_a_wide_char_does_not_hang() {
    let s = "ab 日本 cdefgh\nx";
    for room in 1..8 {
        let r = wrap(s, room);
        assert_eq!(r[0].start, 0);
        assert_eq!(r.last().unwrap().end, s.len());
        assert!(r.windows(2).all(|w| w[0].end == w[1].start));
    }
    assert_eq!(wrapped("日本", 1), ["日", "本"]);
}

fn pinned(anchor: Anchor) -> (Canvas, History) {
    let (mut c, mut h) = setup();
    h.exec(
        &mut c,
        Command::SetEnd {
            id: Id::from("line0000"),
            side: Side::From,
            end: End::Bound(Id::from("aaaaaaaa"), anchor),
        },
    )
    .unwrap();
    (c, h)
}

fn line_of(c: &Canvas) -> Line {
    let Element::Line(l) = c.elements.last().unwrap().clone() else {
        panic!()
    };
    l
}

#[test]
fn a_pinned_end_leaves_its_side_and_auto_faces_the_other_end() {
    let (c, _) = pinned(Anchor::Top(3));
    assert_eq!(c.line_ends(&line_of(&c)).0, (3, -1));
    let (c, _) = pinned(Anchor::Auto);
    assert_eq!(c.line_ends(&line_of(&c)).0, (6, 1));
}

#[test]
fn side_mid_lands_one_cell_outside_the_border() {
    let Element::Shape(mut s) = shape("aaaaaaaa", 2) else {
        panic!()
    };
    (s.y, s.w) = (1, 7);
    let at = |a, t| side_mid(&s, a, t).0;
    assert_eq!(at(Anchor::Top(3), (0, 0)), (5, 0));
    assert_eq!(at(Anchor::Bottom(3), (0, 0)), (5, 4));
    assert_eq!(at(Anchor::Left(1), (0, 0)), (1, 2));
    assert_eq!(at(Anchor::Right(1), (0, 0)), (9, 2));
    assert_eq!(at(Anchor::Top(1), (0, 0)), (3, 0));
    assert_eq!(at(Anchor::Top(0), (0, 0)), (2, 0));
    assert_eq!(at(Anchor::Top(99), (0, 0)), (8, 0));
    assert_eq!(at(Anchor::Left(-4), (0, 0)), (1, 1));
    assert_eq!(at(Anchor::Auto, (50, 2)), (9, 2));
    assert_eq!(at(Anchor::Auto, (5, 40)), (5, 4));
}

#[test]
fn edge_anchor_snaps_to_midpoint_clamps_corners_and_pulls_interior_to_the_border() {
    let mut s = match shape("aaaaaaaa", 0) {
        Element::Shape(s) => s,
        _ => panic!(),
    };
    (s.w, s.h) = (12, 7);
    assert_eq!(edge_anchor(&s, (7, 0)), Anchor::Top(6));
    assert_eq!(edge_anchor(&s, (9, 0)), Anchor::Top(9));
    assert_eq!(edge_anchor(&s, (0, 0)), Anchor::Top(1));
    assert_eq!(edge_anchor(&s, (11, 6)), Anchor::Bottom(10));
    assert_eq!(edge_anchor(&s, (0, 5)), Anchor::Left(5));
    assert_eq!(edge_anchor(&s, (11, 1)), Anchor::Right(1));
    assert_eq!(edge_anchor(&s, (2, 3)), Anchor::Left(3));
    assert_eq!(edge_anchor(&s, (12, 3)), Anchor::Auto);
    assert_eq!(edge_anchor(&s, (5, -1)), Anchor::Auto);
    (s.w, s.h) = (2, 2);
    assert_eq!(edge_anchor(&s, (0, 0)), Anchor::Top(1));
    assert_eq!(edge_anchor(&s, (1, 1)), Anchor::Bottom(1));
}

#[test]
fn undoing_a_shape_delete_restores_the_anchor() {
    let (mut c, mut h) = pinned(Anchor::Top(3));
    h.exec(&mut c, Command::Remove(Id::from("aaaaaaaa")))
        .unwrap();
    assert!(matches!(line_of(&c).from, End::Free(..)));
    h.undo(&mut c).unwrap();
    assert_eq!(
        line_of(&c).from,
        End::Bound(Id::from("aaaaaaaa"), Anchor::Top(3))
    );
}

#[test]
fn clone_elements_keeps_the_anchor() {
    let (c, _) = pinned(Anchor::Left(1));
    let copy = c.clone_elements(&[Id::from("aaaaaaaa"), Id::from("bbbbbbbb")], 0, 5);
    let Element::Line(l) = &copy[2] else { panic!() };
    assert!(matches!(l.from, End::Bound(_, Anchor::Left(1))));
}

fn ids(s: &[&str]) -> Vec<Id> {
    s.iter().map(|i| Id::from(*i)).collect()
}

fn grouped(c: &mut Canvas, groups: &[&[&str]]) {
    let all = groups.iter().map(|g| ids(g)).collect();
    c.apply(Command::SetGroups(all)).unwrap();
}

#[test]
fn group_cmd_needs_two_elements_and_sets_one_group() {
    let mut c = five();
    assert_eq!(c.group_cmd(&ids(&["aaaaaaaa"])), None);
    let cmd = c.group_cmd(&ids(&["aaaaaaaa", "bbbbbbbb"])).unwrap();
    let undo = c.apply(cmd).unwrap();
    assert_eq!(c.groups, vec![ids(&["aaaaaaaa", "bbbbbbbb"])]);
    c.apply(undo).unwrap();
    assert!(c.groups.is_empty());
}

#[test]
fn regrouping_a_member_moves_it_and_drops_a_group_left_with_one() {
    let mut c = five();
    grouped(&mut c, &[&["aaaaaaaa", "bbbbbbbb"]]);
    let cmd = c.group_cmd(&ids(&["bbbbbbbb", "cccccccc"])).unwrap();
    c.apply(cmd).unwrap();
    assert_eq!(c.groups, vec![ids(&["bbbbbbbb", "cccccccc"])]);
}

#[test]
fn ungroup_cmd_removes_the_whole_group_on_a_partial_selection() {
    let mut c = five();
    grouped(
        &mut c,
        &[
            &["aaaaaaaa", "bbbbbbbb", "cccccccc"],
            &["dddddddd", "eeeeeeee"],
        ],
    );
    assert_eq!(c.ungroup_cmd(&ids(&["ffffffff"])), None);
    let cmd = c.ungroup_cmd(&ids(&["bbbbbbbb"])).unwrap();
    c.apply(cmd).unwrap();
    assert_eq!(c.groups, vec![ids(&["dddddddd", "eeeeeeee"])]);
}

#[test]
fn removing_a_member_then_undoing_restores_the_group() {
    let mut c = five();
    grouped(&mut c, &[&["aaaaaaaa", "bbbbbbbb"]]);
    let undo = c.apply(Command::Remove(Id::from("aaaaaaaa"))).unwrap();
    let mut sel = ids(&["bbbbbbbb"]);
    c.expand_groups(&mut sel);
    assert_eq!(sel, ids(&["bbbbbbbb"]));
    c.apply(undo).unwrap();
    let mut sel = ids(&["bbbbbbbb"]);
    c.expand_groups(&mut sel);
    assert_eq!(sel, ids(&["bbbbbbbb", "aaaaaaaa"]));
}

#[test]
fn clone_canvas_remaps_groups_to_the_fresh_ids() {
    let mut c = five();
    grouped(
        &mut c,
        &[&["aaaaaaaa", "bbbbbbbb"], &["cccccccc", "dddddddd"]],
    );
    let copy = c.clone_canvas(&ids(&["aaaaaaaa", "bbbbbbbb", "cccccccc"]), 2, 1);
    assert_eq!(copy.elements.len(), 3);
    assert_eq!(copy.groups.len(), 1);
    let fresh: Vec<Id> = copy.elements[..2].iter().map(|e| e.id().clone()).collect();
    assert_eq!(copy.groups[0], fresh);
}

#[test]
fn undoing_only_a_group_change_reads_group_change() {
    let mut c = five();
    let undo = c
        .apply(c.group_cmd(&ids(&["aaaaaaaa", "bbbbbbbb"])).unwrap())
        .unwrap();
    assert_eq!(undo.label(), "group change");
}

#[test]
fn stroke_defaults_to_solid_when_absent_and_stays_out_of_solid_json() {
    let old =
        r#"{"type":"shape","id":"aaaaaaaa","kind":"Rect","x":0,"y":0,"w":6,"h":3,"label":""}"#;
    let Element::Shape(s) = serde_json::from_str::<Element>(old).unwrap() else {
        panic!("not a shape")
    };
    assert_eq!(s.stroke, Stroke::Solid);
    assert!(!serde_json::to_string(&Element::Shape(s.clone()))
        .unwrap()
        .contains("stroke"));
    let dashed = Element::Shape(Shape {
        stroke: Stroke::Dashed,
        ..s
    });
    assert!(serde_json::to_string(&dashed)
        .unwrap()
        .contains(r#""stroke":"dashed""#));
}

#[test]
fn origin_is_the_minimum_corner_of_shapes_and_text_or_zero() {
    let mut c = Canvas::default();
    assert_eq!(c.origin(), (0, 0));
    c.elements.push(shape("aaaaaaaa", 7));
    c.elements.push(shape("bbbbbbbb", 3));
    assert_eq!(c.origin(), (3, 0));
}

#[test]
fn snap_size_holds_the_ratio_of_bound_kinds() {
    assert_eq!(Shape::snap_size(ShapeKind::Diamond, 12, 5), (11, 6));
    assert_eq!(Shape::snap_size(ShapeKind::Diamond, 1, 1), (5, 3));
    assert_eq!(Shape::snap_size(ShapeKind::Hexagon, 3, 3), (6, 3));
    assert_eq!(Shape::snap_size(ShapeKind::Parallelogram, 20, 2), (12, 6));
}

#[test]
fn snap_size_leaves_free_kinds_alone() {
    for kind in [ShapeKind::Rect, ShapeKind::Cylinder, ShapeKind::Frame] {
        assert_eq!(Shape::snap_size(kind, 7, 2), (7, 2));
    }
}

fn rect(id: &str, kind: ShapeKind, (x, y, w, h): (i32, i32, i32, i32)) -> Element {
    let Element::Shape(s) = shape(id, x) else {
        unreachable!()
    };
    Element::Shape(Shape { kind, y, w, h, ..s })
}

fn elbow(id: &str, from: End, to: End) -> Line {
    let Element::Line(l) = arrow(from, to) else {
        unreachable!()
    };
    Line {
        id: Id::from(id),
        ..l
    }
}

/// A at (0,4), B at (20,4) and an `Auto` arrow between them, which leaves A at (3,5) and enters B at (19,5).
fn ab(extra: Vec<Element>) -> (Canvas, Line) {
    let line = elbow("arrow000", bound("aaaaaaaa"), bound("bbbbbbbb"));
    let mut c = Canvas {
        elements: vec![
            rect("aaaaaaaa", ShapeKind::Rect, (0, 4, 3, 3)),
            rect("bbbbbbbb", ShapeKind::Rect, (20, 4, 3, 3)),
        ],
        ..Canvas::default()
    };
    c.elements.extend(extra);
    c.elements.push(Element::Line(line.clone()));
    (c, line)
}

fn inside(p: (i32, i32), (x, y, w, h): (i32, i32, i32, i32)) -> bool {
    p.0 >= x && p.0 < x + w && p.1 >= y && p.1 < y + h
}

fn walks(cells: &[(i32, i32)]) {
    assert!(cells
        .windows(2)
        .all(|w| (w[0].0 - w[1].0).abs() + (w[0].1 - w[1].1).abs() == 1));
}

const O: (i32, i32, i32, i32) = (10, 4, 3, 3);

#[test]
fn an_elbow_goes_around_a_shape_in_its_way() {
    let (c, l) = ab(vec![rect("oooooooo", ShapeKind::Rect, O)]);
    let cells = c.route(&l);
    assert_eq!((cells[0], cells[cells.len() - 1]), ((3, 5), (19, 5)));
    walks(&cells);
    assert!(!cells.iter().any(|&p| inside(p, O)));
}

fn diagonal(extra: Vec<Element>) -> (Canvas, Line) {
    let line = elbow("arrow000", bound("aaaaaaaa"), bound("bbbbbbbb"));
    let mut c = Canvas {
        elements: vec![
            rect("aaaaaaaa", ShapeKind::Rect, (0, 4, 3, 3)),
            rect("bbbbbbbb", ShapeKind::Rect, (6, 9, 3, 3)),
        ],
        ..Canvas::default()
    };
    c.elements.extend(extra);
    c.elements.push(Element::Line(line.clone()));
    (c, line)
}

#[test]
fn an_arrow_ends_with_a_straight_stub() {
    let (c, l) = diagonal(vec![]);
    let cells = c.route(&l);
    walks(&cells);
    assert_eq!(cells[cells.len() - 3..], [(7, 6), (7, 7), (7, 8)]);
}

#[test]
fn an_arrow_keeps_its_stub_when_it_detours() {
    let o = (3, 7, 3, 1);
    let (c, l) = diagonal(vec![rect("oooooooo", ShapeKind::Rect, o)]);
    let cells = c.route(&l);
    walks(&cells);
    assert!(!cells.iter().any(|&p| inside(p, o)));
    assert_eq!(cells[cells.len() - 3..], [(7, 6), (7, 7), (7, 8)]);
}

#[test]
fn a_far_target_detours_through_a_gap_between_shapes() {
    let (left, right) = ((50, 24, 14, 7), (66, 24, 14, 7));
    let line = elbow(
        "wdkmx63a",
        End::Bound(Id::from("98acfohp"), Anchor::Right(2)),
        End::Bound(Id::from("2295v0fq"), Anchor::Bottom(4)),
    );
    let c = Canvas {
        elements: vec![
            rect("98acfohp", ShapeKind::Rect, (17, 18, 19, 5)),
            rect("kamkkbpr", ShapeKind::Parallelogram, left),
            rect("2295v0fq", ShapeKind::Parallelogram, right),
            rect("yhvz4tlz", ShapeKind::Rect, (-15, 23, 33, 12)),
            rect("m22txh0p", ShapeKind::Parallelogram, (35, 32, 22, 11)),
            Element::Line(line.clone()),
        ],
        ..Canvas::default()
    };
    let cells = c.route(&line);
    walks(&cells);
    assert_eq!(cells[cells.len() - 1], (70, 31));
    assert!(!inside(cells[0], (17, 18, 19, 5)));
    assert!(!cells.iter().any(|&p| inside(p, left) || inside(p, right)));
}

#[test]
fn a_clear_elbow_keeps_the_plain_route() {
    let (c, l) = ab(vec![]);
    let plain = route(
        (3, 5),
        Some(Axis::Horizontal),
        (19, 5),
        Some(Axis::Horizontal),
    );
    assert_eq!(c.route(&l), plain);
}

#[test]
fn a_text_is_an_obstacle() {
    let (c, l) = ab(vec![Element::Text(Text {
        id: Id::from("tttttttt"),
        x: 10,
        y: 5,
        text: "xxxx".into(),
        color: None,
    })]);
    assert!(!c.route(&l).iter().any(|&p| inside(p, (10, 5, 4, 1))));
}

#[test]
fn an_obstacle_taller_than_the_margin_is_still_dodged() {
    let tall = (10, -2, 3, 14);
    let (c, l) = ab(vec![rect("tttttttt", ShapeKind::Rect, tall)]);
    let cells = c.route(&l);
    walks(&cells);
    assert!(!cells.iter().any(|&p| inside(p, tall)));
}

#[test]
fn a_wall_costs_the_fewest_cells_inside_it() {
    let wall = (10, -40, 3, 100);
    let (c, l) = ab(vec![rect("wwwwwwww", ShapeKind::Rect, wall)]);
    let cells = c.route(&l);
    walks(&cells);
    assert_eq!(cells.iter().filter(|&&p| inside(p, wall)).count(), 3);
    assert!(cells.iter().all(|&p| inside(p, (0, 2, 23, 7))));
}

#[test]
fn a_frame_between_the_ends_leaves_the_plain_route_unchanged() {
    let (c, l) = ab(vec![rect("ffffffff", ShapeKind::Frame, (10, 4, 5, 3))]);
    let (plain, _) = ab(vec![]);
    assert_eq!(c.route(&l), plain.route(&l));
}

#[test]
fn a_shape_inside_a_frame_around_both_ends_is_an_obstacle() {
    let (c, l) = ab(vec![
        rect("ffffffff", ShapeKind::Frame, (-2, 2, 27, 10)),
        rect("cccccccc", ShapeKind::Rect, O),
    ]);
    let cells = c.route(&l);
    assert!(!cells.iter().any(|&p| inside(p, O)));
}

#[test]
fn stacking_puts_frames_last() {
    let (c, _) = ab(vec![rect("ffffffff", ShapeKind::Frame, O)]);
    assert_eq!(c.stacking().collect::<Vec<_>>(), vec![0, 1, 3, 2]);
}

#[test]
fn other_lines_only_break_ties_between_clean_paths() {
    for (row, free_row) in [(7, 3), (3, 7)] {
        let (mut c, l) = ab(vec![rect("oooooooo", ShapeKind::Rect, O)]);
        let other = elbow("other000", End::Free(8, free_row), End::Free(14, free_row));
        c.elements.push(Element::Line(other));
        let cells = c.route(&l);
        assert!(cells.iter().any(|&p| p.1 == row && (8..14).contains(&p.0)));
        assert!(!cells
            .iter()
            .any(|&p| p.1 == free_row && (8..14).contains(&p.0)));
    }
}

#[test]
fn an_arrow_does_not_share_a_segment_with_another_arrow() {
    let other = elbow("other000", End::Free(8, 5), End::Free(14, 5));
    let (c, l) = ab(vec![Element::Line(other.clone())]);
    let cells = c.route(&l);
    let theirs = c.route(&other);
    assert_eq!((cells[0], cells[cells.len() - 1]), ((3, 5), (19, 5)));
    walks(&cells);
    assert!(!cells
        .windows(2)
        .any(|w| theirs.contains(&w[0]) && theirs.contains(&w[1])));
}

#[test]
fn an_unavoidable_crossing_keeps_the_plain_route() {
    let other = elbow("other000", End::Free(10, 0), End::Free(10, 12));
    let (c, l) = ab(vec![Element::Line(other)]);
    let plain = route(
        (3, 5),
        Some(Axis::Horizontal),
        (19, 5),
        Some(Axis::Horizontal),
    );
    assert_eq!(c.route(&l), plain);
}

#[test]
fn an_arrow_detours_around_a_crossing_it_can_avoid() {
    let other = elbow("other000", End::Free(10, 3), End::Free(10, 7));
    let (c, l) = ab(vec![Element::Line(other.clone())]);
    let cells = c.route(&l);
    walks(&cells);
    assert_eq!((cells[0], cells[cells.len() - 1]), ((3, 5), (19, 5)));
    assert!(!cells.iter().any(|p| c.route(&other).contains(p)));
}

#[test]
fn an_earlier_arrow_keeps_its_plain_route_when_a_later_one_crosses_it() {
    let other = elbow("other000", End::Free(10, 3), End::Free(10, 7));
    let (mut c, _) = ab(vec![]);
    c.elements.insert(2, Element::Line(other.clone()));
    assert_eq!(
        c.route(&other),
        (3..=7).map(|y| (10, y)).collect::<Vec<_>>()
    );
}

#[test]
fn arrows_leaving_one_cell_in_opposite_directions_keep_their_plain_routes() {
    let up = elbow("up000000", End::Free(5, 5), End::Free(5, 0));
    let down = elbow("down0000", End::Free(5, 5), End::Free(5, 10));
    let c = Canvas {
        elements: vec![Element::Line(up.clone()), Element::Line(down.clone())],
        ..Canvas::default()
    };
    assert_eq!(
        c.route(&up),
        (0..=5).rev().map(|y| (5, y)).collect::<Vec<_>>()
    );
    assert_eq!(c.route(&down), (5..=10).map(|y| (5, y)).collect::<Vec<_>>());
}

#[test]
fn the_route_follows_a_moved_obstacle_live() {
    let (mut c, l) = ab(vec![rect("oooooooo", ShapeKind::Rect, O)]);
    let plain = route(
        (3, 5),
        Some(Axis::Horizontal),
        (19, 5),
        Some(Axis::Horizontal),
    );
    let detour = c.route(&l);
    assert_ne!(detour, plain);
    let mv = |dy| Command::Move {
        id: Id::from("oooooooo"),
        dx: 0,
        dy,
    };
    c.apply(mv(20)).unwrap();
    assert_eq!(c.route(&l), plain);
    c.apply(mv(-20)).unwrap();
    assert_eq!(c.route(&l), detour);
}

#[test]
fn a_bent_line_keeps_its_bends_and_goes_around_an_obstacle() {
    let wall = rect("wwwwwwww", ShapeKind::Rect, O);
    let (mut c, _) = ab(vec![wall]);
    let bend = (16, 9);
    let Some(Element::Line(l)) = c.elements.last_mut() else {
        unreachable!()
    };
    l.bends = vec![bend];
    let l = l.clone();
    let cells = c.route(&l);
    walks(&cells);
    assert!(cells.contains(&bend));
    assert!(!cells.iter().any(|&p| inside(p, O)));
}

#[test]
fn a_bent_arrow_avoids_its_own_start_shape() {
    let start = (52, 7, 24, 7);
    let end = (75, 23, 14, 5);
    let (a, f) = (Id::from("ot2efij1"), Id::from("a5iawbhl"));
    let mut line = elbow(
        "nl9llhzp",
        End::Bound(a, Anchor::Top(12)),
        End::Bound(f, Anchor::Left(2)),
    );
    line.bends = vec![(82, 6), (76, 11), (63, 17), (69, 21), (82, 23)];
    let c = Canvas {
        elements: vec![
            rect("ot2efij1", ShapeKind::Rect, start),
            rect("a5iawbhl", ShapeKind::Frame, end),
            Element::Line(line.clone()),
        ],
        ..Canvas::default()
    };
    let cells = c.route(&line);
    walks(&cells);
    for b in &line.bends {
        assert!(cells.contains(b));
    }
    let bad: Vec<_> = cells
        .iter()
        .filter(|&&p| !line.bends.contains(&p) && inside(p, start))
        .collect();
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn bends_stay_put_when_a_bound_shape_moves() {
    let (mut c, mut line) = ab(vec![]);
    line.bends = vec![(11, 9)];
    *c.elements.last_mut().unwrap() = Element::Line(line.clone());
    c.apply(Command::Move {
        id: Id::from("bbbbbbbb"),
        dx: 0,
        dy: 6,
    })
    .unwrap();
    let cells = c.route(&line);
    assert!(cells.contains(&(11, 9)));
    assert_eq!(cells[0].0, 3);
}

#[test]
fn set_bends_returns_the_old_list_and_cloned_lines_shift_their_bends() {
    let mut c = Canvas::default();
    let mut line = elbow("arrow000", End::Free(0, 0), End::Free(10, 0));
    line.bends = vec![(5, 4)];
    c.elements.push(Element::Line(line));
    let undo = c
        .apply(Command::SetBends {
            id: Id::from("arrow000"),
            bends: vec![],
        })
        .unwrap();
    c.apply(undo).unwrap();
    let Some(Element::Line(l)) = c.get(&Id::from("arrow000")) else {
        unreachable!()
    };
    assert_eq!(l.bends, vec![(5, 4)]);
    let copy = c.clone_elements(&[Id::from("arrow000")], 2, 3);
    let Element::Line(copy) = &copy[0] else {
        unreachable!()
    };
    assert_eq!(copy.bends, vec![(7, 7)]);
}

#[test]
fn arrows_into_the_same_side_run_the_same_direction_for_four_cells() {
    let into = |id: &str, from: &str, off: i32| {
        elbow(
            id,
            bound(from),
            End::Bound(Id::from("bbbbbbbb"), Anchor::Left(off)),
        )
    };
    let (top, bottom) = (
        into("arrow001", "aaaaaaaa", 0),
        into("arrow002", "cccccccc", 2),
    );
    let mut c = Canvas {
        elements: vec![
            rect("aaaaaaaa", ShapeKind::Rect, (10, 0, 3, 3)),
            rect("cccccccc", ShapeKind::Rect, (10, 10, 3, 3)),
            rect("bbbbbbbb", ShapeKind::Rect, (20, 4, 3, 3)),
        ],
        ..Canvas::default()
    };
    c.elements
        .extend([top.clone(), bottom.clone()].map(Element::Line));
    for (l, y) in [(top, 4), (bottom, 6)] {
        let cells = c.route(&l);
        walks(&cells);
        assert_eq!(
            cells[cells.len() - 5..],
            [15, 16, 17, 18, 19].map(|x| (x, y))
        );
    }
}
