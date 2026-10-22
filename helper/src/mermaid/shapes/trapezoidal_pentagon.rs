//! `shapes/trapezoidalPentagon.ts`: the loop limit shape.

use super::util::{
    create_path_from_points, done, get_node_classes, label_helper, node_style, path_element,
    points_bbox, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct TrapezoidalPentagon;

impl Shape for TrapezoidalPentagon {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        let min_width = 15.0;
        let min_height = 5.0;
        if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
            node.height = Some((node.height() - label_padding_y * 2.0).max(min_height));
            node.width = Some((node.width() - label_padding_x * 2.0).max(min_width));
        }
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let w = if node.width.is_some_and(|w| w != 0.0) {
            node.width()
        } else {
            out.bbox.width
        } + label_padding_x * 2.0;
        let h = if node.height.is_some_and(|h| h != 0.0) {
            node.height()
        } else {
            out.bbox.height
        } + label_padding_y * 2.0;
        let points = vec![
            Point::new((-w / 2.0) * 0.8, -h / 2.0),
            Point::new((w / 2.0) * 0.8, -h / 2.0),
            Point::new(w / 2.0, (-h / 2.0) * 0.6),
            Point::new(w / 2.0, h / 2.0),
            Point::new(-w / 2.0, h / 2.0),
            Point::new(-w / 2.0, (-h / 2.0) * 0.6),
        ];
        let path = path_element(
            create_path_from_points(&points),
            "basic label-container outer-path",
            node_style(ctx, node),
            Vec::new(),
        );
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![path], node))
    }
}
