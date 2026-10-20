use clidraw::input::{handle_key, handle_mouse, Corner, Handle, Hit, KeyOutcome, Session, Tool};
use clidraw::model::{Canvas, Element, End, Id, Line, LineKind, Shape, ShapeKind};
use clidraw::ui::palette::Dock;
use clidraw::view::View;
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

/// Palette rows: title at 0, then one row per tool.
fn pick(s: &mut Session, tool: Tool) {
    let row = Tool::ALL.iter().position(|t| *t == tool).unwrap() as u16 + 1;
    click(s, (2, row), Instant::now());
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
    assert_eq!(s.tool, Tool::Select);
    ctrl(&mut s, 'z');
    assert!(s.canvas.elements.is_empty());
    ctrl(&mut s, 'y');
    assert_eq!(shapes(&s).len(), 1);
}

#[test]
fn dragging_a_shape_moves_it_and_undo_restores() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    drag(&mut s, (22, 6), (27, 9));
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (25, 8));
    ctrl(&mut s, 'z');
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (20, 5));
}

#[test]
fn a_long_shape_drag_undoes_and_redoes_in_one_step() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    let t = Instant::now();
    mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 22, 6, t);
    for i in 1..=200 {
        mouse(
            &mut s,
            MouseEventKind::Drag(MouseButton::Left),
            22 + i,
            6,
            t,
        );
    }
    mouse(&mut s, MouseEventKind::Up(MouseButton::Left), 222, 6, t);
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (220, 5));
    ctrl(&mut s, 'z');
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (20, 5));
    ctrl(&mut s, 'y');
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (220, 5));
}

#[test]
fn corner_handle_resizes() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    click(&mut s, (22, 6), Instant::now());
    drag(&mut s, (27, 8), (32, 12));
    let sh = shapes(&s)[0];
    assert_eq!((sh.x, sh.y, sh.w, sh.h), (20, 5, 13, 8));
}

#[test]
fn delete_key_removes_the_selection() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    click(&mut s, (22, 6), Instant::now());
    key(&mut s, KeyCode::Delete);
    assert!(s.canvas.elements.is_empty());
    ctrl(&mut s, 'z');
    assert_eq!(s.canvas.elements.len(), 1);
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
    }));
    pick(&mut s, Tool::Arrow);
    drag(&mut s, (18, 5), (43, 5));
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(l.from, End::Bound(Id::from("aaaaaaaa")));
    assert_eq!(l.to, End::Bound(Id::from("bbbbbbbb")));
    assert_eq!(s.tool, Tool::Select);
    assert_eq!(s.canvas.line_ends(&l), ((22, 5), (40, 5)));

    drag(&mut s, (42, 4), (42, 8));
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(s.canvas.line_ends(&l).1, (40, 9));

    click(&mut s, (46, 9), Instant::now());
    key(&mut s, KeyCode::Delete);
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(l.to, End::Free(40, 9));
    ctrl(&mut s, 'z');
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(l.to, End::Bound(Id::from("bbbbbbbb")));
}

#[test]
fn dragging_a_line_end_onto_empty_canvas_frees_it() {
    let mut s = with_shape("aaaaaaaa", 15, 3);
    pick(&mut s, Tool::Line);
    drag(&mut s, (18, 5), (50, 12));
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(l.to, End::Free(50, 12));
    assert_eq!(l.from, End::Bound(Id::from("aaaaaaaa")));
}

#[test]
fn wheel_pans_and_hand_tool_drag_pans() {
    let mut s = session();
    let t = Instant::now();
    mouse(&mut s, MouseEventKind::ScrollDown, 30, 10, t);
    mouse(&mut s, MouseEventKind::ScrollRight, 30, 10, t);
    assert_eq!((s.view.ox, s.view.oy), (4, 2));
    pick(&mut s, Tool::Hand);
    drag(&mut s, (30, 10), (25, 12));
    assert_eq!((s.view.ox, s.view.oy), (9, 0));
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
fn shift_wheel_pans_sideways_and_ctrl_wheel_does_nothing() {
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
    assert_eq!((s.view.ox, s.view.oy), (0, 0));
}

#[test]
fn rubber_band_selects_what_it_touches() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    drag(&mut s, (15, 2), (23, 7));
    assert_eq!(s.selection, vec![Id::from("aaaaaaaa")]);
    click(&mut s, (60, 20), Instant::now());
    assert!(s.selection.is_empty());
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
fn text_tool_click_places_text_and_esc_cancels_edit() {
    let mut s = session();
    pick(&mut s, Tool::Text);
    click(&mut s, (30, 10), Instant::now());
    key(&mut s, KeyCode::Char('x'));
    key(&mut s, KeyCode::Esc);
    assert!(s.canvas.elements.is_empty());
    assert_eq!(s.tool, Tool::Select);
}

#[test]
fn esc_cancels_a_drag_then_opens_the_menu() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    let t = Instant::now();
    mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 22, 6, t);
    mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 30, 12, t);
    assert_eq!(key(&mut s, KeyCode::Esc), KeyOutcome::None);
    assert_eq!(shapes(&s)[0].x, 20);
    assert_eq!(key(&mut s, KeyCode::Esc), KeyOutcome::Menu);
}

