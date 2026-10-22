//! `shapes/bowTieRect.ts`: stored data.

use super::util::{
    create_path_from_points, done, get_node_classes, label_helper, node_style, path_element,
    points_bbox, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

/// `generateArcPoints(x1, y1, x2, y2, rx, ry, clockwise)` of `bowTieRect.ts`.
#[allow(clippy::too_many_arguments)]
fn generate_arc_points(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    rx: f64,
    ry: f64,
    clockwise: bool,
) -> Result<Vec<Point>, HelperError> {
    let num_points = 20;
    let mid_x = (x1 + x2) / 2.0;
    let mid_y = (y1 + y2) / 2.0;
    let angle = (y2 - y1).atan2(x2 - x1);
    let dx = (x2 - x1) / 2.0;
    let dy = (y2 - y1) / 2.0;
    let transformed_x = dx / rx;
    let transformed_y = dy / ry;
    let distance = (transformed_x.powi(2) + transformed_y.powi(2)).sqrt();
    if distance > 1.0 {
        return Err(HelperError::internal(
            "The given radii are too small to create an arc between the points.",
        ));
    }
    let scaled_center_distance = (1.0 - distance.powi(2)).sqrt();
    let sign = if clockwise { -1.0 } else { 1.0 };
    let center_x = mid_x + scaled_center_distance * ry * angle.sin() * sign;
    let center_y = mid_y - scaled_center_distance * rx * angle.cos() * sign;
    let start_angle = ((y1 - center_y) / ry).atan2((x1 - center_x) / rx);
    let end_angle = ((y2 - center_y) / ry).atan2((x2 - center_x) / rx);
    let mut angle_range = end_angle - start_angle;
    if clockwise && angle_range < 0.0 {
        angle_range += 2.0 * std::f64::consts::PI;
    }
    if !clockwise && angle_range > 0.0 {
        angle_range -= 2.0 * std::f64::consts::PI;
    }
    Ok((0..num_points)
        .map(|i| {
            let t = f64::from(i) / f64::from(num_points - 1);
            let a = start_angle + t * angle_range;
            Point::new(center_x + rx * a.cos(), center_y + ry * a.sin())
        })
        .collect())
}

/// `calculateArcSagitta(chord, radiusX, radiusY)`.
fn calculate_arc_sagitta(chord: f64, radius_x: f64, radius_y: f64) -> f64 {
    let semi_major = radius_x.max(radius_y);
    let semi_minor = radius_x.min(radius_y);
    semi_minor * (1.0 - (1.0 - (chord / semi_major / 2.0).powi(2)).sqrt())
}

pub struct BowTieRect;

impl Shape for BowTieRect {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let total_height = if node.height.is_some_and(|h| h != 0.0) {
            node.height()
        } else {
            out.bbox.height
        } + label_padding_y;
        let ry = total_height / 2.0;
        let rx = ry / (2.5 + total_height / 50.0);
        let sagitta = calculate_arc_sagitta(total_height, rx, ry);
        let total_width = if node.width.is_some_and(|w| w != 0.0) {
            node.width()
        } else {
            out.bbox.width
        } + label_padding_x * 2.0
            + sagitta;
        let w = total_width - sagitta;
        let h = total_height;

        let mut points = vec![
            Point::new(w / 2.0, -h / 2.0),
            Point::new(-w / 2.0, -h / 2.0),
        ];
        points.extend(generate_arc_points(
            -w / 2.0,
            -h / 2.0,
            -w / 2.0,
            h / 2.0,
            rx,
            ry,
            false,
        )?);
        points.push(Point::new(w / 2.0, h / 2.0));
        points.extend(generate_arc_points(
            w / 2.0,
            h / 2.0,
            w / 2.0,
            -h / 2.0,
            rx,
            ry,
            true,
        )?);

        let path = path_element(
            create_path_from_points(&points),
            "basic label-container outer-path",
            node_style(ctx, node),
            translate(rx / 2.0, 0.0),
        );
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![path], node))
    }
}
