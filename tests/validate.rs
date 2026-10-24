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
        check("flowchart TD\nA --> B\nA -->\nA --> ["),
        [(Severity::Error, "A -->"), (Severity::Error, "A --> [")]
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
    let text = "sequenceDiagram\nA->>B: hi\nsubgraph x";
    let found = Flowchart.validate(text);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Error);
    assert_eq!(found[0].message, "only flowchart diagrams can be imported");
    assert_eq!(&text[found[0].range.clone()], "sequenceDiagram");
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
        "flowchart TD\nA --> B\nsubgraph x\nend\nA -->\nB -- t --> C",
        "graph LR\n%% c\nstyle A x;\nA --> [\nclass A b\nA --> B;",
    ] {
        let skipped = Flowchart.import(text).unwrap().skipped;
        assert_eq!(Flowchart.validate(text).len(), skipped, "{text:?}");
    }
}

#[test]
fn unregistered_header_gets_one_hint() {
    let text = "sequenceDiagram\nA->>B: hi\nsubgraph x";
    let found = clidraw::diagram::validate(text);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Error);
    assert_eq!(
        found[0].message,
        "`sequenceDiagram` diagrams are not supported"
    );
    assert_eq!(&text[found[0].range.clone()], "sequenceDiagram");
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
