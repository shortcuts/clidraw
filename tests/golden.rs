use clidraw::export::{mermaid, svg};
use clidraw::model::{Canvas, Element, End, Id, Line, LineKind, Shape, ShapeKind, Text};

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
        })
    };
    let line = |id: &str, kind, from, to, label: &str| {
        Element::Line(Line {
            id: Id::from(id),
            kind,
            from,
            to,
            label: label.into(),
        })
    };
    Canvas {
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
            line(
                "dddddddd",
                LineKind::Arrow,
                End::Bound(Id::from("aaaaaaaa")),
                End::Bound(Id::from("bbbbbbbb")),
                "go",
            ),
            line(
                "eeeeeeee",
                LineKind::Line,
                End::Bound(Id::from("bbbbbbbb")),
                End::Bound(Id::from("cccccccc")),
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
                End::Bound(Id::from("aaaaaaaa")),
                End::Bound(Id::from("cccccccc")),
                "",
            ),
            Element::Text(Text {
                id: Id::from("gggggggg"),
                x: -8,
                y: -3,
                text: "note <1>".into(),
            }),
        ],
        ..Canvas::default()
    }
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
