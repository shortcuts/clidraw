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
        .import("flowchart TD\n%% note\nA --> B\nA -- text --> B")
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
fn flowchart_validate_reports_nothing_for_valid_text() {
    let kind: &dyn DiagramKind = &Flowchart;
    assert!(kind.validate("flowchart TD\nA --> B").is_empty());
}

#[test]
fn flowchart_validate_offers_an_arrow_fix_only_when_it_parses() {
    let fix = |t: &str| {
        let d = Flowchart.validate(t).remove(0);
        d.fix.map(|f| (f.label, f.replacement))
    };
    let fixed = |r: &str| Some(("Use -->".to_string(), r.to_string()));
    assert_eq!(fix("flowchart TD\nA -> B"), fixed("A --> B"));
    assert_eq!(fix("flowchart TD\nA => B"), fixed("A --> B"));
    assert_eq!(
        fix("flowchart TD\nA <-> B"),
        Some(("Use <-->".into(), "A <--> B".into()))
    );
    assert_eq!(fix("flowchart TD\nA ->"), None);
}

#[test]
fn flowchart_arrow_fix_keeps_valid_arrows_in_the_same_line() {
    let d = Flowchart.validate("flowchart TD\nA --> B -> C").remove(0);
    assert_eq!(
        d.fix.map(|f| f.replacement),
        Some("A --> B --> C".to_string())
    );
}

#[test]
fn flowchart_arrow_fix_replaces_the_range_into_clean_text() {
    let text = "flowchart TD\r\n  A -> B\r\n";
    let d = Flowchart.validate(text).remove(0);
    let fixed = format!(
        "{}{}{}",
        &text[..d.range.start],
        d.fix.unwrap().replacement,
        &text[d.range.end..]
    );
    assert_eq!(fixed, "flowchart TD\r\n  A --> B\r\n");
    assert!(Flowchart.validate(&fixed).is_empty());
}
