use clidraw::input::{Handle, Hit, Session, Tool};
use clidraw::model::{
    Anchor, Border, Canvas, Command, Element, End, Id, Line, LineKind, Paint, Shape, ShapeKind,
    Stroke, TextAlign,
};
use clidraw::ui::minimap::Target;
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

fn doc(saved: bool) -> ui::Doc<'static> {
    ui::Doc {
        name: "demo",
        saved,
        picking: false,
    }
}

fn render(s: &Session, overlay: &Overlay) -> Buffer {
    let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
    t.draw(|f| {
        let area = f.area();
        ui::render(f.buffer_mut(), area, s, overlay, None, None, &doc(true))
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
fn a_line_is_drawn_when_only_its_detour_is_on_screen() {
    let mut c = Canvas::default();
    for (id, (x, y, w, h)) in [
        ("aaaaaaaa", (0, 4, 3, 3)),
        ("bbbbbbbb", (6, 9, 3, 3)),
        ("oooooooo", (3, -2, 2, 16)),
    ] {
        let Element::Shape(sh) = shape(id, x, "") else {
            unreachable!()
        };
        c.elements.push(Element::Shape(Shape { y, w, h, ..sh }));
    }
    c.elements.push(Element::Line(Line {
        id: Id::from("arrow000"),
        kind: LineKind::Arrow,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
    }));
    let view = View {
        oy: 10,
        ..View::default()
    };
    let (sx, sy) = view.to_screen(3, 14);
    let s = Session::new(c, view, (80, 24));
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, sy as u16, sx as u16, 2), "──");
}

#[test]
fn a_frame_border_draws_over_a_later_shape() {
    let mut c = Canvas::default();
    c.elements.push(shape("ffffffff", 20, ""));
    c.elements.push(shape("bbbbbbbb", 24, ""));
    if let Element::Shape(f) = &mut c.elements[0] {
        f.kind = ShapeKind::Frame;
    }
    let s = Session::new(c, View::default(), (80, 24));
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 4, 24, 1), "─");
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
            vec!["Shortcuts", "Canvas", "Undo", "Ctrl-r", "Ctrl-wheel"],
        ),
        (1, vec!["Del", "Align left", "Ctrl-b"]),
        (2, vec!["Commit the label", "Alt-Enter, Ctrl-j"]),
        (3, vec!["Tools", "Diamond", "Hexagon"]),
        (4, vec![":wq", ":import <path>", ":layout [td|lr]"]),
        (
            5,
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
    assert!(draw(
        &s,
        &Overlay::Scope(ExportFormat::Svg, ui::scope::list(ExportFormat::Svg))
    )[(20, 4)]
        .modifier
        .contains(Modifier::DIM));
}

#[test]
fn switcher_rows_show_current_and_dim_keys_at_every_width() {
    for w in [40u16, 60, 120] {
        let s = Session::new(Canvas::default(), View::default(), (w, 24));
        let sw = Switcher::new(vec!["main".into(), "other".into()], "main");
        let b = draw(&s, &Overlay::Switcher(sw));
        let all: Vec<String> = (0..24).map(|y| row(&b, y)).collect();
        let at = |needle: &str| all.iter().position(|r| r.contains(needle)).unwrap();
        assert!(
            all[at("main (current)")].contains("r rename  x delete"),
            "{w}"
        );
        let (selected, other) = (at("main (current)") as u16, at("other") as u16);
        let col = |y: u16, needle: &str| row(&b, y).find(needle).unwrap() as u16;
        let x = col(other, "r rename");
        assert!(b[(x, other)].modifier.contains(Modifier::DIM), "{w}");
        assert!(b[(col(selected, "main"), selected)]
            .modifier
            .contains(Modifier::REVERSED));
    }
}

#[test]
fn context_menu_rows_show_their_registry_key() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let b = draw(
        &s,
        &Overlay::Context(ui::menu::element((10, 5), false, false)),
    );
    let line = (0..24)
        .map(|y| row(&b, y))
        .find(|r| r.contains("Raise"))
        .unwrap();
    assert!(line.contains("Raise") && line.contains(" ] "), "{line}");
    let first = (0..24).find(|&y| row(&b, y).contains("Duplicate")).unwrap();
    assert!(b[(12, first)].modifier.contains(Modifier::REVERSED));
}

