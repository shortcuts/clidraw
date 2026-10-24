mod export;
mod import;

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

    fn validate(&self, _text: &str) -> Vec<Diagnostic> {
        // shortcut: no checks yet, validator-behind-the-trait-with-shared-diagnostic-type fills this
        vec![]
    }
}
