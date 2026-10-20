use clidraw::input::{
    handle_key, handle_mouse, handle_paste, Corner, Edit, EditTarget, Handle, Hit, KeyOutcome,
    Session, Tool,
};
use clidraw::model::{
    Anchor, Border, Canvas, Element, End, Id, Line, LineKind, Paint, Routing, Shape, ShapeKind,
    Stroke, TextAlign,
};
use clidraw::ui::palette::{self, Dock, PaletteHit};
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
        stroke: Default::default(),
        border: Default::default(),
        color: None,
        fill: None,
        align: Default::default(),
        bold: false,
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
    }));
    ctrl(&mut s, 'a');
    assert_eq!(
        s.selection,
        vec![Id::from("aaaaaaaa"), Id::from("bbbbbbbb")]
    );
}

#[test]
fn arrow_keys_nudge_the_selection_one_undo_step_per_press() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    click(&mut s, (22, 6), Instant::now());
    key(&mut s, KeyCode::Right);
    key(&mut s, KeyCode::Down);
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (21, 6));
    key(&mut s, KeyCode::Left);
    key(&mut s, KeyCode::Up);
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (20, 5));
    ctrl(&mut s, 'z');
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (20, 6));
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
    }));
    pick(&mut s, Tool::Arrow);
    drag(&mut s, (18, 5), (43, 5));
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(l.from, End::Bound(Id::from("aaaaaaaa"), Anchor::Auto));
    assert_eq!(l.to, End::Bound(Id::from("bbbbbbbb"), Anchor::Auto));
    assert_eq!(s.tool, Tool::Select);
    assert_eq!(s.canvas.line_ends(&l), ((22, 5), (40, 5)));

    drag(&mut s, (42, 4), (42, 9));
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(s.canvas.line_ends(&l).1, (40, 10));

    click(&mut s, (46, 9), Instant::now());
    key(&mut s, KeyCode::Delete);
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(l.to, End::Free(40, 10));
    ctrl(&mut s, 'z');
    let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
        panic!("no line")
    };
    assert_eq!(l.to, End::Bound(Id::from("bbbbbbbb"), Anchor::Auto));
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
        }));
        pick(&mut s, Tool::Arrow);
        drag(&mut s, (18, 5), to);
        let Element::Line(l) = s.canvas.elements.last().unwrap().clone() else {
            panic!("no line")
        };
        l.to
    };
    let b = Id::from("bbbbbbbb");
    assert_eq!(end_on((43, 3)), End::Bound(b.clone(), Anchor::Top));
    assert_eq!(end_on((40, 5)), End::Bound(b.clone(), Anchor::Left));
    assert_eq!(end_on((40, 3)), End::Bound(b.clone(), Anchor::Auto));
    assert_eq!(end_on((43, 5)), End::Bound(b, Anchor::Auto));
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
    assert_eq!(l.from, End::Bound(Id::from("aaaaaaaa"), Anchor::Auto));
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
    }));
    click(&mut s, (22, 6), Instant::now());
    shift_drag(&mut s, (42, 10), (42, 10));
    assert_eq!(
        s.selection,
        vec![Id::from("aaaaaaaa"), Id::from("bbbbbbbb")]
    );
    shift_drag(&mut s, (22, 6), (22, 6));
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
    shift_drag(&mut s, (22, 6), (30, 8));
    assert_eq!((shapes(&s)[0].x, shapes(&s)[0].y), (28, 5));
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
fn palette_entry_changes_a_selected_shape_to_hexagon() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    click(&mut s, (22, 6), Instant::now());
    pick(&mut s, Tool::Hexagon);
    assert_eq!(shapes(&s)[0].kind, ShapeKind::Hexagon);
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
        label_at: None,
        stroke: Default::default(),
        routing: Default::default(),
        color: None,
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
            routing: Default::default(),
            color: None,
        }))
        .unwrap();
    s.selection = vec![Id::from("llllllll")];
    let stroke = |s: &Session| match s.canvas.get(&Id::from("llllllll")) {
        Some(Element::Line(l)) => l.stroke,
        _ => panic!("no line"),
    };
    // Four head options come first, then solid, dashed, dotted.
    let row = Tool::ALL.len() as u16 + 2 + 4;
    click(&mut s, (2, row + 1), Instant::now());
    assert_eq!(stroke(&s), Stroke::Dashed);
    click(&mut s, (2, row + 2), Instant::now());
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
    }));
    s.selection = vec![Id::from("rrrrrrrr")];
    let border = |s: &Session| match s.canvas.get(&Id::from("rrrrrrrr")) {
        Some(Element::Shape(sh)) => sh.border,
        _ => panic!("no shape"),
    };
    // Three stroke options come first, then single, rounded, double, heavy.
    let row = Tool::ALL.len() as u16 + 2 + 3;
    click(&mut s, (2, row + 2), Instant::now());
    assert_eq!(border(&s), Border::Double);
    click(&mut s, (2, row + 3), Instant::now());
    assert_eq!(border(&s), Border::Heavy);
    s.undo();
    assert_eq!(border(&s), Border::Double);
}