#[test]
fn prompt_box_holds_no_hint_text() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let p = Prompt::new(PromptKind::NewCanvas, String::new());
    let b = draw(&s, &Overlay::Prompt(p));
    assert!(!(0..24).any(|y| row(&b, y).contains("Enter confirms")));
}

#[test]
fn help_commands_page_lists_every_command_line_verb() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let b = draw(&s, &Overlay::Help(4, String::new()));
    let page: String = (4..20).map(|y| row(&b, y)).collect();
    for verb in [
        ":q",
        ":w",
        ":wq",
        ":e <name>",
        ":import <path>",
        ":export",
        ":layout [td|lr]",
    ] {
        assert!(page.contains(verb), "{verb}");
    }
}

#[test]
fn help_canvas_page_lists_find_palette_present_and_minimap() {
    let rows = ui::help::rows(0);
    for (keys, action) in [
        ("/", "Find"),
        ("Ctrl-p", "Command palette"),
        ("Ctrl-p", "Present"),
        ("Ctrl-p", "Show minimap"),
    ] {
        assert!(
            rows.contains(&(keys.to_string(), action.to_string())),
            "{action}"
        );
    }
}

#[test]
fn help_bottom_bar_hints_the_tab_keys() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let b = draw(&s, &Overlay::Help(0, String::new()));
    assert!(
        row(&b, 23).contains("\u{2039} \u{203a} switch tab  Esc close"),
        "{}",
        row(&b, 23)
    );
}

#[test]
fn help_canvas_esc_row_stays_inside_the_border() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let b = draw(&s, &Overlay::Help(0, String::new()));
    assert!(row(&b, 4).contains("│ u"), "{}", row(&b, 4));
    assert_eq!(cols(&b, 4, 71, 1), "│", "{}", row(&b, 4));
}

#[test]
fn help_rows_stay_inside_the_border_on_every_page() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    for page in 0..ui::help::SCOPES.len() {
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
            &doc(true),
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
    let top = s.viewport().1 as u16;
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
            &doc(true),
        )
    })
    .unwrap();
    let b = t.backend().buffer().clone();
    assert!(
        row(&b, top).ends_with(" frame 91.0/150.0ms  ev 3  1234B  0 el "),
        "{}",
        row(&b, top)
    );
    assert!(!row(&b, 0).contains("frame"));
    assert!(!row(&draw(&s, &Overlay::None), top).contains("frame"));
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
    assert_eq!(cols(&draw(&s, &Overlay::None), 6, 27, 1), "⤡");
    s.hover = Hit::Handle(Handle::Resize, Id::from("aaaaaaaa"));
    assert_eq!(cols(&draw(&s, &Overlay::None), 6, 27, 1), "◆");
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
fn an_upper_shape_keeps_the_outline_of_the_shape_it_overlaps() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, ""));
    c.elements.push(shape("bbbbbbbb", 24, ""));
    let s = Session::new(c, View::default(), (80, 24));
    let b = draw(&s, &Overlay::None);
    assert_eq!(cols(&b, 5, 20, 12), "│   │  │   │");
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
    assert!(row(&normal, 23).ends_with("demo"));
    s.present = true;
    let b = draw(&s, &Overlay::None);
    assert!((0..24).all(|y| !row(&b, y).contains("demo")));
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
    bar_with(s, w, &doc(saved))
}

