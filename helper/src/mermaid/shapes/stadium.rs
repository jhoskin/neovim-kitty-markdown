//! `shapes/stadium.ts`.

use super::util::{
    create_path_from_points, done, generate_circle_points, get_node_classes, label_helper,
    node_style, points_bbox, rough_path_element, update_node_bounds,
};
use super::{Dims, Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

const ARC_POINT_COUNT: usize = 50;

/// `stadiumDimensions(bbox, paddingX, paddingY)`.
fn stadium_dimensions(bbox: Dims, padding_x: f64, padding_y: f64) -> (f64, f64) {
    let h = bbox.height + padding_y;
    let inscribed_diameter =
        h * (std::f64::consts::PI / (2.0 * (ARC_POINT_COUNT as f64 - 1.0))).cos();
    let cap_width_at_label = (inscribed_diameter.powi(2) - bbox.height.powi(2))
        .max(0.0)
        .sqrt();
    let w = (bbox.width + h / 4.0 + padding_x)
        .max(1.5 * h + (h - inscribed_diameter))
        .max(bbox.width + padding_x + h - cap_width_at_label);
    (w, h)
}

/// The stadium outline as `stadium.ts` builds it.
pub fn stadium_points(w: f64, h: f64) -> Vec<Point> {
    let radius = h / 2.0;
    let mut points = vec![
        Point::new(-w / 2.0 + radius, -h / 2.0),
        Point::new(w / 2.0 - radius, -h / 2.0),
    ];
    points.extend(generate_circle_points(
        -w / 2.0 + radius,
        0.0,
        radius,
        ARC_POINT_COUNT,
        90.0,
        270.0,
    ));
    points.push(Point::new(w / 2.0 - radius, h / 2.0));
    points.extend(generate_circle_points(
        w / 2.0 - radius,
        0.0,
        radius,
        ARC_POINT_COUNT,
        270.0,
        450.0,
    ));
    points
}

pub struct Stadium;

impl Shape for Stadium {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let padding_x = node_padding;
        let padding_y = node_padding;
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let (w, h) = stadium_dimensions(out.bbox.dims(), padding_x, padding_y);
        let points = stadium_points(w, h);
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
