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

/// Side of a shape a bound line end leaves from. `Auto` picks the side facing the other end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    #[default]
    Auto,
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Wire", into = "Wire")]
pub enum End {
    Bound(Id, Anchor),
    Free(i32, i32),
}

// serde cannot add an optional field to a tuple variant, and `Auto` must keep the old bytes.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Wire {
    Bound(Id),
    Anchored(Id, Anchor),
    Free(i32, i32),
}

impl From<Wire> for End {
    fn from(w: Wire) -> End {
        match w {
            Wire::Bound(id) => End::Bound(id, Anchor::Auto),
            Wire::Anchored(id, a) => End::Bound(id, a),
            Wire::Free(x, y) => End::Free(x, y),
        }
    }
}

impl From<End> for Wire {
    fn from(e: End) -> Wire {
        match e {
            End::Bound(id, Anchor::Auto) => Wire::Bound(id),
            End::Bound(id, a) => Wire::Anchored(id, a),
            End::Free(x, y) => Wire::Free(x, y),
        }
    }
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
