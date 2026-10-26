//! `shapes/slopedRect.ts`: manual input.

use super::util::{
    create_path_from_points, done, get_node_classes, label_helper, node_style, points_bbox,
    rough_path_element, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct SlopedRect;

impl Shape for SlopedRect {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
            node.width = Some((node.width() - label_padding_x * 2.0).max(10.0));
            node.height = Some((node.height() / 1.5 - label_padding_y * 2.0).max(10.0));
        }
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let total_width = if node.width.is_some_and(|w| w != 0.0) {
            node.width()
        } else {
            out.bbox.width
        } + label_padding_x * 2.0;
        let total_height = (if node.height.is_some_and(|h| h != 0.0) {
            node.height()
        } else {
            out.bbox.height
        } + label_padding_y * 2.0)
            * 1.5;
        let w = total_width;
        let h = total_height / 1.5;
        let x = -w / 2.0;
        let y = -h / 2.0;
        let points = vec![
            Point::new(x, y),
            Point::new(x, y + h),
            Point::new(x + w, y + h),
            Point::new(x + w, y - h / 2.0),
        ];
        let path = rough_path_element(
            create_path_from_points(&points),
            "basic label-container  outer-path",
            node_style(ctx, node),
            translate(0.0, h / 4.0),
            &ctx.vars,
        );
        out.label.common.transform = translate(
            -w / 2.0 + node.padding() - out.bbox.x,
            -h / 4.0 + node.padding() - out.bbox.y,
        );
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![path], node))
    }
}
