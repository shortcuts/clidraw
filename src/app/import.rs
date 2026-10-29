use super::{template, App};
use crate::config::expand_home;
use crate::diagram::{self, import_failure};
use crate::model::Element;
use crate::ui::Status;

impl App {
    pub(super) fn import(&mut self, path: &str) {
        match std::fs::read_to_string(expand_home(path)) {
            Ok(text) => {
                let name = std::path::Path::new(path)
                    .file_name()
                    .map_or(path.into(), |n| n.to_string_lossy());
                self.import_text(&text, &name)
            }
            Err(e) => self.status = Some(Status::error(format!("Import failed: {e}"))),
        }
    }

    pub(super) fn import_text(&mut self, text: &str, name: &str) {
        let parsed = diagram::import(text);
        self.status = Some(match parsed {
            Ok(input) => {
                let canvas = input.canvas;
                let total = canvas.elements.len();
                let shapes = canvas
                    .elements
                    .iter()
                    .filter(|e| matches!(e, Element::Shape(_)))
                    .count();
                self.insert_framed(canvas, name);
                let mut text = format!("Imported {shapes} shapes, {} lines", total - shapes);
                if input.skipped > 0 {
                    text += &format!(", skipped {} line(s)", input.skipped);
                }
                Status::notice(text)
            }
            Err(e) => Status::error(format!("Import failed: {}", import_failure(text, &e))),
        });
    }

    pub(super) fn insert_framed(&mut self, canvas: crate::model::Canvas, name: &str) {
        let canvas = template::pinned(canvas);
        let (fw, fh) = template::frame_size(&canvas);
        let (vx, vy, ..) = self.session.viewport();
        let (wx, wy) = self.session.view.to_world(vx + 2, vy + 1);
        let (fx, fy) = self.free_spot(wx - 1, wy - 1, fw, fh);
        self.session.insert(template::framed(canvas, name, fx, fy));
    }

    /// Moves the rectangle down past every element it overlaps; each step raises `y`, so it ends.
    fn free_spot(&self, x: i32, mut y: i32, w: i32, h: i32) -> (i32, i32) {
        let canvas = &self.session.canvas;
        let find = canvas.shape_lookup();
        loop {
            let bottom = canvas
                .elements
                .iter()
                .map(|e| canvas.bounds_with(e, &find))
                .filter(|b| b.0 < x + w && x < b.0 + b.2 && b.1 < y + h && y < b.1 + b.3)
                .map(|b| b.1 + b.3)
                .max();
            match bottom {
                Some(b) => y = b + 1,
                None => return (x, y),
            }
        }
    }
}
