//! `shapes/text.ts`: a label with an invisible box (`rect.text { fill:
//! none; stroke-width: 0 }`).

use super::util::{
    apply_declarations, done, get_node_classes, label_helper, rect_element, styles2string,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Paint, Style};

pub struct TextShape;

impl Shape for TextShape {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let total_width = (out.bbox.width + node.padding()).max(node.width());
        let total_height = (out.bbox.height + node.padding()).max(node.height());
        let x = -total_width / 2.0;
        let y = -total_height / 2.0;
        let mut style = Style {
            fill: Some(Paint::None),
            stroke: Some(Paint::Color(ctx.vars.node_border)),
            stroke_width: Some(0.0),
            ..Style::default()
        };
        apply_declarations(&mut style, &styles2string(node).node_styles);
        let rect = rect_element(
            x,
            y,
            total_width,
            total_height,
            Some(0.0),
            Some(0.0),
            "text",
            style,
        );
        node.width = Some(total_width);
        node.height = Some(total_height);
        node.intersector = Intersector::Rect;
        Ok(done(out, vec![rect], node))
    }
}