#[test]
fn palette_ink_cycles_through_the_colors_and_undoes_one_click_at_a_time() {
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
    }));
    s.canvas.elements.push(Element::Text(clidraw::model::Text {
        id: Id::from("tttttttt"),
        x: 50,
        y: 5,
        text: "t".into(),
        color: None,
    }));
    s.selection = vec![Id::from("rrrrrrrr")];
    let ink = |s: &Session| match s.canvas.get(&Id::from("rrrrrrrr")) {
        Some(Element::Shape(sh)) => (sh.color, sh.fill),
        _ => panic!("no shape"),
    };
    // Stroke (3), border (4) and align come first, then ink and fill.
    let row = Tool::ALL.len() as u16 + 2 + 8;
    click(&mut s, (2, row), Instant::now());
    assert_eq!(ink(&s), (Some(Paint::Red), None));
    click(&mut s, (2, row), Instant::now());
    assert_eq!(ink(&s), (Some(Paint::Orange), None));
    s.undo();
    assert_eq!(ink(&s), (Some(Paint::Red), None));
    for _ in 0..6 {
        click(&mut s, (2, row), Instant::now());
    }
    assert_eq!(ink(&s), (None, None));
    click(&mut s, (2, row + 1), Instant::now());
    assert_eq!(ink(&s), (None, Some(Paint::Red)));
    s.undo();
    assert_eq!(ink(&s), (None, None));

    s.selection = vec![Id::from("tttttttt")];
    // A text offers ink only: no stroke, border or fill rows.
    click(&mut s, (2, Tool::ALL.len() as u16 + 2), Instant::now());
    assert!(matches!(
        s.canvas.get(&Id::from("tttttttt")),
        Some(Element::Text(t)) if t.color == Some(Paint::Red)
    ));
}

#[test]
fn palette_option_sets_the_routing_of_a_selected_line_and_undoes() {
    let mut s = session();
    s.canvas
        .push(Element::Line(Line {
            id: Id::from("llllllll"),
            kind: LineKind::Arrow,
            from: End::Free(5, 5),
            to: End::Free(25, 9),
            label: String::new(),
            label_at: None,
            stroke: Default::default(),
            routing: Default::default(),
            color: None,
        }))
        .unwrap();
    s.selection = vec![Id::from("llllllll")];
    let routing = |s: &Session| match s.canvas.get(&Id::from("llllllll")) {
        Some(Element::Line(l)) => l.routing,
        _ => panic!("no line"),
    };
    // Four head options and three strokes come first, then elbow, direct.
    let row = Tool::ALL.len() as u16 + 2 + 4 + 3;
    click(&mut s, (2, row + 1), Instant::now());
    assert_eq!(routing(&s), Routing::Straight);
    s.undo();
    assert_eq!(routing(&s), Routing::Elbow);
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
    assert_eq!(s.snap_at, Some((40, 11)));
    mouse(&mut s, MouseEventKind::Drag(MouseButton::Left), 43, 9, t);
    assert_eq!(s.snap_at, Some((44, 9)));
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
    s.tool = Tool::Hand;
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
    for (c, tool) in "vhrodypxflat".chars().zip(Tool::ALL) {
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
    ctrl(&mut s, 'z');
    assert_eq!(ids(&s), ["aaaaaaaa", "bbbbbbbb"]);
}

#[test]
fn bracket_keys_type_into_a_label_edit() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    click(&mut s, (22, 6), Instant::now());
    click(&mut s, (22, 6), Instant::now() + Duration::from_millis(100));
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
    }));
    s.canvas.elements.push(Element::Line(Line {
        id: Id::from("line0000"),
        kind: LineKind::Arrow,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        routing: Default::default(),
        color: None,
    }));
    s
}

