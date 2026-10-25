use super::import::{header, parse_line, Rejected, Stmt, MAX_SHAPES};
use crate::diagram::{statements, Diagnostic, Fix, Severity};
use std::collections::HashSet;
use std::ops::Range;

fn diagnostic(severity: Severity, message: String, range: Range<usize>) -> Diagnostic {
    Diagnostic {
        severity,
        message,
        range,
        fix: None,
    }
}

fn replace_arrow(line: &str, bad: &str, good: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    for (at, _) in line.match_indices(bad) {
        // `-->` contains `->`: skip the tail of a valid arrow
        if line[..at].ends_with(&bad[..1]) {
            continue;
        }
        out += &line[last..at];
        out += good;
        last = at + bad.len();
    }
    out + &line[last..]
}

/// The broken arrow, its replacement and the fixed line, when the fixed line parses.
fn arrow_fix(line: &str) -> Option<(&'static str, &'static str, String)> {
    // shortcut: replaces every occurrence, even inside quotes; parse_line vets the result, upgrade if a valid label ever loses text
    [("<->", "<-->"), ("->", "-->"), ("=>", "-->")]
        .into_iter()
        .filter(|(bad, _)| line.contains(bad))
        .map(|(bad, good)| (bad, good, replace_arrow(line, bad, good)))
        .find(|(.., fixed)| parse_line(fixed).is_ok())
}

pub(super) fn validate(text: &str) -> Vec<Diagnostic> {
    let mut lines = statements(text);
    let Some((head, head_range)) = lines.next() else {
        return vec![];
    };
    if let Err(message) = header(head) {
        return vec![diagnostic(Severity::Error, message, head_range)];
    }

    let mut out = vec![];
    let mut ids = HashSet::new();
    for (line, range) in lines {
        match parse_line(line) {
            Ok(Stmt::Chain(groups, _)) => ids.extend(groups.into_iter().flatten().map(|n| n.id)),
            Ok(
                Stmt::Style {
                    unmatched: true, ..
                }
                | Stmt::LinkStyle {
                    unmatched: true, ..
                },
            ) => out.push(diagnostic(
                Severity::Warning,
                "Only fill, stroke and color set to a clidraw palette color are imported".into(),
                range,
            )),
            Ok(_) => {}
            Err(Rejected::Unsupported(word)) => out.push(diagnostic(
                Severity::Warning,
                format!("`{word}` is not supported and is skipped on import"),
                range,
            )),
            Err(Rejected::Unparsable) => out.push(match arrow_fix(line) {
                Some((bad, good, replacement)) => Diagnostic {
                    fix: Some(Fix {
                        label: format!("Use {good}"),
                        replacement,
                    }),
                    ..diagnostic(Severity::Error, format!("`{bad}` is not an arrow"), range)
                },
                None => diagnostic(
                    Severity::Error,
                    "Cannot read this line: expected nodes joined by arrows".into(),
                    range,
                ),
            }),
        }
    }
    if ids.len() > MAX_SHAPES {
        out.push(diagnostic(
            Severity::Error,
            format!("Too many nodes (max {MAX_SHAPES})"),
            head_range,
        ));
    } else if ids.is_empty() && out.is_empty() {
        out.push(diagnostic(
            Severity::Warning,
            "No nodes yet".into(),
            head_range,
        ));
    }
    out
}
