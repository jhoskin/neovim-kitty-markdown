//! `shapes/bucket.ts`: an open-top container with an elliptical rim.

use super::util::{
    done, get_node_classes, group_element, label_helper, node_style, path_bbox, path_element,
    translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Common, Element, Ellipse, Paint, PathData, Point, Style};

pub struct Bucket;

impl Shape for Bucket {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let accent = ctx.vars.node_border;
        let padding = node.padding.unwrap_or(12.0);
        let w = (out.bbox.width + padding * 2.0).max(node.width()).max(80.0);
        let rim_ry = (w * 0.08).clamp(5.0, 12.0);
        let total_height = (out.bbox.height + padding * 2.0 + rim_ry).max(node.height());
        let top_y = -total_height / 2.0 + rim_ry;
        let bottom_y = total_height / 2.0;
        let bottom_w = w * 0.72;
        let mut body = PathData::new();
        body.move_to(-w / 2.0, top_y)
            .line_to(-bottom_w / 2.0, bottom_y)
            .arc_to(
                bottom_w / 2.0,
                rim_ry,
                0.0,
                false,
                false,
                bottom_w / 2.0,
                bottom_y,
            )
            .line_to(w / 2.0, top_y)
            .arc_to(w / 2.0, rim_ry, 0.0, false, false, -w / 2.0, top_y)
            .close();
        let mut bbox = path_bbox(&body);
        let rim = Element::Ellipse(Ellipse {
            common: Common::with_style(Style {
                fill: Some(Paint::None),
                stroke: Some(Paint::Color(accent)),
                stroke_width: Some(1.0),
                ..Style::default()
            }),
            cx: 0.0,
            cy: top_y,
            rx: w / 2.0,
            ry: rim_ry,
        });
        bbox = bbox.union(super::util::BBox {
            x: -w / 2.0,
            y: top_y - rim_ry,
            width: w,
            height: rim_ry * 2.0,
        });
        let group = group_element(
            "basic label-container",
            Vec::new(),
            vec![
                path_element(body, "", node_style(ctx, node), Vec::new()),
                rim,
            ],
        );
        update_node_bounds(node, bbox);
        let body_center_y = top_y + (bottom_y - top_y) / 2.0;
        out.label.common.transform = translate(
            -(out.bbox.width / 2.0) - out.bbox.x,
            body_center_y - out.bbox.height / 2.0 - out.bbox.y,
        );

        let arc_segments = 12;
        let arc = |rx: f64, cy: f64, y_sign: f64| -> Vec<Point> {
            (0..=arc_segments)
                .map(|i| {
                    let theta = std::f64::consts::PI
                        - (f64::from(i) * std::f64::consts::PI) / f64::from(arc_segments);
                    Point::new(rx * theta.cos(), cy + y_sign * rim_ry * theta.sin())
                })
                .collect()
        };
        let mut outline = arc(w / 2.0, top_y, -1.0);
        let mut bottom = arc(bottom_w / 2.0, bottom_y, 1.0);
        bottom.reverse();
        outline.extend(bottom);
        node.intersector = Intersector::Polygon(outline);
        Ok(done(out, vec![group], node))
    }
}
