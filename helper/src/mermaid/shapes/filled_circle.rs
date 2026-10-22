//! `shapes/filledCircle.ts`: the junction dot, filled with `nodeBorder`.

use super::util::{
    apply_declarations, circle_element, get_node_classes, node_group, styles2string,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Paint, Style};

pub struct FilledCircle;

impl Shape for FilledCircle {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        node.label = Some(String::new());
        let mut shape_svg = node_group(node, &get_node_classes(node, None));
        let radius = 7.0;
        let mut style = Style {
            fill: Some(Paint::Color(ctx.vars.node_border)),
            stroke: Some(Paint::Color(ctx.vars.node_border)),
            stroke_width: Some(ctx.vars.stroke_width),
            ..Style::default()
        };
        apply_declarations(&mut style, &styles2string(node).node_styles);
        shape_svg
            .children
            .push(circle_element(0.0, 0.0, radius, "", style));
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
