//! `shapes/anchor.ts`: the 1 px dot an edge to a cluster is redirected to.
//! The stylesheet rule `.root .anchor path { fill: lineColor; stroke-width:
//! 0 }` decides its paint.

use super::util::{circle_element, get_node_classes, node_group};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Paint, Style};

pub struct Anchor;

impl Shape for Anchor {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut shape_svg = node_group(node, &get_node_classes(node, None));
        let radius = 1.0;
        let style = Style {
            fill: Some(Paint::Color(ctx.vars.line_color)),
            stroke: Some(Paint::Color(ctx.vars.line_color)),
            stroke_width: Some(0.0),
            ..Style::default()
        };
        shape_svg
            .children
            .push(circle_element(0.0, 0.0, radius, "anchor", style));
        node.width = Some(radius * 2.0);
        node.height = Some(radius * 2.0);
        node.intersector = Intersector::Circle(radius);
        Ok(ShapeOut {
            group: shape_svg,
            width: radius * 2.0,
            height: radius * 2.0,
        })
    }
}
