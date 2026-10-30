use clidraw::config::CommandUi;
use clidraw::input::{
    handle_key, handle_mouse, handle_paste, Edit, EditTarget, Handle, Hit, KeyOutcome, Session,
    Tool,
};
use clidraw::model::{
    Align, Anchor, Border, Canvas, Edge, Element, End, Id, Line, LineKind, Shape, ShapeKind,
    SideLabel, Stroke, TextAlign,
};
use clidraw::ui::palette::{self, Choice, PaletteHit, Strip};
use clidraw::view::{View, Zoom};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::time::{Duration, Instant};

fn session() -> Session {
    Session::new(Canvas::default(), View::default(), (80, 24))
}

fn mouse(s: &mut Session, kind: MouseEventKind, col: u16, row: u16, now: Instant) {
    mouse_mod(s, kind, KeyModifiers::NONE, col, row, now);
}

fn mouse_mod(
    s: &mut Session,
    kind: MouseEventKind,
    modifiers: KeyModifiers,
    col: u16,
    row: u16,
    now: Instant,
) {
    let ev = MouseEvent {
        kind,
        column: col,
        row,
        modifiers,
    };
    handle_mouse(s, ev, now);
}

fn drag(s: &mut Session, from: (u16, u16), to: (u16, u16)) {
    let t = Instant::now();
    mouse(
        s,
        MouseEventKind::Down(MouseButton::Left),
        from.0,
        from.1,
        t,
    );
    mouse(s, MouseEventKind::Drag(MouseButton::Left), to.0, to.1, t);
    mouse(s, MouseEventKind::Up(MouseButton::Left), to.0, to.1, t);
}

fn click(s: &mut Session, at: (u16, u16), now: Instant) {
    mouse(s, MouseEventKind::Down(MouseButton::Left), at.0, at.1, now);
    mouse(s, MouseEventKind::Up(MouseButton::Left), at.0, at.1, now);
}

fn key(s: &mut Session, code: KeyCode) -> KeyOutcome {
    handle_key(s, KeyEvent::new(code, KeyModifiers::NONE))
}

fn ctrl(s: &mut Session, c: char) {
    handle_key(s, KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
}

fn pick(s: &mut Session, tool: Tool) {
    click(s, tool_cell(s, tool), Instant::now());
}

fn option_cells(s: &Session) -> Vec<(PaletteHit, i32, i32, i32)> {
    palette::layout(s, i32::from(s.screen.0), (s.tool_scroll, s.option_scroll))
        .cells
        .into_iter()
        .filter(|c| matches!(c.hit, PaletteHit::Option(_)))
        .map(|c| (c.hit, c.x, c.y, c.w))
        .collect()
}

/// Wheels the option row right until `c` shows.
fn reveal(s: &mut Session, c: Choice) {
    for _ in 0..50 {
        if option_cells(s).iter().any(|k| k.0 == PaletteHit::Option(c)) {
            return;
        }
        mouse(s, MouseEventKind::ScrollRight, 40, 1, Instant::now());
    }
}

/// Clicks the option cell showing `c`.
fn click_choice(s: &mut Session, c: Choice) {
    reveal(s, c);
    let (_, x, y, _) = option_cells(s)
        .into_iter()
        .find(|c2| c2.0 == PaletteHit::Option(c))
        .unwrap();
    click(s, (x as u16, y as u16), Instant::now());
}

/// Clicks the `i`-th option cell of the selection.
fn option(s: &mut Session, i: usize) {
    let c = palette::choices(&palette::selected(s), &s.canvas.groups)[i];
    click_choice(s, c);
}

fn shapes(s: &Session) -> Vec<&Shape> {
    s.canvas
        .elements
        .iter()
        .filter_map(|e| {
            if let Element::Shape(s) = e {
                Some(s)
            } else {
                None
            }
        })
        .collect()
}

fn with_shape(id: &str, x: i32, y: i32) -> Session {
    let mut s = session();
    s.canvas.elements.push(Element::Shape(Shape {
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
        turn: 0,
        meta: Default::default(),
    }));
    s
}

#[test]
fn rect_tool_draws_one_undo_step_and_returns_to_select() {
    let mut s = session();
    pick(&mut s, Tool::Rect);
    drag(&mut s, (20, 5), (30, 10));
    let sh = shapes(&s);
    assert_eq!((sh[0].x, sh[0].y, sh[0].w, sh[0].h), (20, 5, 11, 6));
    assert_eq!(s.tool, Tool::Pointer);
    key(&mut s, KeyCode::Char('u'));
    assert!(s.canvas.elements.is_empty());
    ctrl(&mut s, 'r');
    assert_eq!(shapes(&s).len(), 1);
}

#[test]
fn rect_tool_draws_inside_another_shape() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    pick(&mut s, Tool::Rect);
    drag(&mut s, (23, 6), (26, 8));
    let sh = shapes(&s);
    assert_eq!(sh.len(), 2);
    assert_eq!((sh[0].x, sh[0].y, sh[0].w, sh[0].h), (20, 5, 8, 4));
    assert_eq!((sh[1].x, sh[1].y, sh[1].w, sh[1].h), (23, 6, 4, 3));
}

#[test]
fn dragging_a_shape_moves_it_and_undo_restores() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    drag(&mut s, (20, 6), (25, 9));
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (25, 8));
    key(&mut s, KeyCode::Char('u'));
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (20, 5));
}

#[test]
fn a_long_shape_drag_undoes_and_redoes_in_one_step() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    let t = Instant::now();
    mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 20, 6, t);
    for i in 1..=200 {
        mouse(
            &mut s,
            MouseEventKind::Drag(MouseButton::Left),
            20 + i,
            6,
            t,
        );
    }
    mouse(&mut s, MouseEventKind::Up(MouseButton::Left), 220, 6, t);
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (220, 5));
    key(&mut s, KeyCode::Char('u'));
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (20, 5));
    ctrl(&mut s, 'r');
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (220, 5));
}

#[test]
fn corner_handle_resizes() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    click(&mut s, (20, 6), Instant::now());
    drag(&mut s, (27, 8), (32, 12));
    let sh = shapes(&s)[0];
    assert_eq!((sh.x, sh.y, sh.w, sh.h), (20, 5, 13, 8));
}

#[test]
fn rotate_handle_turns_a_rect_in_90_steps_and_undo_restores_it() {
    let mut s = with_shape("aaaaaaaa", 20, 8);
    let Element::Shape(sh) = &mut s.canvas.elements[0] else {
        unreachable!()
    };
    sh.h = 6;
    click(&mut s, (20, 9), Instant::now());
    click(&mut s, (23, 6), Instant::now());
    let sh = shapes(&s)[0];
    assert_eq!((sh.x, sh.y, sh.w, sh.h), (18, 9, 12, 4));
    click(&mut s, (23, 7), Instant::now());
    let sh = shapes(&s)[0];
    assert_eq!((sh.x, sh.y, sh.w, sh.h), (20, 8, 8, 6));
    key(&mut s, KeyCode::Char('u'));
    let sh = shapes(&s)[0];
    assert_eq!((sh.x, sh.y, sh.w, sh.h), (18, 9, 12, 4));
}

#[test]
fn rotate_handle_turns_a_cylinder_through_four_states_and_undo_restores_it() {
    let mut s = with_shape("aaaaaaaa", 20, 8);
    let Element::Shape(sh) = &mut s.canvas.elements[0] else {
        unreachable!()
    };
    sh.h = 6;
    sh.kind = ShapeKind::Cylinder;
    click(&mut s, (20, 9), Instant::now());
    let mut seen = vec![];
    for dot in [(23, 6), (23, 7), (23, 6), (23, 7)] {
        click(&mut s, dot, Instant::now());
        let sh = shapes(&s)[0];
        seen.push((sh.turn, sh.w, sh.h));
    }
    assert_eq!(seen, [(1, 12, 4), (2, 8, 6), (3, 12, 4), (0, 8, 6)]);
    click(&mut s, (23, 6), Instant::now());
    key(&mut s, KeyCode::Char('u'));
    let sh = shapes(&s)[0];
    assert_eq!((sh.turn, sh.w, sh.h), (0, 8, 6));
}

#[test]
fn rotate_press_followed_by_a_drag_rotates_once() {
    let mut s = with_shape("aaaaaaaa", 20, 4);
    let Element::Shape(sh) = &mut s.canvas.elements[0] else {
        unreachable!()
    };
    sh.h = 6;
    click(&mut s, (20, 5), Instant::now());
    drag(&mut s, (23, 2), (40, 10));
    let sh = shapes(&s)[0];
    assert_eq!((sh.x, sh.y, sh.w, sh.h), (18, 5, 12, 4));
}

#[test]
fn rotate_handle_hovers_above_a_selected_rect() {
    let mut s = with_shape("aaaaaaaa", 20, 4);
    s.selection = vec![Id::from("aaaaaaaa")];
    assert_eq!(
        hover_at(&mut s, (23, 2)),
        Hit::Handle(Handle::Rotate, Id::from("aaaaaaaa"))
    );
}

#[test]
fn rotate_handle_is_absent_on_a_diamond_and_a_hexagon() {
    for kind in [ShapeKind::Diamond, ShapeKind::Hexagon] {
        let mut s = with_shape("aaaaaaaa", 20, 4);
        let Element::Shape(sh) = &mut s.canvas.elements[0] else {
            unreachable!()
        };
        sh.kind = kind;
        s.selection = vec![Id::from("aaaaaaaa")];
        assert!(
            !matches!(hover_at(&mut s, (23, 2)), Hit::Handle(Handle::Rotate, _)),
            "{kind:?}"
        );
    }
}

#[test]
fn delete_key_removes_the_selection() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    click(&mut s, (20, 6), Instant::now());
    key(&mut s, KeyCode::Delete);
    assert!(s.canvas.elements.is_empty());
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(s.canvas.elements.len(), 1);
}

#[test]
fn ctrl_a_selects_everything() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.canvas.elements.push(Element::Shape(Shape {
        id: Id::from("bbbbbbbb"),
        kind: ShapeKind::Rect,
        x: 40,
        y: 5,
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
        turn: 0,
        meta: Default::default(),
    }));
    ctrl(&mut s, 'a');
    assert_eq!(
        s.selection,
        vec![Id::from("aaaaaaaa"), Id::from("bbbbbbbb")]
    );
}

#[test]
fn arrow_and_hjkl_keys_nudge_the_selection_one_undo_step_per_press() {
    for [right, down, left, up] in [
        [KeyCode::Right, KeyCode::Down, KeyCode::Left, KeyCode::Up],
        [
            KeyCode::Char('l'),
            KeyCode::Char('j'),
            KeyCode::Char('h'),
            KeyCode::Char('k'),
        ],
    ] {
        let mut s = with_shape("aaaaaaaa", 20, 5);
        click(&mut s, (20, 6), Instant::now());
        key(&mut s, right);
        key(&mut s, down);
        assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (21, 6));
        key(&mut s, left);
        key(&mut s, up);
        assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (20, 5));
        key(&mut s, KeyCode::Char('u'));
        assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (20, 6));
    }
}

#[test]
fn shift_arrow_connect_does_not_copy_the_source_link() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    let Element::Shape(src) = &mut s.canvas.elements[0] else {
        panic!("shape expected");
    };
    src.link = Some("https://x.test".into());
    s.selection = vec![Id::from("aaaaaaaa")];
    key(&mut s, KeyCode::Char('L'));
    let sh = shapes(&s);
    assert_eq!(sh[0].link.as_deref(), Some("https://x.test"));
    assert_eq!(sh[1].link, None);
}

#[test]
fn shift_arrow_connect_does_not_copy_the_source_meta() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.selection = vec![Id::from("aaaaaaaa")];
    assert!(s.set_meta("file", "a.rs"));
    key(&mut s, KeyCode::Char('L'));
    let sh = shapes(&s);
    assert_eq!(sh[0].meta["file"], "a.rs");
    assert!(sh[1].meta.is_empty());
}

#[test]
fn set_meta_sets_overwrites_clears_undoes_and_rejects_bad_pairs() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.selection = vec![Id::from("aaaaaaaa")];
    let get = |s: &Session| shapes(s)[0].meta.clone();
    assert!(!s.set_meta("", "x"));
    assert!(!s.set_meta("k", "a\nb"));
    assert!(get(&s).is_empty());
    assert!(s.set_meta("file", "a.rs"));
    assert!(s.set_meta("file", "b.rs"));
    assert!(s.set_meta("url", "https://x.test"));
    assert_eq!(get(&s).len(), 2);
    assert_eq!(get(&s)["file"], "b.rs");
    assert!(s.set_meta("file", ""));
    assert_eq!(get(&s).keys().collect::<Vec<_>>(), ["url"]);
    s.undo();
    assert_eq!(get(&s)["file"], "b.rs");
}

#[test]
fn arrow_binds_both_ends_follows_moves_and_unbinds_on_delete() {
    let mut s = with_shape("aaaaaaaa", 15, 3);
    s.canvas.elements.push(Element::Shape(Shape {
        id: Id::from("bbbbbbbb"),
        kind: ShapeKind::Rect,
        x: 40,
        y: 3,
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
        turn: 0,
        meta: Default::default(),
    }));
    pick(&mut s, Tool::Arrow);
    drag(&mut s, (18, 5), (43, 5));
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(l.from, End::Bound(Id::from("aaaaaaaa"), Anchor::Bottom(4)));
    assert_eq!(l.to, End::Bound(Id::from("bbbbbbbb"), Anchor::Bottom(4)));
    assert_eq!(s.tool, Tool::Pointer);
    assert_eq!(s.canvas.line_ends(&l), ((19, 7), (44, 7)));

    drag(&mut s, (40, 4), (40, 9));
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(s.canvas.line_ends(&l).1, (44, 12));

    click(&mut s, (47, 9), Instant::now());
    key(&mut s, KeyCode::Delete);
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(l.to, End::Free(44, 12));
    key(&mut s, KeyCode::Char('u'));
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(l.to, End::Bound(Id::from("bbbbbbbb"), Anchor::Bottom(4)));
}

#[test]
fn a_new_arrow_bends_around_an_arrow_it_would_cross() {
    let mut s = session();
    let wall = Line {
        id: Id::from("wall0000"),
        kind: LineKind::Arrow,
        from: End::Free(31, 2),
        to: End::Free(31, 6),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
        meta: Default::default(),
    };
    s.canvas.elements.push(Element::Line(wall.clone()));
    pick(&mut s, Tool::Arrow);
    drag(&mut s, (10, 5), (50, 5));
    let Element::Line(new) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    let theirs = s.canvas.route(&wall);
    let cells = s.canvas.route(&new);
    assert!(cells.len() > 41 && !cells.iter().any(|p| theirs.contains(p)));
}

#[test]
fn releasing_an_arrow_on_a_border_cell_pins_that_side() {
    let end_on = |to: (u16, u16)| {
        let mut s = with_shape("aaaaaaaa", 15, 3);
        s.canvas.elements.push(Element::Shape(Shape {
            id: Id::from("bbbbbbbb"),
            kind: ShapeKind::Rect,
            x: 40,
            y: 3,
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
            turn: 0,
            meta: Default::default(),
        }));
        pick(&mut s, Tool::Arrow);
        drag(&mut s, (18, 5), to);
        let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
            panic!("no line")
        };
        l.to
    };
    let b = Id::from("bbbbbbbb");
    assert_eq!(end_on((43, 3)), End::Bound(b.clone(), Anchor::Top(4)));
    assert_eq!(end_on((46, 3)), End::Bound(b.clone(), Anchor::Top(6)));
    assert_eq!(end_on((40, 5)), End::Bound(b.clone(), Anchor::Left(2)));
    assert_eq!(end_on((40, 3)), End::Bound(b.clone(), Anchor::Top(1)));
    assert_eq!(end_on((43, 5)), End::Bound(b.clone(), Anchor::Bottom(4)));
    assert_eq!(end_on((60, 5)), End::Free(60, 5));
}

