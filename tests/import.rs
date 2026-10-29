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
fn stadium_imports_and_exports_as_stadium() {
    let out = Flowchart.import("graph TD\nA([hi])").unwrap();
    let s = shapes(&out.canvas);
    assert_eq!(
        (s.len(), s[0].kind, s[0].label.as_str()),
        (1, ShapeKind::Stadium, "hi")
    );
    let text = Flowchart.export(&out.canvas).text;
    assert!(text.contains("n0([\"hi\"])"), "{text}");
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
            (ShapeKind::Subroutine, Border::Single, "Sub"),
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
    assert!(Flowchart.import("gantt\ntitle t").is_err());
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

use clidraw::diagram::{Class, Sequence, State};

#[test]
fn sequence_draws_boxes_lifelines_and_messages() {
    let out = Sequence
        .import("sequenceDiagram\nparticipant A as Alice\nA->>B: hi\nB-->>A: back\nB->A: x\nA--xB")
        .unwrap();
    let (s, l) = (shapes(&out.canvas), lines(&out.canvas));
    assert_eq!(out.skipped, 0);
    assert_eq!(
        s.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(),
        ["Alice", "B"]
    );
    assert_eq!(l.len(), 6);
    assert!(l[..2].iter().all(|l| l.stroke == Stroke::Dotted));
    let got: Vec<_> = l[2..]
        .iter()
        .map(|l| (l.label.as_str(), l.kind, l.stroke))
        .collect();
    assert_eq!(
        got,
        [
            ("hi", LineKind::Arrow, Stroke::Solid),
            ("back", LineKind::Arrow, Stroke::Dashed),
            ("x", LineKind::Line, Stroke::Solid),
            ("", LineKind::Arrow, Stroke::Dashed),
        ]
    );
}

#[test]
fn sequence_message_runs_between_lifeline_columns() {
    let out = Sequence.import("sequenceDiagram\nA->>+B: go").unwrap();
    let l = lines(&out.canvas);
    // boxes are 14 wide with a gap of 10, so the centers are 7 and 31
    assert_eq!(
        (l[2].from.clone(), l[2].to.clone()),
        (
            clidraw::model::End::Free(7, 5),
            clidraw::model::End::Free(31, 5)
        )
    );
}

#[test]
fn sequence_skips_blocks_and_self_messages() {
    let out = Sequence
        .import("sequenceDiagram\nA->>B: hi\nNote over A: x\nloop t\nA->>A: me\nend\nnonsense")
        .unwrap();
    assert_eq!(out.skipped, 5);
    assert_eq!(lines(&out.canvas).len(), 3);
}

#[test]
fn sequence_without_participants_fails() {
    assert!(Sequence.import("sequenceDiagram\nNote over A: x").is_err());
    assert!(Sequence.import("classDiagram\nA->>B").is_err());
}

fn endpoints(c: &clidraw::model::Canvas, l: &clidraw::model::Line) -> (String, String) {
    let label = |end: &clidraw::model::End| match end {
        clidraw::model::End::Bound(id, _) => match c.elements.iter().find(|e| e.id() == id) {
            Some(Element::Shape(s)) => s.label.lines().next().unwrap().to_string(),
            _ => unreachable!(),
        },
        clidraw::model::End::Free(..) => unreachable!(),
    };
    (label(&l.from), label(&l.to))
}

#[test]
fn class_members_sit_under_a_divider() {
    let out = Class
        .import("classDiagram\nclass B {\n+go()\n\n-x: int\n}\nA : +id\nA <|-- B")
        .unwrap();
    let s = shapes(&out.canvas);
    assert_eq!(s[0].label, "B\n───────\n+go()\n-x: int");
    assert_eq!(s[1].label, "A\n───\n+id");
    assert_eq!(out.skipped, 0);
}

#[test]
fn class_relations_point_at_the_marker() {
    let out = Class
        .import(
            "classDiagram\nA <|-- B\nC --> D\nE ..|> F\nG -- H\nI \"1\" *-- \"*\" J : owns\nK ..> L\nFoo--Bar",
        )
        .unwrap();
    let l = lines(&out.canvas);
    let got: Vec<_> = l
        .iter()
        .map(|l| {
            let (a, b) = endpoints(&out.canvas, l);
            (a, b, l.kind, l.stroke, l.label.clone())
        })
        .collect();
    let e = |a: &str, b: &str, k, s, t: &str| (a.to_string(), b.to_string(), k, s, t.to_string());
    assert_eq!(
        got,
        [
            e("B", "A", LineKind::Arrow, Stroke::Solid, ""),
            e("C", "D", LineKind::Arrow, Stroke::Solid, ""),
            e("E", "F", LineKind::Arrow, Stroke::Dotted, ""),
            e("G", "H", LineKind::Line, Stroke::Solid, ""),
            e("J", "I", LineKind::Arrow, Stroke::Solid, "owns"),
            e("K", "L", LineKind::Arrow, Stroke::Dotted, ""),
            e("Foo", "Bar", LineKind::Line, Stroke::Solid, ""),
        ]
    );
}

#[test]
fn class_member_may_contain_an_arrow() {
    let out = Class.import("classDiagram\nA : -->\nA --> B").unwrap();
    assert_eq!(shapes(&out.canvas)[0].label, "A\n───\n-->");
    assert_eq!(out.skipped, 0);
}

#[test]
fn class_skips_unsupported_and_unreadable_lines() {
    let out = Class
        .import("classDiagram\nA --> B\nnamespace n\ndirection LR\n<<interface>> A\nclass A {\n<<enum>>\nx\n}\nwhat is this")
        .unwrap();
    assert_eq!(out.skipped, 5);
    assert_eq!(shapes(&out.canvas)[0].label, "A\n─\nx");
}

#[test]
fn class_unclosed_block_keeps_members() {
    let out = Class.import("classDiagram\nclass A {\n+x").unwrap();
    assert_eq!(shapes(&out.canvas)[0].label, "A\n──\n+x");
}

#[test]
fn class_with_no_classes_fails() {
    assert!(Class.import("classDiagram\ntitle t").is_err());
}

#[test]
fn dispatch_picks_the_kind_by_header() {
    let import = |t| clidraw::diagram::import(t);
    assert_eq!(
        shapes(&import("classDiagram\nA --> B").unwrap().canvas).len(),
        2
    );
    assert_eq!(
        shapes(&import("sequenceDiagram\nA->>B").unwrap().canvas).len(),
        2
    );
    for text in ["stateDiagram\nA --> B", "stateDiagram-v2\nA --> B"] {
        assert_eq!(shapes(&import(text).unwrap().canvas).len(), 2);
    }
    assert_eq!(
        shapes(&import("graph TD\nA --> B").unwrap().canvas).len(),
        2
    );
    assert_eq!(
        import("gantt\ntitle t").err().unwrap(),
        "only flowchart, sequence, class and state diagrams can be imported"
    );
    assert_eq!(import("%% c\n").err().unwrap(), "nothing to import");
}

const TASK_STATUS: &str = "stateDiagram-v2
  [*] --> pending: backlog add
  pending --> in_progress: task-next claims, attempts + 1
  in_progress --> [*]: STATUS SUCCESS, recorded, entry removed
  in_progress --> failed: STATUS FAILED, debug pass used, loop moves on
  in_progress --> blocked: STATUS BLOCKED
  in_progress --> pending: no STATUS line or dirty tree (wip commit), retry
  failed --> in_progress: debug pass, then re-run
  blocked --> in_progress: user settles the question
  pending --> blocked: dependency unresolved or MAX_ATTEMPTS
  failed --> [*]: reconcile lands its branch, Finish entry filed
  blocked --> [*]: reconcile lands its branch, Finish entry filed
";

#[test]
fn state_draws_rounded_states_markers_and_labeled_arrows() {
    let out = State.import(TASK_STATUS).unwrap();
    let (s, l) = (shapes(&out.canvas), lines(&out.canvas));
    assert_eq!((s.len(), l.len(), out.skipped), (6, 11, 0));
    let labels: Vec<_> = s.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels.iter().filter(|&&l| l == "●").count(), 1);
    assert_eq!(labels.iter().filter(|&&l| l == "◉").count(), 1);
    assert!(s.iter().all(|s| s.border == Border::Rounded));
    assert!(l
        .iter()
        .all(|l| l.kind == LineKind::Arrow && !l.label.is_empty()));
}