fn bar_with(s: &Session, w: u16, d: &ui::Doc) -> String {
    let h = s.screen.1;
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| {
        let area = f.area();
        ui::render(f.buffer_mut(), area, s, &Overlay::None, None, None, d)
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
fn info_bar_idle_reads_buttons_badge_hints_then_view_cluster_and_name() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let r = bar(&s, 120, true);
    assert!(
        r.starts_with("[Undo] [Redo]   SELECT   ? help  : command  / find  z fit"),
        "{r}"
    );
    assert!(r.ends_with("[-] 100% [+] [Fit]  demo"), "{r}");
    assert!(bar(&s, 120, false).ends_with("demo ●"));
}

#[test]
fn info_bar_badge_follows_the_mode() {
    let mut s = Session::new(Canvas::default(), View::default(), (80, 24));
    s.tool = Tool::Rect;
    assert!(bar(&s, 120, true).contains(" RECT "));
    s.tool = Tool::Select;
    s.start_edit_new((5, 5));
    assert!(bar(&s, 120, true).contains(" EDIT "));
    let mut z = Session::new(Canvas::default(), View::default(), (80, 24));
    z.view.zoom = Zoom::Half;
    assert!(bar(&z, 120, true).contains(" ZOOM "));
    let idle = Session::new(Canvas::default(), View::default(), (80, 24));
    let picking = ui::Doc {
        picking: true,
        ..doc(true)
    };
    assert!(bar_with(&idle, 120, &picking).contains(" PICK   drag area  Esc cancel"));
}

#[test]
fn info_bar_keeps_buttons_and_drops_hints_first_when_narrow() {
    let s = two_shapes();
    let r = bar(&s, 60, true);
    assert!(r.contains("2 sel") && r.contains("[Fit]"), "{r}");
    let r = bar(&s, 40, true);
    assert!(r.starts_with("[Undo] [Redo]"), "{r}");
    assert!(
        r.contains("[Fit]") && !r.contains("2 sel") && !r.contains("Del"),
        "{r}"
    );
}

#[test]
fn info_bar_truncates_a_long_name_and_keeps_the_marker() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let long = "n".repeat(30);
    let d = ui::Doc {
        name: &long,
        saved: false,
        picking: false,
    };
    let r = bar_with(&s, 60, &d);
    assert!(r.ends_with("… ●"), "{r}");
    assert!(r.contains("[+] [Fit]"), "{r}");
}

#[test]
fn info_bar_buttons_are_hit_where_the_layout_draws_them() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let long = "n".repeat(30);
    let d = ui::Doc {
        name: &long,
        saved: true,
        picking: false,
    };
    for d in [&doc(true), &d] {
        let r = bar_with(&s, 80, d);
        let at = |needle: &str| {
            r.find(needle)
                .map(|i| r[..i].chars().count() as i32)
                .unwrap()
        };
        let hit = |col| ui::button_at(&s, d, col, 23);
        assert_eq!(hit(7), Some(ui::Button::Redo));
        assert_eq!(hit(at("[+]")), Some(ui::Button::ZoomIn));
        assert_eq!(hit(at("[Fit]")), Some(ui::Button::Fit));
        assert_eq!(hit(at("100%")), None);
        assert_eq!(hit(at("SELECT")), None);
    }
}

#[test]
fn info_bar_status_replaces_only_the_hints() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let mut t = Terminal::new(TestBackend::new(100, 24)).unwrap();
    t.draw(|f| {
        let area = f.area();
        let st = ui::Status::notice("saved to disk");
        ui::render(
            f.buffer_mut(),
            area,
            &s,
            &Overlay::None,
            Some(&st),
            None,
            &doc(true),
        )
    })
    .unwrap();
    let b = t.backend().buffer();
    let r: String = (0..100).map(|x| b[(x, 23)].symbol().to_string()).collect();
    assert!(
        r.starts_with("[Undo] [Redo]   SELECT   saved to disk"),
        "{r}"
    );
    assert!(r.ends_with("demo") && !r.contains("? help"), "{r}");
}

#[test]
fn info_bar_hints_come_from_the_registry_at_every_width() {
    let idle = Session::new(Canvas::default(), View::default(), (80, 24));
    assert!(bar(&idle, 120, true).contains("? help  : command  / find  z fit"));
    assert!(bar(&idle, 80, true).contains("? help  : command"));
    assert!(bar(&idle, 70, true).contains("? help  : command"));
    let sel = two_shapes();
    assert!(bar(&sel, 120, true).contains("Del delete  Shift-arrows connect  y copy  g group"));
    assert!(bar(&sel, 80, true).contains("Del delete"));
}

