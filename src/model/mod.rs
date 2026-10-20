mod canvas;
mod command;
mod element;
mod error;
mod geom;
mod history;
mod ids;

pub use canvas::{Canvas, Find};
pub use command::{Align, Command, Side, Z};
pub use element::{Anchor, Element, End, Line, LineKind, Routing, Shape, ShapeKind, Stroke, Text};
pub use error::ModelError;
pub use geom::{corners, edge_anchor, label_pos, route, straight, wrap, Axis};
pub use history::{History, HISTORY_LIMIT};
pub use ids::Id;

pub const VERSION: u32 = 1;