#[test]
fn dragging_a_line_end_onto_empty_canvas_frees_it() {
    let mut s = with_shape("aaaaaaaa", 15, 3);
    pick(&mut s, Tool::Arrow);
    drag(&mut s, (18, 5), (50, 12));
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(l.to, End::Free(50, 12));
    assert_eq!(l.from, End::Bound(Id::from("aaaaaaaa"), Anchor::Bottom(4)));
}

#[test]
fn wheel_pans_the_canvas() {
    let mut s = session();
    let t = Instant::now();
    mouse(&mut s, MouseEventKind::ScrollDown, 30, 10, t);
    mouse(&mut s, MouseEventKind::ScrollRight, 30, 10, t);
    assert_eq!((s.view.ox, s.view.oy), (4, 2));
}

#[test]
fn plain_empty_drag_pans_and_shift_drag_draws_a_band() {
    let mut s = session();
    drag(&mut s, (30, 10), (25, 12));
    assert_eq!((s.view.ox, s.view.oy), (5, -2));
    assert!(s.band.is_none());
    shift_drag(&mut s, (30, 10), (25, 12));
    assert_eq!((s.view.ox, s.view.oy), (5, -2));
}

fn scroll_down(s: &mut Session, t: Instant) {
    mouse(s, MouseEventKind::ScrollDown, 30, 10, t);
}

#[test]
fn a_lone_notch_pans_two_rows_even_after_a_pause() {
    let mut s = session();
    let t = Instant::now();
    scroll_down(&mut s, t);
    assert_eq!(s.view.oy, 2);
    scroll_down(&mut s, t + Duration::from_millis(200));
    assert_eq!(s.view.oy, 4);
}

#[test]
fn a_fast_burst_accelerates_up_to_the_cap() {
    let mut s = session();
    let t = Instant::now();
    for i in 0..10 {
        scroll_down(&mut s, t + Duration::from_millis(10 * i));
    }
    assert_eq!(s.view.oy, 56);
}

#[test]
fn a_pause_resets_the_burst() {
    let mut s = session();
    let t = Instant::now();
    for i in 0..10 {
        scroll_down(&mut s, t + Duration::from_millis(10 * i));
    }
    scroll_down(&mut s, t + Duration::from_millis(240));
    assert_eq!(s.view.oy, 58);
}

#[test]
fn shift_wheel_pans_sideways_and_ctrl_wheel_zooms_in_place() {
    let mut s = session();
    let t = Instant::now();
    mouse_mod(
        &mut s,
        MouseEventKind::ScrollDown,
        KeyModifiers::SHIFT,
        30,
        10,
        t,
    );
    assert_eq!((s.view.ox, s.view.oy), (4, 0));
    let t = t + Duration::from_millis(500);
    mouse_mod(
        &mut s,
        MouseEventKind::ScrollUp,
        KeyModifiers::SHIFT,
        30,
        10,
        t,
    );
    assert_eq!((s.view.ox, s.view.oy), (0, 0));
    let t = t + Duration::from_millis(500);
    mouse_mod(
        &mut s,
        MouseEventKind::ScrollDown,
        KeyModifiers::CONTROL,
        30,
        10,
        t,
    );
    assert_eq!(s.view.zoom, Zoom::Half);
    assert_eq!(s.view.to_world(30, 10), (30, 10));
}

#[test]
fn rubber_band_selects_what_it_touches() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    shift_drag(&mut s, (15, 2), (23, 7));
    assert_eq!(s.selection, vec![Id::from("aaaaaaaa")]);
    click(&mut s, (60, 20), Instant::now());
    assert!(s.selection.is_empty());
}

fn shift_drag(s: &mut Session, from: (u16, u16), to: (u16, u16)) {
    let (t, m) = (Instant::now(), KeyModifiers::SHIFT);
    mouse_mod(
        s,
        MouseEventKind::Down(MouseButton::Left),
        m,
        from.0,
        from.1,
        t,
    );
    mouse_mod(s, MouseEventKind::Drag(MouseButton::Left), m, to.0, to.1, t);
    mouse_mod(s, MouseEventKind::Up(MouseButton::Left), m, to.0, to.1, t);
}

#[test]
fn shift_click_toggles_an_element_in_the_selection() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.canvas.elements.push(Element::Shape(Shape {
        id: Id::from("bbbbbbbb"),
        kind: ShapeKind::Rect,
        x: 40,
        y: 9,
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
        turn: 0,
        meta: Default::default(),
    }));
    click(&mut s, (20, 6), Instant::now());
    shift_drag(&mut s, (40, 10), (40, 10));
    assert_eq!(
        s.selection,
        vec![Id::from("aaaaaaaa"), Id::from("bbbbbbbb")]
    );
    shift_drag(&mut s, (20, 6), (20, 6));
    assert_eq!(s.selection, vec![Id::from("bbbbbbbb")]);
}

#[test]
fn shift_drag_draws_a_square() {
    let mut s = session();
    pick(&mut s, Tool::Rect);
    shift_drag(&mut s, (20, 5), (30, 8));
    let sh = shapes(&s);
    assert_eq!((sh[0].w, sh[0].h), (11, 11));
}

#[test]
fn shift_drag_moves_along_one_axis() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    shift_drag(&mut s, (20, 6), (28, 8));
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (28, 5));
}

#[test]
fn only_the_border_selects_a_shape() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    click(&mut s, (22, 6), Instant::now());
    assert!(s.selection.is_empty());
    click(&mut s, (20, 6), Instant::now());
    assert_eq!(s.selection, vec![Id::from("aaaaaaaa")]);
}

#[test]
fn double_click_inside_a_shape_edits_its_label() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    let t = Instant::now();
    click(&mut s, (22, 6), t);
    click(&mut s, (22, 6), t + Duration::from_millis(100));
    assert!(
        matches!(&s.edit, Some(e) if matches!(&e.target, EditTarget::Existing(i) if *i == Id::from("aaaaaaaa")))
    );
}

#[test]
fn double_click_edits_a_label_within_400ms_only() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    let t = Instant::now();
    click(&mut s, (22, 6), t);
    click(&mut s, (22, 6), t + Duration::from_millis(100));
    assert!(s.edit.is_some());
    for c in "hi".chars() {
        key(&mut s, KeyCode::Char(c));
    }
    key(&mut s, KeyCode::Enter);
    assert_eq!(shapes(&s)[0].label, "hi");

    let mut slow = with_shape("aaaaaaaa", 20, 5);
    click(&mut slow, (22, 6), t);
    click(&mut slow, (22, 6), t + Duration::from_millis(900));
    assert!(slow.edit.is_none());
}

#[test]
fn double_click_on_empty_canvas_places_free_text() {
    let mut s = session();
    let t = Instant::now();
    click(&mut s, (30, 10), t);
    click(&mut s, (30, 10), t + Duration::from_millis(50));
    for c in "note".chars() {
        key(&mut s, KeyCode::Char(c));
    }
    key(&mut s, KeyCode::Enter);
    let Element::Text(txt) = &s.canvas.elements[0] else {
        panic!("no text")
    };
    assert_eq!((txt.x, txt.y, txt.text.as_str()), (30, 10, "note"));
}

#[test]
fn double_click_on_an_unlabeled_arrow_puts_its_label_at_the_click() {
    let mut s = session();
    s.canvas
        .push(Element::Line(Line {
            id: Id::from("llllllll"),
            kind: LineKind::Arrow,
            from: End::Free(5, 5),
            to: End::Free(25, 5),
            label: String::new(),
            label_at: None,
            stroke: Default::default(),
            bends: vec![],
            color: None,
            meta: Default::default(),
        }))
        .unwrap();
    let t = Instant::now();
    click(&mut s, (10, 5), t);
    click(&mut s, (10, 5), t + Duration::from_millis(50));
    let Some(Element::Line(l)) = s.canvas.get(&Id::from("llllllll")) else {
        panic!("no line")
    };
    assert_eq!(l.label_at, Some(250));
    assert_eq!(s.edit_boxes()[0].x, 10);
    key(&mut s, KeyCode::Char('h'));
    key(&mut s, KeyCode::Char('i'));
    key(&mut s, KeyCode::Enter);
    drag(&mut s, (10, 5), (20, 5));
    let Some(Element::Line(l)) = s.canvas.get(&Id::from("llllllll")) else {
        panic!("no line")
    };
    assert_eq!(l.label_at, Some(750));
}

#[test]
fn text_tool_click_places_text_and_esc_cancels_edit() {
    let mut s = session();
    pick(&mut s, Tool::Text);
    click(&mut s, (30, 10), Instant::now());
    key(&mut s, KeyCode::Char('x'));
    key(&mut s, KeyCode::Esc);
    assert!(s.canvas.elements.is_empty());
    assert_eq!(s.tool, Tool::Pointer);
}

#[test]
fn esc_cancels_a_drag() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    let t = Instant::now();
    mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 22, 6, t);
    mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 30, 12, t);
    assert_eq!(key(&mut s, KeyCode::Esc), KeyOutcome::None);
    assert_eq!(shapes(&s)[0].x, 20);
    key(&mut s, KeyCode::Esc);
    assert_eq!(key(&mut s, KeyCode::Esc), KeyOutcome::None);
}

#[test]
fn esc_clears_the_selection() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.selection = vec![shapes(&s)[0].id.clone()];
    assert_eq!(key(&mut s, KeyCode::Esc), KeyOutcome::None);
    assert!(s.selection.is_empty());
    assert_eq!(key(&mut s, KeyCode::Esc), KeyOutcome::None);
}

#[test]
fn select_tool_drag_on_empty_draws_a_band() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.tool = Tool::Select;
    drag(&mut s, (10, 2), (60, 16));
    assert_eq!((s.view.ox, s.view.oy), (0, 0));
    assert_eq!(s.selection, vec![Id::from("aaaaaaaa")]);
    assert_eq!(s.tool, Tool::Select);
}

#[test]
fn select_tool_moves_a_selected_shape_like_the_pointer() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.tool = Tool::Select;
    drag(&mut s, (20, 6), (24, 6));
    assert_eq!(shapes(&s)[0].x, 24);
}

#[test]
fn esc_leaves_the_select_tool_and_clears_the_selection() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.tool = Tool::Select;
    s.selection = vec![Id::from("aaaaaaaa")];
    key(&mut s, KeyCode::Esc);
    assert!(s.selection.is_empty());
    assert_eq!(s.tool, Tool::Pointer);
}

#[test]
fn palette_shape_tool_keeps_the_selected_shape_and_arms_the_tool() {
    for tool in [Tool::Diamond, Tool::Hexagon] {
        let mut s = with_shape("aaaaaaaa", 20, 5);
        click(&mut s, (20, 6), Instant::now());
        pick(&mut s, tool);
        assert_eq!(shapes(&s)[0].kind, ShapeKind::Rect);
        assert!(s.selection.is_empty());
        assert_eq!(s.tool, tool);
    }
}

#[test]
fn dragging_with_a_new_tool_draws_that_kind() {
    for (tool, kind) in [
        (Tool::Cylinder, ShapeKind::Cylinder),
        (Tool::Parallelogram, ShapeKind::Parallelogram),
        (Tool::Hexagon, ShapeKind::Hexagon),
        (Tool::Frame, ShapeKind::Frame),
    ] {
        let mut s = session();
        pick(&mut s, tool);
        drag(&mut s, (20, 5), (30, 10));
        assert_eq!(shapes(&s)[0].kind, kind);
    }
}

#[test]
fn ctrl_c_quits() {
    let mut s = session();
    let out = handle_key(
        &mut s,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    );
    assert_eq!(out, KeyOutcome::Quit);
}

#[test]
fn palette_option_sets_the_head_of_a_selected_line() {
    let mut s = session();
    let line = Element::Line(Line {
        id: Id::from("llllllll"),
        kind: LineKind::Arrow,
        from: End::Free(5, 5),
        to: End::Free(25, 5),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
        meta: Default::default(),
    });
    s.canvas.push(line).unwrap();
    let kind = |s: &Session| match s.canvas.get(&Id::from("llllllll")) {
        Some(Element::Line(l)) => l.kind,
        _ => panic!("no line"),
    };
    // Options on row 1: none, end, start, both.
    assert!(
        option_cells(&s).is_empty(),
        "no options without a selection"
    );
    s.selection = vec![Id::from("llllllll")];
    option(&mut s, 2);
    assert_eq!(kind(&s), LineKind::ArrowStart);
    option(&mut s, 3);
    assert_eq!(kind(&s), LineKind::ArrowBoth);
    option(&mut s, 0);
    assert_eq!(kind(&s), LineKind::Line);
    assert_eq!(s.tool, Tool::Pointer);
    s.undo();
    assert_eq!(kind(&s), LineKind::ArrowBoth);
}

#[test]
fn palette_option_sets_the_stroke_of_a_selected_line_and_undoes() {
    let mut s = session();
    s.canvas
        .push(Element::Line(Line {
            id: Id::from("llllllll"),
            kind: LineKind::Arrow,
            from: End::Free(5, 5),
            to: End::Free(25, 5),
            label: String::new(),
            label_at: None,
            stroke: Default::default(),
            bends: vec![],
            color: None,
            meta: Default::default(),
        }))
        .unwrap();
    s.selection = vec![Id::from("llllllll")];
    let stroke = |s: &Session| match s.canvas.get(&Id::from("llllllll")) {
        Some(Element::Line(l)) => l.stroke,
        _ => panic!("no line"),
    };
    // Four head options and Arrange come first, then solid, dashed, dotted.
    option(&mut s, 6);
    assert_eq!(stroke(&s), Stroke::Dashed);
    option(&mut s, 7);
    assert_eq!(stroke(&s), Stroke::Dotted);
    s.undo();
    assert_eq!(stroke(&s), Stroke::Dashed);
}

#[test]
fn palette_option_sets_the_border_of_a_selected_rect_and_undoes() {
    let mut s = session();
    s.canvas.elements.push(Element::Shape(Shape {
        id: Id::from("rrrrrrrr"),
        kind: ShapeKind::Rect,
        x: 30,
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
        turn: 0,
        meta: Default::default(),
    }));
    s.selection = vec![Id::from("rrrrrrrr")];
    let border = |s: &Session| match s.canvas.get(&Id::from("rrrrrrrr")) {
        Some(Element::Shape(sh)) => sh.border,
        _ => panic!("no shape"),
    };
    // Three stroke options come first, then single, rounded, double, heavy.
    option(&mut s, 5);
    assert_eq!(border(&s), Border::Double);
    option(&mut s, 6);
    assert_eq!(border(&s), Border::Heavy);
    s.undo();
    assert_eq!(border(&s), Border::Double);
}

fn two_shapes_and_arrow_tool() -> Session {
    let mut s = with_shape("aaaaaaaa", 15, 3);
    s.canvas.elements.push(Element::Shape(Shape {
        id: Id::from("bbbbbbbb"),
        kind: ShapeKind::Rect,
        x: 40,
        y: 9,
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
        turn: 0,
        meta: Default::default(),
    }));
    pick(&mut s, Tool::Arrow);
    s
}

#[test]
fn shape_under_a_dragged_line_end_is_the_snap_target_until_release() {
    let mut s = two_shapes_and_arrow_tool();
    let t = Instant::now();
    mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 18, 5, t);
    mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 43, 10, t);
    assert_eq!(s.snap, Some(Id::from("bbbbbbbb")));
    assert_eq!(s.snap_at, Some((44, 8)));
    mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 46, 9, t);
    assert_eq!(s.snap_at, Some((46, 8)));
    mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 30, 20, t);
    assert_eq!(s.snap, None);
    assert_eq!(s.snap_at, None);
    mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 43, 10, t);
    mouse(&mut s, MouseEventKind::Up(MouseButton::Left), 43, 10, t);
    assert_eq!(s.snap, None);
    assert_eq!(s.snap_at, None);
}

