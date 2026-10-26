//! `shapes/curlyBraces.ts`: braces on both sides.

use super::util::{
    create_open_path_from_points, create_path_from_points, done, generate_circle_points,
    get_node_classes, group_element, label_helper, node_style, points_bbox, rough_stroke_element,
    translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct CurlyBraces;

impl Shape for CurlyBraces {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let label_padding_x = node.padding();
        let label_padding_y = node.padding();
        let w = out.bbox.width + label_padding_x;
        let h = out.bbox.height + label_padding_y;
        let radius = (h * 0.1).max(5.0);
        let gcp = generate_circle_points;

        let mut left = Vec::new();
        left.extend(gcp(w / 2.0, -h / 2.0, radius, 30, -90.0, 0.0));
        left.push(Point::new(-w / 2.0 - radius, radius));
        left.extend(gcp(
            w / 2.0 + radius * 2.0,
            -radius,
            radius,
            20,
            -180.0,
            -270.0,
        ));
        left.extend(gcp(
            w / 2.0 + radius * 2.0,
            radius,
            radius,
            20,
            -90.0,
            -180.0,
        ));
        left.push(Point::new(-w / 2.0 - radius, -h / 2.0));
        left.extend(gcp(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0));

        let mut right = Vec::new();
        right.extend(gcp(
            -w / 2.0 + radius + radius / 2.0,
            -h / 2.0,
            radius,
            20,
            -90.0,
            -180.0,
        ));
        right.push(Point::new(w / 2.0 - radius / 2.0, radius));
        right.extend(gcp(-w / 2.0 - radius / 2.0, -radius, radius, 20, 0.0, 90.0));
        right.extend(gcp(-w / 2.0 - radius / 2.0, radius, radius, 20, -90.0, 0.0));
        right.push(Point::new(w / 2.0 - radius / 2.0, -radius));
        right.extend(gcp(
            -w / 2.0 + radius + radius / 2.0,
            h / 2.0,
            radius,
            30,
            -180.0,
            -270.0,
        ));

        let mut rect_points = vec![
            Point::new(w / 2.0, -h / 2.0 - radius),
            Point::new(-w / 2.0, -h / 2.0 - radius),
        ];
        rect_points.extend(gcp(w / 2.0, -h / 2.0, radius, 20, -90.0, 0.0));
        rect_points.push(Point::new(-w / 2.0 - radius, -radius));
        rect_points.extend(gcp(
            w / 2.0 + radius * 2.0,
            -radius,
            radius,
            20,
            -180.0,
            -270.0,
        ));
        rect_points.extend(gcp(
            w / 2.0 + radius * 2.0,
            radius,
            radius,
            20,
            -90.0,
            -180.0,
        ));
        rect_points.push(Point::new(-w / 2.0 - radius, h / 2.0));
        rect_points.extend(gcp(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0));
        rect_points.push(Point::new(-w / 2.0, h / 2.0 + radius));
        rect_points.push(Point::new(
            w / 2.0 - radius - radius / 2.0,
            h / 2.0 + radius,
        ));
        rect_points.extend(gcp(
            -w / 2.0 + radius + radius / 2.0,
            -h / 2.0,
            radius,
            20,
            -90.0,
            -180.0,
        ));
        rect_points.push(Point::new(w / 2.0 - radius / 2.0, radius));
        rect_points.extend(gcp(-w / 2.0 - radius / 2.0, -radius, radius, 20, 0.0, 90.0));
        rect_points.extend(gcp(-w / 2.0 - radius / 2.0, radius, radius, 20, -90.0, 0.0));
        rect_points.push(Point::new(w / 2.0 - radius / 2.0, -radius));
        rect_points.extend(gcp(
            -w / 2.0 + radius + radius / 2.0,
            h / 2.0,
            radius,
            30,
            -180.0,
            -270.0,
        ));

        let style = node_style(ctx, node);
        let mut rect_style = style.clone();
        rect_style.stroke_opacity = Some(0.0);
        let shape = group_element(
            "text",
            translate(radius - radius / 4.0, 0.0),
            vec![
                rough_stroke_element(&create_open_path_from_points(&right), style.clone()),
                rough_stroke_element(&create_open_path_from_points(&left), style),
                rough_stroke_element(&create_path_from_points(&rect_points), rect_style),
            ],
        );
        out.label.common.transform = translate(
            -w / 2.0 + node.padding() / 2.0 - out.bbox.x,
            -h / 2.0 + node.padding() / 2.0 - out.bbox.y,
        );
        let bbox = points_bbox(&left)
            .union(points_bbox(&right))
            .union(points_bbox(&rect_points));
        update_node_bounds(node, bbox);
        node.intersector = Intersector::Polygon(rect_points);
        Ok(done(out, vec![shape], node))
    }
}
