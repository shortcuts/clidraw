mod import;
mod validate;

use super::{Diagnostic, DiagramKind, Imported};

pub struct Sequence;

impl DiagramKind for Sequence {
    fn keywords(&self) -> &'static [&'static str] {
        &["sequenceDiagram"]
    }

    fn import(&self, text: &str) -> Result<Imported, String> {
        import::import(text)
    }

    fn validate(&self, text: &str) -> Vec<Diagnostic> {
        validate::validate(text)
    }
}
