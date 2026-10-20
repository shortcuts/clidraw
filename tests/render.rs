use clidraw::input::{Corner, Handle, Hit, Session};
use clidraw::model::{
    Anchor, Canvas, Command, Element, End, Id, Line, LineKind, Routing, Shape, ShapeKind, Stroke,
};
use clidraw::ui::prompt::{Prompt, PromptKind};
use clidraw::ui::switcher::Switcher;
use clidraw::ui::{self, Overlay, Perf};
use clidraw::view::View;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Modifier;
use ratatui::Terminal;

fn shape(id: &str, x: i32, label: &str) -> Element {
    Element::Shape(Shape {
        id: Id::from(id),
        kind: ShapeKind::Rect,
        x,
        y: 4,
        w: 8,
        h: 3,
        label: label.into(),
        stroke: Default::default(),
    })
}

fn draw(s: &Session, overlay: &Overlay) -> Buffer {
    let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
    t.draw(|f| {
        let area = f.area();
        ui::render(f.buffer_mut(), area, s, overlay, None, None)
    })
    .unwrap();
    t.backend().buffer().clone()
}

fn row(b: &Buffer, y: u16) -> String {
    (0..80).map(|x| b[(x, y)].symbol().to_string()).collect()
}

fn cols(b: &Buffer, y: u16, x: u16, n: u16) -> String {
    (x..x + n).map(|x| b[(x, y)].symbol().to_string()).collect()
}

#[test]
fn draws_palette_and_a_labelled_rect() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, "box"));
    let s = Session::new(c, View::default(), (80, 24));
    let b = draw(&s, &Overlay::None);
    assert!(row(&b, 1).starts_with(" Select"));
    assert!(
        b[(1, 1)].modifier.contains(Modifier::REVERSED),
        "active tool is reversed"
    );
    assert_eq!(cols(&b, 4, 20, 8), "┌──────┐");
    assert_eq!(cols(&b, 5, 20, 8), "│ box  │");
    assert_eq!(cols(&b, 6, 20, 8), "└──────┘");
}

#[test]
fn dashed_and_dotted_strokes_swap_the_border_glyphs() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, ""));
    c.elements.push(shape("bbbbbbbb", 40, ""));
    for (e, st) in c.elements.iter_mut().zip([Stroke::Dashed, Stroke::Dotted]) {
        if let Element::Shape(s) = e {
            s.stroke = st;
        }
    }
    let s = Session::new(c, View::default(), (80, 24));
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 4, 20, 8), "┌╌╌╌╌╌╌┐");
    assert_eq!(cols(&b, 5, 20, 8), "╎      ╎");
    assert_eq!(cols(&b, 4, 40, 8), "┌┄┄┄┄┄┄┐");
    assert_eq!(cols(&b, 5, 40, 8), "┆      ┆");
}

#[test]
fn wide_char_that_overflows_the_border_is_dropped() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, "aaaaa你"));
    c.elements.push(shape("bbbbbbbb", 40, "abc你你"));
    let s = Session::new(c, View::default(), (80, 24));
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 5, 20, 8), "│aaaaa │");
    assert_eq!(cols(&b, 5, 40, 8), "│abc你  │");
}

#[test]
fn panned_view_shifts_the_drawing_and_handles_negative_coordinates() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", -10, ""));
    let s = Session::new(c, View { ox: -30, oy: 0 }, (80, 24));
    assert_eq!(cols(&draw(&s, &Overlay::None), 4, 20, 2), "┌─");
}

#[test]
fn arrow_between_shapes_ends_in_a_head() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 12, ""));
    c.elements.push(shape("bbbbbbbb", 40, ""));
    c.elements.push(Element::Line(Line {
        id: Id::from("cccccccc"),
        kind: LineKind::Arrow,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        routing: Default::default(),
    }));
    let s = Session::new(c, View::default(), (80, 24));
    let expected = format!("│{}▶│", "─".repeat(19));
    let line = row(&draw(&s, &Overlay::None), 5);
    assert_eq!(line.chars().skip(19).take(22).collect::<String>(), expected);
}

