//! `intersect-rect.js`.

use super::Bounds;
use crate::mermaid::scene::Point;

/// `intersectRect(node, point)`: where the ray from the node centre to
/// `point` leaves the node's bounding rectangle.
pub fn rect(node: &Bounds, point: Point) -> Point {
    let x = node.x;
    let y = node.y;
    let dx = point.x - x;
    let dy = point.y - y;
    let mut w = node.width / 2.0;
    let mut h = node.height / 2.0;
    let (sx, sy);
    if dy.abs() * w > dx.abs() * h {
        // Intersection is top or bottom of rect.
        if dy < 0.0 {
            h = -h;
        }
        sx = if dy == 0.0 { 0.0 } else { (h * dx) / dy };
        sy = h;
    } else {
        // Intersection is left or right of rect.
        if dx < 0.0 {
            w = -w;
        }
        sx = w;
        sy = if dx == 0.0 { 0.0 } else { (w * dy) / dx };
    }
    Point::new(x + sx, y + sy)
}
