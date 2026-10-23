use clidraw::input::{Corner, Handle, Hit, Session, Tool};
use clidraw::model::{
    Anchor, Border, Canvas, Command, Element, End, Id, Line, LineKind, Paint, Shape, ShapeKind,
    Stroke, TextAlign,
};
use clidraw::ui::prompt::{Prompt, PromptKind};
use clidraw::ui::scope::ExportFormat;
use clidraw::ui::switcher::Switcher;
use clidraw::ui::theme::Theme;
use clidraw::ui::{self, Overlay, Perf};
use clidraw::view::{View, Zoom};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;
use std::sync::Mutex;

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
        border: Default::default(),
        color: None,
        fill: None,
        align: Default::default(),
        bold: false,
        link: None,
    })
}

static ENV: Mutex<()> = Mutex::new(());

fn draw(s: &Session, overlay: &Overlay) -> Buffer {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    render(s, overlay)
}

/// Draws with the colour environment set, so parallel tests never see it.
fn draw_env(s: &Session, no_color: bool, colorterm: Option<&str>) -> Buffer {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var("NO_COLOR");
    std::env::remove_var("COLORTERM");
    if no_color {
        std::env::set_var("NO_COLOR", "1");
    }
    if let Some(v) = colorterm {
        std::env::set_var("COLORTERM", v);
    }
    let b = render(s, &Overlay::None);
    std::env::remove_var("NO_COLOR");
    std::env::remove_var("COLORTERM");
    b
}

fn render(s: &Session, overlay: &Overlay) -> Buffer {
    let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
    t.draw(|f| {
        let area = f.area();
        ui::render(f.buffer_mut(), area, s, overlay, None, None, true)
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
    assert!(row(&b, 0).starts_with(" Select"));
    assert!(
        b[(1, 0)].modifier.contains(Modifier::REVERSED),
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
fn border_styles_swap_the_glyph_set() {
    let mut c = Canvas::default();
    for (id, x) in [("aaaaaaaa", 20), ("bbbbbbbb", 40), ("cccccccc", 60)] {
        c.elements.push(shape(id, x, ""));
    }
    for (e, b) in c
        .elements
        .iter_mut()
        .zip([Border::Rounded, Border::Double, Border::Heavy])
    {
        if let Element::Shape(s) = e {
            s.border = b;
        }
    }
    let s = Session::new(c, View::default(), (80, 24));
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 4, 20, 8), "╭──────╮");
    assert_eq!(cols(&b, 6, 20, 8), "╰──────╯");
    assert_eq!(cols(&b, 4, 40, 8), "╔══════╗");
    assert_eq!(cols(&b, 5, 40, 8), "║      ║");
    assert_eq!(cols(&b, 4, 60, 8), "┏━━━━━━┓");
    assert_eq!(cols(&b, 5, 60, 8), "┃      ┃");
}

#[test]
fn ink_and_fill_draw_only_on_a_truecolor_terminal() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, ""));
    c.elements.push(shape("bbbbbbbb", 40, ""));
    for e in &mut c.elements {
        if let Element::Shape(s) = e {
            s.color = Some(Paint::Red);
            s.fill = Some(Paint::Blue);
            s.kind = if s.x == 20 {
                ShapeKind::Rect
            } else {
                ShapeKind::Diamond
            };
            s.h = 5;
        }
    }
    let s = Session::new(c, View::default(), (80, 24));
    let on = draw_env(&s, false, Some("truecolor"));
    let off = draw_env(&s, true, Some("truecolor"));
    let basic = draw_env(&s, false, Some("256color"));
    assert_eq!(on[(20, 4)].fg, Color::Rgb(255, 95, 95));
    assert_eq!(on[(23, 5)].bg, Color::Rgb(50, 62, 86));
    // The diamond top row has one outline glyph, so its fill stops there.
    assert_eq!(on[(43, 4)].bg, Color::Rgb(50, 62, 86));
    assert_eq!(on[(41, 4)].bg, Color::Reset);
    for b in [off, basic] {
        assert_eq!((b[(20, 4)].fg, b[(23, 5)].bg), (Color::Reset, Color::Reset));
    }
}