#[test]
fn palette_keys_come_from_the_registry() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let overlay = Overlay::Actions(ui::actions::list(&s));
    let b = draw(&s, &overlay);
    let all: String = (0..24).map(|y| row(&b, y)).collect::<Vec<_>>().join("\n");
    assert!(all.contains("Redo") && all.contains("Ctrl-r"), "{all}");
    assert!(!all.contains("Ctrl+"), "{all}");
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
    s.tool = Tool::Select;
    s.start_edit_new((5, 5));
    assert!(bar(&s, 80, true).contains("Enter commit"));
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
    let hint = "Drag to draw";
    let shown = screen_text(&draw(&s, &Overlay::None));
    assert!(
        shown.contains(hint) && shown.contains("? for keys") && shown.contains(": for commands")
    );
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
    let menu = ui::menu::element((79, 23), true, false);
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
    let b = draw(&s, &Overlay::Actions(clidraw::ui::actions::list(&s)));
    let rows: Vec<String> = (0..24).map(|y| row(&b, y)).collect();
    assert!(rows.iter().any(|r| r.contains("> _")));
    let undo = rows.iter().position(|r| r.contains("Undo")).unwrap();
    assert!(rows[undo].contains("u │"));
    assert!(b[(30, undo as u16)].modifier.contains(Modifier::REVERSED));
    let mut a = clidraw::ui::actions::list(&s);
    a.push('q');
    a.push('q');
    a.push('q');
    let b = draw(&s, &Overlay::Actions(a));
    assert!((0..24).any(|y| row(&b, y).contains("No match")));
}

#[test]
fn minimap_overlay_labels_frames_and_shapes() {
    let mut c = Canvas::default();
    let mut frame = shape("ffffffff", 0, "Zone");
    if let Element::Shape(f) = &mut frame {
        (f.kind, f.y, f.w, f.h) = (ShapeKind::Frame, 0, 40, 12);
    }
    c.elements.push(frame);
    c.elements.push(shape("aaaaaaaa", 2, "box"));
    c.elements.push(shape("bbbbbbbb", 100, ""));
    let s = Session::new(c, View::default(), (80, 24));
    let b = draw(&s, &Overlay::Map(Target::Canvas));
    let rows: Vec<String> = (0..24).map(|y| row(&b, y)).collect();
    assert!(rows.iter().any(|r| r.contains("┌Zone")), "{rows:#?}");
    assert!(rows.iter().any(|r| r.contains("box")));
    assert!(
        rows.iter().any(|r| r.contains("▒▒▒▒")),
        "the unlabelled far shape is a plain box"
    );
    assert!(rows.iter().all(|r| !r.contains("bbbbbbbb")));
    let off = draw(&s, &Overlay::None);
    assert_eq!(
        cols(&off, 15, 56, 1),
        " ",
        "no corner box without the overlay"
    );
}

fn at(mut e: Element, x: i32, y: i32, w: i32, h: i32) -> Element {
    if let Element::Shape(s) = &mut e {
        (s.x, s.y, s.w, s.h) = (x, y, w, h);
    }
    e
}

fn map_rows(s: &Session, target: Target) -> String {
    let b = draw(s, &Overlay::Map(target));
    (0..24).map(|y| row(&b, y)).collect::<Vec<_>>().join("\n")
}

/// A frame around the viewport centre holding "in", plus "out" far to the right.
fn framed() -> Session {
    let mut c = Canvas::default();
    let mut frame = shape("ffffffff", 0, "Zone");
    if let Element::Shape(f) = &mut frame {
        f.kind = ShapeKind::Frame;
    }
    c.elements.push(at(frame, 20, 5, 40, 15));
    c.elements.push(at(shape("aaaaaaaa", 0, "in"), 30, 8, 8, 3));
    c.elements
        .push(at(shape("bbbbbbbb", 0, "out"), 100, 8, 8, 3));
    Session::new(c, View::default(), (80, 24))
}

#[test]
fn minimap_hides_a_label_wider_than_its_block() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 0, "abcdefgh"));
    let s = Session::new(c.clone(), View::default(), (80, 24));
    assert!(map_rows(&s, Target::Canvas).contains("abcdefgh"), "d = 1");
    c.elements.push(shape("bbbbbbbb", 100, ""));
    let s = Session::new(c, View::default(), (80, 24));
    assert!(!map_rows(&s, Target::Canvas).contains("abcd"), "d = 2");
}