#[test]
fn escape_mid_drag_clears_the_line_end_snap_target() {
    let mut s = two_shapes_and_arrow_tool();
    let t = Instant::now();
    mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 18, 5, t);
    mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 43, 10, t);
    assert!(s.snap.is_some());
    key(&mut s, KeyCode::Esc);
    assert_eq!((s.snap, s.snap_at), (None, None));
}

#[test]
fn clicking_the_middle_run_of_a_bent_route_selects_the_line() {
    let mut s = two_shapes_and_arrow_tool();
    drag(&mut s, (18, 5), (43, 10));
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    click(&mut s, (70, 20), Instant::now());
    assert!(s.selection.is_empty());
    click(&mut s, (31, 8), Instant::now());
    assert_eq!(s.selection, vec![l.id]);
}

#[test]
fn configured_double_click_window_applies() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.double_click = Duration::from_millis(50);
    let t = Instant::now();
    click(&mut s, (20, 6), t);
    click(&mut s, (20, 6), t + Duration::from_millis(100));
    assert!(s.edit.is_none());
}

#[test]
fn colon_opens_the_command_line_but_types_into_a_label_edit() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.command_ui = CommandUi::Vim;
    assert_eq!(key(&mut s, KeyCode::Char(':')), KeyOutcome::Command);
    let t = Instant::now();
    click(&mut s, (20, 6), t);
    click(&mut s, (20, 6), t + Duration::from_millis(100));
    assert_eq!(key(&mut s, KeyCode::Char(':')), KeyOutcome::None);
    assert_eq!(s.edit.as_ref().unwrap().buf, ":");
}

#[test]
fn slash_opens_find_but_types_into_a_label_edit_and_ctrl_f_is_unbound() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    assert_eq!(
        handle_key(
            &mut s,
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)
        ),
        KeyOutcome::None
    );
    assert_eq!(key(&mut s, KeyCode::Char('/')), KeyOutcome::Find);
    let t = Instant::now();
    click(&mut s, (20, 6), t);
    click(&mut s, (20, 6), t + Duration::from_millis(100));
    assert_eq!(key(&mut s, KeyCode::Char('/')), KeyOutcome::None);
    assert_eq!(s.edit.as_ref().unwrap().buf, "/");
}

#[test]
fn n_and_capital_n_repeat_the_search_and_wait_for_an_edit() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    assert_eq!(key(&mut s, KeyCode::Char('n')), KeyOutcome::FindNext);
    assert_eq!(key(&mut s, KeyCode::Char('N')), KeyOutcome::FindPrev);
    let t = Instant::now();
    click(&mut s, (20, 6), t);
    click(&mut s, (20, 6), t + Duration::from_millis(100));
    assert_eq!(key(&mut s, KeyCode::Char('n')), KeyOutcome::None);
    assert_eq!(s.edit.as_ref().unwrap().buf, "n");
}

fn hold(s: &mut Session, from: (u16, u16), to: (u16, u16)) {
    let t = Instant::now();
    mouse(
        s,
        MouseEventKind::Down(MouseButton::Left),
        from.0,
        from.1,
        t,
    );
    mouse(s, MouseEventKind::Drag(MouseButton::Left), to.0, to.1, t);
}

#[test]
fn holding_a_shape_at_the_edge_pans_each_tick_and_undoes_in_one_step() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    hold(&mut s, (20, 6), (79, 6));
    let x0 = shapes(&s)[0].x;
    let t0 = Instant::now();
    assert!(s.edge_pan(t0));
    assert_eq!((s.view.ox, shapes(&s)[0].x), (3, x0 + 3));
    assert!(!s.edge_pan(t0 + Duration::from_millis(10)));
    assert_eq!(s.view.ox, 3);
    assert_eq!(
        s.edge_wait(t0 + Duration::from_millis(10)),
        Some(Duration::from_millis(20))
    );
    assert!(s.edge_pan(t0 + Duration::from_millis(30)));
    assert_eq!((s.view.ox, shapes(&s)[0].x), (6, x0 + 6));
    mouse(&mut s, MouseEventKind::Up(MouseButton::Left), 79, 6, t0);
    assert_eq!(s.edge_wait(t0), None);
    key(&mut s, KeyCode::Char('u'));
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (20, 5));
}

#[test]
fn edge_pan_speed_follows_depth_and_direction() {
    let t0 = Instant::now();
    for ((col, row), pan) in [
        ((78, 12), (1, 0)),
        ((0, 12), (-3, 0)),
        ((1, 12), (-1, 0)),
        ((40, 0), (0, -3)),
        ((40, 23), (0, 3)),
        ((79, 23), (3, 3)),
        ((40, 12), (0, 0)),
        ((77, 12), (0, 0)),
    ] {
        let mut s = with_shape("aaaaaaaa", 20, 5);
        hold(&mut s, (20, 6), (col, row));
        assert_eq!(s.edge_pan(t0), pan != (0, 0), "{col},{row}");
        assert_eq!((s.view.ox, s.view.oy), pan, "{col},{row}");
    }
}

#[test]
fn holding_a_band_at_the_edge_extends_it() {
    let mut s = session();
    let m = KeyModifiers::SHIFT;
    let t = Instant::now();
    mouse_mod(&mut s, MouseEventKind::Down(MouseButton::Left), m, 30, 5, t);
    mouse_mod(
        &mut s,
        MouseEventKind::Drag(MouseButton::Left),
        m,
        79,
        10,
        t,
    );
    let before = s.band.unwrap();
    assert!(s.edge_pan(Instant::now()));
    assert_eq!(s.band.unwrap(), (before.0, (before.1 .0 + 3, before.1 .1)));
}

#[test]
fn pan_and_idle_never_edge_pan() {
    let mut s = session();
    assert_eq!(s.edge_wait(Instant::now()), None);
    hold(&mut s, (40, 10), (79, 10));
    assert_eq!(s.edge_wait(Instant::now()), None);
}

#[test]
fn hover_tracks_shape_handle_and_empty_space() {
    let mut s = with_shape("aaaaaaaa", 20, 4);
    let t = Instant::now();
    mouse(&mut s, MouseEventKind::Moved, 22, 4, t);
    assert_eq!(s.hover, Hit::Element(Id::from("aaaaaaaa")));
    mouse(&mut s, MouseEventKind::Moved, 60, 15, t);
    assert_eq!(s.hover, Hit::Empty);
    s.selection = vec![Id::from("aaaaaaaa")];
    mouse(&mut s, MouseEventKind::Moved, 20, 4, t);
    assert_eq!(s.hover, Hit::Element(Id::from("aaaaaaaa")));
    mouse(&mut s, MouseEventKind::Moved, 27, 7, t);
    assert_eq!(s.hover, Hit::Handle(Handle::Resize, Id::from("aaaaaaaa")));
}

#[test]
fn hover_covers_every_cell_of_a_text() {
    let mut s = session();
    let id = Id::from("tttttttt");
    s.canvas.elements.push(Element::Text(clidraw::model::Text {
        id: id.clone(),
        x: 30,
        y: 5,
        text: "hello".into(),
        color: None,
        meta: Default::default(),
    }));
    let t = Instant::now();
    for col in 30..35 {
        mouse(&mut s, MouseEventKind::Moved, col, 5, t);
        assert_eq!(s.hover, Hit::Element(id.clone()), "col {col}");
    }
    mouse(&mut s, MouseEventKind::Moved, 36, 5, t);
    assert_eq!(s.hover, Hit::Empty);
}

fn hover_at(s: &mut Session, at: (u16, u16)) -> Hit {
    mouse(s, MouseEventKind::Moved, at.0, at.1, Instant::now());
    s.hover.clone()
}

#[test]
fn hover_picks_a_shape_one_cell_outside_its_border_but_not_its_interior() {
    let mut s = with_shape("aaaaaaaa", 20, 4);
    let a = Hit::Element(Id::from("aaaaaaaa"));
    for at in [(19, 3), (28, 5), (22, 3), (22, 8)] {
        assert_eq!(hover_at(&mut s, at), a, "{at:?}");
    }
    assert_eq!(hover_at(&mut s, (18, 4)), Hit::Empty);
    assert_eq!(hover_at(&mut s, (23, 5)), Hit::Empty);
}

#[test]
fn hover_on_an_angled_shape_follows_its_drawn_outline() {
    let mut s = with_shape("aaaaaaaa", 20, 4);
    let Element::Shape(sh) = &mut s.canvas.elements[0] else {
        unreachable!()
    };
    (sh.kind, sh.w, sh.h) = (ShapeKind::Diamond, 9, 5);
    let a = Hit::Element(Id::from("aaaaaaaa"));
    for at in [(24, 4), (22, 5), (20, 6), (26, 7), (21, 5)] {
        assert_eq!(hover_at(&mut s, at), a, "{at:?}");
    }
    for at in [(20, 4), (28, 8), (24, 6)] {
        assert_eq!(hover_at(&mut s, at), Hit::Empty, "{at:?}");
    }
}

#[test]
fn hover_picks_a_text_one_cell_around_it() {
    let mut s = session();
    let id = Id::from("tttttttt");
    s.canvas.elements.push(Element::Text(clidraw::model::Text {
        id: id.clone(),
        x: 30,
        y: 5,
        text: "hello".into(),
        color: None,
        meta: Default::default(),
    }));
    for at in [(29, 4), (35, 6), (32, 4)] {
        assert_eq!(hover_at(&mut s, at), Hit::Element(id.clone()), "{at:?}");
    }
    assert_eq!(hover_at(&mut s, (36, 5)), Hit::Empty);
    assert_eq!(hover_at(&mut s, (32, 7)), Hit::Empty);
}

#[test]
fn an_exact_hit_beats_a_near_hit_on_another_element() {
    let mut s = with_shape("aaaaaaaa", 20, 4);
    put(&mut s, "bbbbbbbb", ShapeKind::Rect, (28, 4, 4, 4));
    assert_eq!(
        hover_at(&mut s, (28, 5)),
        Hit::Element(Id::from("bbbbbbbb"))
    );
}

#[test]
fn hover_picks_a_line_one_cell_beside_its_route() {
    let mut s = session();
    put(&mut s, "aaaaaaaa", ShapeKind::Rect, (0, 4, 3, 3));
    put(&mut s, "bbbbbbbb", ShapeKind::Rect, (20, 4, 3, 3));
    s.canvas.elements.push(Element::Line(Line {
        id: Id::from("arrow000"),
        kind: LineKind::Arrow,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
        meta: Default::default(),
    }));
    let l = Hit::Element(Id::from("arrow000"));
    assert_eq!(hover_at(&mut s, (10, 5)), l);
    assert_eq!(hover_at(&mut s, (10, 6)), l);
    assert_eq!(hover_at(&mut s, (10, 4)), l);
    assert_eq!(hover_at(&mut s, (10, 7)), Hit::Empty);
}

#[test]
fn press_and_right_click_one_cell_outside_select_for_the_pointer_only() {
    let mut s = with_shape("aaaaaaaa", 20, 4);
    s.tool = Tool::Select;
    click(&mut s, (19, 3), Instant::now());
    assert_eq!(s.selection, vec![Id::from("aaaaaaaa")]);
    s.selection.clear();
    assert_eq!(s.right_down(28, 5), Some(true));
    assert_eq!(s.selection, vec![Id::from("aaaaaaaa")]);
    s.selection.clear();
    s.tool = Tool::Rect;
    click(&mut s, (19, 3), Instant::now());
    assert!(!s.selection.contains(&Id::from("aaaaaaaa")));
}

#[test]
fn a_cell_beside_the_resize_handle_is_not_the_handle() {
    let mut s = with_shape("aaaaaaaa", 20, 4);
    s.selection = vec![Id::from("aaaaaaaa")];
    assert_eq!(
        hover_at(&mut s, (27, 7)),
        Hit::Handle(Handle::Resize, Id::from("aaaaaaaa"))
    );
    assert_eq!(
        hover_at(&mut s, (28, 8)),
        Hit::Element(Id::from("aaaaaaaa"))
    );
}

#[test]
fn hover_is_empty_for_other_tools_mid_gesture_and_over_the_palette() {
    let t = Instant::now();
    let mut s = with_shape("aaaaaaaa", 20, 4);
    s.tool = Tool::Rect;
    mouse(&mut s, MouseEventKind::Moved, 22, 5, t);
    assert_eq!(s.hover, Hit::Empty);
    let mut s = with_shape("aaaaaaaa", 20, 4);
    mouse(&mut s, MouseEventKind::Moved, 22, 5, t);
    mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 22, 5, t);
    assert_eq!(s.hover, Hit::Empty, "a press clears hover");
    mouse(&mut s, MouseEventKind::Moved, 22, 5, t);
    assert_eq!(s.hover, Hit::Empty, "no hover while pressed");
    let mut s = with_shape("aaaaaaaa", 2, 0);
    mouse(&mut s, MouseEventKind::Moved, 2, 0, t);
    assert_eq!(s.hover, Hit::Empty, "palette owns its cells");
}

fn tool_cell(s: &Session, tool: Tool) -> (u16, u16) {
    (0..80u16)
        .flat_map(|c| (0..24u16).map(move |r| (c, r)))
        .find(|&(c, r)| palette::hit(s, c.into(), r.into()) == Some(PaletteHit::Tool(tool)))
        .unwrap()
}

#[test]
fn palette_hover_follows_the_pointer_and_clears_on_press_or_gesture() {
    let t = Instant::now();
    let mut s = with_shape("aaaaaaaa", 20, 4);
    assert_eq!(s.palette_hover, None);
    let (c, r) = tool_cell(&s, Tool::Rect);
    mouse(&mut s, MouseEventKind::Moved, c, r, t);
    assert_eq!(s.palette_hover, Some(PaletteHit::Tool(Tool::Rect)));
    mouse(&mut s, MouseEventKind::Moved, 60, 15, t);
    assert_eq!(s.palette_hover, None);
    s.tool = Tool::Arrow;
    mouse(&mut s, MouseEventKind::Moved, c, r, t);
    assert_eq!(
        s.palette_hover,
        Some(PaletteHit::Tool(Tool::Rect)),
        "any tool"
    );
    mouse(&mut s, MouseEventKind::Down(MouseButton::Left), c, r, t);
    assert_eq!(s.palette_hover, None, "a press clears it");
    let mut s = with_shape("aaaaaaaa", 20, 4);
    mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 22, 5, t);
    mouse(&mut s, MouseEventKind::Moved, c, r, t);
    assert_eq!(s.palette_hover, None, "none mid-gesture");
}

#[test]
fn tool_keys_switch_tools_but_type_into_an_edit() {
    let mut s = session();
    for tool in Tool::ALL {
        let Some(c) = tool.key() else { continue };
        key(&mut s, KeyCode::Char(c));
        assert_eq!(s.tool, tool);
    }
    key(&mut s, KeyCode::Char('v'));
    s.edit = Some(Edit::new(EditTarget::New(0, 0), String::new()));
    key(&mut s, KeyCode::Char('r'));
    assert_eq!(s.tool, Tool::Select);
    assert_eq!(s.edit.as_ref().unwrap().buf, "r");
}

