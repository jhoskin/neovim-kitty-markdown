//! `shapes/hexagon.ts`.

use super::util::{
    done, get_node_classes, insert_polygon_shape, label_helper, node_style, points_bbox,
    update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct Hexagon;

impl Shape for Hexagon {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let f = 4.0;
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
            let original_height = node.height();
            let m = original_height / f;
            node.width = Some(node.width() - 2.0 * m - label_padding_y);
            node.height = Some(node.height() - label_padding_x);
        }
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let h = if node.height.is_some_and(|h| h != 0.0) {
            node.height()
        } else {
            out.bbox.height
        } + label_padding_x;
        let m = h / f;
        let w = if node.width.is_some_and(|w| w != 0.0) {
            node.width()
        } else {
            out.bbox.width
        } + 2.0 * m
            + label_padding_y;
        let points = vec![
            Point::new(m, 0.0),
            Point::new(w - m, 0.0),
            Point::new(w, -h / 2.0),
            Point::new(w - m, -h),
            Point::new(m, -h),
            Point::new(0.0, -h / 2.0),
        ];
        let polygon = insert_polygon_shape(w, h, &points, node_style(ctx, node));
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![polygon], node))
    }
}