#[test]
fn state_labels_come_from_state_and_colon_forms() {
    let out = State
        .import("stateDiagram\nstate \"Long name\" as a\nb : Bee\na --> b\nstate c")
        .unwrap();
    let got: Vec<_> = shapes(&out.canvas)
        .iter()
        .map(|s| s.label.clone())
        .collect();
    assert_eq!(got, ["Long name", "Bee", "c"]);
}

#[test]
fn state_rejects_composite_note_and_concurrency() {
    for line in ["state A {", "note left of A: x", "--"] {
        let text = format!("stateDiagram-v2\nA --> B\n{line}");
        assert_eq!(
            State.import(&text).err().unwrap(),
            "composite states, notes and concurrency cannot be imported",
            "{line:?}"
        );
    }
}

#[test]
fn state_skips_unsupported_and_unreadable_lines() {
    let out = State
        .import("stateDiagram-v2\nA --> B\ndirection LR\nA --> A\nwhat is this\nstate X <<choice>>")
        .unwrap();
    assert_eq!((shapes(&out.canvas).len(), out.skipped), (2, 4));
}

#[test]
fn state_with_no_states_fails() {
    assert_eq!(
        State.import("stateDiagram-v2\ndirection LR").err().unwrap(),
        "nothing to import"
    );
}

#[test]
fn subroutine_round_trips_through_export() {
    let out = Flowchart.import("graph TD\nA[[Sub]]").unwrap();
    let text = Flowchart.export(&out.canvas).text;
    assert!(text.contains("n0[[\"Sub\"]]"), "{text}");
}