#[test]
fn menu_lists_its_entries() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let b = draw(&s, &Overlay::Menu);
    let all: String = (0..24).map(|y| row(&b, y)).collect();
    for e in [
        "Canvases",
        "Fit all",
        "Export SVG",
        "Export Mermaid",
        "Quit",
    ] {
        assert!(all.contains(e), "{e}");
    }
    for k in ["c", "f", "s", "m", "q"] {
        assert!(all.contains(&format!("{k} │")), "{k}");
    }
}

#[test]
fn arrow_start_has_its_head_at_the_first_cell() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 12, ""));
    c.elements.push(shape("bbbbbbbb", 40, ""));
    c.elements.push(Element::Line(Line {
        id: Id::from("cccccccc"),
        kind: LineKind::ArrowStart,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        routing: Default::default(),
    }));
    let s = Session::new(c, View::default(), (80, 24));
    let expected = format!("│◀{}│", "─".repeat(19));
    let line = row(&draw(&s, &Overlay::None), 5);
    assert_eq!(line.chars().skip(19).take(22).collect::<String>(), expected);
}

#[test]
fn arrow_both_has_a_head_at_each_end() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 12, ""));
    c.elements.push(shape("bbbbbbbb", 40, ""));
    c.elements.push(Element::Line(Line {
        id: Id::from("cccccccc"),
        kind: LineKind::ArrowBoth,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        routing: Default::default(),
    }));
    let s = Session::new(c, View::default(), (80, 24));
    let expected = format!("│◀{}▶│", "─".repeat(18));
    let line = row(&draw(&s, &Overlay::None), 5);
    assert_eq!(line.chars().skip(19).take(22).collect::<String>(), expected);
}

#[test]
fn arrow_between_shapes_on_different_rows_bends_with_rounded_corners() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 12, ""));
    let Element::Shape(mut b) = shape("bbbbbbbb", 40, "") else {
        unreachable!()
    };
    b.y = 10;
    c.elements.push(Element::Shape(b));
    c.elements.push(Element::Line(Line {
        id: Id::from("cccccccc"),
        kind: LineKind::Arrow,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        routing: Default::default(),
    }));
    let b = draw(&Session::new(c, View::default(), (80, 24)), &Overlay::None);
    assert_eq!(cols(&b, 5, 19, 12), "│─────────╮ ");
    assert_eq!(cols(&b, 8, 29, 1), "│");
    assert_eq!(cols(&b, 11, 28, 14), " ╰─────────▶│ ");
}

#[test]
fn straight_arrow_between_shapes_on_different_rows_draws_diagonal_cells() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 12, ""));
    let Element::Shape(mut b) = shape("bbbbbbbb", 40, "") else {
        unreachable!()
    };
    b.y = 10;
    c.elements.push(Element::Shape(b));
    c.elements.push(Element::Line(Line {
        id: Id::from("cccccccc"),
        kind: LineKind::Arrow,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        routing: Routing::Straight,
    }));
    let b = draw(&Session::new(c, View::default(), (80, 24)), &Overlay::None);
    let screen: String = (0..24).map(|y| row(&b, y)).collect();
    assert!(screen.contains('╲'), "{screen}");
    assert!(!screen.contains('╭'));
}

#[test]
fn help_pages_list_their_scope() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    for (page, wants) in [
        (
            0,
            vec!["Shortcuts", "Canvas", "Delete, Backspace", "Ctrl-z, Ctrl-y"],
        ),
        (1, vec!["Fit all", "Export Mermaid"]),
        (2, vec!["Commit the label"]),
        (3, vec!["Tools", "Diamond", "Hexagon"]),
    ] {
        let b = draw(&s, &Overlay::Help(page));
        let all: String = (0..24).map(|y| row(&b, y)).collect();
        for w in wants {
            assert!(all.contains(w), "page {page}: {w}");
        }
    }
}

