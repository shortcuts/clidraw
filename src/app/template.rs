use super::App;
use crate::diagram::{DiagramKind, Flowchart};
use crate::model::{Anchor, Canvas, Element, End, Line, LineKind, Paint, Shape, ShapeKind, Stroke};
use crate::ui::actions::Action;
use crate::ui::list::{List, Row};
use crate::ui::Status;

type Build = fn() -> Result<Canvas, String>;

const TEMPLATES: [(&str, Build); 5] = [
    ("Flowchart", || imported(include_str!("templates/flow.mmd"))),
    ("Architecture dependency", || {
        imported(include_str!("templates/architecture.mmd"))
    }),
    ("Gantt chart", || Ok(gantt())),
    ("Sequence diagram", || Ok(sequence_diagram())),
    ("State diagram", || {
        imported(include_str!("templates/state.mmd"))
    }),
];

// Imports stay compact; templates need room for new shapes.
fn imported(src: &str) -> Result<Canvas, String> {
    let mut canvas = Flowchart.import(src)?.canvas;
    for element in &mut canvas.elements {
        if let Element::Shape(shape) = element {
            shape.x *= 2;
            shape.y *= 2;
        }
    }
    Ok(canvas)
}

const LABEL_W: i32 = 16;
const WEEK_W: i32 = 6;
const ROW_H: i32 = 3;
const WEEKS: i32 = 8;
const TASK_ROWS: usize = 6;
const TASKS: [(&str, i32, i32); 4] = [
    ("Design", 0, 2),
    ("Build", 2, 3),
    ("Test", 5, 1),
    ("Docs", 1, 3),
];

const PARTICIPANTS: [&str; 3] = ["Client", "Server", "Database"];
// (from, to, label, stroke) by participant index.
const MESSAGES: [(usize, usize, &str, Stroke); 4] = [
    (0, 1, "request", Stroke::Solid),
    (1, 2, "query", Stroke::Solid),
    (2, 1, "result", Stroke::Dashed),
    (1, 0, "response", Stroke::Dashed),
];
const BOX_W: i32 = 14;
const BOX_H: i32 = 3;
const SPACING: i32 = 24;
const ROW_GAP: i32 = 4;
const SPARE_MESSAGES: i32 = 3;

fn sequence_diagram() -> Canvas {
    let mut elements = vec![];
    let bottom = BOX_H + 2 + ROW_GAP * (MESSAGES.len() as i32 + SPARE_MESSAGES);
    let center = |i: usize| i as i32 * SPACING + BOX_W / 2;
    for (i, name) in PARTICIPANTS.iter().enumerate() {
        let shape = Shape::new(ShapeKind::Rect, i as i32 * SPACING, 0, BOX_W, BOX_H, *name);
        let id = shape.id.clone();
        elements.push(Element::Shape(shape));
        elements.push(Element::Line(Line {
            stroke: Stroke::Dotted,
            ..Line::new(
                LineKind::Line,
                End::Bound(id, Anchor::Auto),
                End::Free(center(i), bottom),
            )
        }));
    }
    for (k, &(from, to, label, stroke)) in MESSAGES.iter().enumerate() {
        let y = BOX_H + 2 + ROW_GAP * k as i32;
        elements.push(Element::Line(Line {
            label: label.to_string(),
            stroke,
            ..Line::new(
                LineKind::Arrow,
                End::Free(center(from), y),
                End::Free(center(to), y),
            )
        }));
    }
    Canvas {
        elements,
        ..Canvas::default()
    }
}

impl App {
    pub(super) fn insert_template(&mut self, name: &str) {
        let Some((_, build)) = TEMPLATES.iter().find(|(n, _)| *n == name) else {
            self.status = Some(Status::error(format!("No template {name}")));
            return;
        };
        self.status = Some(match build() {
            Ok(canvas) => {
                self.insert_framed(canvas, name);
                Status::notice(format!("Inserted template {name}"))
            }
            Err(e) => Status::error(e),
        });
    }
}

fn gantt() -> Canvas {
    let mut shapes = vec![cell(0, 0, LABEL_W, "")];
    shapes.extend((0..WEEKS).map(|week| {
        cell(
            LABEL_W + week * WEEK_W,
            0,
            WEEK_W,
            &format!("W{}", week + 1),
        )
    }));
    for row in 0..TASK_ROWS {
        let y = (row as i32 + 1) * ROW_H;
        let task = TASKS.get(row);
        shapes.push(cell(0, y, LABEL_W, task.map_or("", |t| t.0)));
        if let Some(&(_, start, weeks)) = task {
            shapes.push(Shape {
                fill: Some(Paint::Blue),
                ..cell(LABEL_W + start * WEEK_W, y, weeks * WEEK_W, "")
            });
        }
    }
    Canvas {
        elements: shapes.into_iter().map(Element::Shape).collect(),
        ..Canvas::default()
    }
}

fn cell(x: i32, y: i32, w: i32, label: &str) -> Shape {
    Shape::new(ShapeKind::Rect, x, y, w, ROW_H, label)
}

pub(super) fn list(title: &'static str, blank: bool) -> List<Action> {
    let blank = blank.then(|| Row {
        value: Action::Blank,
        label: "Blank".to_string(),
        key: String::new(),
    });
    let rows = blank
        .into_iter()
        .chain(TEMPLATES.iter().map(|&(name, _)| Row {
            value: Action::Template(name),
            label: name.to_string(),
            key: String::new(),
        }))
        .collect();
    List::new(title, rows).with_query().min_width(52)
}