#[test]
fn light_theme_darkens_ink_and_fill_and_the_selection_ink() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, ""));
    c.elements.push(shape("bbbbbbbb", 40, ""));
    for e in &mut c.elements {
        if let Element::Shape(s) = e {
            s.color = Some(Paint::Red);
            s.fill = Some(Paint::Red);
            s.h = 5;
        }
    }
    let mut s = Session::new(c, View::default(), (80, 24));
    s.theme = Theme::Light;
    s.selection.push(Id::from("bbbbbbbb"));
    let on = draw_env(&s, false, Some("truecolor"));
    let off = draw_env(&s, true, Some("truecolor"));
    assert_eq!(on[(20, 4)].fg, Color::Rgb(114, 42, 42));
    assert_eq!(on[(23, 5)].bg, Color::Rgb(255, 215, 215));
    assert_eq!(on[(41, 4)].fg, Color::Rgb(42, 114, 114));
    assert_eq!(
        (off[(20, 4)].fg, off[(23, 5)].bg),
        (Color::Reset, Color::Reset)
    );
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
    let s = Session::new(
        c,
        View {
            ox: -30,
            oy: 0,
            ..View::default()
        },
        (80, 24),
    );
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
        bends: vec![],
        color: None,
    }));
    let s = Session::new(c, View::default(), (80, 24));
    let expected = format!("│{}▶│", "─".repeat(19));
    let line = row(&draw(&s, &Overlay::None), 5);
    assert_eq!(line.chars().skip(19).take(22).collect::<String>(), expected);
}

#[test]
fn selected_arrow_keeps_its_head_visible() {
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
        bends: vec![],
        color: None,
    }));
    let mut s = Session::new(c, View::default(), (80, 24));
    s.selection.push(Id::from("cccccccc"));
    let line = row(&draw(&s, &Overlay::None), 5);
    assert!(line.contains('▶') && !line.contains('■'), "{line}");
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
        bends: vec![],
        color: None,
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
        bends: vec![],
        color: None,
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
        bends: vec![],
        color: None,
    }));
    let b = draw(&Session::new(c, View::default(), (80, 24)), &Overlay::None);
    assert_eq!(cols(&b, 5, 19, 12), "│─────────╮ ");
    assert_eq!(cols(&b, 8, 29, 1), "│");
    assert_eq!(cols(&b, 11, 28, 14), " ╰─────────▶│ ");
}

#[test]
fn help_pages_list_their_scope() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    for (page, wants) in [
        (
            0,
            vec!["Shortcuts", "Canvas", "Delete, Backspace", "u, Ctrl-r"],
        ),
        (1, vec!["Commit the label"]),
        (2, vec!["Tools", "Diamond", "Hexagon"]),
        (
            3,
            vec![
                "github.com/shortcuts/clidraw",
                "github.com/shortcuts/clidraw#contributing",
            ],
        ),
    ] {
        let b = draw(&s, &Overlay::Help(page, String::new()));
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
    assert!(draw(&s, &Overlay::Scope(ExportFormat::Svg))[(20, 4)]
        .modifier
        .contains(Modifier::DIM));
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
    let b = draw(&s, &Overlay::Help(0, String::new()));
    assert!(
        row(&b, 21).contains("Commands q w wq e u redo layout"),
        "{}",
        row(&b, 21)
    );
    assert_eq!(cols(&b, 20, 71, 1), "│");
}

#[test]
fn help_canvas_esc_row_stays_inside_the_border() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let b = draw(&s, &Overlay::Help(0, String::new()));
    assert!(row(&b, 4).contains("│ Esc"), "{}", row(&b, 4));
    assert_eq!(cols(&b, 4, 71, 1), "│", "{}", row(&b, 4));
}

#[test]
fn help_rows_stay_inside_the_border_on_every_page() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    for page in 0..4 {
        let b = draw(&s, &Overlay::Help(page, String::new()));
        let edge = row(&b, 2).trim_end().chars().count();
        for i in 0..ui::help::rows(page).len() as u16 {
            let r = row(&b, 4 + i);
            let r = r.trim_end();
            assert!(
                r.ends_with('│') && r.chars().count() == edge,
                "page {page} row {i}: {r}"
            );
        }
    }
}

