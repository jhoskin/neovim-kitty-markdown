//! `shapes/shadedProcess.ts`: the lined rectangle.

use super::util::{
    create_path_from_points, done, get_node_classes, label_helper, node_style, points_bbox,
    rough_path_element, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

const FRAME_WIDTH: f64 = 8.0;

pub struct ShadedProcess;

impl Shape for ShadedProcess {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let padding_x = node.padding();
        let padding_y = node.padding();
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let total_width =
            node.width.unwrap_or(out.bbox.width) + padding_x * 2.0 + FRAME_WIDTH * 2.0;
        let total_height = node.height.unwrap_or(out.bbox.height) + padding_y * 2.0;
        let w = total_width - FRAME_WIDTH;
        let h = total_height;
        let x = FRAME_WIDTH - total_width / 2.0;
        let y = -total_height / 2.0;
        let points = vec![
            Point::new(x, y),
            Point::new(x + w, y),
            Point::new(x + w, y + h),
            Point::new(x - FRAME_WIDTH, y + h),
            Point::new(x - FRAME_WIDTH, y),
            Point::new(x, y),
            Point::new(x, y + h),
        ];
        let path = rough_path_element(
            create_path_from_points(&points),
            "basic label-container outer-path",
            node_style(ctx, node),
            Vec::new(),
            &ctx.vars,
        );
        out.label.common.transform = translate(
            FRAME_WIDTH / 2.0 - out.bbox.width / 2.0 - out.bbox.x,
            -(out.bbox.height / 2.0) - out.bbox.y,
        );
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Rect;
        Ok(done(out, vec![path], node))
    }
}
