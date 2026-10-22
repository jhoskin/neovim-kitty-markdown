//! `shapes/circle.ts`.

use super::util::{circle_element, done, get_node_classes, label_helper, node_style};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;

/// `circle(parent, node, options)`; `padding` is `MindmapOptions.padding`
/// when the mindmap passes it, else `halfPadding`.
pub fn draw_circle(node: &mut Node, ctx: &ShapeCtx, padding: Option<f64>) -> ShapeOut {
    let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), true);
    let padding = padding.unwrap_or(out.half_padding);
    let label_radius = (out.bbox.width.powi(2) + out.bbox.height.powi(2)).sqrt() / 2.0;
    let radius = label_radius + padding;
    let circle = circle_element(
        0.0,
        0.0,
        radius,
        "basic label-container",
        node_style(ctx, node),
    );
    node.width = Some(radius * 2.0);
    node.height = Some(radius * 2.0);
    node.intersector = Intersector::Circle(radius);
    done(out, vec![circle], node)
}

pub struct Circle;

impl Shape for Circle {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        Ok(draw_circle(node, ctx, None))
    }
}