#[test]
fn copy_paste_adds_offset_copies_in_one_undo_step() {
    let mut s = diagram();
    ctrl(&mut s, 'a');
    key(&mut s, KeyCode::Char('c'));
    ctrl(&mut s, 'v');
    assert_eq!(s.canvas.elements.len(), 6);
    assert_eq!(s.selection.len(), 3);
    assert_eq!(shapes(&s)[2].x, 7);
    let Element::Line(l) = &s.canvas.elements[5] else {
        panic!()
    };
    assert_eq!(l.from, End::Bound(shapes(&s)[2].id.clone(), Anchor::Auto));
    ctrl(&mut s, 'v');
    assert_eq!(shapes(&s)[4].x, 9);
    ctrl(&mut s, 'z');
    assert_eq!(s.canvas.elements.len(), 6);
    ctrl(&mut s, 'z');
    assert_eq!(s.canvas.elements.len(), 3);
}

#[test]
fn cut_removes_and_paste_restores() {
    let mut s = diagram();
    ctrl(&mut s, 'a');
    ctrl(&mut s, 'x');
    assert!(s.canvas.elements.is_empty());
    ctrl(&mut s, 'v');
    assert_eq!(s.canvas.elements.len(), 3);
    ctrl(&mut s, 'z');
    ctrl(&mut s, 'z');
    assert_eq!(s.canvas.elements.len(), 3);
}

#[test]
fn duplicate_leaves_the_buffer_alone() {
    let mut s = with_shape("aaaaaaaa", 5, 5);
    ctrl(&mut s, 'a');
    ctrl(&mut s, 'd');
    assert_eq!(shapes(&s).len(), 2);
    assert_eq!(shapes(&s)[1].x, 7);
    assert!(s.clipboard.elements.is_empty());
}

#[test]
fn clipboard_keys_do_nothing_without_input_or_while_editing() {
    let mut s = with_shape("aaaaaaaa", 5, 5);
    key(&mut s, KeyCode::Char('c'));
    ctrl(&mut s, 'v');
    ctrl(&mut s, 'd');
    assert_eq!(s.canvas.elements.len(), 1);
    ctrl(&mut s, 'a');
    key(&mut s, KeyCode::Char('c'));
    s.start_edit_new((0, 0));
    ctrl(&mut s, 'v');
    ctrl(&mut s, 'd');
    assert_eq!(s.canvas.elements.len(), 1);
}

#[test]
fn d_still_picks_the_diamond_tool() {
    let mut s = session();
    key(&mut s, KeyCode::Char('d'));
    assert_eq!(s.tool, Tool::Diamond);
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
fn overview_blocks_edits_and_returns_to_the_same_view() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    click(&mut s, (22, 6), Instant::now());
    let before = s.view;
    key(&mut s, KeyCode::Char('z'));
    assert!(s.overview);
    key(&mut s, KeyCode::Delete);
    assert_eq!(s.canvas.elements.len(), 1);
    key(&mut s, KeyCode::Char('z'));
    assert!(!s.overview);
    assert_eq!(s.view, before);
    key(&mut s, KeyCode::Char('z'));
    key(&mut s, KeyCode::Esc);
    assert!(!s.overview);
}

#[test]
fn z_types_into_a_label_edit_instead_of_entering_overview() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    let t = Instant::now();
    click(&mut s, (22, 6), t);
    click(&mut s, (22, 6), t + Duration::from_millis(100));
    key(&mut s, KeyCode::Char('z'));
    assert!(!s.overview);
    assert_eq!(s.edit.as_ref().unwrap().buf, "z");
}

#[test]
fn mouse_does_nothing_in_overview() {
    let mut s = with_shape("aaaaaaaa", 20, 5);
    key(&mut s, KeyCode::Char('z'));
    click(&mut s, (22, 6), Instant::now());
    assert!(s.selection.is_empty());
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
            routing: Default::default(),
            color: None,
        }))
        .unwrap();
    s.selection = vec![id.clone()];
    let at = |s: &Session| match s.canvas.get(&Id::from("llllllll")) {
        Some(Element::Line(l)) => l.label_at,
        _ => panic!("no line"),
    };
    drag(&mut s, (14, 5), (20, 5));
    assert_eq!(at(&s), Some(750));
    ctrl(&mut s, 'z');
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
        }));
    }
    s
}

fn grouped_ab() -> Session {
    let mut s = three_shapes();
    click(&mut s, (22, 6), Instant::now());
    shift_drag(&mut s, (42, 6), (42, 6));
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
    click(&mut s, (22, 6), Instant::now());
    assert_eq!(s.selection, ab());
    shift_drag(&mut s, (62, 6), (62, 6));
    assert_eq!(s.selection.len(), 3);
    shift_drag(&mut s, (42, 6), (42, 6));
    assert_eq!(s.selection, vec![Id::from("cccccccc")]);
}

