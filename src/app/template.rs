use super::App;
use crate::diagram::{DiagramKind, Flowchart};
use crate::model::{Canvas, Element, Id, Paint, Shape, ShapeKind};
use crate::ui::actions::Action;
use crate::ui::list::{List, Row};
use crate::ui::Status;

type Build = fn() -> Result<Canvas, String>;

const TEMPLATES: [(&str, Build); 3] = [
    ("Flowchart", || {
        Flowchart
            .import(include_str!("../../docs/tapes/seed/flow.mmd"))
            .map(|input| input.canvas)
    }),
    ("Architecture dependency", || {
        Flowchart
            .import(include_str!("../../docs/tapes/seed/architecture.mmd"))
            .map(|input| input.canvas)
    }),
    ("Gantt chart", || Ok(gantt())),
];

const LABEL_W: i32 = 16;
const WEEK_W: i32 = 6;
const ROW_H: i32 = 3;
const WEEKS: i32 = 6;
const TASKS: [(&str, i32, i32); 4] = [
    ("Design", 0, 2),
    ("Build", 2, 3),
    ("Test", 5, 1),
    ("Docs", 1, 3),
];

impl App {
    pub(super) fn insert_template(&mut self, name: &str) {
        let Some((_, build)) = TEMPLATES.iter().find(|(n, _)| *n == name) else {
            self.status = Some(Status::error(format!("No template {name}")));
            return;
        };
        self.status = Some(match build() {
            Ok(canvas) => {
                self.insert_framed(canvas);
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
    for (row, &(task, start, weeks)) in TASKS.iter().enumerate() {
        let y = (row as i32 + 1) * ROW_H;
        shapes.push(cell(0, y, LABEL_W, task));
        shapes.push(Shape {
            fill: Some(Paint::Blue),
            ..cell(LABEL_W + start * WEEK_W, y, weeks * WEEK_W, "")
        });
    }
    Canvas {
        elements: shapes.into_iter().map(Element::Shape).collect(),
        ..Canvas::default()
    }
}

fn cell(x: i32, y: i32, w: i32, label: &str) -> Shape {
    Shape {
        id: Id::random(),
        kind: ShapeKind::Rect,
        x,
        y,
        w,
        h: ROW_H,
        label: label.to_string(),
        stroke: Default::default(),
        border: Default::default(),
        color: None,
        fill: None,
        align: Default::default(),
        bold: false,
        link: None,
        side_labels: vec![],
    }
}

pub(super) fn list(title: &'static str) -> List<Action> {
    let rows = TEMPLATES
        .iter()
        .map(|&(name, _)| Row {
            value: Action::Template(name),
            label: name.to_string(),
            key: String::new(),
        })
        .collect();
    List::new(title, rows).with_query().min_width(52)
}
