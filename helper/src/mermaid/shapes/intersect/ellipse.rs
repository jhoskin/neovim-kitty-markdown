//! `intersect-ellipse.js`.

use super::Bounds;
use crate::mermaid::scene::Point;

/// `intersectEllipse(node, rx, ry, point)`; formulae from MathWorld's
/// ellipse-line intersection, as the JS cites.
pub fn ellipse(node: &Bounds, rx: f64, ry: f64, point: Point) -> Point {
    let cx = node.x;
    let cy = node.y;
    let px = cx - point.x;
    let py = cy - point.y;
    let det = (rx * rx * py * py + ry * ry * px * px).sqrt();
    let mut dx = ((rx * ry * px) / det).abs();
    if point.x < cx {
        dx = -dx;
    }
    let mut dy = ((rx * ry * py) / det).abs();
    if point.y < cy {
        dy = -dy;
    }
    Point::new(cx + dx, cy + dy)
}
