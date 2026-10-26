//! `shapes/taggedRect.ts`.

use super::util::{
    create_path_from_points, done, get_node_classes, group_element, label_helper, node_style,
    points_bbox, rough_path_element, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

const TAG_RATIO: f64 = 0.2;

pub struct TaggedRect;

impl Shape for TaggedRect {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
            node.height = Some((node.height() - label_padding_y * 2.0).max(10.0));
            node.width = Some(
                (node.width()
                    - label_padding_x * 2.0
                    - TAG_RATIO * (node.height() + label_padding_y * 2.0))
                    .max(10.0),
            );
        }
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let total_height = if node.height.is_some_and(|h| h != 0.0) {
            node.height()
        } else {
            out.bbox.height
        } + label_padding_y * 2.0;
        let tag_width = TAG_RATIO * total_height;
        let tag_height = TAG_RATIO * total_height;
        let total_width = if node.width.is_some_and(|w| w != 0.0) {
            node.width()
        } else {
            out.bbox.width
        } + label_padding_x * 2.0
            + tag_width;
        let w = total_width - tag_width;
        let h = total_height;
        let x = -w / 2.0;
        let y = -h / 2.0;
        let rect_points = vec![
            Point::new(x - tag_width / 2.0, y),
            Point::new(x + w + tag_width / 2.0, y),
            Point::new(x + w + tag_width / 2.0, y + h),
            Point::new(x - tag_width / 2.0, y + h),
        ];
        let tag_points = vec![
            Point::new(x + w - tag_width / 2.0, y + h),
            Point::new(x + w + tag_width / 2.0, y + h),
            Point::new(x + w + tag_width / 2.0, y + h - tag_height),
        ];
        let style = node_style(ctx, node);
        let group = group_element(
            "basic label-container outer-path",
            Vec::new(),
            vec![
                rough_path_element(
                    create_path_from_points(&rect_points),
                    "",
                    style.clone(),
                    Vec::new(),
                    &ctx.vars,
                ),
                rough_path_element(
                    create_path_from_points(&tag_points),
                    "",
                    style,
                    Vec::new(),
                    &ctx.vars,
                ),
            ],
        );
        update_node_bounds(node, points_bbox(&rect_points));
        node.intersector = Intersector::Polygon(rect_points);
        Ok(done(out, vec![group], node))
    }
}
