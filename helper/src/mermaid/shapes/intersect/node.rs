//! `intersect-node.js`.

use crate::mermaid::scene::Point;
use crate::mermaid::shapes::Node;

/// `intersectNode(node, point)`: `node.intersect(point)`.
pub fn node(node: &Node, point: Point) -> Point {
    node.intersect(point)
}
