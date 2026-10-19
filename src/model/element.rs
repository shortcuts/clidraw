use super::Id;
use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Rect,
    Ellipse,
    Diamond,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineKind {
    Line,
    Arrow,
    ArrowStart,
    ArrowBoth,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum End {
    Bound(Id),
    Free(i32, i32),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shape {
    pub id: Id,
    pub kind: ShapeKind,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub id: Id,
    pub kind: LineKind,
    pub from: End,
    pub to: End,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    pub id: Id,
    pub x: i32,
    pub y: i32,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Element {
    Shape(Shape),
    Line(Line),
    Text(Text),
}

impl Element {
    pub fn id(&self) -> &Id {
        match self {
            Element::Shape(s) => &s.id,
            Element::Line(l) => &l.id,
            Element::Text(t) => &t.id,
        }
    }
}

impl Text {
    pub fn width(&self) -> i32 {
        self.text.width().max(1) as i32
    }
}
