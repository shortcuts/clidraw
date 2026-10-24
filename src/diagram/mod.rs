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
    fn import(&self, text: &str) -> Result<Imported, String>;
    fn export(&self, canvas: &Canvas) -> Exported;
    fn validate(&self, text: &str) -> Vec<Diagnostic>;
}
