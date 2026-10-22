//! `shapes/multiRect.ts`: stacked rectangles.

use super::util::{
    create_path_from_points, done, get_node_classes, group_element, label_helper, node_style,
    path_element, points_bbox, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct MultiRect;

impl Shape for MultiRect {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        let rect_offset = 5.0;
        if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
            node.width = Some((node.width() - label_padding_x * 2.0 - 2.0 * rect_offset).max(10.0));
            node.height =
                Some((node.height() - label_padding_y * 2.0 - 2.0 * rect_offset).max(10.0));
        }
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let total_width = if node.width.is_some_and(|w| w != 0.0) {
            node.width()
        } else {
            out.bbox.width
        } + label_padding_x * 2.0
            + 2.0 * rect_offset;
        let total_height = if node.height.is_some_and(|h| h != 0.0) {
            node.height()
        } else {
            out.bbox.height
        } + label_padding_y * 2.0
            + 2.0 * rect_offset;
        let w = total_width - 2.0 * rect_offset;
        let h = total_height - 2.0 * rect_offset;
        let x = -w / 2.0;
        let y = -h / 2.0;
        let outer = vec![
            Point::new(x - rect_offset, y + rect_offset),
            Point::new(x - rect_offset, y + h + rect_offset),
            Point::new(x + w - rect_offset, y + h + rect_offset),
            Point::new(x + w - rect_offset, y + h),
            Point::new(x + w, y + h),
            Point::new(x + w, y + h - rect_offset),
            Point::new(x + w + rect_offset, y + h - rect_offset),
            Point::new(x + w + rect_offset, y - rect_offset),
            Point::new(x + rect_offset, y - rect_offset),
            Point::new(x + rect_offset, y),
            Point::new(x, y),
            Point::new(x, y + rect_offset),
        ];
        let inner = vec![
            Point::new(x, y + rect_offset),
            Point::new(x + w - rect_offset, y + rect_offset),
            Point::new(x + w - rect_offset, y + h),
            Point::new(x + w, y + h),
            Point::new(x + w, y),
            Point::new(x, y),
        ];
        let style = node_style(ctx, node);
        let shape = group_element(
            "basic label-container outer-path",
            Vec::new(),
            vec![
                path_element(
                    create_path_from_points(&outer),
                    "",
                    style.clone(),
                    Vec::new(),
                ),
                path_element(create_path_from_points(&inner), "", style, Vec::new()),
            ],
        );
        out.label.common.transform = translate(
            -(out.bbox.width / 2.0) - rect_offset - out.bbox.x,
            -(out.bbox.height / 2.0) + rect_offset - out.bbox.y,
        );
        update_node_bounds(node, points_bbox(&outer));
        node.intersector = Intersector::Polygon(outer);
        Ok(done(out, vec![shape], node))
    }
}
