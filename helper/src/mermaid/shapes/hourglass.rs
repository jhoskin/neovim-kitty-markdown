//! `shapes/hourglass.ts`: the collate shape, label-less.

use super::util::{
    create_path_from_points, done, get_node_classes, label_helper, node_style, points_bbox,
    rough_path_element, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct Hourglass;

impl Shape for Hourglass {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        node.label = Some(String::new());
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let w = node.width().max(30.0);
        let h = node.height().max(30.0);
        let points = vec![
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(0.0, h),
            Point::new(w, h),
        ];
        let path = rough_path_element(
            create_path_from_points(&points),
            "basic label-container outer-path",
            node_style(ctx, node),
            translate(-w / 2.0, -h / 2.0),
            &ctx.vars,
        );
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![path], node))
    }
}