#[test]
fn overlay_dims_the_canvas_behind_it() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, ""));
    let s = Session::new(c, View::default(), (80, 24));
    assert!(!draw(&s, &Overlay::None)[(20, 4)]
        .modifier
        .contains(Modifier::DIM));
    assert!(draw(&s, &Overlay::Menu)[(20, 4)]
        .modifier
        .contains(Modifier::DIM));
}

#[test]
fn menu_hotkeys_are_dim_and_labels_are_not() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let b = draw(&s, &Overlay::Menu);
    assert!(b[(48, 9)].modifier.contains(Modifier::DIM), "hotkey c");
    assert!(!b[(31, 9)].modifier.contains(Modifier::DIM), "label C");
}

#[test]
fn switcher_action_tags_are_dim_and_new_row_stays_reversed() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let sw = Switcher {
        names: vec!["main".into()],
        current: "main".into(),
    };
    let b = draw(&s, &Overlay::Switcher(sw));
    assert!(b[(54, 12)].modifier.contains(Modifier::DIM), "tag [");
    assert!(b[(20, 11)].modifier.contains(Modifier::REVERSED), "+ New");
}

#[test]
fn prompt_shows_a_dim_hint_for_keyboard_dismissal() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let p = Prompt::new(PromptKind::NewCanvas, String::new());
    let b = draw(&s, &Overlay::Prompt(p));
    assert_eq!(cols(&b, 11, 16, 27), "Enter confirms, Esc cancels");
    assert!(b[(16, 11)].modifier.contains(Modifier::DIM));
}

#[test]
fn help_canvas_row_lists_every_command_line_verb() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let b = draw(&s, &Overlay::Help(0));
    assert!(
        row(&b, 20).contains("Commands q w wq e u redo"),
        "{}",
        row(&b, 20)
    );
    assert_eq!(cols(&b, 20, 62, 1), "│");
}

#[test]
fn help_canvas_esc_row_stays_inside_the_border() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let b = draw(&s, &Overlay::Help(0));
    assert!(row(&b, 4).contains("│ Esc"), "{}", row(&b, 4));
    assert_eq!(cols(&b, 4, 62, 1), "│", "{}", row(&b, 4));
}

#[test]
fn help_draws_on_a_tiny_screen() {
    let mut t = Terminal::new(TestBackend::new(80, 8)).unwrap();
    let s = Session::new(Canvas::default(), View::default(), (80, 8));
    t.draw(|f| {
        let area = f.area();
        ui::render(f.buffer_mut(), area, &s, &Overlay::Help(0), None, None)
    })
    .unwrap();
}

#[test]
fn selected_shape_is_reversed_and_others_are_not() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, "box"));
    c.elements.push(shape("bbbbbbbb", 40, "box"));
    let mut s = Session::new(c, View::default(), (80, 24));
    s.selection.push(Id::from("aaaaaaaa"));
    let b = draw(&s, &Overlay::None);
    assert!(b[(22, 4)].modifier.contains(Modifier::REVERSED), "selected");
    assert!(
        !b[(42, 4)].modifier.contains(Modifier::REVERSED),
        "unselected"
    );
}

#[test]
fn cached_layout_follows_an_edit() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, ""));
    let mut s = Session::new(c, View::default(), (80, 24));
    assert_eq!(cols(&draw(&s, &Overlay::None), 4, 20, 2), "┌─");
    s.exec(Command::Move {
        id: Id::from("aaaaaaaa"),
        dx: 10,
        dy: 0,
    });
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 4, 20, 2), "  ");
    assert_eq!(cols(&b, 4, 30, 2), "┌─");
}

