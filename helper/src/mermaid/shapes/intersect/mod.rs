//! Port of `rendering-util/rendering-elements/intersect/*.js`, dagre-d3's
//! node boundary intersections. The arithmetic and the tie-breaking follow
//! the JS statement by statement so edge end points land where the oracle
//! puts them.

mod circle;
mod ellipse;
mod line;
mod node;
mod polygon;
mod rect;

pub use circle::circle;
pub use ellipse::ellipse;
pub use line::line;
pub use node::node;
pub use polygon::polygon;
pub use rect::rect;

/// The `{x, y, width, height}` subset of a node the intersect functions
/// read: the centre and the full size.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Bounds {
        Bounds {
            x,
            y,
            width,
            height,
        }
    }
}
