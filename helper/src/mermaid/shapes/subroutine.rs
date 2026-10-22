//! `shapes/subroutine.ts`.

use super::util::{
    done, get_node_classes, insert_polygon_shape, label_helper, node_style, points_bbox,
    update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

const FRAME_WIDTH: f64 = 8.0;

pub struct Subroutine;

impl Shape for Subroutine {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding.unwrap_or(8.0);
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let total_width = (out.bbox.width + 2.0 * FRAME_WIDTH + label_padding_x).max(node.width());
        let total_height = (out.bbox.height + label_padding_y).max(node.height());
        let w = total_width - 2.0 * FRAME_WIDTH;
        let h = total_height;
        let points = vec![
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, -h),
            Point::new(0.0, -h),
            Point::new(0.0, 0.0),
            Point::new(-8.0, 0.0),
            Point::new(w + 8.0, 0.0),
            Point::new(w + 8.0, -h),
            Point::new(-8.0, -h),
            Point::new(-8.0, 0.0),
        ];
        let el = insert_polygon_shape(w, h, &points, node_style(ctx, node));
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![el], node))
    }
}