#[test]
fn help_draws_on_a_tiny_screen() {
    let mut t = Terminal::new(TestBackend::new(80, 8)).unwrap();
    let s = Session::new(Canvas::default(), View::default(), (80, 8));
    t.draw(|f| {
        let area = f.area();
        ui::render(
            f.buffer_mut(),
            area,
            &s,
            &Overlay::Help(0, String::new()),
            None,
            None,
            true,
        )
    })
    .unwrap();
}

#[test]
fn selected_shape_takes_a_turned_ink_on_truecolor_and_reverses_without_it() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, "box"));
    c.elements.push(shape("bbbbbbbb", 40, "box"));
    c.elements.push(shape("cccccccc", 60, "box"));
    if let Element::Shape(s) = &mut c.elements[1] {
        s.color = Some(Paint::Red);
    }
    let mut s = Session::new(c, View::default(), (80, 24));
    s.selection.push(Id::from("aaaaaaaa"));
    s.selection.push(Id::from("bbbbbbbb"));
    let on = draw_env(&s, false, Some("truecolor"));
    let off = draw_env(&s, false, Some("256color"));
    for x in [22, 42, 62] {
        assert!(!on[(x, 4)].modifier.contains(Modifier::REVERSED), "{x}");
    }
    assert_eq!(on[(22, 4)].fg, Color::Rgb(95, 215, 255));
    assert_eq!(on[(42, 4)].fg, Color::Rgb(135, 255, 255));
    assert_eq!(on[(62, 4)].fg, Color::Reset);
    assert!(off[(22, 4)].modifier.contains(Modifier::REVERSED));
    assert!(!off[(62, 4)].modifier.contains(Modifier::REVERSED));
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
    s.view = View {
        ox: -10,
        oy: 0,
        ..View::default()
    };
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
        bends: vec![],
        color: None,
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
        ui::render(
            f.buffer_mut(),
            area,
            &s,
            &Overlay::None,
            None,
            Some(&perf),
            true,
        )
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
        border: Default::default(),
        color: None,
        fill: None,
        align: Default::default(),
        bold: false,
        link: None,
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
        bends: vec![],
        color: None,
    }));
    let mut s = Session::new(c, View::default(), (80, 24));
    s.start_edit_existing(Id::from("cccccccc"));
    let b = draw(&s, &Overlay::None);
    let at = reversed_blanks(&b, 5);
    assert_eq!(at.len(), 1);
    assert_eq!(cols(&b, 5, at[0] - 2, 2), "ab");
}

fn half_session(x: i32, y: i32) -> Session {
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
        border: Default::default(),
        color: None,
        fill: None,
        align: Default::default(),
        bold: false,
        link: None,
    }));
    Session::new(
        c,
        View {
            zoom: Zoom::Half,
            ox: x - 80,
            oy: y - 24,
        },
        (80, 24),
    )
}

fn count(b: &Buffer, glyph: &str) -> usize {
    (0..24).map(|y| row(b, y).matches(glyph).count()).sum()
}

#[test]
fn half_zoom_draws_blocks_without_border_label_or_palette() {
    let b = draw(&half_session(100, 40), &Overlay::None);
    assert!(count(&b, "▒") > 0);
    assert_eq!(count(&b, "┌"), 0);
    assert_eq!(count(&b, "box"), 0);
    assert!(!row(&b, 1).contains(" Select"));
}

#[test]
fn present_hides_palette_and_status_line() {
    let mut s = half_session(100, 40);
    s.view = View::default();
    let normal = draw(&s, &Overlay::None);
    assert!(row(&normal, 23).contains("saved"));
    s.present = true;
    let b = draw(&s, &Overlay::None);
    assert!((0..24).all(|y| !row(&b, y).contains("saved")));
    assert!(!row(&b, 1).contains(" Select"));
}

#[test]
fn half_zoom_shows_a_negative_coordinate_shape() {
    let b = draw(&half_session(-300, -90), &Overlay::None);
    assert!(count(&b, "▒") > 0);
}

#[test]
fn half_zoom_of_an_empty_canvas_draws_nothing() {
    let mut s = half_session(0, 0);
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
        border: Default::default(),
        color: None,
        fill: None,
        align: Default::default(),
        bold: false,
        link: None,
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
fn frame_draws_its_label_on_the_border_and_leaves_the_interior() {
    let mut s = kind_session(ShapeKind::Frame, 12, 5);
    let Element::Shape(sh) = &mut s.canvas.elements[0] else {
        unreachable!()
    };
    sh.label = "Auth".into();
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 4, 20, 12), "┌ Auth ────┐");
    assert_eq!(cols(&b, 5, 20, 12), "│          │");
    assert_eq!(cols(&b, 8, 20, 12), "└──────────┘");
    let Element::Shape(sh) = &mut s.canvas.elements[0] else {
        unreachable!()
    };
    sh.label = "Authentication".into();
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 4, 20, 12), "┌ Authenti ┐");
}

