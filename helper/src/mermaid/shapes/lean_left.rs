//! `shapes/leanLeft.ts`.

use super::util::{
    done, get_node_classes, insert_polygon_shape, label_helper, node_style, points_bbox,
    update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct LeanLeft;

impl Shape for LeanLeft {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_y = node_padding;
        let label_padding_x = node_padding;
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let h = (out.bbox.height + label_padding_y).max(node.height());
        let w = (out.bbox.width + label_padding_x).max(node.width() - h);
        let points = vec![
            Point::new(0.0, 0.0),
            Point::new(w + (3.0 * h) / 6.0, 0.0),
            Point::new(w, -h),
            Point::new(-(3.0 * h) / 6.0, -h),
        ];
        let polygon = insert_polygon_shape(w, h, &points, node_style(ctx, node));
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![polygon], node))
    }
}
