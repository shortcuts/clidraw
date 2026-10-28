use super::App;
use crate::diagram::{DiagramKind, Flowchart};
use crate::ui::actions::Action;
use crate::ui::list::{List, Row};
use crate::ui::Status;

const TEMPLATES: [(&str, &str); 1] =
    [("Flowchart", include_str!("../../docs/tapes/seed/flow.mmd"))];

impl App {
    pub(super) fn insert_template(&mut self, name: &str) {
        let Some((_, src)) = TEMPLATES.iter().find(|(n, _)| *n == name) else {
            self.status = Some(Status::error(format!("No template {name}")));
            return;
        };
        self.status = Some(match Flowchart.import(src) {
            Ok(input) => {
                self.insert_framed(input.canvas);
                Status::notice(format!("Inserted template {name}"))
            }
            Err(e) => Status::error(e),
        });
    }
}

pub(super) fn list() -> List<Action> {
    let rows = TEMPLATES
        .iter()
        .map(|&(name, _)| Row {
            value: Action::Template(name),
            label: name.to_string(),
            key: String::new(),
        })
        .collect();
    List::new("Templates", rows).with_query().min_width(52)
}
