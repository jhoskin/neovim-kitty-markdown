//! `shapes/windowPane.ts`: internal storage.

use super::util::{
    done, get_node_classes, label_helper, node_style, points_bbox, rough_path_element, translate,
    update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{PathData, Point};

const RECT_OFFSET: f64 = 10.0;

pub struct WindowPane;

impl Shape for WindowPane {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let padding_x = node.padding();
        let padding_y = node.padding();
        if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
            node.width = Some((node.width() - padding_x * 2.0 - RECT_OFFSET).max(10.0));
            node.height = Some((node.height() - padding_y * 2.0 - RECT_OFFSET).max(10.0));
        }
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let total_width = if node.width.is_some_and(|w| w != 0.0) {
            node.width()
        } else {
            out.bbox.width
        } + padding_x * 2.0
            + RECT_OFFSET;
        let total_height = if node.height.is_some_and(|h| h != 0.0) {
            node.height()
        } else {
            out.bbox.height
        } + padding_y * 2.0
            + RECT_OFFSET;
        let w = total_width - RECT_OFFSET;
        let h = total_height - RECT_OFFSET;
        let x = -w / 2.0;
        let y = -h / 2.0;
        let outer_path_points = vec![
            Point::new(x - RECT_OFFSET, y - RECT_OFFSET),
            Point::new(x - RECT_OFFSET, y + h),
            Point::new(x + w, y + h),
            Point::new(x + w, y - RECT_OFFSET),
        ];
        let mut d = PathData::new();
        d.move_to(x - RECT_OFFSET, y - RECT_OFFSET)
            .line_to(x + w, y - RECT_OFFSET)
            .line_to(x + w, y + h)
            .line_to(x - RECT_OFFSET, y + h)
            .line_to(x - RECT_OFFSET, y - RECT_OFFSET)
            .move_to(x - RECT_OFFSET, y)
            .line_to(x + w, y)
            .move_to(x, y - RECT_OFFSET)
            .line_to(x, y + h);
        let path = rough_path_element(
            d,
            "basic label-container outer-path",
            node_style(ctx, node),
            translate(RECT_OFFSET / 2.0, RECT_OFFSET / 2.0),
            &ctx.vars,
        );
        out.label.common.transform = translate(
            -(out.bbox.width / 2.0) + RECT_OFFSET / 2.0 - out.bbox.x,
            -(out.bbox.height / 2.0) + RECT_OFFSET / 2.0 - out.bbox.y,
        );
        update_node_bounds(node, points_bbox(&outer_path_points));
        node.intersector = Intersector::Polygon(outer_path_points);
        Ok(done(out, vec![path], node))
    }
}
