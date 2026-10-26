//! `shapes/curlyBraceRight.ts`. Unlike the left brace, its local
//! `generateCirclePoints` does not negate the points.

use super::util::{
    create_open_path_from_points, create_path_from_points, done, get_node_classes, group_element,
    label_helper, node_style, points_bbox, rough_stroke_element, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

/// The un-negated `generateCirclePoints` local to `curlyBraceRight.ts`.
fn circle_points(cx: f64, cy: f64, radius: f64, n: usize, start: f64, end: f64) -> Vec<Point> {
    let start = (start * std::f64::consts::PI) / 180.0;
    let end = (end * std::f64::consts::PI) / 180.0;
    let step = (end - start) / (n as f64 - 1.0);
    (0..n)
        .map(|i| {
            let a = start + i as f64 * step;
            Point::new(cx + radius * a.cos(), cy + radius * a.sin())
        })
        .collect()
}

pub struct CurlyBraceRight;

impl Shape for CurlyBraceRight {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let label_padding_x = node.padding();
        let label_padding_y = node.padding();
        let w = out.bbox.width + label_padding_x;
        let h = out.bbox.height + label_padding_y;
        let radius = (h * 0.1).max(5.0);

        let mut points = Vec::new();
        points.extend(circle_points(w / 2.0, -h / 2.0, radius, 20, -90.0, 0.0));
        points.push(Point::new(w / 2.0 + radius, -radius));
        points.extend(circle_points(
            w / 2.0 + radius * 2.0,
            -radius,
            radius,
            20,
            -180.0,
            -270.0,
        ));
        points.extend(circle_points(
            w / 2.0 + radius * 2.0,
            radius,
            radius,
            20,
            -90.0,
            -180.0,
        ));
        points.push(Point::new(w / 2.0 + radius, h / 2.0));
        points.extend(circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0));

        let mut rect_points = vec![
            Point::new(-w / 2.0, -h / 2.0 - radius),
            Point::new(w / 2.0, -h / 2.0 - radius),
        ];
        rect_points.extend(circle_points(w / 2.0, -h / 2.0, radius, 20, -90.0, 0.0));
        rect_points.push(Point::new(w / 2.0 + radius, -radius));
        rect_points.extend(circle_points(
            w / 2.0 + radius * 2.0,
            -radius,
            radius,
            20,
            -180.0,
            -270.0,
        ));
        rect_points.extend(circle_points(
            w / 2.0 + radius * 2.0,
            radius,
            radius,
            20,
            -90.0,
            -180.0,
        ));
        rect_points.push(Point::new(w / 2.0 + radius, h / 2.0));
        rect_points.extend(circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0));
        rect_points.push(Point::new(w / 2.0, h / 2.0 + radius));
        rect_points.push(Point::new(-w / 2.0, h / 2.0 + radius));

        let style = node_style(ctx, node);
        let mut rect_style = style.clone();
        rect_style.stroke_opacity = Some(0.0);
        let shape = group_element(
            "text",
            translate(-radius, 0.0),
            vec![
                rough_stroke_element(&create_open_path_from_points(&points), style),
                rough_stroke_element(&create_path_from_points(&rect_points), rect_style),
            ],
        );
        out.label.common.transform = translate(
            -w / 2.0 + node.padding() / 2.0 - out.bbox.x,
            -h / 2.0 + node.padding() / 2.0 - out.bbox.y,
        );
        update_node_bounds(node, points_bbox(&points).union(points_bbox(&rect_points)));
        node.intersector = Intersector::Polygon(rect_points);
        Ok(done(out, vec![shape], node))
    }
}
