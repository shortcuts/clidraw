use super::import::{parse_line, Node, Rejected, Stmt, MAX_SHAPES};
use crate::diagram::{diagnostic, statements, Diagnostic, Severity};
use std::collections::HashSet;

pub(super) fn validate(text: &str) -> Vec<Diagnostic> {
    let Some((_, head_range)) = statements(text).next() else {
        return vec![];
    };

    let mut out = vec![];
    let mut nodes = HashSet::new();
    for (line, range) in statements(text).skip(1) {
        match parse_line(line) {
            Ok(Stmt::State { id, .. }) => {
                nodes.insert(Node::State(id));
            }
            Ok(Stmt::Transition { from, to, .. }) => {
                nodes.extend([from, to]);
            }
            Err(Rejected::Syntax(word)) => out.push(diagnostic(
                Severity::Error,
                format!("`{word}` cannot be imported"),
                range,
            )),
            Err(Rejected::Unsupported(word)) => out.push(diagnostic(
                Severity::Warning,
                format!("`{word}` is not supported and is skipped on import"),
                range,
            )),
            Err(Rejected::Unparsable) => out.push(diagnostic(
                Severity::Error,
                "Cannot read this line: expected `A --> B: text`".into(),
                range,
            )),
        }
    }
    if nodes.len() > MAX_SHAPES {
        out.push(diagnostic(
            Severity::Error,
            format!("Too many states (max {MAX_SHAPES})"),
            head_range,
        ));
    } else if nodes.is_empty() && out.is_empty() {
        out.push(diagnostic(
            Severity::Warning,
            "No states yet".into(),
            head_range,
        ));
    }
    out
}
