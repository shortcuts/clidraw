use clidraw::model::{
    corners, label_pos, route, straight, wrap, Anchor, Axis, Canvas, Command, Element, End,
    History, Id, Line, LineKind, Routing, Shape, ShapeKind, Side, Stroke, Z,
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
        routing: Default::default(),
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
    assert_eq!(label_pos(&cells, None), (2, 0));
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
fn straight_joins_the_endpoints_in_unit_steps() {
    assert_eq!(
        straight((0, 0), (4, 2)),
        [(0, 0), (1, 1), (2, 1), (3, 2), (4, 2)]
    );
    assert_eq!(straight((3, 3), (3, 3)), [(3, 3)]);
    assert_eq!(straight((2, 0), (0, 2)), [(2, 0), (1, 1), (0, 2)]);
}

#[test]
fn routing_defaults_to_elbow_when_absent_and_stays_out_of_elbow_json() {
    let old = r#"{"type":"line","id":"aaaaaaaa","kind":"Arrow","from":{"free":[0,0]},"to":{"free":[4,2]},"label":""}"#;
    let Element::Line(l) = serde_json::from_str::<Element>(old).unwrap() else {
        panic!("not a line")
    };
    assert_eq!(l.routing, Routing::Elbow);
    assert!(!serde_json::to_string(&Element::Line(l.clone()))
        .unwrap()
        .contains("routing"));
    let direct = Element::Line(Line {
        routing: Routing::Straight,
        color: None,
        ..l
    });
    assert!(serde_json::to_string(&direct)
        .unwrap()
        .contains(r#""routing":"straight""#));
}

#[test]
fn set_routing_changes_the_route_and_undoes_as_restyle() {
    let mut c = Canvas::default();
    let mut h = History::default();
    let id = Id::from("llllllll");
    let line = Element::Line(Line {
        id: id.clone(),
        kind: LineKind::Line,
        from: End::Free(0, 0),
        to: End::Free(4, 2),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        routing: Default::default(),
        color: None,
    });
    h.exec(&mut c, Command::Insert(0, line)).unwrap();
    let routing = |c: &Canvas| match c.get(&id) {
        Some(Element::Line(l)) => (l.routing, c.route(l)),
        _ => panic!("no line"),
    };
    let cmd = Command::SetRouting {
        id: id.clone(),
        routing: Routing::Straight,
    };
    h.exec(&mut c, cmd).unwrap();
    assert_eq!(routing(&c), (Routing::Straight, straight((0, 0), (4, 2))));
    assert_eq!(
        h.undo(&mut c).unwrap().as_deref(),
        Some("restyle 1 element")
    );
    assert_eq!(routing(&c).0, Routing::Elbow);
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
                routing: Default::default(),
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
    assert_eq!(l.to, End::Free(20, 1));
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
    let (c, _) = pinned(Anchor::Top);
    assert_eq!(c.line_ends(&line_of(&c)).0, (3, 0));
    let (c, _) = pinned(Anchor::Auto);
    assert_eq!(c.line_ends(&line_of(&c)).0, (5, 1));
}

#[test]
fn undoing_a_shape_delete_restores_the_anchor() {
    let (mut c, mut h) = pinned(Anchor::Top);
    h.exec(&mut c, Command::Remove(Id::from("aaaaaaaa")))
        .unwrap();
    assert!(matches!(line_of(&c).from, End::Free(..)));
    h.undo(&mut c).unwrap();
    assert_eq!(
        line_of(&c).from,
        End::Bound(Id::from("aaaaaaaa"), Anchor::Top)
    );
}

#[test]
fn clone_elements_keeps_the_anchor() {
    let (c, _) = pinned(Anchor::Left);
    let copy = c.clone_elements(&[Id::from("aaaaaaaa"), Id::from("bbbbbbbb")], 0, 5);
    let Element::Line(l) = &copy[2] else { panic!() };
    assert!(matches!(l.from, End::Bound(_, Anchor::Left)));
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
