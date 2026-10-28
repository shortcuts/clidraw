use clidraw::diagram::{DiagramKind, Flowchart};
use clidraw::export::{svg, svg_padded};
use clidraw::model::{
    Anchor, Border, Canvas, Edge, Element, End, Id, Line, LineKind, Paint, Shape, ShapeKind,
    SideLabel, Stroke, Text, TextAlign,
};
use clidraw::store::encode;

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
            align: Default::default(),
            bold: false,
            link: None,
            side_labels: vec![],
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
            bends: vec![],
            color: None,
        })
    };
    let mut c = Canvas {
        elements: vec![
            shape("aaaaaaaa", ShapeKind::Rect, 0, 0, 12, 5, "Start"),
            shape("bbbbbbbb", ShapeKind::Diamond, 30, 0, 12, 5, "OK?"),
            shape(
                "cccccccc",
                ShapeKind::Rect,
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
            shape("nnnnnnnn", ShapeKind::Frame, -1, 18, 44, 9, "Group"),
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
                s.link = Some("https://example.com/a?x=1&y=2".into());
                s.fill = Some(Paint::Blue);
                s.color = Some(Paint::Red);
            }
            Element::Shape(s) if s.id == Id::from("cccccccc") => {
                s.border = Border::Double;
                s.align = TextAlign::Right;
                s.bold = true;
                s.side_labels = vec![SideLabel {
                    edge: Edge::Top,
                    at: None,
                    text: "side <a>".into(),
                }];
            }
            Element::Line(l) if l.id == Id::from("eeeeeeee") => l.stroke = Stroke::Dashed,
            Element::Line(l) if l.id == Id::from("dddddddd") => l.color = Some(Paint::Red),
            Element::Text(t) => t.color = Some(Paint::Green),
            Element::Shape(s) if s.id == Id::from("mmmmmmmm") => s.fill = Some(Paint::Yellow),
            Element::Line(l) if l.id == Id::from("hhhhhhhh") => {
                l.stroke = Stroke::Dotted;
                l.bends = vec![(20, 9)];
            }
            Element::Line(l) if l.id == Id::from("iiiiiiii") => l.bends = vec![(26, 8)],
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
fn svg_dashed_edge_uses_the_dotted_pattern_and_shape_keeps_its_own() {
    let out = svg(&sample());
    assert!(out.contains("stroke-dasharray=\"2 4\" stroke-linejoin=\"round\"/>"));
    assert!(out.contains("rx=\"8\" fill=\"none\" stroke=\"black\" stroke-dasharray=\"8 4\"/>"));
}

#[test]
fn mermaid_matches_golden_and_counts_skipped() {
    let out = Flowchart.export(&sample());
    check("sample.mmd", &out.text);
    assert_eq!(out.skipped, 4);
}

#[test]
fn mermaid_click_url_cannot_break_out_of_its_quotes() {
    let mut c = sample();
    let Element::Shape(s) = &mut c.elements[0] else {
        unreachable!()
    };
    s.link = Some("https://a.test/\"x\ny".into());
    let text = Flowchart.export(&c).text;
    assert!(text.contains("  click n0 \"https://a.test/%22xy\" _blank\n"));
}

#[test]
fn empty_canvas_exports_without_panicking() {
    assert!(svg(&Canvas::default()).contains("<svg"));
    assert_eq!(Flowchart.export(&Canvas::default()).text, "flowchart TD\n");
}

#[test]
fn area_exports_match_golden() {
    let area = sample().within((0, 0, 42, 5));
    check("sample_area.svg", &svg(&area));
    let out = Flowchart.export(&area);
    check("sample_area.mmd", &out.text);
    assert_eq!(out.skipped, 0);
}

#[test]
fn svg_padding_widens_the_view_box() {
    let one = sample().within((0, 0, 12, 5));
    assert!(svg_padded(&one, 0).contains("viewBox=\"0 0 96 80\""));
    assert!(svg_padded(&one, 3).contains("viewBox=\"-24 -48 144 176\""));
}

#[test]
fn a_default_frame_draws_grey_and_dotted_but_a_colored_one_keeps_its_color() {
    let frame = |color| {
        let mut c = Canvas::default();
        c.elements.push(Element::Shape(Shape {
            id: Id::from("ffffffff"),
            kind: ShapeKind::Frame,
            x: 0,
            y: 0,
            w: 12,
            h: 5,
            label: "Auth".into(),
            stroke: Default::default(),
            border: Default::default(),
            color,
            fill: None,
            align: Default::default(),
            bold: false,
            link: None,
            side_labels: vec![],
        }));
        svg(&c)
    };
    let plain = frame(None);
    assert!(plain.contains("stroke=\"#767676\" stroke-dasharray=\"2 4\""));
    assert!(plain.contains("fill=\"#767676\">Auth"));
    assert!(!frame(Some(Paint::Blue)).contains("#767676"));
}

#[test]
fn clidraw_matches_golden() {
    check("sample.clidraw.json", &encode(&sample()).unwrap());
}

#[test]
fn svg_follows_the_free_anchor_cell() {
    let shape = |id: &str, x| {
        Element::Shape(Shape {
            id: Id::from(id),
            kind: ShapeKind::Rect,
            x,
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
        })
    };
    let with = |a| Canvas {
        elements: vec![
            shape("aaaaaaaa", 0),
            shape("bbbbbbbb", 20),
            Element::Line(Line {
                id: Id::from("cccccccc"),
                kind: LineKind::Arrow,
                from: End::Bound(Id::from("aaaaaaaa"), a),
                to: End::Bound(Id::from("bbbbbbbb"), Anchor::Auto),
                label: String::new(),
                label_at: None,
                stroke: Default::default(),
                bends: vec![],
                color: None,
            }),
        ],
        ..Canvas::default()
    };
    assert!(svg(&with(Anchor::Top(1))).contains("d=\"M12 72 L12 56 "));
    assert!(svg(&with(Anchor::Top(4))).contains("d=\"M36 72 L36 56 "));
}