#[test]
fn new_kinds_draw_at_minimum_and_odd_sizes_without_panicking() {
    for kind in [
        ShapeKind::Cylinder,
        ShapeKind::Parallelogram,
        ShapeKind::Hexagon,
        ShapeKind::Frame,
    ] {
        for (w, h) in [(3, 3), (5, 3), (3, 9), (4, 8)] {
            draw(&kind_session(kind, w, h), &Overlay::None);
        }
    }
}

fn bar(s: &Session, w: u16, saved: bool) -> String {
    let h = s.screen.1;
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| {
        let area = f.area();
        ui::render(f.buffer_mut(), area, s, &Overlay::None, None, None, saved)
    })
    .unwrap();
    let b = t.backend().buffer();
    (0..w).map(|x| b[(x, h - 1)].symbol().to_string()).collect()
}

fn two_shapes() -> Session {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, "a"));
    c.elements.push(shape("bbbbbbbb", 40, "b"));
    let mut s = Session::new(c, View::default(), (80, 25));
    s.selection = vec![Id::from("aaaaaaaa"), Id::from("bbbbbbbb")];
    s
}

#[test]
fn info_bar_idle_shows_hints_left_and_tool_with_save_state_right() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let r = bar(&s, 80, true);
    assert!(
        r.contains("[Export]  ? help  : command  Ctrl-f find"),
        "{r}"
    );
    assert!(r.ends_with("saved"), "{r}");
    assert!(bar(&s, 120, false).ends_with("Select  unsaved"));
}

#[test]
fn info_bar_idle_hides_the_repo_link() {
    let s = Session::new(Canvas::default(), View::default(), (120, 24));
    assert!(!bar(&s, 120, true).contains("repo github.com/shortcuts/clidraw"));
}

#[test]
fn info_bar_hints_follow_the_state() {
    let mut s = two_shapes();
    let r = bar(&s, 120, true);
    assert!(r.contains("Del delete"), "{r}");
    assert!(r.contains("2 sel"), "{r}");
    s.tool = Tool::Rect;
    assert!(bar(&s, 80, true).contains("drag draw"));
    s.tool = Tool::Hand;
    assert!(bar(&s, 80, true).contains("drag pan"));
    s.tool = Tool::Select;
    s.start_edit_new((5, 5));
    assert!(bar(&s, 80, true).contains("Enter commit"));
}

#[test]
fn info_bar_drops_the_right_side_first_on_a_narrow_screen() {
    let s = two_shapes();
    let r = bar(&s, 40, true);
    assert!(r.ends_with("saved"), "{r}");
    assert!(!r.contains("2 sel"), "{r}");
    assert!(r.starts_with("Del delete  Shift+arrows connect"), "{r}");
}

#[test]
fn tool_bar_sits_on_row_0_and_options_on_row_1() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, "box"));
    let mut s = Session::new(c, View::default(), (80, 24));
    s.selection = vec![Id::from("aaaaaaaa")];
    let b = draw(&s, &Overlay::None);
    assert!(row(&b, 0).contains("Select"), "{}", row(&b, 0));
    assert!(row(&b, 1).contains("solid"), "{}", row(&b, 1));
    assert!(row(&b, 23).contains("Del delete"));
}

fn screen_text(b: &Buffer) -> String {
    (0..24).map(|y| row(b, y)).collect::<Vec<_>>().join("\n")
}

#[test]
fn empty_canvas_shows_welcome_until_the_first_edit() {
    let mut s = Session::new(Canvas::default(), View::default(), (80, 24));
    let hint = "drag to draw";
    assert!(screen_text(&draw(&s, &Overlay::None)).contains(hint));
    s.exec(Command::Insert(0, shape("aaaaaaaa", 20, "box")));
    assert!(!screen_text(&draw(&s, &Overlay::None)).contains(hint));
    s.undo();
    assert!(!screen_text(&draw(&s, &Overlay::None)).contains(hint));
}

