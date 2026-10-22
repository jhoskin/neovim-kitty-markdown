//! `shapes/curlyBraceLeft.ts`: the comment shape.
//!
//! The brace is drawn with `fill: 'none'` but sits under `.node path`,
//! whose `fill: mainBkg` wins over the attribute, so both paths take the
//! node style; the box path only hides its stroke (`stroke-opacity 0`).

use super::util::{
    create_open_path_from_points, create_path_from_points, done, generate_circle_points,
    get_node_classes, group_element, label_helper, node_style, path_element, points_bbox,
    translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct CurlyBraceLeft;

impl Shape for CurlyBraceLeft {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let padding_x = node.padding();
        let padding_y = node.padding();
        let w = out.bbox.width + padding_x;
        let h = out.bbox.height + padding_y;
        let radius = (h * 0.1).max(5.0);

        let mut points = Vec::new();
        points.extend(generate_circle_points(
            w / 2.0,
            -h / 2.0,
            radius,
            30,
            -90.0,
            0.0,
        ));
        points.push(Point::new(-w / 2.0 - radius, radius));
        points.extend(generate_circle_points(
            w / 2.0 + radius * 2.0,
            -radius,
            radius,
            20,
            -180.0,
            -270.0,
        ));
        points.extend(generate_circle_points(
            w / 2.0 + radius * 2.0,
            radius,
            radius,
            20,
            -90.0,
            -180.0,
        ));
        points.push(Point::new(-w / 2.0 - radius, -h / 2.0));
        points.extend(generate_circle_points(
            w / 2.0,
            h / 2.0,
            radius,
            20,
            0.0,
            90.0,
        ));

        let mut rect_points = vec![
            Point::new(w / 2.0, -h / 2.0 - radius),
            Point::new(-w / 2.0, -h / 2.0 - radius),
        ];
        rect_points.extend(generate_circle_points(
            w / 2.0,
            -h / 2.0,
            radius,
            20,
            -90.0,
            0.0,
        ));
        rect_points.push(Point::new(-w / 2.0 - radius, -radius));
        rect_points.extend(generate_circle_points(
            w / 2.0 + w * 0.1,
            -radius,
            radius,
            20,
            -180.0,
            -270.0,
        ));
        rect_points.extend(generate_circle_points(
            w / 2.0 + w * 0.1,
            radius,
            radius,
            20,
            -90.0,
            -180.0,
        ));
        rect_points.push(Point::new(-w / 2.0 - radius, h / 2.0));
        rect_points.extend(generate_circle_points(
            w / 2.0,
            h / 2.0,
            radius,
            20,
            0.0,
            90.0,
        ));
        rect_points.push(Point::new(-w / 2.0, h / 2.0 + radius));
        rect_points.push(Point::new(w / 2.0, h / 2.0 + radius));

        let style = node_style(ctx, node);
        let mut rect_style = style.clone();
        rect_style.stroke_opacity = Some(0.0);
        let shape = group_element(
            "text",
            translate(radius, 0.0),
            vec![
                path_element(create_open_path_from_points(&points), "", style, Vec::new()),
                path_element(
                    create_path_from_points(&rect_points),
                    "",
                    rect_style,
                    Vec::new(),
                ),
            ],
        );
        out.label.common.transform = translate(
            -w / 2.0 + radius - out.bbox.x,
            -h / 2.0 + node.padding() / 2.0 - out.bbox.y,
        );
        update_node_bounds(node, points_bbox(&points).union(points_bbox(&rect_points)));
        node.intersector = Intersector::Polygon(rect_points);
        Ok(done(out, vec![shape], node))
    }
}
