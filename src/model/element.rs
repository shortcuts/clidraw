use super::Id;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Rect,
    Diamond,
    Cylinder,
    Parallelogram,
    Hexagon,
    Stadium,
    Subroutine,
    Frame,
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

/// Border glyph set of a rect. `Single` keeps the kind's own corners.
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

/// Ink and fill color: a name or a `#rrggbb` hex. The terminal and both exporters read the same RGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Paint {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Violet,
    Rgb(u8, u8, u8),
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
            Paint::Rgb(r, g, b) => (r, g, b),
        }
    }

    /// The name of a named color, else the lowercase `#rrggbb`.
    pub fn label(self) -> String {
        match self {
            Paint::Red => "red".into(),
            Paint::Orange => "orange".into(),
            Paint::Yellow => "yellow".into(),
            Paint::Green => "green".into(),
            Paint::Blue => "blue".into(),
            Paint::Violet => "violet".into(),
            Paint::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        }
    }

    /// A lowercase name, or `#` plus six hex digits of either case.
    pub fn parse(s: &str) -> Option<Paint> {
        if let Some(p) = Paint::ALL.into_iter().find(|p| p.label() == s) {
            return Some(p);
        }
        let hex = s
            .strip_prefix('#')
            .filter(|h| h.len() == 6 && h.is_ascii())?;
        let ch = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Paint::Rgb(ch(0)?, ch(2)?, ch(4)?))
    }
}

impl TryFrom<String> for Paint {
    type Error = String;

    fn try_from(s: String) -> Result<Paint, String> {
        Paint::parse(&s).ok_or_else(|| format!("unknown color {s:?}"))
    }
}

impl From<Paint> for String {
    fn from(p: Paint) -> String {
        p.label()
    }
}

/// Side of a shape a bound line end leaves from, and the cell along that side counted from its
/// start. `Auto` picks the side facing the other end, at its midpoint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    #[default]
    Auto,
    Top(i32),
    Right(i32),
    Bottom(i32),
    Left(i32),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Top,
    Right,
    Bottom,
    Left,
}

/// Text drawn on one border side of a shape. `at` is the position along the side in thousandths;
/// `None` centers it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SideLabel {
    pub edge: Edge,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<u16>,
    pub text: String,
}

/// Free-form key to value pairs; keys such as `file` or `url` are convention only.
pub type Meta = std::collections::BTreeMap<String, String>;

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
    /// Target of the shape in the SVG and Mermaid exports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub meta: Meta,
    /// At most one per edge, never empty, ordered by edge.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub side_labels: Vec<SideLabel>,
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
    /// Cells the user bent the route through, from `from` to `to`. Empty means the automatic route.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bends: Vec<(i32, i32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Paint>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    pub id: Id,
    pub x: i32,
    pub y: i32,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Paint>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub meta: Meta,
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

    pub fn meta(&self) -> &Meta {
        match self {
            Element::Shape(s) => &s.meta,
            Element::Line(l) => &l.meta,
            Element::Text(t) => &t.meta,
        }
    }
}

/// Smallest width and height of any shape.
pub const MIN_SIZE: i32 = 3;

impl Shape {
    pub fn new(kind: ShapeKind, x: i32, y: i32, w: i32, h: i32, label: impl Into<String>) -> Shape {
        Shape {
            id: Id::random(),
            kind,
            x,
            y,
            w,
            h,
            label: label.into(),
            stroke: Stroke::default(),
            border: Border::default(),
            color: None,
            fill: None,
            align: TextAlign::default(),
            bold: false,
            link: None,
            meta: Meta::new(),
            side_labels: vec![],
        }
    }

    pub fn side_label(&self, edge: Edge) -> Option<&SideLabel> {
        self.side_labels.iter().find(|l| l.edge == edge)
    }

    /// Only rect draws a border glyph set.
    pub fn has_border(&self) -> bool {
        self.kind == ShapeKind::Rect
    }

    /// Width a `kind` shape of height `h` needs to keep its silhouette; `None` for free-form kinds.
    pub fn ratio_width(kind: ShapeKind, h: i32) -> Option<i32> {
        match kind {
            ShapeKind::Diamond => Some(2 * h - 1),
            ShapeKind::Hexagon | ShapeKind::Parallelogram => Some(2 * h),
            _ => None,
        }
    }