#[test]
fn snap_target_cell_is_marked_with_a_reversed_diamond() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, ""));
    let mut s = Session::new(c, View::default(), (80, 24));
    s.snap = Some(Id::from("aaaaaaaa"));
    s.snap_at = Some((20, 5));
    let b = draw(&s, &Overlay::None);
    let (x, y) = s.view.to_screen(20, 5);
    let cell = &b[(x as u16, y as u16)];
    assert_eq!(cell.symbol(), "◆");
    assert!(cell.modifier.contains(Modifier::REVERSED));
}

#[test]
fn infobar_shows_size_while_drawing_a_rect_and_length_while_drawing_a_line() {
    let t = std::time::Instant::now();
    let mouse = |s: &mut Session, kind, col, row| {
        let ev = crossterm::event::MouseEvent {
            kind,
            column: col,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        clidraw::input::handle_mouse(s, ev, t);
    };
    use crossterm::event::{MouseButton::Left, MouseEventKind::*};
    for (tool, want) in [(Tool::Rect, "11x6"), (Tool::Arrow, "16 cells")] {
        let mut s = Session::new(Canvas::default(), View::default(), (80, 24));
        s.tool = tool;
        mouse(&mut s, Down(Left), 20, 5);
        mouse(&mut s, Drag(Left), 30, 10);
        let b = draw(&s, &Overlay::None);
        assert!(row(&b, 23).contains(want), "{}", row(&b, 23));
        mouse(&mut s, Up(Left), 30, 10);
        assert!(!row(&draw(&s, &Overlay::None), 23).contains(want));
    }
}

#[test]
fn label_align_places_the_text_and_bold_sets_the_modifier() {
    let mut c = Canvas::default();
    for (id, x) in [("aaaaaaaa", 20), ("bbbbbbbb", 40)] {
        c.elements.push(shape(id, x, "box"));
    }
    for (e, a) in c
        .elements
        .iter_mut()
        .zip([TextAlign::Left, TextAlign::Right])
    {
        if let Element::Shape(s) = e {
            s.align = a;
            s.bold = a == TextAlign::Right;
        }
    }
    let s = Session::new(c, View::default(), (80, 24));
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 5, 20, 8), "│box   │");
    assert_eq!(cols(&b, 5, 40, 8), "│   box│");
    assert!(!b[(21, 5)].modifier.contains(Modifier::BOLD));
    assert!(b[(44, 5)].modifier.contains(Modifier::BOLD));
}

#[test]
fn context_menu_near_the_corner_stays_on_screen() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let menu = ui::menu::Context::for_element((79, 23), (80, 24), true, false);
    let b = draw(&s, &Overlay::Context(menu));
    let all: String = (0..24).map(|y| row(&b, y)).collect();
    for e in ["Duplicate", "Send to back"] {
        assert!(all.contains(e), "{e}");
    }
    assert_eq!(b[(79, 23)].symbol(), "┘");
}

#[test]
fn actions_palette_draws_query_rows_and_selection() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let b = draw(
        &s,
        &Overlay::Actions(clidraw::ui::actions::Actions::new(&s)),
    );
    let rows: Vec<String> = (0..24).map(|y| row(&b, y)).collect();
    assert!(rows.iter().any(|r| r.contains("> _")));
    let undo = rows.iter().position(|r| r.contains("Undo")).unwrap();
    assert!(rows[undo].contains("u │"));
    assert!(b[(30, undo as u16)].modifier.contains(Modifier::REVERSED));
    let mut a = clidraw::ui::actions::Actions::new(&s);
    a.push('q');
    a.push('q');
    a.push('q');
    let b = draw(&s, &Overlay::Actions(a));
    assert!((0..24).any(|y| row(&b, y).contains("No match")));
}

#[test]
fn minimap_shows_only_while_content_is_off_screen() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 0, ""));
    let mut s = Session::new(c, View::default(), (80, 24));
    assert_eq!(cols(&draw(&s, &Overlay::None), 15, 56, 1), " ");
    s.canvas.elements.push(shape("bbbbbbbb", 1000, ""));
    s.rev += 1;
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 15, 56, 1), "┌");
    assert!((57..79).any(|x| b[(x, 17)].symbol() == "▒" || b[(x, 18)].symbol() == "▒"));
}

