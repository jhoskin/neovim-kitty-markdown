//! `shapes/triangle.ts`: the extract shape.

use super::util::{
    create_path_from_points, done, get_node_classes, label_helper, node_style, path_element,
    points_bbox, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

const MIN_HEIGHT: f64 = 10.0;
const MIN_WIDTH: f64 = 10.0;

pub struct Triangle;

impl Shape for Triangle {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
            node.width = Some(((node.width() - label_padding_x) / 2.0).max(MIN_WIDTH));
            node.height = Some(node.height().max(MIN_HEIGHT));
        }
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        // `evaluate(getConfig().flowchart?.htmlLabels)`: the deprecated
        // flowchart key defaults to `null`, which `evaluate` reads as false.
        let use_html_labels = false;
        let w = if node.width.is_some_and(|w| w != 0.0) {
            node.width()
        } else {
            out.bbox.width
        } + label_padding_x;
        let h = if node.height.is_some_and(|h| h != 0.0) {
            node.height()
        } else {
            w + out.bbox.height
        };
        let tw = h;
        let points = vec![
            Point::new(0.0, 0.0),
            Point::new(tw, 0.0),
            Point::new(tw / 2.0, -h),
        ];
        let path = path_element(
            create_path_from_points(&points),
            "outer-path",
            node_style(ctx, node),
            translate(-h / 2.0, h / 2.0),
        );
        update_node_bounds(node, points_bbox(&points));
        let divisor = if use_html_labels { 2.0 } else { 1.0 };
        out.label.common.transform = translate(
            -out.bbox.width / 2.0 - out.bbox.x,
            h / 2.0 - (out.bbox.height + node.padding() / divisor - out.bbox.y),
        );
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![path], node))
    }
}
