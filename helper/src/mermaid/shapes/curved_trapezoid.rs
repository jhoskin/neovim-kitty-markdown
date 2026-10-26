//! `shapes/curvedTrapezoid.ts`: the display shape.

use super::util::{
    create_path_from_points, done, generate_circle_points, get_node_classes, label_helper,
    node_style, points_bbox, rough_path_element, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct CurvedTrapezoid;

impl Shape for CurvedTrapezoid {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        let min_width: f64 = 20.0;
        let min_height: f64 = 5.0;
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let h = min_height
            .max(out.bbox.height + label_padding_y * 2.0)
            .max(node.height());
        let radius = h / 2.0;
        let cap_clearance = radius
            - (radius.powi(2) - (out.bbox.height / 2.0).powi(2))
                .max(0.0)
                .sqrt();
        let side_clearance = (h / 4.0).max(cap_clearance);
        let w = min_width
            .max(out.bbox.width + label_padding_x * 2.0 + side_clearance * 2.0)
            .max((out.bbox.width + label_padding_x * 2.0) * 1.25)
            .max(node.width());

        let total_width = w;
        let total_height = h;
        let rw = total_width - radius;
        let tw = total_height / 4.0;
        let mut points = vec![
            Point::new(rw, 0.0),
            Point::new(tw, 0.0),
            Point::new(0.0, total_height / 2.0),
            Point::new(tw, total_height),
            Point::new(rw, total_height),
        ];
        points.extend(generate_circle_points(
            -rw,
            -total_height / 2.0,
            radius,
            50,
            270.0,
            90.0,
        ));
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