#[test]
fn cached_layout_follows_a_pan() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, ""));
    let mut s = Session::new(c, View::default(), (80, 24));
    assert_eq!(cols(&draw(&s, &Overlay::None), 4, 20, 2), "┌─");
    s.view = View { ox: -10, oy: 0 };
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 4, 20, 2), "  ");
    assert_eq!(cols(&b, 4, 30, 2), "┌─");
}

#[test]
fn cached_layout_follows_a_push_without_a_rev_bump() {
    let mut s = Session::new(Canvas::default(), View::default(), (80, 24));
    draw(&s, &Overlay::None);
    s.canvas.elements.push(shape("aaaaaaaa", 20, ""));
    assert_eq!(cols(&draw(&s, &Overlay::None), 4, 20, 2), "┌─");
}

#[test]
fn line_crossing_the_view_draws_when_both_ends_are_off_screen() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", -100, ""));
    c.elements.push(shape("bbbbbbbb", 200, ""));
    c.elements.push(Element::Line(Line {
        id: Id::from("cccccccc"),
        kind: LineKind::Arrow,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        routing: Default::default(),
    }));
    let s = Session::new(c, View::default(), (80, 24));
    assert_eq!(cols(&draw(&s, &Overlay::None), 5, 30, 4), "────");
}

#[test]
fn perf_row_shows_the_last_120_frames_at_the_top_right() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let mut perf = Perf::default();
    for ms in 1..=150 {
        perf.events = 3;
        perf.record_frame(std::time::Duration::from_millis(ms), 1234);
    }
    let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
    t.draw(|f| {
        let area = f.area();
        ui::render(f.buffer_mut(), area, &s, &Overlay::None, None, Some(&perf))
    })
    .unwrap();
    let b = t.backend().buffer().clone();
    assert!(
        row(&b, 0).ends_with(" frame 91.0/150.0ms  ev 3  1234B  0 el "),
        "{}",
        row(&b, 0)
    );
    assert!(!row(&draw(&s, &Overlay::None), 0).contains("frame"));
}

#[test]
fn hovered_unselected_shape_is_underlined_and_a_selected_one_is_not() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, "box"));
    let mut s = Session::new(c, View::default(), (80, 24));
    s.hover = Hit::Element(Id::from("aaaaaaaa"));
    let b = draw(&s, &Overlay::None);
    assert!(
        b[(22, 4)].modifier.contains(Modifier::UNDERLINED),
        "hovered"
    );
    s.selection.push(Id::from("aaaaaaaa"));
    let b = draw(&s, &Overlay::None);
    assert!(
        !b[(22, 4)].modifier.contains(Modifier::UNDERLINED),
        "selected"
    );
}

#[test]
fn hovered_handle_draws_a_diamond() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, ""));
    let mut s = Session::new(c, View::default(), (80, 24));
    s.selection.push(Id::from("aaaaaaaa"));
    assert_eq!(cols(&draw(&s, &Overlay::None), 4, 20, 1), "■");
    s.hover = Hit::Handle(Handle::Corner(Corner::Nw), Id::from("aaaaaaaa"));
    assert_eq!(cols(&draw(&s, &Overlay::None), 4, 20, 1), "◆");
}

fn reversed_blanks(b: &Buffer, y: u16) -> Vec<u16> {
    (0..80)
        .filter(|&x| b[(x, y)].modifier.contains(Modifier::REVERSED) && b[(x, y)].symbol() == " ")
        .collect()
}

#[test]
fn edit_caret_is_a_reversed_cell_after_the_text() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, "box"));
    let mut s = Session::new(c, View::default(), (80, 24));
    s.start_edit_existing(Id::from("aaaaaaaa"));
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 5, 22, 3), "box");
    assert_eq!(reversed_blanks(&b, 5), vec![25]);
    assert!(!row(&b, 4).contains('_'));
}

