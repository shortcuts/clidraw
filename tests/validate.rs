use clidraw::diagram::{DiagramKind, Flowchart, Severity};

fn check(text: &str) -> Vec<(Severity, &str)> {
    Flowchart
        .validate(text)
        .into_iter()
        .map(|d| (d.severity, &text[d.range]))
        .collect()
}

#[test]
fn valid_text_is_clean() {
    for text in [
        "flowchart TD\nA --> B",
        "flowchart TD\nA --> B --> C;\n%% note\nA[\"é\"] -->|x| C",
        "graph LR\r\nA --> B\r\n",
        "flowchart TD\nA -- t --> B\nA ==> B\nA & B --> C\nA --o B\nA --x B",
        "flowchart TD\nA --> B\nstyle A fill:#6ea0ff3f,stroke:#ff5f5f\nlinkStyle 0 stroke:#ff5f5f\nclick A \"https://a.test\" _blank",
        "flowchart TD\nA((x))\nB[[y]]\nC>z] --> D[\\w/]",
    ] {
        assert!(check(text).is_empty(), "{text:?}");
    }
}

#[test]
fn unsupported_lines_warn() {
    assert_eq!(
        check("flowchart TD\nA --> B\nsubgraph x"),
        [(Severity::Warning, "subgraph x")]
    );
    assert_eq!(
        check("flowchart TD\nA --> B\n  style A fill:#f00;\n"),
        [(Severity::Warning, "style A fill:#f00;")]
    );
}

#[test]
fn unreadable_lines_error() {
    assert_eq!(
        check("flowchart TD\nA --> B\nA -->\nA --> [\nA o--o B"),
        [
            (Severity::Error, "A -->"),
            (Severity::Error, "A --> ["),
            (Severity::Error, "A o--o B")
        ]
    );
}

#[test]
fn ranges_survive_multibyte_text_and_crlf() {
    assert_eq!(
        check("flowchart TD\r\nA[\"é\"] --> B\r\nsubgraph x\r\n"),
        [(Severity::Warning, "subgraph x")]
    );
}

#[test]
fn other_header_errors_alone() {
    let text = "gantt\ntitle t\nsubgraph x";
    let found = Flowchart.validate(text);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Error);
    assert_eq!(found[0].message, "only flowchart diagrams can be imported");
    assert_eq!(&text[found[0].range.clone()], "gantt");
}

#[test]
fn header_only_warns_and_blank_is_clean() {
    let found = Flowchart.validate("flowchart TD");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Warning);
    assert_eq!(found[0].message, "No nodes yet");
    assert!(check("").is_empty());
    assert!(check(" \n\t\n").is_empty());
}

#[test]
fn too_many_nodes_errors_on_header() {
    let mut text = String::from("flowchart TD\n");
    for i in 0..2001 {
        text += &format!("N{i}\n");
    }
    let found = Flowchart.validate(&text);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].message, "Too many nodes (max 2000)");
    assert_eq!(&text[found[0].range.clone()], "flowchart TD");
}

#[test]
fn skipped_count_matches_statement_diagnostics() {
    for text in [
        "flowchart TD\nA --> B",
        "flowchart TD\nA --> B\nsubgraph x\nend\nA -->\nB ==>",
        "graph LR\n%% c\nstyle A x;\nA --> [\nclass A b\nA --> B;",
    ] {
        let skipped = Flowchart.import(text).unwrap().skipped;
        assert_eq!(Flowchart.validate(text).len(), skipped, "{text:?}");
    }
}

#[test]
fn unregistered_header_gets_one_hint() {
    let text = "gantt\ntitle t\nsubgraph x";
    let found = clidraw::diagram::validate(text);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Error);
    assert_eq!(found[0].message, "`gantt` diagrams are not supported");
    assert_eq!(&text[found[0].range.clone()], "gantt");
}

#[test]
fn registered_header_dispatches_to_its_kind() {
    let text = "graph LR;\nA --> B\nsubgraph x";
    assert_eq!(
        clidraw::diagram::validate(text).len(),
        Flowchart.validate(text).len()
    );
    assert_eq!(clidraw::diagram::validate(text).len(), 1);
    assert!(clidraw::diagram::validate(" \n").is_empty());
}

#[test]
fn import_failure_names_the_line_of_each_hint() {
    let text = "%% c\r\n\r\nflowchart TD\r\nA[\"é\"] --> B\r\nsubgraph x\r\nA -->\r\n";
    assert_eq!(
        clidraw::diagram::import_failure(text, "nothing to import"),
        "nothing to import; line 5: `subgraph` is not supported and is skipped on import; line 6: Cannot read this line: expected nodes joined by arrows"
    );
    assert_eq!(
        clidraw::diagram::import_failure("%%c\n\ngantt\nA->>B", "x"),
        "x; line 3: `gantt` diagrams are not supported"
    );
}

#[test]
fn arrow_fix_offered_only_when_it_parses() {
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
fn arrow_fix_keeps_valid_arrows_in_the_same_line() {
    let d = Flowchart.validate("flowchart TD\nA --> B -> C").remove(0);
    assert_eq!(
        d.fix.map(|f| f.replacement),
        Some("A --> B --> C".to_string())
    );
}

#[test]
fn arrow_fix_replaces_the_range_into_clean_text() {
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

#[test]
fn sequence_and_class_hints() {
    use clidraw::diagram::{Class, Sequence};
    let hints = |kind: &dyn DiagramKind, text: &str| -> Vec<(Severity, String)> {
        kind.validate(text)
            .into_iter()
            .map(|d| (d.severity, text[d.range].to_string()))
            .collect()
    };
    assert!(hints(&Sequence, "sequenceDiagram\nparticipant A\nA->>B: hi").is_empty());
    assert_eq!(
        hints(&Sequence, "sequenceDiagram\nA->>B\nloop x\nA->>A: m\nA B"),
        [
            (Severity::Warning, "loop x".into()),
            (Severity::Warning, "A->>A: m".into()),
            (Severity::Error, "A B".into()),
        ]
    );
    assert_eq!(
        hints(&Sequence, "sequenceDiagram"),
        [(Severity::Warning, "sequenceDiagram".into())]
    );
    assert!(hints(
        &Class,
        "classDiagram\nclass A {\n+x\n}\nA : -->\nA <|-- B : is"
    )
    .is_empty());
    assert_eq!(
        hints(
            &Class,
            "classDiagram\nA --> B\nnote for A \"x\"\nwhat is this"
        ),
        [
            (Severity::Warning, "note for A \"x\"".into()),
            (Severity::Error, "what is this".into()),
        ]
    );
    assert_eq!(
        hints(&Class, "classDiagram"),
        [(Severity::Warning, "classDiagram".into())]
    );
}

#[test]
fn new_kinds_skipped_count_matches_diagnostics() {
    use clidraw::diagram::{Class, Sequence};
    for text in [
        "sequenceDiagram\nA->>B\nNote over A: x\nA->>A: m\nzzz",
        "classDiagram\nA --> B\nnamespace n\nA B C\nclass A {\n<<x>>\n}",
    ] {
        let kind: &dyn DiagramKind = if text.starts_with('s') {
            &Sequence
        } else {
            &Class
        };
        assert_eq!(
            kind.validate(text).len(),
            kind.import(text).map_or(0, |i| i.skipped),
            "{text:?}"
        );
    }
}
