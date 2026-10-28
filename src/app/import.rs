use super::{expand_home, App};
use crate::diagram::{import_failure, DiagramKind, Flowchart};
use crate::model::{Element, Id, Shape, ShapeKind};
use crate::ui::Status;

impl App {
    pub(super) fn import(&mut self, path: &str) {
        match std::fs::read_to_string(expand_home(path)) {
            Ok(text) => self.import_text(&text),
            Err(e) => self.status = Some(Status::error(format!("Import failed: {e}"))),
        }
    }

    pub(super) fn import_text(&mut self, text: &str) {
        let parsed = Flowchart.import(text);
        self.status = Some(match parsed {
            Ok(input) => {
                let canvas = input.canvas;
                let total = canvas.elements.len();
                let shapes = canvas
                    .elements
                    .iter()
                    .filter(|e| matches!(e, Element::Shape(_)))
                    .count();
                self.insert_framed(canvas);
                let mut text = format!("Imported {shapes} shapes, {} lines", total - shapes);
                if input.skipped > 0 {
                    text += &format!(", skipped {} line(s)", input.skipped);
                }
                Status::notice(text)
            }
            Err(e) => Status::error(format!("Import failed: {}", import_failure(text, &e))),
        });
    }

    pub(super) fn insert_framed(&mut self, canvas: crate::model::Canvas) {
        let ids: Vec<_> = canvas.elements.iter().map(|e| e.id().clone()).collect();
        let (bx, by, bw, bh) = self.extent(&canvas);
        let (vx, vy, ..) = self.session.viewport();
        let (wx, wy) = self.session.view.to_world(vx + 2, vy + 1);
        let (fw, fh) = (bw + 2, bh + 2);
        let (fx, fy) = self.free_spot(wx - 1, wy - 1, fw, fh);
        let mut framed = canvas.clone_canvas(&ids, fx + 1 - bx, fy + 1 - by);
        framed.elements.insert(
            0,
            Element::Shape(Shape {
                id: Id::random(),
                kind: ShapeKind::Frame,
                x: fx,
                y: fy,
                w: fw,
                h: fh,
                label: String::new(),
                stroke: Default::default(),
                border: Default::default(),
                color: None,
                fill: None,
                align: Default::default(),
                bold: false,
                link: None,
                side_labels: vec![],
            }),
        );
        self.session.insert(framed);
    }

    fn extent(&self, canvas: &crate::model::Canvas) -> (i32, i32, i32, i32) {
        let boxes = canvas.elements.iter().map(|e| canvas.bounds(e));
        let (x0, y0, x1, y1) = boxes.fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |m, b| {
            (
                m.0.min(b.0),
                m.1.min(b.1),
                m.2.max(b.0 + b.2),
                m.3.max(b.1 + b.3),
            )
        });
        (x0, y0, x1 - x0, y1 - y0)
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
