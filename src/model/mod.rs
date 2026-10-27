mod canvas;
mod command;
mod element;
mod error;
mod geom;
mod history;
mod ids;
mod layout;
mod router;

pub use canvas::{Canvas, Find};
pub use command::{kind_cmds, Align, Command, Side, Z};
pub use element::{
    Anchor, Border, Edge, Element, End, Line, LineKind, Paint, Shape, ShapeKind, SideLabel, Stroke,
    Text, TextAlign, FILL_ALPHA, MIN_SIZE,
};
pub use error::ModelError;
pub use geom::{
    corners, edge_anchor, inside, label_at_nearest, label_pos, route, side_anchor, side_at,
    side_index, side_label_at, side_mid, wrap, Axis,
};
pub use history::{History, HISTORY_LIMIT};
pub use ids::Id;
pub use layout::Dir;
pub use router::{Router, MARGIN, REACH};

pub const VERSION: u32 = 1;
