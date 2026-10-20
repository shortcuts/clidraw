use super::App;
use crate::import;
use crate::model::Element;
use crate::ui::Status;

impl App {
    pub(super) fn import(&mut self, path: &str) {
        let parsed = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|text| import::mermaid(&text));
        self.status = Some(match parsed {
            Ok(input) => {
                let canvas = input.canvas;
                let ids: Vec<_> = canvas.elements.iter().map(|e| e.id().clone()).collect();
                let shapes = canvas
                    .elements
                    .iter()
                    .filter(|e| matches!(e, Element::Shape(_)))
                    .count();
                let (x, y) = canvas
                    .elements
                    .iter()
                    .filter_map(|e| match e {
                        Element::Shape(s) => Some((s.x, s.y)),
                        _ => None,
                    })
                    .fold((i32::MAX, i32::MAX), |m, p| (m.0.min(p.0), m.1.min(p.1)));
                let (wx, wy) = self.session.view.to_world(2, 1);
                self.session
                    .insert(canvas.clone_canvas(&ids, wx - x, wy - y));
                let mut text = format!("imported {shapes} shapes, {} lines", ids.len() - shapes);
                if input.skipped > 0 {
                    text += &format!(", skipped {} line(s)", input.skipped);
                }
                Status::notice(text)
            }
            Err(e) => Status::error(format!("import failed: {e}")),
        });
    }
}