#[test]
fn minimap_draws_the_route_between_linked_shapes() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 0, ""));
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
    assert!(map_rows(&s, Target::Canvas).contains("▒·"));
}

#[test]
fn minimap_frame_target_shows_only_the_frame_around_the_viewport_centre() {
    let s = framed();
    let frame = map_rows(&s, Target::Frame);
    assert!(frame.contains("┌Zone") && frame.contains("in"));
    assert!(!frame.contains("out"));
    assert!(map_rows(&s, Target::Canvas).contains("out"));
}

#[test]
fn minimap_selection_target_shows_only_the_selected_shape() {
    let mut s = framed();
    s.selection.push(Id::from("aaaaaaaa"));
    let sel = map_rows(&s, Target::Selection);
    assert!(sel.contains("in▒▒▒▒▒▒"), "whole block at d = 1");
    assert!(!sel.contains("out"));
    s.selection.clear();
    assert_eq!(
        map_rows(&s, Target::Selection),
        map_rows(&s, Target::Canvas),
        "no selection falls back to the canvas"
    );
}

#[test]
fn minimap_frame_target_without_a_frame_draws_the_canvas() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 0, "in"));
    c.elements.push(shape("bbbbbbbb", 100, "out"));
    let s = Session::new(c, View::default(), (80, 24));
    assert_eq!(map_rows(&s, Target::Frame), map_rows(&s, Target::Canvas));
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
        ui::render(
            f.buffer_mut(),
            area,
            s,
            &Overlay::None,
            None,
            None,
            &doc(true),
        )
    })
    .unwrap();
    let b = t.backend().buffer().clone();
    (row(&b, 0), b)
}

#[test]
fn tool_bar_shows_each_key_dimmed_when_the_row_fits() {
    let s = Session::new(Canvas::default(), View::default(), (100, 24));
    let (line, b) = top_row(&s, 100);
    assert!(line.starts_with(" Select v  Rect r "), "{line}");
    assert!(b[(8, 0)].modifier.contains(Modifier::DIM));
    assert_eq!(b[(8, 0)].symbol(), "v");
    assert!(!b[(1, 0)].modifier.contains(Modifier::DIM));
    let (line, _) = top_row(&s, 80);
    assert!(line.starts_with(" Select  Rect "), "{line}");
}

#[test]
fn help_search_line_shows_hint_then_query_and_keeps_the_box_aligned() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let b = draw(&s, &Overlay::Help(0, String::new()));
    assert!(row(&b, 3).contains("Type to filter"), "{}", row(&b, 3));
    let b = draw(&s, &Overlay::Help(0, "remove".into()));
    assert!(row(&b, 3).contains("> remove"), "{}", row(&b, 3));
    assert!(row(&b, 4).contains("Del"), "{}", row(&b, 4));
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
    assert!(bar(&s, 80, true).contains("100%"));
    s.view.zoom = Zoom::Half;
    let r = bar(&s, 120, true);
    assert!(r.contains("Esc 1x  Ctrl-wheel zoom  ? help"), "{r}");
    assert!(r.contains("50%"), "{r}");
    s.view.zoom = Zoom::Three;
    assert!(bar(&s, 120, true).contains("300%"));
}

#[test]
fn option_cells_leave_no_canvas_gap_after_wide_glyph_labels() {
    let mut c = Canvas::default();
    let Element::Shape(mut sh) = shape("aaaaaaaa", 0, "") else {
        unreachable!()
    };
    (sh.y, sh.w, sh.h) = (1, 80, 40);
    c.elements.push(Element::Shape(sh));
    let mut s = Session::new(c, View::default(), (80, 24));
    s.selection.push(Id::from("aaaaaaaa"));
    let b = draw(&s, &Overlay::None);
    assert_eq!(
        cols(&b, 1, 3, 62),
        " __ solid  -- dashed  .. dotted │ ┌┐ single  ╭╮ round  ╔╗ doub",
    );
}

fn render_at(s: &Session, w: u16, h: u16) -> Buffer {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| {
        let area = f.area();
        ui::render(
            f.buffer_mut(),
            area,
            s,
            &Overlay::None,
            None,
            None,
            &doc(true),
        )
    })
    .unwrap();
    t.backend().buffer().clone()
}

