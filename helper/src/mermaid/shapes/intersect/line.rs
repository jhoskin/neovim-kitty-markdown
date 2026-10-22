//! `intersect-line.js`: Graphics Gems II segment intersection, in the
//! 12.2.0 form without the integer-division half-unit nudge.

use crate::mermaid::scene::Point;

/// `intersectLine(p1, p2, q1, q2)`: the intersection of segments `p` and
/// `q`, or `None` when they do not meet (`DON'T_INTERSECT`, `COLLINEAR`).
pub fn line(p1: Point, p2: Point, q1: Point, q2: Point) -> Option<Point> {
    // Line joining points 1 and 2 is F(x,y) = a1 x + b1 y + c1 = 0.
    let a1 = p2.y - p1.y;
    let b1 = p1.x - p2.x;
    let c1 = p2.x * p1.y - p1.x * p2.y;

    let r3 = a1 * q1.x + b1 * q1.y + c1;
    let r4 = a1 * q2.x + b1 * q2.y + c1;

    let epsilon = 1e-6;

    // Both of q's end points on the same side of line 1: no intersection.
    if r3 != 0.0 && r4 != 0.0 && same_sign(r3, r4) {
        return None;
    }

    let a2 = q2.y - q1.y;
    let b2 = q1.x - q2.x;
    let c2 = q2.x * q1.y - q1.x * q2.y;

    let r1 = a2 * p1.x + b2 * p1.y + c2;
    let r2 = a2 * p2.x + b2 * p2.y + c2;

    if r1.abs() < epsilon && r2.abs() < epsilon && same_sign(r1, r2) {
        return None;
    }

    let denom = a1 * b2 - a2 * b1;
    if denom == 0.0 {
        return None;
    }

    let x = (b1 * c2 - b2 * c1) / denom;
    let y = (a2 * c1 - a1 * c2) / denom;
    Some(Point::new(x, y))
}

fn same_sign(r1: f64, r2: f64) -> bool {
    r1 * r2 > 0.0
}
