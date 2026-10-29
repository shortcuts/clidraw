mod import;
mod validate;

use super::{Diagnostic, DiagramKind, Imported};

pub struct Class;

impl DiagramKind for Class {
    fn keywords(&self) -> &'static [&'static str] {
        &["classDiagram"]
    }

    fn import(&self, text: &str) -> Result<Imported, String> {
        import::import(text)
    }

    fn validate(&self, text: &str) -> Vec<Diagnostic> {
        validate::validate(text)
    }
}