#[test]
fn palette_entry_changes_the_kind_of_a_selected_shape() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    click(&mut s, (22, 6), Instant::now());
    pick(&mut s, Tool::Diamond);
    assert_eq!(shapes(&s)[0].kind, ShapeKind::Diamond);
    assert_eq!(s.tool, Tool::Select);
}

#[test]
fn palette_title_drag_snaps_to_the_nearest_edge() {
    let mut s = session();
    drag(&mut s, (2, 0), (75, 10));
    assert_eq!(s.dock, Dock::Right);
    drag(&mut s, (75, 0), (40, 22));
    assert_eq!(s.dock, Dock::Bottom);
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
    });
    s.canvas.push(line).unwrap();
    let kind = |s: &Session| match s.canvas.get(&Id::from("llllllll")) {
        Some(Element::Line(l)) => l.kind,
        _ => panic!("no line"),
    };
    // Options sit two rows under the last tool: none, end, start, both.
    let option =
        |s: &mut Session, i: u16| click(s, (2, Tool::ALL.len() as u16 + 2 + i), Instant::now());
    option(&mut s, 2);
    assert_eq!(kind(&s), LineKind::Arrow, "no options without a selection");
    s.selection = vec![Id::from("llllllll")];
    option(&mut s, 2);
    assert_eq!(kind(&s), LineKind::ArrowStart);
    option(&mut s, 3);
    assert_eq!(kind(&s), LineKind::ArrowBoth);
    option(&mut s, 0);
    assert_eq!(kind(&s), LineKind::Line);
    assert_eq!(s.tool, Tool::Select);
    s.undo();
    assert_eq!(kind(&s), LineKind::ArrowBoth);
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
    mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 30, 20, t);
    assert_eq!(s.snap, None);
    mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 43, 10, t);
    mouse(&mut s, MouseEventKind::Up(MouseButton::Left), 43, 10, t);
    assert_eq!(s.snap, None);
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
    click(&mut s, (22, 6), t);
    click(&mut s, (22, 6), t + Duration::from_millis(100));
    assert!(s.edit.is_none());
}

#[test]
fn colon_opens_the_command_line_but_types_into_a_label_edit() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    assert_eq!(key(&mut s, KeyCode::Char(':')), KeyOutcome::Command);
    let t = Instant::now();
    click(&mut s, (22, 6), t);
    click(&mut s, (22, 6), t + Duration::from_millis(100));
    assert_eq!(key(&mut s, KeyCode::Char(':')), KeyOutcome::None);
    assert_eq!(s.edit.as_ref().unwrap().buf, ":");
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
    hold(&mut s, (22, 6), (79, 6));
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
    ctrl(&mut s, 'z');
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
        hold(&mut s, (22, 6), (col, row));
        assert_eq!(s.edge_pan(t0), pan != (0, 0), "{col},{row}");
        assert_eq!((s.view.ox, s.view.oy), pan, "{col},{row}");
    }
}

#[test]
fn holding_a_band_at_the_edge_extends_it() {
    let mut s = session();
    hold(&mut s, (30, 5), (79, 10));
    let before = s.band.unwrap();
    assert!(s.edge_pan(Instant::now()));
    assert_eq!(s.band.unwrap(), (before.0, (before.1 .0 + 3, before.1 .1)));
}

#[test]
fn hand_tool_and_idle_never_edge_pan() {
    let mut s = session();
    assert_eq!(s.edge_wait(Instant::now()), None);
    pick(&mut s, Tool::Hand);
    hold(&mut s, (40, 10), (79, 10));
    assert_eq!(s.edge_wait(Instant::now()), None);
}

#[test]
fn hover_tracks_shape_handle_and_empty_space() {
    let mut s = with_shape("aaaaaaaa", 20, 4);
    let t = Instant::now();
    mouse(&mut s, MouseEventKind::Moved, 22, 5, t);
    assert_eq!(s.hover, Hit::Element(Id::from("aaaaaaaa")));
    mouse(&mut s, MouseEventKind::Moved, 60, 15, t);
    assert_eq!(s.hover, Hit::Empty);
    s.selection = vec![Id::from("aaaaaaaa")];
    mouse(&mut s, MouseEventKind::Moved, 20, 4, t);
    assert_eq!(
        s.hover,
        Hit::Handle(Handle::Corner(Corner::Nw), Id::from("aaaaaaaa"))
    );
}

#[test]
fn hover_is_empty_for_other_tools_mid_gesture_and_over_the_palette() {
    let t = Instant::now();
    let mut s = with_shape("aaaaaaaa", 20, 4);
    s.tool = Tool::Hand;
    mouse(&mut s, MouseEventKind::Moved, 22, 5, t);
    assert_eq!(s.hover, Hit::Empty);
    let mut s = with_shape("aaaaaaaa", 20, 4);
    mouse(&mut s, MouseEventKind::Moved, 22, 5, t);
    mouse(&mut s, MouseEventKind::Down(MouseButton::Left), 22, 5, t);
    assert_eq!(s.hover, Hit::Empty, "a press clears hover");
    mouse(&mut s, MouseEventKind::Moved, 22, 5, t);
    assert_eq!(s.hover, Hit::Empty, "no hover while pressed");
    let mut s = with_shape("aaaaaaaa", 2, 1);
    mouse(&mut s, MouseEventKind::Moved, 2, 1, t);
    assert_eq!(s.hover, Hit::Empty, "palette owns its cells");
}
