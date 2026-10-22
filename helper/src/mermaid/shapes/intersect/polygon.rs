//! `intersect-polygon.js`.

use std::cmp::Ordering;

use super::{Bounds, line};
use crate::mermaid::scene::Point;

/// `intersectPolygon(node, polyPoints, point)`: the polygon is given in its
/// own coordinates and shifted so its minimum corner sits at the node's
/// top-left; the nearest intersection to `point` wins.
pub fn polygon(node: &Bounds, poly_points: &[Point], point: Point) -> Point {
    let x1 = node.x;
    let y1 = node.y;
    let center = Point::new(x1, y1);

    let mut intersections = Vec::new();

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    for entry in poly_points {
        min_x = min_x.min(entry.x);
        min_y = min_y.min(entry.y);
    }

    let left = x1 - node.width / 2.0 - min_x;
    let top = y1 - node.height / 2.0 - min_y;

    for i in 0..poly_points.len() {
        let p1 = poly_points[i];
        let p2 = poly_points[if i < poly_points.len() - 1 { i + 1 } else { 0 }];
        if let Some(intersect) = line(
            center,
            point,
            Point::new(left + p1.x, top + p1.y),
            Point::new(left + p2.x, top + p2.y),
        ) {
            intersections.push(intersect);
        }
    }

    if intersections.is_empty() {
        return center;
    }

    if intersections.len() > 1 {
        // More intersections, find the one nearest to edge end point. JS
        // `Array.prototype.sort` is stable, like `sort_by`.
        intersections.sort_by(|p, q| {
            let pdx = p.x - point.x;
            let pdy = p.y - point.y;
            let distp = (pdx * pdx + pdy * pdy).sqrt();
            let qdx = q.x - point.x;
            let qdy = q.y - point.y;
            let distq = (qdx * qdx + qdy * qdy).sqrt();
            if distp < distq {
                Ordering::Less
            } else if distp == distq {
                Ordering::Equal
            } else {
                Ordering::Greater
            }
        });
    }
    intersections[0]
}
