use clidraw::diagram::{DiagramKind, Flowchart};
use clidraw::model::{Border, Element, LineKind, Paint, Shape, ShapeKind, Stroke};

fn shapes(c: &clidraw::model::Canvas) -> Vec<&Shape> {
    c.elements
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

fn lines(c: &clidraw::model::Canvas) -> Vec<&clidraw::model::Line> {
    c.elements
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
fn sample_round_trip() {
    let out = Flowchart
        .import(&std::fs::read_to_string("tests/golden/sample.mmd").unwrap())
        .unwrap();
    let s = shapes(&out.canvas);
    let kinds: Vec<_> = s.iter().map(|s| (s.kind, s.border)).collect();
    assert_eq!(
        kinds,
        [
            (ShapeKind::Rect, Border::Single),
            (ShapeKind::Diamond, Border::Single),
            (ShapeKind::Rect, Border::Single),
            (ShapeKind::Rect, Border::Rounded),
            (ShapeKind::Cylinder, Border::Single),
            (ShapeKind::Parallelogram, Border::Single),
            (ShapeKind::Hexagon, Border::Single),
        ]
    );
    assert_eq!(s[2].label, "Done & \"dusted\"");
    assert_eq!(s[3].label, "alpha beta gamma\ndelta");
    let l = lines(&out.canvas);
    assert_eq!(l.len(), 4);
    assert_eq!(l[0].label, "go");
    assert_eq!((l[1].kind, l[1].stroke), (LineKind::Line, Stroke::Dotted));
    assert_eq!(l[3].kind, LineKind::ArrowBoth);
    assert_eq!(out.skipped, 0);
    assert_eq!(
        (s[0].fill, s[0].color),
        (Some(Paint::Blue), Some(Paint::Red))
    );
    assert_eq!((s[6].fill, s[6].color), (Some(Paint::Yellow), None));
    assert_eq!(s[0].link.as_deref(), Some("https://example.com/a?x=1&y=2"));
    assert_eq!(l[0].color, Some(Paint::Red));
}

#[test]
fn off_palette_style_keeps_the_match_and_counts_skipped() {
    let out = Flowchart
        .import("flowchart TD\nA --> B\nstyle A fill:#123456,stroke:#ff5f5f\nstyle Z fill:#ff5f5f")
        .unwrap();
    let s = shapes(&out.canvas);
    assert_eq!((s[0].fill, s[0].color), (None, Some(Paint::Red)));
    assert_eq!(out.skipped, 2);
}

#[test]
fn chain_makes_one_line_per_arrow() {
    let out = Flowchart.import("graph TD\nA --> B --> C").unwrap();
    assert_eq!(
        (shapes(&out.canvas).len(), lines(&out.canvas).len()),
        (3, 2)
    );
}

/// `(from label, to label, kind, stroke, line label)` per line, in order.
fn edges(c: &clidraw::model::Canvas) -> Vec<(String, String, LineKind, Stroke, String)> {
    use clidraw::model::End;
    let name = |end: &End| match end {
        End::Bound(id, _) => shapes(c)
            .iter()
            .find(|s| &s.id == id)
            .unwrap()
            .label
            .clone(),
        _ => unreachable!(),
    };
    lines(c)
        .iter()
        .map(|l| {
            (
                name(&l.from),
                name(&l.to),
                l.kind,
                l.stroke,
                l.label.clone(),
            )
        })
        .collect()
}

fn edge(
    a: &str,
    b: &str,
    kind: LineKind,
    stroke: Stroke,
    text: &str,
) -> (String, String, LineKind, Stroke, String) {
    (a.into(), b.into(), kind, stroke, text.into())
}

#[test]
fn edge_text_forms_label_the_line() {
    let out = Flowchart
        .import("graph TD\nA -- text --> B\nA -. t .-> C\nA == u ==> D\nA -- v --- E")
        .unwrap();
    assert_eq!(out.skipped, 0);
    assert_eq!(
        edges(&out.canvas),
        [
            edge("A", "B", LineKind::Arrow, Stroke::Solid, "text"),
            edge("A", "C", LineKind::Arrow, Stroke::Dotted, "t"),
            edge("A", "D", LineKind::Arrow, Stroke::Solid, "u"),
            edge("A", "E", LineKind::Line, Stroke::Solid, "v"),
        ]
    );
}

#[test]
fn thick_arrows_import_as_solid() {
    let out = Flowchart
        .import("graph TD\nA ==> B\nA === C\nA <==> D")
        .unwrap();
    assert_eq!(out.skipped, 0);
    assert_eq!(
        edges(&out.canvas),
        [
            edge("A", "B", LineKind::Arrow, Stroke::Solid, ""),
            edge("A", "C", LineKind::Line, Stroke::Solid, ""),
            edge("A", "D", LineKind::ArrowBoth, Stroke::Solid, ""),
        ]
    );
}

#[test]
fn ampersand_fans_out_in_mermaid_order() {
    let out = Flowchart
        .import("graph TD\nA & B --> C\nE --> F & G\nH & I --> J & K")
        .unwrap();
    assert_eq!(out.skipped, 0);
    let pairs: Vec<_> = edges(&out.canvas).into_iter().map(|e| (e.0, e.1)).collect();
    let want = [
        ("A", "C"),
        ("B", "C"),
        ("E", "F"),
        ("E", "G"),
        ("H", "J"),
        ("H", "K"),
        ("I", "J"),
        ("I", "K"),
    ];
    assert_eq!(pairs, want.map(|(a, b)| (a.to_string(), b.to_string())));
    assert_eq!(shapes(&out.canvas).len(), 10);
}

#[test]
fn circle_and_cross_heads_import_as_arrows() {
    let out = Flowchart
        .import("graph TD\nA --o B\nA --x C\nA---oD")
        .unwrap();
    assert_eq!(out.skipped, 0);
    assert_eq!(
        edges(&out.canvas),
        [
            edge("A", "B", LineKind::Arrow, Stroke::Solid, ""),
            edge("A", "C", LineKind::Arrow, Stroke::Solid, ""),
            edge("A", "oD", LineKind::Line, Stroke::Solid, ""),
        ]
    );
}

#[test]
fn stadium_imports_as_rounded_rect() {
    let out = Flowchart.import("graph TD\nA([hi])").unwrap();
    let s = shapes(&out.canvas);
    assert_eq!(
        (s.len(), s[0].kind, s[0].border, s[0].label.as_str()),
        (1, ShapeKind::Rect, Border::Rounded, "hi")
    );
}

#[test]
fn doubled_and_slanted_openers_keep_only_the_inner_text() {
    let out = Flowchart
        .import("graph TD\nA((Circle))\nB[[Sub]]\nC>Flag] --> D[\\Trap/]")
        .unwrap();
    let got: Vec<_> = shapes(&out.canvas)
        .iter()
        .map(|s| (s.kind, s.border, s.label.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            (ShapeKind::Rect, Border::Rounded, "Circle"),
            (ShapeKind::Rect, Border::Double, "Sub"),
            (ShapeKind::Rect, Border::Single, "Flag"),
            (ShapeKind::Parallelogram, Border::Single, "Trap"),
        ]
    );
    assert_eq!(lines(&out.canvas).len(), 1);
}

#[test]
fn arrow_inside_quotes_does_not_split() {
    let out = Flowchart
        .import("flowchart TD\nA[\"a --> b\"] --> B")
        .unwrap();
    let s = shapes(&out.canvas);
    assert_eq!((s.len(), s[0].label.as_str()), (2, "a --> b"));
}

#[test]
fn later_definition_names_an_earlier_bare_node() {
    let out = Flowchart.import("flowchart TD\nA --> B\nA{Pick}").unwrap();
    let s = shapes(&out.canvas);
    assert_eq!(
        (s[0].kind, s[0].label.as_str()),
        (ShapeKind::Diamond, "Pick")
    );
}

#[test]
fn other_diagrams_fail() {
    assert!(Flowchart.import("sequenceDiagram\nA->>B: hi").is_err());
}

#[test]
fn lr_lays_out_left_to_right() {
    let out = Flowchart.import("flowchart LR\nA --> B").unwrap();
    let s = shapes(&out.canvas);
    assert!(s[1].x > s[0].x, "{} {}", s[0].x, s[1].x);
}

#[test]
fn comments_are_not_skipped_lines() {
    let out = Flowchart
        .import("flowchart TD\n%% note\nA --> B\nA ==>")
        .unwrap();
    assert_eq!(out.skipped, 1);
}

#[test]
fn header_only_is_empty() {
    assert_eq!(
        Flowchart.import("flowchart TD").err().unwrap(),
        "nothing to import"
    );
}

#[test]
fn wide_label_sizes_shape_by_cell_width() {
    let out = Flowchart
        .import("flowchart LR\n  A[日本語ラベル]\n")
        .unwrap();
    assert_eq!(shapes(&out.canvas)[0].w, 18);
}