#[test]
fn bracket_keys_reorder_the_selection_one_undo_step_per_press() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.canvas.elements.push(Element::Shape(Shape {
        id: Id::from("bbbbbbbb"),
        kind: ShapeKind::Rect,
        x: 20,
        y: 5,
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
        turn: 0,
        meta: Default::default(),
    }));
    let ids = |s: &Session| -> Vec<String> {
        s.canvas
            .elements
            .iter()
            .map(|e| e.id().to_string())
            .collect()
    };
    s.selection = vec![Id::from("aaaaaaaa")];
    key(&mut s, KeyCode::Char('['));
    assert_eq!(ids(&s), ["aaaaaaaa", "bbbbbbbb"]);
    key(&mut s, KeyCode::Char(']'));
    assert_eq!(ids(&s), ["bbbbbbbb", "aaaaaaaa"]);
    key(&mut s, KeyCode::Char('{'));
    assert_eq!(ids(&s), ["aaaaaaaa", "bbbbbbbb"]);
    key(&mut s, KeyCode::Char('}'));
    assert_eq!(ids(&s), ["bbbbbbbb", "aaaaaaaa"]);
    key(&mut s, KeyCode::Char('}'));
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(ids(&s), ["aaaaaaaa", "bbbbbbbb"]);
}

#[test]
fn bracket_keys_type_into_a_label_edit() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    click(&mut s, (20, 6), Instant::now());
    click(&mut s, (20, 6), Instant::now() + Duration::from_millis(100));
    assert!(s.edit.is_some());
    key(&mut s, KeyCode::Char(']'));
    assert_eq!(s.edit.as_ref().unwrap().buf, "]");
}

fn diagram() -> Session {
    let mut s = with_shape("aaaaaaaa", 5, 5);
    s.canvas.elements.push(Element::Shape(Shape {
        id: Id::from("bbbbbbbb"),
        kind: ShapeKind::Rect,
        x: 30,
        y: 5,
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
        turn: 0,
        meta: Default::default(),
    }));
    s.canvas.elements.push(Element::Line(Line {
        id: Id::from("line0000"),
        kind: LineKind::Arrow,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
        meta: Default::default(),
    }));
    s
}

#[test]
fn undo_redo_delete_and_select_all_keys_do_nothing_mid_drag() {
    let mut s = diagram();
    let t = Instant::now();
    mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 8, 7, t);
    mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 10, 7, t);
    assert!(s.dragging());
    let (elements, selection) = (s.canvas.elements.clone(), s.selection.clone());
    key(&mut s, KeyCode::Char('u'));
    key(&mut s, KeyCode::Delete);
    key(&mut s, KeyCode::Backspace);
    ctrl(&mut s, 'r');
    ctrl(&mut s, 'a');
    assert_eq!(
        (s.canvas.elements.clone(), s.selection.clone()),
        (elements, selection)
    );
}

#[test]
fn copy_paste_adds_offset_copies_in_one_undo_step() {
    let mut s = diagram();
    ctrl(&mut s, 'a');
    key(&mut s, KeyCode::Char('y'));
    key(&mut s, KeyCode::Char('p'));
    assert_eq!(s.canvas.elements.len(), 6);
    assert_eq!(s.selection.len(), 3);
    assert_eq!(shapes(&s)[2].x, 7);
    let Element::Line(l) = &s.canvas.elements[5] else {
        panic!()
    };
    assert_eq!(l.from, End::Bound(shapes(&s)[2].id.clone(), Anchor::Auto));
    key(&mut s, KeyCode::Char('p'));
    assert_eq!(shapes(&s)[4].x, 9);
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(s.canvas.elements.len(), 6);
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(s.canvas.elements.len(), 3);
}

#[test]
fn cut_removes_and_paste_restores() {
    let mut s = diagram();
    ctrl(&mut s, 'a');
    key(&mut s, KeyCode::Char('x'));
    assert!(s.canvas.elements.is_empty());
    key(&mut s, KeyCode::Char('p'));
    assert_eq!(s.canvas.elements.len(), 3);
    key(&mut s, KeyCode::Char('u'));
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(s.canvas.elements.len(), 3);
}

#[test]
fn duplicate_leaves_the_buffer_alone() {
    let mut s = with_shape("aaaaaaaa", 5, 5);
    ctrl(&mut s, 'a');
    s.duplicate();
    assert_eq!(shapes(&s).len(), 2);
    assert_eq!(shapes(&s)[1].x, 7);
    assert!(s.clipboard.elements.is_empty());
}

#[test]
fn clipboard_keys_do_nothing_without_input_or_while_editing() {
    let mut s = with_shape("aaaaaaaa", 5, 5);
    key(&mut s, KeyCode::Char('y'));
    key(&mut s, KeyCode::Char('p'));
    assert_eq!(s.canvas.elements.len(), 1);
    ctrl(&mut s, 'a');
    key(&mut s, KeyCode::Char('y'));
    s.start_edit_new((0, 0));
    key(&mut s, KeyCode::Char('p'));
    assert_eq!(s.canvas.elements.len(), 1);
}

#[test]
fn c_s_e_d_pick_cylinder_slant_hexagon_diamond() {
    let mut s = session();
    for (c, t) in [
        ('c', Tool::Cylinder),
        ('s', Tool::Parallelogram),
        ('e', Tool::Hexagon),
        ('d', Tool::Diamond),
    ] {
        key(&mut s, KeyCode::Char(c));
        assert_eq!(s.tool, t);
    }
}

fn editing(buf: &str) -> Session {
    let mut s = session();
    s.edit = Some(Edit::new(EditTarget::New(0, 0), buf.into()));
    s
}

fn mod_key(s: &mut Session, code: KeyCode, m: KeyModifiers) {
    handle_key(s, KeyEvent::new(code, m));
}

fn typed(s: &mut Session, text: &str) {
    for c in text.chars() {
        key(s, KeyCode::Char(c));
    }
}

fn buf(s: &Session) -> &str {
    &s.edit.as_ref().unwrap().buf
}

fn caret(s: &Session) -> usize {
    s.edit.as_ref().unwrap().caret
}

#[test]
fn edit_caret_keys_insert_and_delete_at_the_caret() {
    let mut s = editing("abc");
    key(&mut s, KeyCode::Left);
    typed(&mut s, "X");
    assert_eq!(buf(&s), "abXc");
    key(&mut s, KeyCode::Home);
    typed(&mut s, "Y");
    key(&mut s, KeyCode::Backspace);
    key(&mut s, KeyCode::Backspace);
    assert_eq!(buf(&s), "abXc");
    key(&mut s, KeyCode::Delete);
    assert_eq!(buf(&s), "bXc");
    key(&mut s, KeyCode::End);
    key(&mut s, KeyCode::Delete);
    key(&mut s, KeyCode::Right);
    assert_eq!((buf(&s), caret(&s)), ("bXc", 3));
}

#[test]
fn edit_word_jumps_cross_runs_of_spaces() {
    let mut s = editing("foo  bar baz");
    mod_key(&mut s, KeyCode::Left, KeyModifiers::CONTROL);
    assert_eq!(caret(&s), 9);
    mod_key(&mut s, KeyCode::Char('b'), KeyModifiers::ALT);
    assert_eq!(caret(&s), 5);
    mod_key(&mut s, KeyCode::Left, KeyModifiers::ALT);
    assert_eq!(caret(&s), 0);
    mod_key(&mut s, KeyCode::Right, KeyModifiers::CONTROL);
    assert_eq!(caret(&s), 3);
    mod_key(&mut s, KeyCode::Char('f'), KeyModifiers::ALT);
    assert_eq!(caret(&s), 8);
}

#[test]
fn ctrl_w_and_ctrl_u_delete_backward() {
    let mut s = editing("one two  ");
    ctrl(&mut s, 'w');
    assert_eq!(buf(&s), "one ");
    key(&mut s, KeyCode::Left);
    ctrl(&mut s, 'u');
    assert_eq!((buf(&s), caret(&s)), (" ", 0));
}

#[test]
fn edit_selection_is_replaced_by_typing_and_collapsed_by_arrows() {
    let mut s = editing("abc");
    mod_key(&mut s, KeyCode::Left, KeyModifiers::SHIFT);
    mod_key(&mut s, KeyCode::Left, KeyModifiers::SHIFT);
    typed(&mut s, "X");
    assert_eq!(buf(&s), "aX");
    mod_key(&mut s, KeyCode::Home, KeyModifiers::SHIFT);
    key(&mut s, KeyCode::Left);
    assert_eq!(caret(&s), 0);
    assert!(s.edit.as_ref().unwrap().selection().is_none());
    mod_key(&mut s, KeyCode::End, KeyModifiers::SHIFT);
    key(&mut s, KeyCode::Right);
    assert_eq!(caret(&s), 2);
}

#[test]
fn ctrl_a_in_an_edit_selects_the_text_not_the_canvas() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.edit = Some(Edit::new(EditTarget::New(0, 0), "abc".into()));
    ctrl(&mut s, 'a');
    assert!(s.selection.is_empty());
    typed(&mut s, "X");
    assert_eq!(buf(&s), "X");
}

#[test]
fn edit_caret_stays_on_char_boundaries() {
    let mut s = editing("é世x");
    for _ in 0..3 {
        key(&mut s, KeyCode::Left);
        assert!(buf(&s).is_char_boundary(caret(&s)));
    }
    assert_eq!(caret(&s), 0);
    key(&mut s, KeyCode::Right);
    assert_eq!(caret(&s), 2);
    key(&mut s, KeyCode::Delete);
    key(&mut s, KeyCode::Backspace);
    assert_eq!(buf(&s), "x");
}

#[test]
fn paste_inserts_at_the_caret_and_replaces_the_selection() {
    let mut s = editing("abcd");
    key(&mut s, KeyCode::Left);
    handle_paste(&mut s, "X\nY");
    assert_eq!(buf(&s), "abcXYd");
    mod_key(&mut s, KeyCode::Left, KeyModifiers::SHIFT);
    mod_key(&mut s, KeyCode::Left, KeyModifiers::SHIFT);
    handle_paste(&mut s, "-");
    assert_eq!(buf(&s), "abc-d");
}

#[test]
fn click_on_a_label_places_the_caret_and_elsewhere_commits() {
    let mut s = session();
    let id = Id::from("tttttttt");
    s.canvas.elements.push(Element::Text(clidraw::model::Text {
        id: id.clone(),
        x: 30,
        y: 5,
        text: "hello".into(),
        color: None,
        meta: Default::default(),
    }));
    s.start_edit_existing(id);
    let t = Instant::now();
    click(&mut s, (32, 5), t);
    assert_eq!(caret(&s), 2);
    click(&mut s, (35, 5), t);
    assert_eq!(caret(&s), 5);
    click(&mut s, (60, 15), t);
    assert!(s.edit.is_none());
}

#[test]
fn z_types_into_a_label_edit_instead_of_fitting() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    let t = Instant::now();
    click(&mut s, (20, 6), t);
    click(&mut s, (20, 6), t + Duration::from_millis(100));
    let before = s.view;
    key(&mut s, KeyCode::Char('z'));
    assert_eq!(s.view, before);
    assert_eq!(s.edit.as_ref().unwrap().buf, "z");
}

fn wheel(s: &mut Session, up: bool, at: (u16, u16)) {
    let kind = if up {
        MouseEventKind::ScrollUp
    } else {
        MouseEventKind::ScrollDown
    };
    mouse_mod(s, kind, KeyModifiers::CONTROL, at.0, at.1, Instant::now());
}

#[test]
fn caret_click_at_double_zoom_lands_on_the_stretched_label() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.view.zoom = Zoom::Two;
    s.start_edit_existing(Id::from("aaaaaaaa"));
    let b = &s.edit_boxes()[0];
    // The 8x4 rect covers 16x8 cells from 40,10; the empty label sits at its center.
    assert_eq!((b.x, b.y), (40 + 1 + 7, 10 + 4));
    key(&mut s, KeyCode::Char('a'));
    key(&mut s, KeyCode::Char('b'));
    key(&mut s, KeyCode::Char('c'));
    let b = &s.edit_boxes()[0];
    click(&mut s, (b.x as u16 + 1, b.y as u16), Instant::now());
    assert_eq!(caret(&s), 1);
}

#[test]
fn ctrl_wheel_zooms_around_the_cursor() {
    let mut s = session();
    s.view = View {
        ox: 7,
        oy: 3,
        ..View::default()
    };
    wheel(&mut s, true, (10, 5));
    assert_eq!(s.view.zoom, Zoom::Two);
    assert_eq!(s.view.to_world(10, 5), (17, 8));
    wheel(&mut s, false, (10, 5));
    wheel(&mut s, false, (10, 5));
    assert_eq!(s.view.zoom, Zoom::Half);
    assert_eq!(s.view.to_world(10, 5), (17, 8));
}

#[test]
fn zoom_clamps_at_both_ends() {
    let mut s = session();
    for _ in 0..5 {
        wheel(&mut s, true, (0, 0));
    }
    assert_eq!(s.view.zoom, Zoom::Three);
    for _ in 0..9 {
        wheel(&mut s, false, (0, 0));
    }
    assert_eq!(s.view.zoom, Zoom::Quarter);
}

#[test]
fn zoom_waits_for_an_edit_or_a_gesture_to_end() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.start_edit_existing(Id::from("aaaaaaaa"));
    wheel(&mut s, true, (0, 0));
    assert_eq!(s.view.zoom, Zoom::One);
    key(&mut s, KeyCode::Esc);
    mouse(
        &mut s,
        MouseEventKind::Down(MouseButton::Left),
        22,
        6,
        Instant::now(),
    );
    mouse(
        &mut s,
        MouseEventKind::Drag(MouseButton::Left),
        25,
        8,
        Instant::now(),
    );
    wheel(&mut s, true, (0, 0));
    assert_eq!(s.view.zoom, Zoom::One);
}

#[test]
fn half_zoom_is_read_only_and_esc_returns_to_one() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    click(&mut s, (20, 6), Instant::now());
    wheel(&mut s, false, (40, 12));
    assert_eq!(s.view.zoom, Zoom::Half);
    assert!(s.selection.is_empty());
    key(&mut s, KeyCode::Delete);
    key(&mut s, KeyCode::Char('r'));
    assert_eq!(s.tool, Tool::Pointer);
    assert_eq!(s.canvas.elements.len(), 1);
    let at = s.view.to_screen(21, 6);
    click(&mut s, (at.0 as u16, at.1 as u16), Instant::now());
    assert!(s.selection.is_empty());
    key(&mut s, KeyCode::Esc);
    assert_eq!(s.view.zoom, Zoom::One);
    assert_eq!(s.view.to_world(40, 12), (40, 12));
}

#[test]
fn half_zoom_drag_pans_whole_blocks() {
    let mut s = session();
    wheel(&mut s, false, (0, 0));
    drag(&mut s, (10, 5), (7, 4));
    assert_eq!((s.view.ox, s.view.oy), (6, 2));
}

#[test]
fn fit_fits_the_diagram_on_the_zoom_scale() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.canvas.elements.push(Element::Shape(Shape {
        id: Id::from("bbbbbbbb"),
        x: 300,
        ..shape_at_origin()
    }));
    s.motion = false;
    s.fit(s.canvas.all_bounds());
    assert_eq!(s.view.zoom, Zoom::Quarter);
    s.fit(s.canvas.all_bounds());
    assert_eq!(s.view.zoom, Zoom::Quarter);
    assert!(s.view.to_screen(20, 5).0 >= 0 && s.view.to_screen(308, 5).0 < 80);
}

fn shape_at_origin() -> Shape {
    let Some(Element::Shape(sh)) = with_shape("aaaaaaaa", 0, 0).canvas.elements.pop() else {
        unreachable!()
    };
    sh
}

