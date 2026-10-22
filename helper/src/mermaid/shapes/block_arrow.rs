//! `shapes/blockArrow.ts`. The block diagram passes `directions`,
//! `positioned`, and `widthInColumns`, which the shared node model does
//! not carry, so this is the natural-width form with the `point` outline.

use super::util::{
    done, get_node_classes, insert_polygon_shape, label_helper, node_style, points_bbox,
    update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct BlockArrow;

impl Shape for BlockArrow {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let node_padding = node.padding();
        let height = out.bbox.height + 2.0 * node_padding;
        let midpoint = height / 2.0;
        let width = out.bbox.width + 2.0 * midpoint + node_padding;
        // `getArrowPoints([], ...)` resolves to the `point` factory.
        let points = vec![Point::new(0.0, 0.0)];
        let polygon = insert_polygon_shape(width, height, &points, node_style(ctx, node));
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![polygon], node))
    }
}
