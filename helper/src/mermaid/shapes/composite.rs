//! `shapes/composite.ts`: the block diagram composite. `positioned` is a
//! block diagram field the node model does not carry, so the natural-size
//! branch is the one ported.

use super::util::{done, label_helper, node_style, rect_element};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;

pub struct Composite;

impl Shape for Composite {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let classes = format!("node {}", node.css_classes).trim_end().to_string();
        let out = label_helper(node, ctx, Some(&classes), false);
        let node_padding = node.padding();
        let total_width = out.bbox.width + node_padding;
        let total_height = out.bbox.height + node_padding;
        let x = -out.bbox.width / 2.0 - out.half_padding;
        let y = -out.bbox.height / 2.0 - out.half_padding;
        let rect = rect_element(
            x,
            y,
            total_width,
            total_height,
            node.rx,
            node.ry,
            "basic cluster composite label-container",
            node_style(ctx, node),
        );
        node.width = Some(total_width);
        node.height = Some(total_height);
        node.intersector = Intersector::Rect;
        Ok(done(out, vec![rect], node))
    }
}