#[test]
fn edit_caret_in_the_middle_reverses_the_char_under_it() {
    let mut s = Session::new(Canvas::default(), View::default(), (80, 24));
    s.start_edit_new((30, 10));
    s.edit.as_mut().unwrap().insert("hey");
    s.edit.as_mut().unwrap().left(false);
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 10, 30, 3), "hey");
    assert!(b[(32, 10)].modifier.contains(Modifier::REVERSED));
    assert!(!b[(31, 10)].modifier.contains(Modifier::REVERSED));
}

#[test]
fn edit_selection_draws_reversed_chars() {
    let mut s = Session::new(Canvas::default(), View::default(), (80, 24));
    s.start_edit_new((30, 10));
    s.edit.as_mut().unwrap().insert("hey");
    s.edit.as_mut().unwrap().home(true);
    let b = draw(&s, &Overlay::None);
    assert!((30..33).all(|x| b[(x, 10)].modifier.contains(Modifier::REVERSED)));
    assert!(reversed_blanks(&b, 10).is_empty());
}

#[test]
fn long_shape_label_wraps_inside_the_shape() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, "ab cd ef"));
    let mut s = Session::new(c, View::default(), (80, 24));
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 5, 21, 6), "ab cd ");
    s.canvas.elements.clear();
    s.canvas.elements.push(Element::Shape(Shape {
        id: Id::from("aaaaaaaa"),
        kind: ShapeKind::Rect,
        x: 20,
        y: 4,
        w: 7,
        h: 5,
        label: "ab cd ef".into(),
        stroke: Default::default(),
    }));
    s.rev += 1;
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 6, 21, 5), "ab cd");
    assert_eq!(cols(&b, 7, 21, 5), " ef  ");
    assert_eq!(cols(&b, 8, 20, 7), "└─────┘");
}

#[test]
fn a_shape_too_short_for_its_label_draws_only_inside_its_rows() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, "a\nb\nc\nd"));
    let s = Session::new(c, View::default(), (80, 24));
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 3, 20, 8), "        ");
    assert_eq!(cols(&b, 7, 20, 8), "        ");
    assert_eq!(cols(&b, 4, 20, 8), "┌──────┐");
    assert_eq!(cols(&b, 6, 20, 8), "└──────┘");
}

#[test]
fn edit_draws_every_wrapped_line() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, "ab cd ef"));
    let mut s = Session::new(c, View::default(), (80, 24));
    s.start_edit_existing(Id::from("aaaaaaaa"));
    let b = draw(&s, &Overlay::None);
    let all: String = (3..9).map(|y| row(&b, y)).collect();
    assert!(all.contains("ab cd") && all.contains("ef"));
}

#[test]
fn edit_caret_shows_on_a_line_label() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 12, ""));
    c.elements.push(shape("bbbbbbbb", 40, ""));
    c.elements.push(Element::Line(Line {
        id: Id::from("cccccccc"),
        kind: LineKind::Arrow,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: "ab".into(),
        label_at: None,
        stroke: Default::default(),
        routing: Default::default(),
    }));
    let mut s = Session::new(c, View::default(), (80, 24));
    s.start_edit_existing(Id::from("cccccccc"));
    let b = draw(&s, &Overlay::None);
    let at = reversed_blanks(&b, 5);
    assert_eq!(at.len(), 1);
    assert_eq!(cols(&b, 5, at[0] - 2, 2), "ab");
}

fn overview_session(x: i32, y: i32) -> Session {
    let mut c = Canvas::default();
    c.elements.push(Element::Shape(Shape {
        id: Id::from("aaaaaaaa"),
        kind: ShapeKind::Rect,
        x,
        y,
        w: 8,
        h: 3,
        label: "box".into(),
        stroke: Default::default(),
    }));
    let mut s = Session::new(c, View::default(), (80, 24));
    s.overview = true;
    s
}

fn count(b: &Buffer, glyph: &str) -> usize {
    (0..24).map(|y| row(b, y).matches(glyph).count()).sum()
}