#[test]
fn click_at_triple_zoom_selects_the_shape_under_the_stretched_cell() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.view = View {
        ox: 20,
        oy: 5,
        zoom: Zoom::Three,
    };
    click(&mut s, (2, 5), Instant::now());
    assert_eq!(s.selection, vec![Id::from("aaaaaaaa")]);
    s.selection.clear();
    click(&mut s, (60, 14), Instant::now());
    assert!(s.selection.is_empty());
}

#[test]
fn move_drag_at_double_zoom_moves_by_whole_world_cells() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.view = View {
        zoom: Zoom::Two,
        ..View::default()
    };
    drag(&mut s, (40, 12), (41, 14));
    let sh = s.canvas.shape(&Id::from("aaaaaaaa")).unwrap();
    assert_eq!((sh.x, sh.y), (20, 6));
}

#[test]
fn pan_drag_at_triple_zoom_carries_the_remainder() {
    let mut s = session();
    s.view.zoom = Zoom::Three;
    let t = Instant::now();
    mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 20, 5, t);
    mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 15, 5, t);
    assert_eq!(s.view.ox, 1);
    mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 13, 5, t);
    assert_eq!(s.view.ox, 2);
}

#[test]
fn present_blocks_edits_and_esc_returns() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.present = true;
    key(&mut s, KeyCode::Char('r'));
    click(&mut s, (20, 6), Instant::now());
    assert!(s.selection.is_empty());
    assert_eq!(s.tool, Tool::Pointer);
    key(&mut s, KeyCode::Esc);
    assert!(!s.present);
}

#[test]
fn wheel_still_scrolls_while_presenting() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.present = true;
    let before = s.view;
    mouse(&mut s, MouseEventKind::ScrollDown, 5, 5, Instant::now());
    assert_ne!(s.view, before);
}

fn alt_enter(s: &mut Session) {
    mod_key(s, KeyCode::Enter, KeyModifiers::ALT);
}

#[test]
fn newline_keys_insert_in_a_shape_label_only() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.start_edit_existing(Id::from("aaaaaaaa"));
    typed(&mut s, "a");
    alt_enter(&mut s);
    mod_key(&mut s, KeyCode::Char('j'), KeyModifiers::CONTROL);
    mod_key(&mut s, KeyCode::Enter, KeyModifiers::SHIFT);
    assert_eq!(buf(&s), "a\n\n\n");
    assert!(s.edit.is_some());
    key(&mut s, KeyCode::Enter);
    assert!(s.edit.is_none());

    s.start_edit_new((0, 0));
    mod_key(&mut s, KeyCode::Char('j'), KeyModifiers::CONTROL);
    assert_eq!(buf(&s), "");
}

#[test]
fn committing_a_wrapped_label_grows_the_shape_in_one_undo_step() {
    let mut s = session();
    s.canvas.elements.push(Element::Shape(Shape {
        id: Id::from("aaaaaaaa"),
        kind: ShapeKind::Rect,
        x: 20,
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
        turn: 0,
        meta: Default::default(),
    }));
    s.start_edit_existing(Id::from("aaaaaaaa"));
    typed(&mut s, "ab");
    alt_enter(&mut s);
    typed(&mut s, "cd");
    alt_enter(&mut s);
    typed(&mut s, "ef");
    key(&mut s, KeyCode::Enter);
    assert_eq!(
        (shapes(&s)[0].h, shapes(&s)[0].label.as_str()),
        (5, "ab\ncd\nef")
    );
    s.undo();
    assert_eq!((shapes(&s)[0].h, shapes(&s)[0].label.as_str()), (3, ""));

    let mut tall = with_shape("aaaaaaaa", 20, 5);
    tall.start_edit_existing(Id::from("aaaaaaaa"));
    typed(&mut tall, "x");
    key(&mut tall, KeyCode::Enter);
    assert_eq!(shapes(&tall)[0].h, 4);
}

#[test]
fn paste_keeps_newlines_in_a_shape_label_only() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.start_edit_existing(Id::from("aaaaaaaa"));
    handle_paste(&mut s, "a\r\nb");
    assert_eq!(buf(&s), "a\nb");
    s.start_edit_new((0, 0));
    handle_paste(&mut s, "a\r\nb");
    assert_eq!(buf(&s), "ab");
}

#[test]
fn click_puts_the_caret_on_the_clicked_wrapped_line() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.start_edit_existing(Id::from("aaaaaaaa"));
    typed(&mut s, "ab");
    alt_enter(&mut s);
    typed(&mut s, "cd");
    let boxes = s.edit_boxes();
    assert_eq!(boxes.len(), 2);
    assert_eq!(boxes[1].y, boxes[0].y + 1);
    let t = Instant::now();
    click(&mut s, (boxes[0].x as u16, boxes[0].y as u16), t);
    assert_eq!(caret(&s), 0);
    click(&mut s, (boxes[1].x as u16 + 1, boxes[1].y as u16), t);
    assert_eq!(caret(&s), 4);
}

#[test]
fn dragging_a_selected_lines_label_slides_it_along_the_line() {
    let mut s = session();
    let id = Id::from("llllllll");
    s.canvas
        .push(Element::Line(Line {
            id: id.clone(),
            kind: LineKind::Line,
            from: End::Free(5, 5),
            to: End::Free(25, 5),
            label: "ab".into(),
            label_at: None,
            stroke: Default::default(),
            bends: vec![],
            color: None,
            meta: Default::default(),
        }))
        .unwrap();
    s.selection = vec![id.clone()];
    let at = |s: &Session| match s.canvas.get(&Id::from("llllllll")) {
        Some(Element::Line(l)) => l.label_at,
        _ => panic!("no line"),
    };
    drag(&mut s, (14, 5), (20, 5));
    assert_eq!(at(&s), Some(750));
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(at(&s), None);
    s.selection = vec![id.clone()];
    drag(&mut s, (15, 5), (21, 5));
    s.start_edit_existing(id);
    assert_eq!(s.edit_boxes()[0].x, 20);
}

fn three_shapes() -> Session {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    for (id, x) in [("bbbbbbbb", 40), ("cccccccc", 60)] {
        s.canvas.elements.push(Element::Shape(Shape {
            id: Id::from(id),
            kind: ShapeKind::Rect,
            x,
            y: 5,
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
            turn: 0,
            meta: Default::default(),
        }));
    }
    s
}

fn grouped_ab() -> Session {
    let mut s = three_shapes();
    click(&mut s, (20, 6), Instant::now());
    shift_drag(&mut s, (40, 6), (40, 6));
    key(&mut s, KeyCode::Char('g'));
    click(&mut s, (70, 20), Instant::now());
    assert!(s.selection.is_empty());
    s
}

fn ab() -> Vec<Id> {
    vec![Id::from("aaaaaaaa"), Id::from("bbbbbbbb")]
}

#[test]
fn clicking_a_group_member_selects_the_group_and_shift_click_toggles_it() {
    let mut s = grouped_ab();
    click(&mut s, (20, 6), Instant::now());
    assert_eq!(s.selection, ab());
    shift_drag(&mut s, (60, 6), (60, 6));
    assert_eq!(s.selection.len(), 3);
    shift_drag(&mut s, (40, 6), (40, 6));
    assert_eq!(s.selection, vec![Id::from("cccccccc")]);
}

#[test]
fn a_band_over_one_member_selects_the_group() {
    let mut s = grouped_ab();
    shift_drag(&mut s, (36, 3), (50, 12));
    assert_eq!(
        s.selection,
        vec![Id::from("bbbbbbbb"), Id::from("aaaaaaaa")]
    );
}

#[test]
fn a_group_moves_together() {
    let mut s = grouped_ab();
    click(&mut s, (20, 6), Instant::now());
    key(&mut s, KeyCode::Right);
    assert_eq!(
        shapes(&s).iter().map(|s| s.x).collect::<Vec<_>>(),
        vec![21, 41, 60]
    );
}

#[test]
fn capital_g_ungroups_and_one_undo_regroups() {
    let mut s = grouped_ab();
    click(&mut s, (20, 6), Instant::now());
    key(&mut s, KeyCode::Char('G'));
    click(&mut s, (70, 20), Instant::now());
    click(&mut s, (20, 6), Instant::now());
    assert_eq!(s.selection, vec![Id::from("aaaaaaaa")]);
    key(&mut s, KeyCode::Char('u'));
    click(&mut s, (20, 6), Instant::now() + Duration::from_secs(1));
    assert_eq!(s.selection, ab());
}

#[test]
fn copy_paste_makes_a_second_group_and_one_undo_removes_it() {
    let mut s = grouped_ab();
    click(&mut s, (20, 6), Instant::now());
    key(&mut s, KeyCode::Char('y'));
    key(&mut s, KeyCode::Char('p'));
    assert_eq!(s.canvas.groups.len(), 2);
    assert_eq!(s.canvas.groups[0], ab());
    assert_eq!(s.selection, s.canvas.groups[1]);
    assert_eq!(s.undo().as_deref(), Some("Undid: add 2 elements"));
    assert_eq!(s.canvas.groups, vec![ab()]);
    assert_eq!(s.canvas.elements.len(), 3);
}

#[test]
fn g_types_a_letter_while_editing_a_label() {
    let mut s = three_shapes();
    s.start_edit_existing(Id::from("aaaaaaaa"));
    key(&mut s, KeyCode::Char('g'));
    assert!(s.canvas.groups.is_empty());
    assert_eq!(s.edit.as_ref().unwrap().buf, "g");
}

fn at(id: &str, x: i32, y: i32, w: i32, h: i32) -> Element {
    Element::Shape(Shape {
        id: Id::from(id),
        kind: ShapeKind::Rect,
        x,
        y,
        w,
        h,
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
    })
}

fn boxes(s: &Session) -> Vec<(i32, i32)> {
    shapes(s).iter().map(|s| (s.x, s.y)).collect()
}

fn aligned(keys: &str, selected: usize) -> Session {
    let mut s = session();
    s.canvas.elements = vec![at("aaaaaaaa", 2, 3, 4, 2), at("bbbbbbbb", 20, 10, 10, 6)];
    s.canvas.elements.push(at("cccccccc", 12, 30, 6, 4));
    s.selection = s
        .canvas
        .elements
        .iter()
        .take(selected)
        .map(|e| e.id().clone())
        .collect();
    for c in keys.chars() {
        key(&mut s, KeyCode::Char(c));
    }
    s
}

#[test]
fn align_keys_line_up_the_selection() {
    assert_eq!(boxes(&aligned("<", 3)), [(2, 3), (2, 10), (2, 30)]);
    assert_eq!(boxes(&aligned(">", 3)), [(26, 3), (20, 10), (24, 30)]);
    assert_eq!(boxes(&aligned("^", 3)), [(2, 3), (20, 3), (12, 3)]);
    assert_eq!(boxes(&aligned("_", 3)), [(2, 32), (20, 28), (12, 30)]);
    assert_eq!(boxes(&aligned("|", 3)), [(14, 3), (11, 10), (13, 30)]);
    assert_eq!(boxes(&aligned("-", 3)), [(2, 17), (20, 15), (12, 16)]);
}

#[test]
fn distribute_keys_even_the_gaps_and_undo_in_one_step() {
    let mut s = aligned("=", 3);
    assert_eq!(boxes(&s), [(2, 3), (20, 10), (10, 30)]);
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(boxes(&s), [(2, 3), (20, 10), (12, 30)]);
    assert_eq!(boxes(&aligned("#", 3)), [(2, 3), (20, 14), (12, 30)]);
}

#[test]
fn align_needs_two_and_distribute_three() {
    assert_eq!(boxes(&aligned("<=#", 1)), boxes(&aligned("", 0)));
    assert_eq!(boxes(&aligned("=", 2)), boxes(&aligned("", 0)));
}

#[test]
fn bound_arrow_follows_an_aligned_shape() {
    let mut s = aligned("", 0);
    s.canvas.elements.push(Element::Line(Line {
        id: Id::from("llllllll"),
        kind: LineKind::Arrow,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Free(40, 40),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
        meta: Default::default(),
    }));
    let before = s.canvas.route(match &s.canvas.elements[3] {
        Element::Line(l) => l,
        _ => unreachable!(),
    });
    s.selection = vec![Id::from("aaaaaaaa"), Id::from("bbbbbbbb")];
    key(&mut s, KeyCode::Char('>'));
    let after = s.canvas.route(match &s.canvas.elements[3] {
        Element::Line(l) => l,
        _ => unreachable!(),
    });
    assert_ne!(before, after);
}

#[test]
fn shift_arrow_adds_a_linked_shape_in_one_undo_step_and_edits_its_label() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.selection = vec![Id::from("aaaaaaaa")];
    handle_key(&mut s, KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
    let sh = shapes(&s);
    assert_eq!((sh[1].x, sh[1].y, sh[1].w, sh[1].h), (32, 5, 8, 4));
    let Element::Line(l) = &s.canvas.elements[2] else {
        panic!("no line")
    };
    assert_eq!(l.kind, LineKind::Arrow);
    assert_eq!(l.from, End::Bound(Id::from("aaaaaaaa"), Anchor::Auto));
    assert_eq!(l.to, End::Bound(sh[1].id.clone(), Anchor::Auto));
    assert!(
        matches!(&s.edit, Some(e) if matches!(&e.target, EditTarget::Existing(i) if *i == sh[1].id))
    );
    s.undo();
    assert_eq!(s.canvas.elements.len(), 1);
}

#[test]
fn palette_cycles_label_align_and_ctrl_b_toggles_bold() {
    let mut s = session();
    s.canvas.elements.push(Element::Shape(Shape {
        id: Id::from("rrrrrrrr"),
        kind: ShapeKind::Rect,
        x: 30,
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
        turn: 0,
        meta: Default::default(),
    }));
    s.selection = vec![Id::from("rrrrrrrr")];
    let get = |s: &Session| match s.canvas.get(&Id::from("rrrrrrrr")) {
        Some(Element::Shape(sh)) => (sh.align, sh.bold),
        _ => panic!("no shape"),
    };
    // Three strokes and four borders come first, then the align cell.
    option(&mut s, 7);
    assert_eq!(get(&s), (TextAlign::Right, false));
    option(&mut s, 7);
    assert_eq!(get(&s), (TextAlign::Left, false));
    s.undo();
    assert_eq!(get(&s), (TextAlign::Right, false));
    ctrl(&mut s, 'b');
    assert_eq!(get(&s), (TextAlign::Right, true));
    ctrl(&mut s, 'b');
    assert_eq!(get(&s), (TextAlign::Right, false));
}

#[test]
fn right_down_selects_unselected_keeps_selected_and_ignores_empty() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    assert_eq!(s.right_down(70, 20), Some(false));
    assert!(s.selection.is_empty());
    assert_eq!(s.right_down(20, 6), Some(true));
    assert_eq!(s.selection, vec![Id::from("aaaaaaaa")]);
    s.selection = vec![Id::from("zzzzzzzz"), Id::from("aaaaaaaa")];
    assert_eq!(s.right_down(20, 6), Some(true));
    assert_eq!(s.selection.len(), 2);
    s.view.zoom = Zoom::Half;
    assert_eq!(s.right_down(20, 6), None);
}

#[test]
fn ctrl_p_opens_the_palette_only_when_idle() {
    let ctrl_p = || KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
    let mut s = session();
    assert_eq!(handle_key(&mut s, ctrl_p()), KeyOutcome::Actions);
    s.view.zoom = Zoom::Half;
    assert_eq!(handle_key(&mut s, ctrl_p()), KeyOutcome::Actions);
    s.view.zoom = Zoom::One;
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.start_edit_existing(Id::from("aaaaaaaa"));
    assert_eq!(handle_key(&mut s, ctrl_p()), KeyOutcome::None);
}

