use super::import::{parse, Rejected, Stmt, MAX_SHAPES};
use crate::diagram::{diagnostic, statements, Diagnostic, Severity};
use std::collections::HashSet;

pub(super) fn validate(text: &str) -> Vec<Diagnostic> {
    let Some((_, head_range)) = statements(text).next() else {
        return vec![];
    };

    let mut out = vec![];
    let mut ids = HashSet::new();
    for (stmt, range) in parse(text) {
        match stmt {
            Ok(Stmt::Class(class, _) | Stmt::Member(class, _)) => {
                ids.insert(class.id);
            }
            Ok(Stmt::Relation(from, to, ..)) => {
                ids.extend([from.id, to.id]);
            }
            Err(Rejected::Unsupported(word)) => out.push(diagnostic(
                Severity::Warning,
                format!("`{word}` is not supported and is skipped on import"),
                range,
            )),
            Err(Rejected::Unparsable) => out.push(diagnostic(
                Severity::Error,
                "Cannot read this line: expected a class, a member or a relation".into(),
                range,
            )),
        }
    }
    if ids.len() > MAX_SHAPES {
        out.push(diagnostic(
            Severity::Error,
            format!("Too many classes (max {MAX_SHAPES})"),
            head_range,
        ));
    } else if ids.is_empty() && out.is_empty() {
        out.push(diagnostic(
            Severity::Warning,
            "No classes yet".into(),
            head_range,
        ));
    }
    out
}