#[test]
fn overview_draws_blocks_without_border_label_or_palette() {
    let b = draw(&overview_session(100, 40), &Overlay::None);
    assert!(count(&b, "▒") > 0);
    assert_eq!(count(&b, "┌"), 0);
    assert_eq!(count(&b, "box"), 0);
    assert!(!row(&b, 1).contains(" Select"));
}

#[test]
fn overview_shows_a_negative_coordinate_shape() {
    let b = draw(&overview_session(-300, -90), &Overlay::None);
    assert!(count(&b, "▒") > 0);
}

#[test]
fn overview_of_an_empty_canvas_draws_nothing() {
    let mut s = overview_session(0, 0);
    s.canvas.elements.clear();
    assert_eq!(count(&draw(&s, &Overlay::None), "▒"), 0);
}

#[test]
fn guide_draws_on_blank_cells_only() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, ""));
    let mut s = Session::new(c, View::default(), (80, 24));
    s.guides = vec![((24, 2), (24, 8))];
    let b = draw(&s, &Overlay::None);
    assert_eq!(b[(24, 3)].symbol(), "┆");
    assert_eq!(b[(24, 4)].symbol(), "─");
    assert_eq!(b[(24, 6)].symbol(), "─");
}

fn kind_session(kind: ShapeKind, w: i32, h: i32) -> Session {
    let mut c = Canvas::default();
    c.elements.push(Element::Shape(Shape {
        id: Id::from("aaaaaaaa"),
        kind,
        x: 20,
        y: 4,
        w,
        h,
        label: String::new(),
        stroke: Default::default(),
    }));
    Session::new(c, View::default(), (80, 24))
}

#[test]
fn cylinder_draws_a_lid_under_the_top_row() {
    let b = draw(&kind_session(ShapeKind::Cylinder, 12, 5), &Overlay::None);
    assert_eq!(cols(&b, 4, 20, 12), "╭──────────╮");
    assert_eq!(cols(&b, 5, 20, 12), "╰──────────╯");
    assert_eq!(cols(&b, 6, 20, 1), "│");
    assert_eq!(cols(&b, 7, 31, 1), "│");
    assert_eq!(cols(&b, 8, 20, 12), "╰──────────╯");
}

#[test]
fn parallelogram_slants_every_row() {
    let b = draw(
        &kind_session(ShapeKind::Parallelogram, 12, 5),
        &Overlay::None,
    );
    assert_eq!(cols(&b, 4, 24, 8), "╱──────╱");
    assert_eq!(cols(&b, 5, 23, 1), "╱");
    assert_eq!(cols(&b, 5, 30, 1), "╱");
    assert_eq!(cols(&b, 8, 20, 8), "╱──────╱");
}

#[test]
fn hexagon_points_left_and_right_on_the_middle_row() {
    let b = draw(&kind_session(ShapeKind::Hexagon, 12, 5), &Overlay::None);
    assert_eq!(cols(&b, 4, 22, 8), "────────");
    assert_eq!(cols(&b, 5, 21, 1), "╱");
    assert_eq!(cols(&b, 5, 30, 1), "╲");
    assert_eq!(cols(&b, 6, 20, 1), "<");
    assert_eq!(cols(&b, 6, 31, 1), ">");
    assert_eq!(cols(&b, 7, 21, 1), "╲");
    assert_eq!(cols(&b, 7, 30, 1), "╱");
    assert_eq!(cols(&b, 8, 22, 8), "────────");
}

#[test]
fn new_kinds_draw_at_minimum_and_odd_sizes_without_panicking() {
    for kind in [
        ShapeKind::Cylinder,
        ShapeKind::Parallelogram,
        ShapeKind::Hexagon,
    ] {
        for (w, h) in [(3, 3), (5, 3), (3, 9), (4, 8)] {
            draw(&kind_session(kind, w, h), &Overlay::None);
        }
    }
}
