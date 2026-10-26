//! `shapes/flippedTriangle.ts`: the manual file shape.

use super::util::{
    create_path_from_points, done, get_node_classes, label_helper, node_style, points_bbox,
    rough_path_element, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

const MIN_HEIGHT: f64 = 10.0;
const MIN_WIDTH: f64 = 10.0;

pub struct FlippedTriangle;

impl Shape for FlippedTriangle {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
            node.height = Some(node.height().max(MIN_HEIGHT));
            node.width =
                Some((node.width() - label_padding_x - label_padding_x / 2.0).max(MIN_WIDTH));
        }
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
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
            Point::new(0.0, -h),
            Point::new(tw, -h),
            Point::new(tw / 2.0, 0.0),
        ];
        let path = rough_path_element(
            create_path_from_points(&points),
            "outer-path",
            node_style(ctx, node),
            translate(-h / 2.0, h / 2.0),
            &ctx.vars,
        );
        update_node_bounds(node, points_bbox(&points));
        // The TS adds the label origin here instead of subtracting it.
        out.label.common.transform = translate(
            -out.bbox.width / 2.0 - out.bbox.x,
            -h / 2.0 + node.padding() / 2.0 + out.bbox.y,
        );
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![path], node))
    }
}
