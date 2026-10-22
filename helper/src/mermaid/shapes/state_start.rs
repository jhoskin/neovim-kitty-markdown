//! `shapes/stateStart.ts`: the filled start dot. `.node circle.state-start
//! { fill: specialStateColor; stroke: specialStateColor }` paints it.

use super::util::{circle_element, node_group, paint_style};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Paint;

/// The `width`/`height` floor and default of the start and end dots.
pub(crate) fn fix_state_dot_size(node: &mut Node) {
    if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
        if node.width() < 14.0 {
            node.width = Some(14.0);
        }
        if node.height() < 14.0 {
            node.height = Some(14.0);
        }
    }
    if !node.width.is_some_and(|w| w != 0.0) {
        node.width = Some(14.0);
    }
    if !node.height.is_some_and(|h| h != 0.0) {
        node.height = Some(14.0);
    }
}

pub struct StateStart;

impl Shape for StateStart {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        fix_state_dot_size(node);
        let mut shape_svg = node_group(node, "node default");
        let r = node.width() / 2.0;
        let color = Paint::Color(ctx.vars.special_state_color);
        shape_svg.children.push(circle_element(
            0.0,
            0.0,
            r,
            "state-start",
            paint_style(&ctx.vars, color.clone(), color),
        ));
        node.width = Some(r * 2.0);
        node.height = Some(r * 2.0);
        node.intersector = Intersector::Circle(r);
        Ok(ShapeOut {
            group: shape_svg,
            width: r * 2.0,
            height: r * 2.0,
        })
    }
}
