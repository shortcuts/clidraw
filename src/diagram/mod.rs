mod flowchart;

pub use flowchart::Flowchart;

use crate::model::Canvas;
use std::ops::Range;

pub struct Imported {
    pub canvas: Canvas,
    pub skipped: usize,
}

pub struct Exported {
    pub text: String,
    pub skipped: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// Replaces the diagnostic's `range` with `replacement`.
#[derive(Debug)]
pub struct Fix {
    pub label: String,
    pub replacement: String,
}

#[derive(Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    /// Byte offsets into the validated text.
    pub range: Range<usize>,
    pub fix: Option<Fix>,
}

pub trait DiagramKind {
    /// First words of the header lines this kind reads.
    fn keywords(&self) -> &'static [&'static str];
    fn import(&self, text: &str) -> Result<Imported, String>;
    fn export(&self, canvas: &Canvas) -> Exported;
    fn validate(&self, text: &str) -> Vec<Diagnostic>;
}

const KINDS: [&dyn DiagramKind; 1] = [&Flowchart];

/// Non-blank, non-comment lines, trimmed, with their byte range in `text`.
fn statements(text: &str) -> impl Iterator<Item = (&str, Range<usize>)> {
    let mut offset = 0;
    text.split_inclusive('\n').filter_map(move |raw| {
        let start = offset + raw.len() - raw.trim_start().len();
        offset += raw.len();
        let line = raw.trim();
        (!line.is_empty() && !line.starts_with("%%")).then(|| (line, start..start + line.len()))
    })
}

/// Validates `text` with the kind its header names; an unregistered header yields one hint.
pub fn validate(text: &str) -> Vec<Diagnostic> {
    let Some((head, range)) = statements(text).next() else {
        return vec![];
    };
    let word = head
        .trim_end_matches(';')
        .split_whitespace()
        .next()
        .unwrap_or("");
    match KINDS.iter().find(|k| k.keywords().contains(&word)) {
        Some(kind) => kind.validate(text),
        None => vec![Diagnostic {
            severity: Severity::Error,
            message: format!("`{word}` diagrams are not supported"),
            range,
            fix: None,
        }],
    }
}

/// Appends the live-hint diagnostics of `text` to why its import failed.
pub fn import_failure(text: &str, reason: &str) -> String {
    validate(text).iter().fold(reason.to_string(), |out, d| {
        let line = text[..d.range.start].matches('\n').count() + 1;
        format!("{out}; line {line}: {}", d.message)
    })
}
