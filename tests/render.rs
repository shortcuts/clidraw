use clidraw::input::Session;
use clidraw::model::{Canvas, Element, End, Id, Line, LineKind, Shape, ShapeKind};
use clidraw::ui::{self, Overlay};
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
    })
}

fn draw(s: &Session, overlay: &Overlay) -> Buffer {
    let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
    t.draw(|f| {
        let area = f.area();
        ui::render(f.buffer_mut(), area, s, overlay, None)
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
        from: End::Bound(Id::from("aaaaaaaa")),
        to: End::Bound(Id::from("bbbbbbbb")),
        label: String::new(),
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
        from: End::Bound(Id::from("aaaaaaaa")),
        to: End::Bound(Id::from("bbbbbbbb")),
        label: String::new(),
    }));
    let s = Session::new(c, View::default(), (80, 24));
    let expected = format!("│◀{}│", "─".repeat(19));
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
        from: End::Bound(Id::from("aaaaaaaa")),
        to: End::Bound(Id::from("bbbbbbbb")),
        label: String::new(),
    }));
    let b = draw(&Session::new(c, View::default(), (80, 24)), &Overlay::None);
    assert_eq!(cols(&b, 5, 19, 12), "│─────────╮ ");
    assert_eq!(cols(&b, 8, 29, 1), "│");
    assert_eq!(cols(&b, 11, 28, 14), " ╰─────────▶│ ");
}