#[test]
fn linked_shape_draws_a_marker_past_its_top_right_corner() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, "box"));
    if let Element::Shape(s) = &mut c.elements[0] {
        s.link = Some("https://example.com".into());
    }
    let s = Session::new(c, View::default(), (80, 24));
    let b = draw(&s, &Overlay::None);
    assert_eq!(b[(28, 4)].symbol(), "↗");
}

fn top_row(s: &Session, w: u16) -> (String, Buffer) {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let mut t = Terminal::new(TestBackend::new(w, 24)).unwrap();
    t.draw(|f| {
        let area = f.area();
        ui::render(f.buffer_mut(), area, s, &Overlay::None, None, None, true)
    })
    .unwrap();
    let b = t.backend().buffer().clone();
    (row(&b, 0), b)
}

#[test]
fn tool_bar_shows_each_key_dimmed_when_the_row_fits() {
    let s = Session::new(Canvas::default(), View::default(), (100, 24));
    let (line, b) = top_row(&s, 100);
    assert!(line.starts_with(" Select v  Hand  Rect r "), "{line}");
    assert!(b[(8, 0)].modifier.contains(Modifier::DIM));
    assert_eq!(b[(8, 0)].symbol(), "v");
    assert!(!b[(1, 0)].modifier.contains(Modifier::DIM));
    let (line, _) = top_row(&s, 80);
    assert!(line.starts_with(" Select  Hand  Rect "), "{line}");
}

#[test]
fn help_search_line_shows_hint_then_query_and_keeps_the_box_aligned() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let b = draw(&s, &Overlay::Help(0, String::new()));
    assert!(row(&b, 3).contains("Type to filter"), "{}", row(&b, 3));
    let b = draw(&s, &Overlay::Help(0, "remove".into()));
    assert!(row(&b, 3).contains("> remove"), "{}", row(&b, 3));
    assert!(row(&b, 4).contains("Delete, Backspace"), "{}", row(&b, 4));
    assert_eq!(cols(&b, 4, 71, 1), "│");
    assert_eq!(cols(&b, 20, 71, 1), "│");
    let b = draw(&s, &Overlay::Help(0, "zzzz".into()));
    assert!(row(&b, 4).contains("No match"), "{}", row(&b, 4));
}

#[test]
fn double_zoom_stretches_the_border_and_keeps_the_label_one_cell_per_char() {
    let mut s = half_session(10, 5);
    s.view = View {
        zoom: Zoom::Two,
        ..View::default()
    };
    // The 8x3 "box" rect at 10,5 covers 16x6 cells from 20,10.
    let b = draw(&s, &Overlay::None);
    assert_eq!(b[(20, 10)].symbol(), "┌");
    assert_eq!(b[(35, 10)].symbol(), "┐");
    assert_eq!(b[(20, 15)].symbol(), "└");
    assert_eq!(b[(35, 15)].symbol(), "┘");
    assert_eq!(b[(35, 13)].symbol(), "│");
    assert_eq!(cols(&b, 13, 26, 3), "box");
}

#[test]
fn double_zoom_draws_the_arrow_head_at_the_scaled_end() {
    let mut c = Canvas::default();
    c.elements.push(Element::Line(Line {
        id: Id::from("llllllll"),
        from: End::Free(10, 10),
        to: End::Free(15, 10),
        kind: LineKind::Arrow,
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
    }));
    let view = View {
        zoom: Zoom::Two,
        ..View::default()
    };
    let b = draw(&Session::new(c, view, (80, 24)), &Overlay::None);
    assert_eq!(cols(&b, 20, 20, 11), "──────────▶");
}

#[test]
fn info_bar_shows_the_zoom_percent_and_zoomed_out_hints() {
    let mut s = Session::new(Canvas::default(), View::default(), (80, 24));
    assert!(!bar(&s, 80, true).contains('%'));
    s.view.zoom = Zoom::Half;
    let r = bar(&s, 120, true);
    assert!(r.contains("Esc 1x  Ctrl+wheel zoom  ? help"), "{r}");
    assert!(r.contains("50%"), "{r}");
    s.view.zoom = Zoom::Three;
    assert!(bar(&s, 120, true).contains("300%"));
}
