//! `shapes/person.ts`: a circular head above a rounded body.

use super::util::{
    circle_element, done, generate_circle_points, get_node_classes, group_element, label_helper,
    node_style, rect_element, translate,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;

pub struct Person;

impl Shape for Person {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let padding = node.padding.unwrap_or(20.0);
        let w = (out.bbox.width + padding * 2.0)
            .max(node.width())
            .max(100.0);
        let head_radius = (w * 0.23).clamp(16.0, 56.0);
        let overlap = head_radius * 0.27;
        let body_height = (out.bbox.height + padding * 2.0).max(if node.height() != 0.0 {
            node.height() - (2.0 * head_radius - overlap)
        } else {
            0.0
        });
        let body_radius = (w * 0.177).min(body_height * 0.45);
        let total_height = body_height + 2.0 * head_radius - overlap;
        let top = -total_height / 2.0;
        let body_top = top + 2.0 * head_radius - overlap;

        let style = node_style(ctx, node);
        let group = group_element(
            "basic label-container",
            Vec::new(),
            vec![
                rect_element(
                    -w / 2.0,
                    body_top,
                    w,
                    body_height,
                    Some(body_radius),
                    Some(body_radius),
                    "",
                    style.clone(),
                ),
                circle_element(0.0, top + head_radius, head_radius, "", style),
            ],
        );
        node.width = Some(w);
        node.height = Some(total_height);
        let body_center_y = body_top + body_height / 2.0;
        out.label.common.transform = translate(
            -(out.bbox.width / 2.0) - out.bbox.x,
            body_center_y - out.bbox.height / 2.0 - out.bbox.y,
        );

        let head_center_y = top + head_radius;
        let phi_right_deg = ((body_top - head_center_y) / head_radius).min(1.0).asin() * 180.0
            / std::f64::consts::PI;
        let mut outline = generate_circle_points(
            0.0,
            -head_center_y,
            head_radius,
            24,
            180.0 + phi_right_deg,
            -phi_right_deg,
        );
        outline.extend(generate_circle_points(
            -(-w / 2.0 + body_radius),
            -(body_top + body_radius),
            body_radius,
            12,
            90.0,
            0.0,
        ));
        outline.extend(generate_circle_points(
            -(-w / 2.0 + body_radius),
            -(total_height / 2.0 - body_radius),
            body_radius,
            12,
            360.0,
            270.0,
        ));
        outline.extend(generate_circle_points(
            -(w / 2.0 - body_radius),
            -(total_height / 2.0 - body_radius),
            body_radius,
            12,
            270.0,
            180.0,
        ));
        outline.extend(generate_circle_points(
            -(w / 2.0 - body_radius),
            -(body_top + body_radius),
            body_radius,
            12,
            180.0,
            90.0,
        ));
        node.intersector = Intersector::Polygon(outline);
        Ok(done(out, vec![group], node))
    }
}