    /// Nearest size to `(w, h)` that holds the ratio of `kind`; free-form kinds keep `(w, h)`.
    pub fn snap_size(kind: ShapeKind, w: i32, h: i32) -> (i32, i32) {
        let Some(offset) = Self::ratio_width(kind, 0) else {
            return (w, h);
        };
        // Closest point to (w, h) on the ratio line, with width halved to weigh both axes alike.
        let h = ((2 * h + w - offset + 2) / 4).max(MIN_SIZE);
        (Self::ratio_width(kind, h).unwrap_or(w), h)
    }

    /// Rect after one 90° turn: width and height swap around the center.
    pub fn rotated(&self) -> (i32, i32, i32, i32) {
        let x = self.x + (self.w - self.h) / 2;
        let y = self.y + (self.h - self.w) / 2;
        (x, y, self.h, self.w)
    }

    /// Columns the slanted kinds cut off the bounding box at the top and bottom rows.
    pub fn inset(&self) -> i32 {
        match self.kind {
            ShapeKind::Parallelogram => (self.h - 1).min(self.w - 2),
            ShapeKind::Hexagon => ((self.h - 1) / 2).min((self.w - 1) / 2),
            _ => 0,
        }
    }

    /// First and last column the outline occupies on absolute row `row`.
    pub fn row_span(&self, row: i32) -> (i32, i32) {
        let (m, j) = (self.inset(), row - self.y);
        let (l, r) = match self.kind {
            ShapeKind::Parallelogram => (m - j.min(m), -j.min(m)),
            ShapeKind::Hexagon => {
                let mid = (self.h - 1) / 2;
                let d = match j.cmp(&mid) {
                    Ordering::Equal => 0,
                    Ordering::Less => (m - j).max(0),
                    Ordering::Greater => (j - (self.h - 1 - m)).max(0),
                };
                (d, -d)
            }
            _ => (0, 0),
        };
        (self.x + l, self.x + self.w - 1 + r)
    }

    /// First and last column of the diamond outline on absolute row `row`.
    pub fn diamond_span(&self, row: i32) -> (i32, i32) {
        let mid = f64::from(self.h - 1) / 2.0;
        let spread = 1.0 - (f64::from(row - self.y) - mid).abs() / mid;
        let off = (f64::from(self.w - 1) / 2.0 * spread).round() as i32;
        (self.x + (self.w - 1) / 2 - off, self.x + self.w / 2 + off)
    }

    /// True when `p` lies on a cell the diamond, parallelogram or hexagon outline draws.
    pub fn on_slanted_outline(&self, p: (i32, i32)) -> bool {
        let (l, r) = match self.kind {
            ShapeKind::Diamond => self.diamond_span(p.1),
            _ => self.row_span(p.1),
        };
        let flat = p.1 == self.y || p.1 == self.y + self.h - 1;
        let edge = p.0 == l || p.0 == r;
        let top_or_bottom = flat && self.kind != ShapeKind::Diamond && (l..=r).contains(&p.0);
        (0..self.h).contains(&(p.1 - self.y)) && (edge || top_or_bottom)
    }
}

impl Line {
    pub fn new(kind: LineKind, from: End, to: End) -> Line {
        Line {
            id: Id::random(),
            kind,
            from,
            to,
            label: String::new(),
            label_at: None,
            stroke: Stroke::default(),
            bends: vec![],
            color: None,
            meta: Meta::new(),
        }
    }
}

impl Text {
    pub fn width(&self) -> i32 {
        self.text.width().max(1) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_takes_names_and_six_digit_hex_only() {
        assert_eq!(Paint::parse("red"), Some(Paint::Red));
        assert_eq!(Paint::parse("#FF00aa"), Some(Paint::Rgb(255, 0, 170)));
        for bad in [
            "",
            "Red",
            "#f0a",
            "#ff00aa80",
            " #ff00aa",
            "#gg0000",
            "ff00aa",
            "#é0000",
        ] {
            assert_eq!(Paint::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn serde_writes_lowercase_hex_and_still_reads_old_names() {
        let p = Paint::Rgb(255, 0, 170);
        assert_eq!(serde_json::to_string(&p).unwrap(), "\"#ff00aa\"");
        assert_eq!(serde_json::from_str::<Paint>("\"#FF00AA\"").unwrap(), p);
        assert_eq!(
            serde_json::from_str::<Paint>("\"red\"").unwrap(),
            Paint::Red
        );
        assert!(serde_json::from_str::<Paint>("\"nope\"").is_err());
    }
}
