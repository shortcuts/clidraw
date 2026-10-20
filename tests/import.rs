use clidraw::import::mermaid;
use clidraw::model::{Border, Dir, Element, LineKind, Shape, ShapeKind, Stroke};

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
    let out = mermaid(&std::fs::read_to_string("tests/golden/sample.mmd").unwrap()).unwrap();
    let s = shapes(&out.canvas);
    let kinds: Vec<_> = s.iter().map(|s| (s.kind, s.border)).collect();
    assert_eq!(
        kinds,
        [
            (ShapeKind::Rect, Border::Single),
            (ShapeKind::Diamond, Border::Single),
            (ShapeKind::Ellipse, Border::Single),
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
    assert_eq!(out.skipped, 3);
}

#[test]
fn chain_makes_one_line_per_arrow() {
    let out = mermaid("graph TD\nA --> B --> C").unwrap();
    assert_eq!(
        (shapes(&out.canvas).len(), lines(&out.canvas).len()),
        (3, 2)
    );
}

#[test]
fn arrow_inside_quotes_does_not_split() {
    let out = mermaid("flowchart TD\nA[\"a --> b\"] --> B").unwrap();
    let s = shapes(&out.canvas);
    assert_eq!((s.len(), s[0].label.as_str()), (2, "a --> b"));
}

#[test]
fn later_definition_names_an_earlier_bare_node() {
    let out = mermaid("flowchart TD\nA --> B\nA{Pick}").unwrap();
    let s = shapes(&out.canvas);
    assert_eq!(
        (s[0].kind, s[0].label.as_str()),
        (ShapeKind::Diamond, "Pick")
    );
}

#[test]
fn other_diagrams_fail() {
    assert!(mermaid("sequenceDiagram\nA->>B: hi").is_err());
}

#[test]
fn lr_lays_out_left_to_right() {
    let out = mermaid("flowchart LR\nA --> B").unwrap();
    assert_eq!(out.dir, Dir::LeftRight);
    let s = shapes(&out.canvas);
    assert!(s[1].x > s[0].x, "{} {}", s[0].x, s[1].x);
}

#[test]
fn comments_are_not_skipped_lines() {
    let out = mermaid("flowchart TD\n%% note\nA --> B\nA -- text --> B").unwrap();
    assert_eq!(out.skipped, 1);
}

#[test]
fn header_only_is_empty() {
    assert_eq!(mermaid("flowchart TD").err().unwrap(), "nothing to import");
}
