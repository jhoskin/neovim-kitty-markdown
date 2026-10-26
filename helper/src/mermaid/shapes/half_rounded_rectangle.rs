//! `shapes/halfRoundedRectangle.ts`: the delay shape.

use super::util::{
    create_path_from_points, done, generate_circle_points, get_node_classes, label_helper,
    node_style, points_bbox, rough_path_element, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct HalfRoundedRectangle;

impl Shape for HalfRoundedRectangle {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let min_width = 15.0;
        let min_height = 10.0;
        let padding_x = node.padding();
        let padding_y = node.padding();
        if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
            node.height = Some((node.height() - padding_y * 2.0).max(min_height));
            node.width = Some((node.width() - padding_x * 2.0).max(min_width));
        }
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let h = if node.height.is_some_and(|h| h != 0.0) {
            node.height()
        } else {
            min_height.max(out.bbox.height)
        } + padding_y * 2.0;
        let radius = h / 2.0;
        let cap_clearance = radius
            - (radius.powi(2) - (out.bbox.height / 2.0).powi(2))
                .max(0.0)
                .sqrt();
        let w = if node.width.is_some_and(|w| w != 0.0) {
            node.width()
        } else {
            min_width.max(out.bbox.width) + cap_clearance * 2.0
        } + padding_x * 2.0;

        let mut points = vec![
            Point::new(-w / 2.0, -h / 2.0),
            Point::new(w / 2.0 - radius, -h / 2.0),
        ];
        points.extend(generate_circle_points(
            -w / 2.0 + radius,
            0.0,
            radius,
            50,
            90.0,
            270.0,
        ));
        points.push(Point::new(w / 2.0 - radius, h / 2.0));
        points.push(Point::new(-w / 2.0, h / 2.0));
        let path = rough_path_element(
            create_path_from_points(&points),
            "basic label-container outer-path",
            node_style(ctx, node),
            Vec::new(),
            &ctx.vars,
        );
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![path], node))
    }
}