fn line_at(b: &Buffer, y: u16) -> String {
    (0..b.area.width)
        .map(|x| b[(x, y)].symbol().to_string())
        .collect()
}

#[test]
fn chrome_rows_never_show_canvas_cells() {
    for w in [40, 60, 120] {
        let mut c = Canvas::default();
        let mut e = shape("aaaaaaaa", 20, "box");
        if let Element::Shape(sh) = &mut e {
            sh.y = 0;
            sh.h = 30;
            sh.border = Border::Double;
        }
        c.elements.push(e);
        let mut s = Session::new(c, View::default(), (w, 12));
        s.selection = vec![Id::from("aaaaaaaa")];
        let top = ui::palette::rows(&s) as u16;
        assert_eq!(top, 2);
        let b = render_at(&s, w, 12);
        for y in (0..top).chain([11]) {
            assert!(!line_at(&b, y).contains('║'), "w={w} row {y}");
        }
        assert!(line_at(&b, top).contains('║'), "w={w} first viewport row");
    }
}

fn strip_session(w: u16) -> Session {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, "box"));
    c.elements.push(shape("bbbbbbbb", 40, ""));
    c.elements.push(Element::Line(Line {
        id: Id::from("llllllll"),
        kind: LineKind::Arrow,
        from: End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
        to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
    }));
    let mut s = Session::new(c, View::default(), (w, 12));
    s.selection = vec![Id::from("aaaaaaaa"), Id::from("llllllll")];
    s
}

#[test]
fn strip_is_one_row_with_group_separators_and_arrows_when_it_overflows() {
    for w in [40, 60, 120] {
        let mut s = strip_session(w);
        if w == 120 {
            s.selection.truncate(1);
        }
        assert_eq!(ui::palette::rows(&s), 2, "w={w}");
        let b = render_at(&s, w, 12);
        let row = line_at(&b, 1);
        assert!(w == 40 || row.contains('│'), "w={w} {row}");
        assert!(
            !line_at(&b, 2).contains("single"),
            "w={w} no second option row"
        );
        let (first, last) = (cols(&b, 1, 0, 3), cols(&b, 1, w - 3, 3));
        if w == 120 {
            assert!(!row.contains('›') && !row.contains('‹'), "{row}");
        } else {
            assert_eq!((first.as_str(), last.as_str()), (" ‹ ", " › "), "w={w}");
            assert!(
                b[(1, 1)].modifier.contains(Modifier::DIM),
                "w={w} left arrow is dimmed"
            );
            assert!(
                !b[(w - 2, 1)].modifier.contains(Modifier::DIM),
                "w={w} right arrow is live"
            );
        }
    }
}

#[test]
fn tool_row_scrolls_at_58_columns_and_ends_with_text() {
    let mut s = Session::new(Canvas::default(), View::default(), (58, 12));
    let b = render_at(&s, 58, 12);
    assert_eq!(cols(&b, 0, 55, 3), " › ");
    s.tool_scroll = 1000;
    let b = render_at(&s, 58, 12);
    assert_eq!(cols(&b, 0, 0, 3), " ‹ ");
    assert_eq!(cols(&b, 0, 55, 3), " › ");
    assert!(b[(56, 0)].modifier.contains(Modifier::DIM));
    assert!(line_at(&b, 0).contains(" Text "), "{}", line_at(&b, 0));
}

#[test]
fn zoomed_out_keeps_the_tool_row_and_one_top_row() {
    let mut c = Canvas::default();
    c.elements.push(shape("aaaaaaaa", 20, "box"));
    let view = View {
        zoom: Zoom::Half,
        ..View::default()
    };
    let mut s = Session::new(c, view, (80, 24));
    s.selection = vec![Id::from("aaaaaaaa")];
    assert_eq!(ui::palette::rows(&s), 1);
    let b = render_at(&s, 80, 24);
    assert!(line_at(&b, 0).starts_with(" Select"));
}