#[test]
fn only_the_key_of_the_chosen_command_ui_opens_it() {
    let ctrl_p = || KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
    let mut s = session();
    assert_eq!(key(&mut s, KeyCode::Char(':')), KeyOutcome::None);
    assert_eq!(handle_key(&mut s, ctrl_p()), KeyOutcome::Actions);
    s.command_ui = CommandUi::Vim;
    assert_eq!(key(&mut s, KeyCode::Char(':')), KeyOutcome::Command);
    assert_eq!(handle_key(&mut s, ctrl_p()), KeyOutcome::None);
    assert_eq!(key(&mut s, KeyCode::Char('/')), KeyOutcome::Find);
    s.view.zoom = Zoom::Half;
    assert_eq!(handle_key(&mut s, ctrl_p()), KeyOutcome::None);
}

fn put(s: &mut Session, id: &str, kind: ShapeKind, r: (i32, i32, i32, i32)) {
    s.canvas.elements.push(Element::Shape(Shape {
        id: Id::from(id),
        kind,
        x: r.0,
        y: r.1,
        w: r.2,
        h: r.3,
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
    }));
}

fn framed() -> Session {
    let mut s = session();
    put(&mut s, "inner000", ShapeKind::Rect, (22, 7, 6, 3));
    put(&mut s, "frame000", ShapeKind::Frame, (20, 5, 20, 10));
    s
}

fn xs(s: &Session) -> Vec<i32> {
    shapes(s).iter().map(|sh| sh.x).collect()
}

#[test]
fn frame_border_click_selects_contents_and_drag_moves_all_with_one_undo() {
    let mut s = framed();
    drag(&mut s, (30, 5), (33, 5));
    assert_eq!(s.selection.len(), 2);
    assert_eq!(xs(&s), vec![25, 23]);
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(xs(&s), vec![22, 20]);
}

#[test]
fn frame_interior_click_selects_the_content_under_it_or_starts_a_band() {
    let mut s = framed();
    click(&mut s, (22, 8), Instant::now());
    assert_eq!(s.selection, vec![Id::from("inner000")]);
    click(&mut s, (35, 12), Instant::now());
    assert!(s.selection.is_empty());
}

#[test]
fn frame_border_wins_over_a_later_shape_but_its_interior_does_not() {
    let mut s = framed();
    put(&mut s, "later000", ShapeKind::Rect, (36, 8, 6, 3));
    click(&mut s, (39, 8), Instant::now());
    assert!(s.selection.contains(&Id::from("frame000")));
    click(&mut s, (37, 8), Instant::now());
    assert_eq!(s.selection, vec![Id::from("later000")]);
}

#[test]
fn delete_removes_frame_and_contents_and_one_undo_restores_both() {
    let mut s = framed();
    click(&mut s, (30, 5), Instant::now());
    key(&mut s, KeyCode::Delete);
    assert!(s.canvas.elements.is_empty());
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(s.canvas.elements.len(), 2);
}

#[test]
fn band_inside_a_frame_skips_it_and_band_around_it_takes_it() {
    let mut s = framed();
    shift_drag(&mut s, (23, 6), (30, 12));
    assert_eq!(s.selection, vec![Id::from("inner000")]);
    shift_drag(&mut s, (18, 3), (45, 17));
    assert!(s.selection.contains(&Id::from("frame000")));
    assert!(s.selection.contains(&Id::from("inner000")));
}

#[test]
fn frame_with_contents_shows_corner_handles_and_resize_moves_only_the_frame() {
    let mut s = framed();
    click(&mut s, (30, 5), Instant::now());
    drag(&mut s, (39, 14), (44, 16));
    let f = shapes(&s)[1].clone();
    assert_eq!((f.w, f.h), (25, 12));
    assert_eq!(shapes(&s)[0].x, 22);
}

#[test]
fn fit_glides_to_the_target_with_ease_out() {
    let mut s = session();
    let t0 = Instant::now();
    s.fit(Some((100, 50, 10, 4)));
    assert_eq!((s.view.ox, s.view.oy), (0, 0));
    assert_eq!(s.glide_wait(), Some(Duration::from_millis(16)));
    assert!(!s.glide_step(t0));
    assert!(s.glide_step(t0 + Duration::from_millis(60)));
    assert_eq!((s.view.ox, s.view.oy), (57, 35));
    assert!(s.glide_step(t0 + Duration::from_millis(120)));
    assert_eq!((s.view.ox, s.view.oy), (65, 40));
    assert_eq!(s.glide_wait(), None);
    assert!(!s.glide_step(t0 + Duration::from_millis(200)));
}

#[test]
fn fit_snaps_when_motion_is_off() {
    let mut s = session();
    s.motion = false;
    s.fit(Some((100, 50, 10, 4)));
    assert_eq!((s.view.ox, s.view.oy), (65, 40));
    assert_eq!(s.glide_wait(), None);
}

#[test]
fn scroll_speed_scales_the_pan_without_a_one_cell_floor() {
    let mut s = session();
    s.scroll_speed = 150;
    scroll_down(&mut s, Instant::now());
    assert_eq!(s.view.oy, 3);
    let mut s = session();
    s.scroll_speed = 25;
    let t0 = Instant::now();
    for i in 0..4 {
        scroll_down(&mut s, t0 + Duration::from_millis(200 * i));
    }
    assert_eq!(s.view.oy, 2);
}

#[test]
fn scrolling_cancels_a_glide() {
    let mut s = session();
    let t0 = Instant::now();
    s.fit(Some((100, 50, 10, 4)));
    s.glide_step(t0);
    s.scroll((0, 2), 1, t0);
    assert_eq!(s.glide_wait(), None);
    assert!(!s.glide_step(t0 + Duration::from_millis(60)));
    assert_eq!((s.view.ox, s.view.oy), (0, 2));
}

#[test]
fn panning_cancels_a_glide() {
    let mut s = session();
    let t0 = Instant::now();
    s.fit(Some((100, 50, 10, 4)));
    s.glide_step(t0);
    drag(&mut s, (30, 10), (27, 10));
    assert_eq!(s.glide_wait(), None);
    assert_eq!((s.view.ox, s.view.oy), (3, 0));
}

#[test]
fn set_link_sets_clears_undoes_and_rejects_unsafe_urls() {
    let mut s = session();
    s.canvas.elements.push(Element::Shape(Shape {
        id: Id::from("rrrrrrrr"),
        kind: ShapeKind::Rect,
        x: 30,
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
        turn: 0,
        meta: Default::default(),
    }));
    s.selection = vec![Id::from("rrrrrrrr")];
    let get = |s: &Session| match s.canvas.get(&Id::from("rrrrrrrr")) {
        Some(Element::Shape(sh)) => sh.link.clone(),
        _ => panic!("no shape"),
    };
    assert!(!s.set_link("javascript:alert(1)"));
    assert!(!s.set_link("https://a b"));
    assert_eq!(get(&s), None);
    assert!(s.set_link("https://example.com"));
    assert_eq!(get(&s).as_deref(), Some("https://example.com"));
    assert!(s.set_link(""));
    assert_eq!(get(&s), None);
    s.undo();
    assert_eq!(get(&s).as_deref(), Some("https://example.com"));
}

fn two_selected() -> Session {
    let mut s = two_shapes_and_arrow_tool();
    s.tool = Tool::Pointer;
    s.selection = vec![Id::from("aaaaaaaa"), Id::from("bbbbbbbb")];
    s
}

#[test]
fn bar_group_cell_groups_then_ungroup_cell_appears_and_clears_it() {
    let mut s = two_selected();
    assert!(!option_cells(&s)
        .iter()
        .any(|c| c.0 == PaletteHit::Option(Choice::Ungroup)));
    click_choice(&mut s, Choice::Group);
    assert_eq!(s.canvas.groups.len(), 1);
    click_choice(&mut s, Choice::Ungroup);
    assert!(s.canvas.groups.is_empty());
}

#[test]
fn bar_align_left_cell_aligns_left_edges() {
    let mut s = two_selected();
    click_choice(&mut s, Choice::Place(Align::Left));
    let xs: Vec<i32> = shapes(&s).iter().map(|sh| sh.x).collect();
    assert_eq!(xs, [15, 15]);
}

#[test]
fn bar_hides_place_and_group_with_one_selected_and_all_with_none() {
    let mut s = two_selected();
    s.selection = vec![Id::from("aaaaaaaa")];
    let has = |s: &Session, c| option_cells(s).iter().any(|k| k.0 == PaletteHit::Option(c));
    assert!(!has(&s, Choice::Group));
    assert!(!has(&s, Choice::Place(Align::Left)));
    s.selection.clear();
    assert!(option_cells(&s).is_empty());
}

fn narrow_two_selected(w: u16) -> Session {
    let mut s = Session::new(Canvas::default(), View::default(), (w, 24));
    s.canvas = two_selected().canvas;
    s.selection = vec![Id::from("aaaaaaaa"), Id::from("bbbbbbbb")];
    s
}

fn arrow(s: &Session, dir: i32) -> Option<(u16, u16)> {
    (0..s.screen.0)
        .flat_map(|c| (0..2u16).map(move |r| (c, r)))
        .find(|&(c, r)| {
            palette::hit(s, c.into(), r.into()) == Some(PaletteHit::Scroll(Strip::Options, dir))
        })
}

#[test]
fn strip_stays_on_one_row_and_a_click_on_a_clipped_cell_runs_it() {
    let s = narrow_two_selected(40);
    assert_eq!(palette::rows(&s), 2);
    let cells = option_cells(&s);
    assert!(cells.iter().all(|&(_, x, y, w)| y == 1 && x + w <= 40));
    let &(h, x, y, _) = cells.last().unwrap();
    assert_eq!(palette::hit(&s, x, y), Some(h));
}

#[test]
fn strip_wheel_and_shift_wheel_scroll_the_row_and_not_the_canvas() {
    let mut s = narrow_two_selected(40);
    let view = s.view;
    let t = Instant::now();
    mouse(&mut s, MouseEventKind::ScrollRight, 10, 1, t);
    assert_eq!(s.option_scroll, 4);
    mouse_mod(
        &mut s,
        MouseEventKind::ScrollDown,
        KeyModifiers::SHIFT,
        10,
        1,
        t,
    );
    assert_eq!(s.option_scroll, 8);
    mouse(&mut s, MouseEventKind::ScrollLeft, 10, 1, t);
    mouse_mod(
        &mut s,
        MouseEventKind::ScrollUp,
        KeyModifiers::SHIFT,
        10,
        1,
        t,
    );
    mouse(&mut s, MouseEventKind::ScrollLeft, 10, 1, t);
    assert_eq!(s.option_scroll, 0, "never below 0");
    for _ in 0..100 {
        mouse(&mut s, MouseEventKind::ScrollRight, 10, 1, t);
    }
    let max = s.option_scroll;
    mouse(&mut s, MouseEventKind::ScrollRight, 10, 1, t);
    assert_eq!(s.option_scroll, max, "never above max");
    assert_eq!(s.tool_scroll, 0);
    assert_eq!(s.view, view);
    mouse(&mut s, MouseEventKind::ScrollRight, 10, 0, t);
    assert!(s.tool_scroll > 0, "row 0 scrolls the tools");
    mouse(&mut s, MouseEventKind::ScrollDown, 10, 5, t);
    assert_ne!(s.view, view, "a plain wheel below the chrome still pans");
}

#[test]
fn strip_arrows_scroll_by_group_and_a_dimmed_arrow_does_nothing() {
    let mut s = narrow_two_selected(40);
    assert_eq!(arrow(&s, -1), None, "dimmed arrow is no hit");
    let right = arrow(&s, 1).unwrap();
    click(&mut s, right, Instant::now());
    let layout = palette::layout(&s, 40, (0, 0));
    assert!(s.option_scroll > 0);
    assert!(layout.cells.len() > 1);
    assert!(arrow(&s, -1).is_some());
    let back = arrow(&s, -1).unwrap();
    click(&mut s, back, Instant::now());
    assert_eq!(s.option_scroll, 0);
    click(&mut s, back, Instant::now());
    assert_eq!(s.option_scroll, 0);
}

#[test]
fn captions_are_not_hits() {
    let s = narrow_two_selected(120);
    let cap = palette::layout(&s, 120, (0, 0)).caps[0];
    assert_eq!(palette::hit(&s, cap.x, cap.y), None);
}

#[test]
fn tool_dividers_are_not_hits_and_arrows_jump_to_a_group_start() {
    let mut s = Session::new(Canvas::default(), View::default(), (40, 24));
    assert_eq!(
        palette::hit(&s, 11, 0),
        None,
        "divider after Select is no hit"
    );
    palette::jump(&mut s, Strip::Tools, 1);
    assert_eq!(s.tool_scroll, 9, "Select width plus its divider");
}

fn rect_of(s: &Session) -> (i32, i32, i32, i32) {
    let sh = shapes(s)[0];
    (sh.x, sh.y, sh.w, sh.h)
}

#[test]
fn ratio_create_drag_snaps_bound_kinds_and_leaves_free_ones() {
    for (tool, size) in [
        (Tool::Diamond, (11, 6)),
        (Tool::Hexagon, (12, 6)),
        (Tool::Parallelogram, (12, 6)),
        (Tool::Rect, (12, 5)),
        (Tool::Cylinder, (12, 5)),
        (Tool::Frame, (12, 5)),
    ] {
        let mut s = session();
        pick(&mut s, tool);
        drag(&mut s, (20, 5), (31, 9));
        assert_eq!(rect_of(&s), (20, 5, size.0, size.1), "{tool:?}");
    }
}

#[test]
fn ratio_create_drag_never_leaves_a_noisy_minimum() {
    let mut s = session();
    pick(&mut s, Tool::Diamond);
    drag(&mut s, (20, 5), (22, 5));
    assert_eq!(rect_of(&s), (20, 5, 5, 3));
}

#[test]
fn ratio_handle_resize_snaps_and_keeps_the_fixed_corner() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.selection = vec![Id::from("aaaaaaaa")];
    if let Element::Shape(sh) = &mut s.canvas.elements[0] {
        sh.kind = ShapeKind::Hexagon;
    }
    drag(&mut s, (27, 8), (33, 10));
    assert_eq!(rect_of(&s), (20, 5, 14, 7));
    drag(&mut s, (33, 11), (43, 15));
    assert_eq!(rect_of(&s), (20, 5, 24, 12));
}

#[test]
fn ratio_shift_drag_ends_at_the_ratio_size() {
    let mut s = session();
    pick(&mut s, Tool::Hexagon);
    shift_drag(&mut s, (20, 5), (30, 8));
    assert_eq!(rect_of(&s), (20, 5, 16, 8));
}

#[test]
fn ratio_label_growth_widens_a_bound_shape() {
    let mut s = session();
    s.canvas.elements.push(Element::Shape(Shape {
        id: Id::from("aaaaaaaa"),
        kind: ShapeKind::Diamond,
        x: 20,
        y: 5,
        w: 9,
        h: 5,
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
    }));
    s.start_edit_existing(Id::from("aaaaaaaa"));
    for line in ["a", "b", "c"] {
        typed(&mut s, line);
        alt_enter(&mut s);
    }
    typed(&mut s, "d");
    key(&mut s, KeyCode::Enter);
    assert_eq!(rect_of(&s), (20, 5, 11, 6));
}

#[test]
fn hover_picks_a_detour_cell_for_the_line_and_the_crossed_shape_elsewhere() {
    let mut s = session();
    for (id, x) in [("aaaaaaaa", 0), ("oooooooo", 10), ("bbbbbbbb", 20)] {
        put(&mut s, id, ShapeKind::Rect, (x, 4, 3, 3));
    }
    s.canvas.elements.push(Element::Line(Line {
        id: Id::from("arrow000"),
        kind: LineKind::Arrow,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
        meta: Default::default(),
    }));
    let Some(Element::Line(l)) = s.canvas.elements.last() else {
        unreachable!()
    };
    let detour = s.canvas.route(l).into_iter().find(|p| p.0 == 11).unwrap();
    assert_ne!(detour.1, 5);
    let t = Instant::now();
    mouse(
        &mut s,
        MouseEventKind::Moved,
        detour.0 as u16,
        detour.1 as u16,
        t,
    );
    assert_eq!(s.hover, Hit::Element(Id::from("arrow000")));
    mouse(&mut s, MouseEventKind::Moved, 10, 5, t);
    assert_eq!(s.hover, Hit::Element(Id::from("oooooooo")));
}

