use super::Id;
use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Rect,
    Ellipse,
    Diamond,
    Cylinder,
    Parallelogram,
    Hexagon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineKind {
    Line,
    Arrow,
    ArrowStart,
    ArrowBoth,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stroke {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

impl Stroke {
    fn is_solid(&self) -> bool {
        *self == Stroke::Solid
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Routing {
    #[default]
    Elbow,
    Straight,
}

impl Routing {
    fn is_elbow(&self) -> bool {
        *self == Routing::Elbow
    }
}

/// Border glyph set of a rect or ellipse. `Single` keeps the kind's own corners.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Border {
    #[default]
    Single,
    Rounded,
    Double,
    Heavy,
}

impl Border {
    fn is_single(&self) -> bool {
        *self == Border::Single
    }
}

/// Horizontal placement of a shape label inside its box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    Left,
    #[default]
    Center,
    Right,
}

impl TextAlign {
    fn is_center(&self) -> bool {
        *self == TextAlign::Center
    }

    pub fn next(self) -> TextAlign {
        match self {
            TextAlign::Left => TextAlign::Center,
            TextAlign::Center => TextAlign::Right,
            TextAlign::Right => TextAlign::Left,
        }
    }

    /// Cells between the left edge of a `room`-wide line box and a `used`-wide line.
    pub fn offset(self, room: usize, used: usize) -> i32 {
        let free = room.saturating_sub(used) as i32;
        match self {
            TextAlign::Left => 0,
            TextAlign::Center => free / 2,
            TextAlign::Right => free,
        }
    }
}

/// Named ink and fill color. The terminal and both exporters read the same RGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Paint {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Violet,
}

/// Opacity of a fill over the page, in percent.
pub const FILL_ALPHA: u8 = 25;

impl Paint {
    pub const ALL: [Paint; 6] = [
        Paint::Red,
        Paint::Orange,
        Paint::Yellow,
        Paint::Green,
        Paint::Blue,
        Paint::Violet,
    ];

    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            Paint::Red => (255, 95, 95),
            Paint::Orange => (255, 160, 60),
            Paint::Yellow => (240, 220, 80),
            Paint::Green => (100, 220, 120),
            Paint::Blue => (110, 160, 255),
            Paint::Violet => (190, 140, 255),
        }
    }

    /// The click cycle: none, each color in order, then none again.
    pub fn next(cur: Option<Paint>) -> Option<Paint> {
        let i = cur.map_or(0, |p| Paint::ALL.iter().position(|&a| a == p).unwrap() + 1);
        Paint::ALL.get(i).copied()
    }
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
    #[serde(default, skip_serializing_if = "Stroke::is_solid")]
    pub stroke: Stroke,
    #[serde(default, skip_serializing_if = "Border::is_single")]
    pub border: Border,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Paint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Paint>,
    #[serde(default, skip_serializing_if = "TextAlign::is_center")]
    pub align: TextAlign,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub id: Id,
    pub kind: LineKind,
    pub from: End,
    pub to: End,
    pub label: String,
    /// Where the label sits along the route, in thousandths. `None` centers it on the longest segment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_at: Option<u16>,
    #[serde(default, skip_serializing_if = "Stroke::is_solid")]
    pub stroke: Stroke,
    #[serde(default, skip_serializing_if = "Routing::is_elbow")]
    pub routing: Routing,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Paint>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    pub id: Id,
    pub x: i32,
    pub y: i32,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Paint>,
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

impl Shape {
    /// Only rect and ellipse draw a border glyph set.
    pub fn has_border(&self) -> bool {
        matches!(self.kind, ShapeKind::Rect | ShapeKind::Ellipse)
    }

    /// Columns the slanted kinds cut off the bounding box at the top and bottom rows.
    pub fn inset(&self) -> i32 {
        match self.kind {
            ShapeKind::Parallelogram => (self.h - 1).min(self.w - 2),
            ShapeKind::Hexagon => ((self.h - 1) / 2).min((self.w - 1) / 2),
            _ => 0,
        }
    }
}

impl Text {
    pub fn width(&self) -> i32 {
        self.text.width().max(1) as i32
    }
}