#[test]
fn active_chrome_uses_the_accent_with_truecolor_and_plain_reverse_without() {
    let mut s = Session::new(Canvas::default(), View::default(), (80, 24));
    let reversed = |b: &Buffer| -> Vec<Color> {
        (0..24)
            .flat_map(|y| (0..80).map(move |x| (x, y)))
            .filter(|&p| b[p].modifier.contains(ratatui::style::Modifier::REVERSED))
            .map(|p| b[p].fg)
            .collect()
    };
    let on = reversed(&draw_env(&s, false, Some("truecolor")));
    assert!(!on.is_empty());
    assert!(on.iter().all(|&c| c == Color::Rgb(95, 215, 255)));
    s.chrome_accent = Some(Color::Rgb(255, 136, 0));
    let custom = reversed(&draw_env(&s, false, Some("truecolor")));
    assert!(custom.iter().all(|&c| c == Color::Rgb(255, 136, 0)));
    for (no_color, term) in [(true, Some("truecolor")), (false, Some("256color"))] {
        let off = reversed(&draw_env(&s, no_color, term));
        assert!(!off.is_empty());
        assert!(off.iter().all(|&c| c == Color::Reset));
    }
}

#[test]
fn export_scope_box_title_names_its_format() {
    let s = Session::new(Canvas::default(), View::default(), (80, 24));
    let text = screen_text(&draw(
        &s,
        &Overlay::Scope(ExportFormat::Svg, ui::scope::list(ExportFormat::Svg)),
    ));
    assert!(text.contains("Export SVG"));
}

fn command_prompt_at(buf: &str, w: u16, h: u16) -> Buffer {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let s = Session::new(Canvas::default(), View::default(), (w, h));
    let overlay = Overlay::Prompt(Prompt::new(PromptKind::Command, buf.into()));
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| {
        let area = f.area();
        ui::render(f.buffer_mut(), area, &s, &overlay, None, None, &doc(true))
    })
    .unwrap();
    t.backend().buffer().clone()
}

#[test]
fn command_line_replaces_the_bottom_row_with_a_caret_and_a_list_above() {
    for w in [40, 60, 120] {
        let b = command_prompt_at("lay", w, 24);
        if w >= 60 {
            assert!(line_at(&b, 21).contains(":layout [td|lr]"), "{w}");
        }
        let all: String = (0..24).map(|y| line_at(&b, y)).collect();
        assert!(line_at(&b, 23).starts_with(":lay "), "{w}");
        assert!(b[(4, 23)].modifier.contains(Modifier::REVERSED), "{w}");
        assert!(!all.replace("Commands", "").contains("Command"), "{w}");
        assert!(!all.contains("[ OK ]"), "{w}");
        assert!(line_at(&b, 22).starts_with("└─"), "{w}");
        assert!(line_at(&b, 21).contains(":layout"), "{w}");
    }
}

#[test]
fn find_line_and_unmatched_command_draw_no_list() {
    let b = command_prompt_at("/x", 60, 24);
    assert!(line_at(&b, 23).starts_with("/x "));
    assert!(!(0..23).map(|y| line_at(&b, y)).any(|l| l.contains("└─")));
    let b = command_prompt_at("zzz", 60, 24);
    assert!(!(0..23).map(|y| line_at(&b, y)).any(|l| l.contains("└─")));
}

#[test]
fn empty_command_line_lists_every_verb() {
    let b = command_prompt_at("", 120, 40);
    let all: String = (0..40).map(|y| line_at(&b, y)).collect();
    for verb in [
        ":perf",
        ":redo",
        ":u",
        ":import <path>",
        ":lib list|save|insert|delete [name]",
    ] {
        assert!(all.contains(verb), "{verb}");
    }
}

#[test]
fn command_line_keeps_the_caret_on_screen_when_the_buffer_overflows() {
    let b = command_prompt_at(&"a".repeat(80), 40, 24);
    assert!(b[(39, 23)].modifier.contains(Modifier::REVERSED));
}

fn mermaid_prompt_at(buf: &str, w: u16, h: u16) -> Buffer {
    mermaid_prompt_fixes(buf, w, h, false)
}

fn mermaid_prompt_fixes(buf: &str, w: u16, h: u16, fixes: bool) -> Buffer {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let s = Session::new(Canvas::default(), View::default(), (w, h));
    let mut p = Prompt::new(PromptKind::Mermaid("flowchart TD".into()), buf.into());
    p.fixes = fixes;
    let overlay = Overlay::Prompt(p);
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| {
        let area = f.area();
        ui::render(f.buffer_mut(), area, &s, &overlay, None, None, &doc(true))
    })
    .unwrap();
    t.backend().buffer().clone()
}

