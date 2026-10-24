mod export;
mod import;
mod validate;

use super::{Diagnostic, DiagramKind, Exported, Imported};
use crate::model::Canvas;

pub struct Flowchart;

impl DiagramKind for Flowchart {
    fn import(&self, text: &str) -> Result<Imported, String> {
        import::import(text)
    }

    fn export(&self, canvas: &Canvas) -> Exported {
        export::export(canvas)
    }

    fn validate(&self, text: &str) -> Vec<Diagnostic> {
        validate::validate(text)
    }
}
