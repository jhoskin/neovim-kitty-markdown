//! `intersect-circle.js`.

use super::{Bounds, ellipse};
use crate::mermaid::scene::Point;

/// `intersectCircle(node, rx, point)`: an ellipse with both radii `rx`.
pub fn circle(node: &Bounds, rx: f64, point: Point) -> Point {
    ellipse(node, rx, rx, point)
}