#[test]
fn hover_reaches_a_detour_cell_past_the_margin_of_the_end_cells() {
    let mut s = session();
    put(&mut s, "aaaaaaaa", ShapeKind::Rect, (0, 4, 3, 3));
    put(&mut s, "bbbbbbbb", ShapeKind::Rect, (6, 9, 3, 3));
    put(&mut s, "oooooooo", ShapeKind::Rect, (3, -2, 2, 16));
    s.canvas.elements.push(Element::Line(Line {
        id: Id::from("arrow000"),
        kind: LineKind::Arrow,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
        meta: Default::default(),
    }));
    let Some(Element::Line(l)) = s.canvas.elements.last() else {
        unreachable!()
    };
    let (_, y, _, h) = s.canvas.bounds(&Element::Line(l.clone()));
    let low = s.canvas.route(l).into_iter().max_by_key(|p| p.1).unwrap();
    assert!(low.1 >= y + h + 3, "the detour must pass the margin");
    mouse(
        &mut s,
        MouseEventKind::Moved,
        low.0 as u16,
        low.1 as u16,
        Instant::now(),
    );
    assert_eq!(s.hover, Hit::Element(Id::from("arrow000")));
}

fn free_line(id: &str, bends: Vec<(i32, i32)>) -> Element {
    Element::Line(Line {
        id: Id::from(id),
        kind: LineKind::Arrow,
        from: End::Free(5, 5),
        to: End::Free(25, 5),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends,
        color: None,
        meta: Default::default(),
    })
}

fn bends_of(s: &Session) -> Vec<(i32, i32)> {
    match s.canvas.get(&Id::from("llllllll")) {
        Some(Element::Line(l)) => l.bends.clone(),
        _ => panic!("no line"),
    }
}

#[test]
fn dragging_a_selected_line_adds_a_bend_and_one_undo_removes_it() {
    let mut s = session();
    s.canvas.push(free_line("llllllll", vec![])).unwrap();
    s.selection = vec![Id::from("llllllll")];
    drag(&mut s, (15, 5), (15, 8));
    assert_eq!(bends_of(&s), vec![(15, 8)]);
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(bends_of(&s), vec![]);
}

#[test]
fn dragging_a_selected_line_off_a_dot_moves_the_segment_and_undo_restores_it() {
    let mut s = session();
    s.canvas.push(free_line("llllllll", vec![])).unwrap();
    s.selection = vec![Id::from("llllllll")];
    drag(&mut s, (10, 5), (10, 8));
    assert_eq!(bends_of(&s), vec![(5, 8), (25, 8)]);
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(bends_of(&s), vec![]);
}

#[test]
fn dragging_a_middle_leg_moves_both_its_bends() {
    let mut s = session();
    let bends = vec![(10, 8), (20, 8)];
    s.canvas.push(free_line("llllllll", bends.clone())).unwrap();
    s.selection = vec![Id::from("llllllll")];
    drag(&mut s, (12, 8), (12, 10));
    assert_eq!(bends_of(&s), vec![(10, 10), (20, 10)]);
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(bends_of(&s), bends);
}

#[test]
fn dragging_the_leg_at_the_head_adds_a_bend_so_the_head_stays() {
    let mut s = session();
    s.canvas
        .push(free_line("llllllll", vec![(10, 8), (20, 8)]))
        .unwrap();
    s.selection = vec![Id::from("llllllll")];
    drag(&mut s, (10, 6), (12, 6));
    assert_eq!(bends_of(&s), vec![(7, 5), (12, 8), (20, 8)]);
}

#[test]
fn dragging_one_of_two_selected_elements_still_moves_the_line_whole() {
    let mut s = with_shape("aaaaaaaa", 40, 20);
    s.canvas.push(free_line("llllllll", vec![])).unwrap();
    s.selection = vec![Id::from("llllllll"), Id::from("aaaaaaaa")];
    drag(&mut s, (10, 5), (10, 8));
    let Some(Element::Line(l)) = s.canvas.get(&Id::from("llllllll")) else {
        unreachable!()
    };
    assert_eq!(s.canvas.line_ends(l), ((5, 8), (25, 8)));
}

#[test]
fn after_a_bend_drag_each_new_leg_shows_a_dot_that_hovers_as_mid() {
    let mut s = session();
    s.canvas.push(free_line("llllllll", vec![])).unwrap();
    s.selection = vec![Id::from("llllllll")];
    drag(&mut s, (15, 5), (15, 9));
    let id = Id::from("llllllll");
    assert_eq!(
        hover_at(&mut s, (12, 5)),
        Hit::Handle(Handle::Mid(0), id.clone())
    );
    assert_eq!(hover_at(&mut s, (22, 9)), Hit::Handle(Handle::Mid(1), id));
    s.selection = vec![];
    assert_eq!(
        hover_at(&mut s, (12, 5)),
        Hit::Element(Id::from("llllllll"))
    );
}

#[test]
fn the_first_drag_on_an_unselected_line_moves_its_segment() {
    let mut s = session();
    s.canvas.push(free_line("llllllll", vec![])).unwrap();
    drag(&mut s, (10, 5), (10, 8));
    assert_eq!(bends_of(&s), vec![(5, 8), (25, 8)]);
}

#[test]
fn releasing_a_bend_past_the_end_drops_the_spike() {
    let mut s = session();
    s.canvas.push(free_line("llllllll", vec![(15, 8)])).unwrap();
    s.selection = vec![Id::from("llllllll")];
    drag(&mut s, (15, 8), (30, 5));
    assert_eq!(bends_of(&s), vec![]);
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(bends_of(&s), vec![(15, 8)]);
}

#[test]
fn dragging_a_bend_handle_moves_that_bend_and_undo_restores_it() {
    let mut s = session();
    s.canvas.push(free_line("llllllll", vec![(15, 8)])).unwrap();
    s.selection = vec![Id::from("llllllll")];
    drag(&mut s, (15, 8), (18, 10));
    assert_eq!(bends_of(&s), vec![(18, 10)]);
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(bends_of(&s), vec![(15, 8)]);
}

#[test]
fn reset_route_pins_the_automatic_route_and_undo_restores_the_bends() {
    let mut s = session();
    s.canvas.push(free_line("llllllll", vec![(15, 8)])).unwrap();
    s.selection = vec![Id::from("llllllll")];
    assert!(s.line_has_bends());
    s.reset_route();
    assert_eq!(bends_of(&s), vec![(15, 5)]);
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(bends_of(&s), vec![(15, 8)]);
}

#[test]
fn arrange_pins_a_fresh_route_around_a_shape_and_undo_restores_the_bends() {
    let mut s = with_shape("aaaaaaaa", 12, 3);
    let Element::Line(fresh) = free_line("llllllll", vec![]) else {
        unreachable!()
    };
    let expected = s.canvas.route(&fresh);
    s.canvas.push(free_line("llllllll", vec![(15, 9)])).unwrap();
    s.selection = vec![Id::from("llllllll")];
    click_choice(&mut s, Choice::Arrange);
    assert_ne!(bends_of(&s), vec![(15, 9)]);
    assert_eq!(route_of(&s), expected);
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(bends_of(&s), vec![(15, 9)]);
}

#[test]
fn a_second_arrange_records_no_history() {
    let mut s = session();
    s.canvas.push(free_line("llllllll", vec![])).unwrap();
    s.selection = vec![Id::from("llllllll")];
    click_choice(&mut s, Choice::Arrange);
    let pinned = bends_of(&s);
    assert!(!pinned.is_empty());
    click_choice(&mut s, Choice::Arrange);
    assert_eq!(bends_of(&s), pinned);
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(bends_of(&s), vec![]);
}

fn route_of(s: &Session) -> Vec<(i32, i32)> {
    let Some(Element::Line(l)) = s.canvas.get(&Id::from("llllllll")) else {
        unreachable!()
    };
    s.canvas.route(l)
}

fn fresh_route_of(s: &Session) -> Vec<(i32, i32)> {
    let Some(Element::Line(l)) = s.canvas.get(&Id::from("llllllll")) else {
        unreachable!()
    };
    s.canvas.route(&Line {
        bends: vec![],
        ..l.clone()
    })
}

fn shape_y(s: &Session, id: &str) -> i32 {
    s.canvas.shape(&Id::from(id)).unwrap().y
}

/// Shapes a and b side by side, an arrow drawn between them with the mouse, and a third shape c below.
fn drawn_arrow() -> Session {
    let mut s = with_shape("aaaaaaaa", 10, 10);
    for (id, x, y) in [("bbbbbbbb", 40, 10), ("cccccccc", 24, 20)] {
        let mut t = with_shape(id, x, y);
        s.canvas.elements.append(&mut t.canvas.elements);
    }
    pick(&mut s, Tool::Arrow);
    drag(&mut s, (14, 12), (44, 12));
    let Some(Element::Line(l)) = s.canvas.elements.last_mut() else {
        unreachable!()
    };
    l.id = Id::from("llllllll");
    s.selection.clear();
    s
}

#[test]
fn drawing_a_shape_over_a_drawn_arrow_keeps_its_route() {
    let mut s = drawn_arrow();
    let before = route_of(&s);
    pick(&mut s, Tool::Rect);
    drag(&mut s, (24, 15), (30, 19));
    assert_eq!(shapes(&s).len(), 4);
    assert_eq!(route_of(&s), before);
}

#[test]
fn moving_an_unrelated_shape_across_an_arrow_keeps_its_route() {
    let mut s = drawn_arrow();
    let before = route_of(&s);
    drag(&mut s, (24, 21), (24, 16));
    assert_eq!(shape_y(&s, "cccccccc"), 15);
    assert_eq!(route_of(&s), before);
}

#[test]
fn moving_an_end_shape_reroutes_the_arrow_and_one_undo_restores_both() {
    let mut s = drawn_arrow();
    let (before, bends) = (route_of(&s), bends_of(&s));
    drag(&mut s, (40, 11), (40, 17));
    assert_ne!(shape_y(&s, "bbbbbbbb"), 10);
    assert_ne!(route_of(&s), before);
    assert_eq!(route_of(&s), fresh_route_of(&s));
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(shape_y(&s, "bbbbbbbb"), 10);
    assert_eq!((route_of(&s), bends_of(&s)), (before, bends));
}

#[test]
fn nudging_an_end_shape_pins_the_new_route_in_one_undo_step() {
    let mut s = drawn_arrow();
    let (before, bends) = (route_of(&s), bends_of(&s));
    s.selection = vec![Id::from("bbbbbbbb")];
    key(&mut s, KeyCode::Char('j'));
    assert_eq!(shape_y(&s, "bbbbbbbb"), 11);
    assert_eq!(route_of(&s), fresh_route_of(&s));
    assert_eq!(s.undo().as_deref(), Some("Undid: move 1 element"));
    s.redo();
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(shape_y(&s, "bbbbbbbb"), 10);
    assert_eq!((route_of(&s), bends_of(&s)), (before, bends));
}

#[test]
fn hjkl_pan_with_empty_selection_and_move_with_one() {
    let mut s = session();
    let (ox, oy) = (s.view.ox, s.view.oy);
    for c in ['l', 'l', 'j', 'h', 'k'] {
        key(&mut s, KeyCode::Char(c));
    }
    assert_eq!((s.view.ox, s.view.oy), (ox + 1, oy));
    assert_eq!(s.tool, Tool::Pointer);
    key(&mut s, KeyCode::Char('H'));
    assert_eq!((s.view.ox, s.view.oy), (ox + 1, oy));
}

#[test]
fn a_dragged_head_over_a_shape_routes_to_its_edge() {
    let mut s = with_shape("aaaaaaaa", 10, 10);
    for (id, y) in [("bbbbbbbb", 10), ("cccccccc", 20)] {
        let mut t = with_shape(id, 40, y);
        s.canvas.elements.append(&mut t.canvas.elements);
    }
    let line = Id::from("line0000");
    s.canvas.elements.push(Element::Line(Line {
        id: line.clone(),
        kind: LineKind::ArrowBoth,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
        meta: Default::default(),
    }));
    s.selection = vec![line.clone()];
    let Some(Element::Line(l)) = s.canvas.get(&line) else {
        unreachable!()
    };
    let (_, head) = s.canvas.line_ends(l);
    let t = Instant::now();
    mouse(
        &mut s,
        MouseEventKind::Down(MouseButton::Left),
        head.0 as u16,
        head.1 as u16,
        t,
    );
    mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 43, 21, t);
    let Some(Element::Line(l)) = s.canvas.get(&line) else {
        unreachable!()
    };
    let cells = s.canvas.route(l);
    let tip = *cells.last().unwrap();
    assert!(
        !(40..48).contains(&tip.0) || !(20..24).contains(&tip.1),
        "head inside C: {cells:?}"
    );
}

#[test]
fn viewport_sits_between_the_bars_and_present_takes_the_screen() {
    let mut s = session();
    assert_eq!(s.viewport(), (0, 1, 80, 22));
    s.present = true;
    assert_eq!(s.viewport(), (0, 0, 80, 24));
}

#[test]
fn a_click_in_a_chrome_row_starts_nothing() {
    let t = Instant::now();
    let mut s = with_shape("aaaaaaaa", 20, 0);
    s.selection = vec![Id::from("aaaaaaaa")];
    mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 60, 0, t);
    assert_eq!(s.selection.len(), 1);
    mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 70, 23, t);
    assert_eq!(s.selection.len(), 1);
    mouse(&mut s, MouseEventKind::Up(MouseButton::Left), 70, 23, t);
    assert_eq!(s.selection.len(), 1);
}

#[test]
fn fit_all_keeps_the_content_inside_the_viewport() {
    let mut s = with_shape("aaaaaaaa", 100, 100);
    s.motion = false;
    s.fit(s.canvas.all_bounds());
    let (_, vy, _, vh) = s.viewport();
    let (_, y) = s.view.to_screen(100, 100);
    let (_, y2) = s.view.to_screen(100, 103);
    assert!(y >= vy && y2 < vy + vh);
}

fn side_labels(s: &Session) -> Vec<SideLabel> {
    shapes(s)[0].side_labels.clone()
}

fn type_text(s: &mut Session, text: &str) {
    for c in text.chars() {
        key(s, KeyCode::Char(c));
    }
    key(s, KeyCode::Enter);
}

fn double_click(s: &mut Session, at: (u16, u16)) {
    let t = Instant::now();
    click(s, at, t);
    click(s, at, t + Duration::from_millis(100));
}

#[test]
fn double_click_on_a_border_edits_that_side_at_the_clicked_cell() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    double_click(&mut s, (24, 5));
    assert!(matches!(
        &s.edit,
        Some(e) if matches!(&e.target, EditTarget::Side(_, Edge::Top, Some(600)))
    ));
    type_text(&mut s, "yes");
    assert_eq!(
        side_labels(&s),
        [SideLabel {
            edge: Edge::Top,
            at: Some(600),
            text: "yes".into()
        }]
    );
    assert_eq!(shapes(&s)[0].label, "");
}

#[test]
fn double_click_on_a_left_border_cell_places_the_label_at_that_row() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    double_click(&mut s, (20, 7));
    type_text(&mut s, "l");
    assert_eq!(
        side_labels(&s),
        [SideLabel {
            edge: Edge::Left,
            at: Some(1000),
            text: "l".into()
        }]
    );
}