#[test]
fn a_band_over_one_member_selects_the_group() {
    let mut s = grouped_ab();
    drag(&mut s, (36, 3), (50, 12));
    assert_eq!(
        s.selection,
        vec![Id::from("bbbbbbbb"), Id::from("aaaaaaaa")]
    );
}

#[test]
fn a_group_moves_together() {
    let mut s = grouped_ab();
    click(&mut s, (22, 6), Instant::now());
    key(&mut s, KeyCode::Right);
    assert_eq!(
        shapes(&s).iter().map(|s| s.x).collect::<Vec<_>>(),
        vec![21, 41, 60]
    );
}

#[test]
fn capital_g_ungroups_and_one_undo_regroups() {
    let mut s = grouped_ab();
    click(&mut s, (22, 6), Instant::now());
    key(&mut s, KeyCode::Char('G'));
    click(&mut s, (70, 20), Instant::now());
    click(&mut s, (22, 6), Instant::now());
    assert_eq!(s.selection, vec![Id::from("aaaaaaaa")]);
    ctrl(&mut s, 'z');
    click(&mut s, (22, 6), Instant::now() + Duration::from_secs(1));
    assert_eq!(s.selection, ab());
}

#[test]
fn copy_paste_makes_a_second_group_and_one_undo_removes_it() {
    let mut s = grouped_ab();
    click(&mut s, (22, 6), Instant::now());
    key(&mut s, KeyCode::Char('c'));
    ctrl(&mut s, 'v');
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
    ctrl(&mut s, 'z');
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
        routing: Default::default(),
        color: None,
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
    ctrl(&mut s, 'z');
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
    }));
    s.selection = vec![Id::from("rrrrrrrr")];
    let get = |s: &Session| match s.canvas.get(&Id::from("rrrrrrrr")) {
        Some(Element::Shape(sh)) => (sh.align, sh.bold),
        _ => panic!("no shape"),
    };
    // Three strokes and four borders come first, then the align cell.
    let row = Tool::ALL.len() as u16 + 2 + 7;
    click(&mut s, (2, row), Instant::now());
    assert_eq!(get(&s), (TextAlign::Right, false));
    click(&mut s, (2, row), Instant::now());
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
    assert_eq!(s.right_down(22, 6), Some(true));
    assert_eq!(s.selection, vec![Id::from("aaaaaaaa")]);
    s.selection = vec![Id::from("zzzzzzzz"), Id::from("aaaaaaaa")];
    assert_eq!(s.right_down(22, 6), Some(true));
    assert_eq!(s.selection.len(), 2);
    s.overview = true;
    assert_eq!(s.right_down(22, 6), None);
}

#[test]
fn ctrl_p_opens_the_palette_only_when_idle() {
    let ctrl_p = || KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
    let mut s = session();
    assert_eq!(handle_key(&mut s, ctrl_p()), KeyOutcome::Actions);
    s.overview = true;
    assert_eq!(handle_key(&mut s, ctrl_p()), KeyOutcome::None);
    s.overview = false;
    let mut s = with_shape("aaaaaaaa", 20, 5);
    s.start_edit_existing(Id::from("aaaaaaaa"));
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
    ctrl(&mut s, 'z');
    assert_eq!(xs(&s), vec![22, 20]);
}

#[test]
fn frame_interior_click_selects_the_content_under_it_or_starts_a_band() {
    let mut s = framed();
    click(&mut s, (24, 8), Instant::now());
    assert_eq!(s.selection, vec![Id::from("inner000")]);
    click(&mut s, (35, 12), Instant::now());
    assert!(s.selection.is_empty());
}

#[test]
fn delete_removes_frame_and_contents_and_one_undo_restores_both() {
    let mut s = framed();
    click(&mut s, (30, 5), Instant::now());
    key(&mut s, KeyCode::Delete);
    assert!(s.canvas.elements.is_empty());
    ctrl(&mut s, 'z');
    assert_eq!(s.canvas.elements.len(), 2);
}

#[test]
fn band_inside_a_frame_skips_it_and_band_around_it_takes_it() {
    let mut s = framed();
    drag(&mut s, (23, 6), (30, 12));
    assert_eq!(s.selection, vec![Id::from("inner000")]);
    drag(&mut s, (18, 3), (45, 17));
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
