use clidraw::export::{mermaid, svg};
use clidraw::model::{
    Anchor, Border, Canvas, Element, End, Id, Line, LineKind, Paint, Routing, Shape, ShapeKind,
    Stroke, Text,
};

fn sample() -> Canvas {
    let shape = |id: &str, kind, x, y, w, h, label: &str| {
        Element::Shape(Shape {
            id: Id::from(id),
            kind,
            x,
            y,
            w,
            h,
            label: label.into(),
            stroke: Default::default(),
            border: Default::default(),
            color: None,
            fill: None,
        })
    };
    let line = |id: &str, kind, from, to, label: &str| {
        Element::Line(Line {
            id: Id::from(id),
            kind,
            from,
            to,
            label: label.into(),
            label_at: None,
            stroke: Default::default(),
            routing: Default::default(),
            color: None,
        })
    };
    let mut c = Canvas {
        elements: vec![
            shape("aaaaaaaa", ShapeKind::Rect, 0, 0, 12, 5, "Start"),
            shape("bbbbbbbb", ShapeKind::Diamond, 30, 0, 12, 5, "OK?"),
            shape(
                "cccccccc",
                ShapeKind::Ellipse,
                30,
                12,
                12,
                5,
                "Done & \"dusted\"",
            ),
            shape(
                "jjjjjjjj",
                ShapeKind::Rect,
                0,
                20,
                12,
                5,
                "alpha beta gamma\ndelta",
            ),
            shape("kkkkkkkk", ShapeKind::Cylinder, 16, 20, 12, 5, "DB"),
            shape("llllllll", ShapeKind::Parallelogram, 30, 20, 12, 5, "In"),
            shape("mmmmmmmm", ShapeKind::Hexagon, 16, 12, 12, 5, "Prep"),
            line(
                "dddddddd",
                LineKind::Arrow,
                End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
                End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
                "go",
            ),
            line(
                "eeeeeeee",
                LineKind::Line,
                End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
                End::Bound(Id::from("cccccccc"), Anchor::Auto),
                "",
            ),
            line(
                "ffffffff",
                LineKind::Arrow,
                End::Free(-6, 8),
                End::Free(-6, 14),
                "",
            ),
            line(
                "hhhhhhhh",
                LineKind::ArrowStart,
                End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
                End::Bound(Id::from("cccccccc"), Anchor::Auto),
                "",
            ),
            line(
                "iiiiiiii",
                LineKind::ArrowBoth,
                End::Bound(Id::from("cccccccc"), Anchor::Auto),
                End::Bound(Id::from("aaaaaaaa"), Anchor::Auto),
                "",
            ),
            Element::Text(Text {
                id: Id::from("gggggggg"),
                x: -8,
                y: -3,
                text: "note <1>".into(),
                color: None,
            }),
        ],
        ..Canvas::default()
    };
    for e in &mut c.elements {
        match e {
            Element::Shape(s) if s.id == Id::from("jjjjjjjj") => {
                s.stroke = Stroke::Dashed;
                s.border = Border::Rounded;
            }
            Element::Shape(s) if s.id == Id::from("aaaaaaaa") => {
                s.border = Border::Heavy;
                s.fill = Some(Paint::Blue);
                s.color = Some(Paint::Red);
            }
            Element::Shape(s) if s.id == Id::from("cccccccc") => s.border = Border::Double,
            Element::Line(l) if l.id == Id::from("eeeeeeee") => l.stroke = Stroke::Dashed,
            Element::Line(l) if l.id == Id::from("dddddddd") => l.color = Some(Paint::Red),
            Element::Text(t) => t.color = Some(Paint::Green),
            Element::Shape(s) if s.id == Id::from("mmmmmmmm") => s.fill = Some(Paint::Yellow),
            Element::Line(l) if l.id == Id::from("hhhhhhhh") => {
                l.stroke = Stroke::Dotted;
                l.routing = Routing::Straight;
            }
            _ => {}
        }
    }
    c
}

fn check(name: &str, actual: &str) {
    let path = format!("{}/tests/golden/{name}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).unwrap();
    assert_eq!(actual, expected, "golden file {name} differs");
}

#[test]
fn svg_matches_golden() {
    check("sample.svg", &svg(&sample()));
}

#[test]
fn mermaid_matches_golden_and_counts_skipped() {
    let out = mermaid(&sample());
    check("sample.mmd", &out.text);
    assert_eq!(out.skipped, 2);
}

#[test]
fn empty_canvas_exports_without_panicking() {
    assert!(svg(&Canvas::default()).contains("<svg"));
    assert_eq!(mermaid(&Canvas::default()).text, "flowchart TD\n");
}

#[test]
fn area_exports_match_golden() {
    let area = sample().within((0, 0, 42, 5));
    check("sample_area.svg", &svg(&area));
    let out = mermaid(&area);
    check("sample_area.mmd", &out.text);
    assert_eq!(out.skipped, 0);
}