#[test]
fn double_click_on_a_corner_or_inside_keeps_editing_the_main_label() {
    for at in [(20, 5), (27, 8), (22, 6)] {
        let mut s = with_shape("aaaaaaaa", 20, 5);
        double_click(&mut s, at);
        assert!(
            matches!(&s.edit, Some(e) if matches!(&e.target, EditTarget::Existing(_))),
            "{at:?}"
        );
    }
}

#[test]
fn double_click_on_a_side_label_keeps_its_position_and_empty_text_removes_it() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    double_click(&mut s, (24, 5));
    type_text(&mut s, "yes");
    key(&mut s, KeyCode::Esc);
    double_click(&mut s, (23, 5));
    assert!(matches!(
        &s.edit,
        Some(e) if e.buf == "yes" && matches!(&e.target, EditTarget::Side(_, Edge::Top, Some(600)))
    ));
    for _ in 0..3 {
        key(&mut s, KeyCode::Backspace);
    }
    key(&mut s, KeyCode::Enter);
    assert!(side_labels(&s).is_empty());
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(side_labels(&s)[0].text, "yes");
}

#[test]
fn an_empty_side_edit_on_a_fresh_edge_records_nothing() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    double_click(&mut s, (24, 5));
    key(&mut s, KeyCode::Enter);
    assert!(side_labels(&s).is_empty());
    key(&mut s, KeyCode::Char('u'));
    assert!(side_labels(&s).is_empty());
}

#[test]
fn dragging_a_selected_rects_side_label_slides_it_along_its_side_in_one_undo_step() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    double_click(&mut s, (22, 5));
    type_text(&mut s, "a");
    s.selection = vec![Id::from("aaaaaaaa")];
    let before = side_labels(&s);
    assert_eq!(before[0].at, Some(200));
    drag(&mut s, (22, 5), (25, 7));
    assert_eq!(
        side_labels(&s),
        [SideLabel {
            edge: Edge::Top,
            at: Some(800),
            text: "a".into()
        }]
    );
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (20, 5));
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(side_labels(&s), before);
}

fn picked(id: &str, x: i32, y: i32) -> Session {
    let mut s = with_shape(id, x, y);
    s.selection = vec![Id::from(id)];
    s
}

fn lines(s: &Session) -> Vec<&Line> {
    s.canvas
        .elements
        .iter()
        .filter_map(|e| {
            if let Element::Line(l) = e {
                Some(l)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn quick_connect_ghosts_beside_a_selected_shape_and_a_click_adds_both_in_one_step() {
    let t = Instant::now();
    let mut s = picked("aaaaaaaa", 20, 6);
    mouse(&mut s, MouseEventKind::Moved, 28, 7, t);
    let q = s.quick.clone().expect("ghost right of the shape");
    assert_eq!((q.edge, q.rect), (Edge::Right, (34, 6, 8, 4)));
    click(&mut s, (28, 7), t);
    let sh = shapes(&s);
    assert_eq!(sh.len(), 2);
    assert_eq!((sh[1].x, sh[1].y, sh[1].w, sh[1].h), (34, 6, 8, 4));
    let l = lines(&s);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].kind, LineKind::Arrow);
    assert_eq!(l[0].from, End::Bound(Id::from("aaaaaaaa"), Anchor::Auto));
    assert_eq!(l[0].to, End::Bound(sh[1].id.clone(), Anchor::Auto));
    assert_eq!(s.selection, vec![sh[1].id.clone()]);
    key(&mut s, KeyCode::Char('u'));
    assert_eq!(s.canvas.elements.len(), 1);
}

fn trio() -> Session {
    let mut c = Canvas::default();
    for (id, kind, x, y, w, h) in [
        ("rect", ShapeKind::Rect, -70, -4, 38, 20),
        ("cyl", ShapeKind::Cylinder, 5, -19, 36, 26),
        ("para", ShapeKind::Parallelogram, -9, 11, 34, 17),
    ] {
        c.elements.push(Element::Shape(Shape {
            id: Id::from(id),
            ..Shape::new(kind, x, y, w, h, "")
        }));
    }
    Session::new(
        c,
        View {
            ox: -80,
            oy: -25,
            zoom: Zoom::One,
        },
        (160, 50),
    )
}

fn target_at(s: &mut Session, from: &str, at: (i32, i32)) -> Option<Id> {
    s.selection = vec![Id::from(from)];
    mouse(
        s,
        MouseEventKind::Moved,
        (at.0 + 80) as u16,
        (at.1 + 25) as u16,
        Instant::now(),
    );
    s.quick.clone().and_then(|q| q.target)
}

#[test]
fn quick_connect_reaches_every_pair_of_three_shapes_by_pointer_position() {
    let mut s = trio();
    for (from, at, to) in [
        ("rect", (-31, 0), "cyl"),
        ("rect", (-31, 12), "para"),
        ("rect", (-31, 9), "para"),
        ("cyl", (4, 0), "rect"),
        ("cyl", (20, 8), "para"),
        ("para", (10, 10), "cyl"),
        ("para", (-10, 15), "rect"),
    ] {
        assert_eq!(
            target_at(&mut s, from, at),
            Some(Id::from(to)),
            "{from} {at:?}"
        );
    }
}

fn user_canvas() -> Session {
    let mut c = Canvas::default();
    for (id, kind, x, y, w, h) in [
        ("ak8lf1jc", ShapeKind::Rect, 49, 18, 24, 14),
        ("q3e0zfog", ShapeKind::Rect, 93, 24, 25, 10),
        ("g98rj7ip", ShapeKind::Rect, 93, 11, 25, 10),
        ("rd1z4z49", ShapeKind::Rect, 50, 40, 44, 10),
        ("2of3quxh", ShapeKind::Rect, 36, 7, 21, 7),
        ("nzvbgput", ShapeKind::Parallelogram, 7, 25, 26, 13),
        ("miibxbva", ShapeKind::Rect, 21, 18, 24, 14),
        ("4latlq5k", ShapeKind::Rect, -7, 18, 24, 14),
        ("7bd3kj0i", ShapeKind::Rect, 93, -1, 25, 10),
        ("6t7j8y04", ShapeKind::Rect, 93, 12, 25, 10),
    ] {
        c.elements.push(Element::Shape(Shape {
            id: Id::from(id),
            ..Shape::new(kind, x, y, w, h, "")
        }));
    }
    Session::new(c, View::default(), (160, 50))
}

#[test]
fn quick_connect_offers_a_copy_when_no_shape_lies_in_the_pointer_row() {
    let t = Instant::now();
    let mut s = user_canvas();
    s.selection = vec![Id::from("rd1z4z49")];
    mouse(&mut s, MouseEventKind::Moved, 49, 45, t);
    let q = s.quick.clone().expect("copy on the left face");
    assert_eq!(
        (q.edge, q.target, q.rect),
        (Edge::Left, None, (0, 40, 44, 10))
    );
}

#[test]
fn quick_connect_reaches_a_shape_a_cell_off_the_pointer_column() {
    let t = Instant::now();
    let mut s = user_canvas();
    s.selection = vec![Id::from("ak8lf1jc")];
    mouse(&mut s, MouseEventKind::Moved, 49, 33, t);
    assert_eq!(s.quick.and_then(|q| q.target), Some(Id::from("rd1z4z49")));
}

#[test]
fn quick_connect_to_an_existing_shape_inserts_only_an_arrow_in_one_undo_step() {
    let mut s = trio();
    assert!(target_at(&mut s, "rect", (-31, 0)).is_some());
    click(&mut s, (49, 25), Instant::now());
    assert_eq!(shapes(&s).len(), 3);
    let l = lines(&s);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].kind, LineKind::Arrow);
    assert_eq!(l[0].from, End::Bound(Id::from("rect"), Anchor::Auto));
    assert_eq!(l[0].to, End::Bound(Id::from("cyl"), Anchor::Auto));
    assert_eq!(s.selection, vec![Id::from("cyl")]);
    key(&mut s, KeyCode::Char('u'));
    assert!(lines(&s).is_empty());
}

#[test]
fn quick_connect_skips_a_shape_already_joined_to_the_selection_by_an_arrow() {
    let mut s = trio();
    s.canvas.elements.push(Element::Line(Line::new(
        LineKind::Arrow,
        End::Bound(Id::from("cyl"), Anchor::Auto),
        End::Bound(Id::from("rect"), Anchor::Auto),
    )));
    assert_eq!(target_at(&mut s, "rect", (-31, 12)), Some(Id::from("para")));
    s.canvas.elements.push(Element::Line(Line::new(
        LineKind::Arrow,
        End::Bound(Id::from("rect"), Anchor::Auto),
        End::Bound(Id::from("para"), Anchor::Auto),
    )));
    assert_eq!(target_at(&mut s, "rect", (-31, 12)), None);
    assert!(s.quick.is_none(), "the copy spot overlaps the other shapes");
    assert_eq!(target_at(&mut s, "cyl", (42, 0)), None);
    assert!(s
        .quick
        .is_some_and(|q| q.edge == Edge::Right && q.target.is_none()));
}

#[test]
fn quick_connect_ignores_a_shift_click_after_an_unshifted_hover() {
    let t = Instant::now();
    let mut s = picked("aaaaaaaa", 20, 6);
    mouse(&mut s, MouseEventKind::Moved, 28, 7, t);
    assert!(s.quick.is_some());
    mouse_mod(
        &mut s,
        MouseEventKind::Down(MouseButton::Left),
        KeyModifiers::SHIFT,
        28,
        7,
        t,
    );
    assert_eq!(shapes(&s).len(), 1);
}

#[test]
fn quick_connect_places_the_copy_at_the_layout_gap_on_every_side() {
    let t = Instant::now();
    for (at, rect) in [
        ((22, 5), (20, 6 - 3 - 4, 8, 4)),
        ((22, 10), (20, 13, 8, 4)),
        ((19, 7), (20 - 6 - 8, 6, 8, 4)),
    ] {
        let mut s = picked("aaaaaaaa", 20, 6);
        mouse(&mut s, MouseEventKind::Moved, at.0, at.1, t);
        assert_eq!(s.quick.map(|q| q.rect), Some(rect), "{at:?}");
    }
}

#[test]
fn quick_connect_copies_the_look_but_not_the_text() {
    let t = Instant::now();
    let mut s = picked("aaaaaaaa", 20, 6);
    if let Some(Element::Shape(sh)) = s.canvas.elements.first_mut() {
        sh.kind = ShapeKind::Diamond;
        sh.label = "text".into();
        sh.bold = true;
        sh.link = Some("x".into());
    }
    mouse(&mut s, MouseEventKind::Moved, 28, 7, t);
    click(&mut s, (28, 7), t);
    let copy = shapes(&s)[1];
    assert_eq!(copy.kind, ShapeKind::Diamond);
    assert!(copy.bold && copy.label.is_empty() && copy.link.is_none());
}

#[test]
fn quick_connect_stays_off_unless_one_shape_is_selected_and_the_pointer_is_just_beside_a_side() {
    let t = Instant::now();
    let ghost = |s: &mut Session, at: (u16, u16)| {
        mouse(s, MouseEventKind::Moved, at.0, at.1, t);
        s.quick.is_some()
    };
    let mut s = picked("aaaaaaaa", 20, 6);
    assert!(ghost(&mut s, (28, 7)));
    assert!(ghost(&mut s, (29, 7)), "two cells out still counts");
    assert!(!ghost(&mut s, (30, 7)), "three cells out");
    assert!(!ghost(&mut s, (28, 5)), "corner zone");
    assert!(!ghost(&mut s, (23, 7)), "on the shape");
    s.quick_connect = false;
    assert!(!ghost(&mut s, (28, 7)), "config off");
    let mut s = picked("aaaaaaaa", 20, 6);
    s.selection.clear();
    assert!(!ghost(&mut s, (28, 7)), "no selection");
    let mut s = picked("aaaaaaaa", 20, 6);
    s.tool = Tool::Rect;
    assert!(!ghost(&mut s, (28, 7)), "rect tool");
    let mut s = picked("aaaaaaaa", 20, 6);
    s.canvas
        .elements
        .push(with_shape("bbbbbbbb", 34, 6).canvas.elements.remove(0));
    assert!(ghost(&mut s, (28, 7)), "an arrow to the shape in the way");
    assert_eq!(
        s.quick.take().and_then(|q| q.target),
        Some(Id::from("bbbbbbbb"))
    );
    s.selection.push(Id::from("bbbbbbbb"));
    assert!(!ghost(&mut s, (28, 7)), "two selected");
}

fn only_line(s: &Session) -> Line {
    let lines: Vec<_> = s
        .canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Line(l) => Some(l.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 1);
    lines[0].clone()
}

#[test]
fn clicking_bends_then_a_shape_makes_one_bound_arrow() {
    let mut s = two_shapes_and_arrow_tool();
    let t = Instant::now();
    click(&mut s, (30, 16), t);
    click(&mut s, (44, 16), t + Duration::from_secs(1));
    click(&mut s, (43, 12), t + Duration::from_secs(2));
    let l = only_line(&s);
    assert_eq!(l.from, End::Free(30, 16));
    assert_eq!(l.bends, vec![(44, 16)]);
    assert!(matches!(&l.to, End::Bound(id, _) if *id == Id::from("bbbbbbbb")));
    assert_eq!(s.tool, Tool::Pointer);
    s.undo();
    assert!(!s
        .canvas
        .elements
        .iter()
        .any(|e| matches!(e, Element::Line(_))));
}

#[test]
fn enter_ends_the_arrow_free_at_the_last_click() {
    let mut s = two_shapes_and_arrow_tool();
    let t = Instant::now();
    click(&mut s, (30, 16), t);
    click(&mut s, (60, 16), t + Duration::from_secs(1));
    mouse(
        &mut s,
        MouseEventKind::Moved,
        70,
        25,
        t + Duration::from_secs(2),
    );
    key(&mut s, KeyCode::Enter);
    let l = only_line(&s);
    assert_eq!(l.to, End::Free(60, 16));
    assert!(l.bends.is_empty());
    assert_eq!(s.tool, Tool::Pointer);
}

#[test]
fn double_click_ends_the_arrow_free_at_the_click() {
    let mut s = two_shapes_and_arrow_tool();
    let t = Instant::now();
    click(&mut s, (30, 16), t);
    click(&mut s, (60, 16), t + Duration::from_secs(1));
    let d = t + Duration::from_secs(2);
    click(&mut s, (70, 14), d);
    click(&mut s, (70, 14), d + Duration::from_millis(10));
    let l = only_line(&s);
    assert_eq!(l.bends, vec![(60, 16)]);
    assert_eq!(l.to, End::Free(70, 14));
}

#[test]
fn escape_cancels_a_click_arrow() {
    let mut s = two_shapes_and_arrow_tool();
    click(&mut s, (30, 16), Instant::now());
    key(&mut s, KeyCode::Esc);
    assert!(!s
        .canvas
        .elements
        .iter()
        .any(|e| matches!(e, Element::Line(_))));
    assert!(s.undo().is_none());
}

#[test]
fn backspace_keeps_the_arrow_being_traced() {
    let mut s = two_shapes_and_arrow_tool();
    click(&mut s, (30, 16), Instant::now());
    key(&mut s, KeyCode::Backspace);
    only_line(&s);
}

#[test]
fn dragging_with_the_arrow_tool_still_makes_a_single_arrow() {
    let mut s = two_shapes_and_arrow_tool();
    drag(&mut s, (30, 16), (60, 14));
    let l = only_line(&s);
    assert_eq!(l.to, End::Free(60, 14));
    assert_eq!(s.tool, Tool::Pointer);
}
