use super::import::{parse_line, Rejected, Stmt, MAX_SHAPES};
use crate::diagram::{diagnostic, statements, Diagnostic, Severity};
use std::collections::HashSet;

pub(super) fn validate(text: &str) -> Vec<Diagnostic> {
    let mut lines = statements(text);
    let Some((_, head_range)) = lines.next() else {
        return vec![];
    };

    let mut out = vec![];
    let mut ids = HashSet::new();
    let mut messages = 0;
    for (line, range) in lines {
        match parse_line(line) {
            Ok(Stmt::Participant { id, .. }) => {
                ids.insert(id);
            }
            Ok(Stmt::Message { from, to, .. }) => {
                ids.extend([from, to]);
                messages += 1;
            }
            Err(Rejected::Unsupported(word)) => out.push(diagnostic(
                Severity::Warning,
                format!("`{word}` is not supported and is skipped on import"),
                range,
            )),
            Err(Rejected::Unparsable) => out.push(diagnostic(
                Severity::Error,
                "Cannot read this line: expected `A->>B: text`".into(),
                range,
            )),
        }
    }
    if ids.len() + messages > MAX_SHAPES {
        out.push(diagnostic(
            Severity::Error,
            format!("Too many participants and messages (max {MAX_SHAPES})"),
            head_range,
        ));
    } else if ids.is_empty() && out.is_empty() {
        out.push(diagnostic(
            Severity::Warning,
            "No participants yet".into(),
            head_range,
        ));
    }
    out
}