#[test]
fn mermaid_prompt_hints_the_first_diagnostic_after_the_text() {
    for w in [40, 60, 120] {
        let ok = line_at(&mermaid_prompt_at("A --> B", w, 24), 23);
        assert!(ok.starts_with("mmd> A --> B"), "{w}");
        assert!(!ok.contains("error") && !ok.contains("warning"), "{w}");

        let bad = line_at(&mermaid_prompt_at("A -->", w, 24), 23);
        assert!(bad.starts_with("mmd> A -->  error: Cann"), "{w}: {bad}");

        let b = mermaid_prompt_at("subgraph x", w, 24);
        let at = line_at(&b, 23).find("warning:").expect("warning hint") as u16;
        assert!(b[(at, 23)].modifier.contains(Modifier::DIM), "{w}");

        let empty = line_at(&mermaid_prompt_at("", w, 24), 23);
        assert!(
            !empty.contains("error") && !empty.contains("warning"),
            "{w}"
        );
        let all: String = (0..24)
            .map(|y| line_at(&mermaid_prompt_at("A -->", w, 24), y))
            .collect();
        assert!(!all.contains("[ OK ]"), "{w}");
    }
}

#[test]
fn mermaid_prompt_shows_the_fix_only_when_enabled() {
    for w in [60, 120] {
        let on = line_at(&mermaid_prompt_fixes("A -> B", w, 24, true), 23);
        assert!(on.contains("is not an arrow  Tab: Use -->"), "{w}: {on}");
        let off = line_at(&mermaid_prompt_fixes("A -> B", w, 24, false), 23);
        assert!(
            off.contains("is not an arrow") && !off.contains("Tab"),
            "{w}: {off}"
        );
    }
    let narrow = line_at(&mermaid_prompt_fixes("A -> B", 40, 24, true), 23);
    assert!(narrow.starts_with("mmd> A -> B"), "{narrow}");
}

#[test]
fn change_shape_overlay_draws_the_kind_rows() {
    for width in [40u16, 60, 120] {
        let s = Session::new(Canvas::default(), View::default(), (width, 24));
        let overlay = Overlay::Kinds(vec![], clidraw::ui::kinds::list(ShapeKind::Rect, 23));
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let mut t = Terminal::new(TestBackend::new(width, 24)).unwrap();
        t.draw(|f| {
            let area = f.area();
            ui::render(f.buffer_mut(), area, &s, &overlay, None, None, &doc(true))
        })
        .unwrap();
        let b = t.backend().buffer().clone();
        let rows: Vec<String> = (0..24).map(|y| line_at(&b, y)).collect();
        assert!(rows.iter().any(|r| r.contains("Change shape")), "{width}");
        for name in [
            "Rect",
            "Diamond",
            "Cylinder",
            "Parallelogram",
            "Hexagon",
            "Frame",
        ] {
            let at = rows.iter().position(|r| r.contains(name)).expect(name);
            assert!(at < 23, "{name} at {at}");
        }
    }
}

fn free_line(id: &str, from: (i32, i32), to: (i32, i32)) -> Element {
    Element::Line(Line {
        id: Id::from(id),
        from: End::Free(from.0, from.1),
        to: End::Free(to.0, to.1),
        kind: LineKind::Line,
        label: String::new(),
        label_at: None,
        stroke: Default::default(),
        bends: vec![],
        color: None,
    })
}

#[test]
fn a_branch_off_a_line_draws_a_junction_glyph() {
    let mut c = Canvas::default();
    c.elements.push(free_line("aaaaaaaa", (10, 10), (30, 10)));
    c.elements.push(free_line("bbbbbbbb", (20, 10), (26, 14)));
    let b = draw(&Session::new(c, View::default(), (80, 24)), &Overlay::None);
    assert_eq!(cols(&b, 10, 19, 3), "─┬─");
    assert_eq!(cols(&b, 14, 20, 2), "╰─");
}
