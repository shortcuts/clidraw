mod import;
mod validate;

use super::{Diagnostic, DiagramKind, Imported};

pub struct State;

impl DiagramKind for State {
    fn keywords(&self) -> &'static [&'static str] {
        &["stateDiagram", "stateDiagram-v2"]
    }

    fn import(&self, text: &str) -> Result<Imported, String> {
        import::import(text)
    }

    fn validate(&self, text: &str) -> Vec<Diagnostic> {
        validate::validate(text)
    }
}
