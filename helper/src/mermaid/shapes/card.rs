//! `shapes/card.ts`: the notched rectangle.

use super::util::{
    done, get_node_classes, insert_polygon_shape, label_helper, node_style, points_bbox,
    update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

/// Size of the notch on the top left corner.
const NOTCH_SIZE: f64 = 12.0;

pub struct Card;

impl Shape for Card {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let w = node.width.unwrap_or(out.bbox.width) + label_padding_x + NOTCH_SIZE;
        let h = node.height.unwrap_or(out.bbox.height) + label_padding_y;
        let left = 0.0;
        let right = w;
        let top = -h;
        let bottom = 0.0;
        let points = vec![
            Point::new(left + NOTCH_SIZE, top),
            Point::new(right, top),
            Point::new(right, bottom),
            Point::new(left, bottom),
            Point::new(left, top + NOTCH_SIZE),
            Point::new(left + NOTCH_SIZE, top),
        ];
        let polygon = insert_polygon_shape(w, h, &points, node_style(ctx, node));
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![polygon], node))
    }
}
