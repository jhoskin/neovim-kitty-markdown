//! `shapes/labelRect.ts` `labelRect`: the 0.1 by 0.1 node that carries an
//! edge label or a self-loop dummy.

use super::util::{done, label_helper, rect_element, translate};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Style;

pub struct LabelRect;

impl Shape for LabelRect {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut out = label_helper(node, ctx, Some("label"), false);
        let total_width = 0.1;
        let total_height = 0.1;
        let rect = rect_element(
            0.0,
            0.0,
            total_width,
            total_height,
            None,
            None,
            "",
            Style::default(),
        );
        out.shape_svg.common.class = Some("label edgeLabel".to_string());
        out.label.common.transform = translate(
            -(out.bbox.width / 2.0) - out.bbox.x,
            -(out.bbox.height / 2.0) - out.bbox.y,
        );
        node.width = Some(total_width);
        node.height = Some(total_height);
        node.intersector = Intersector::Rect;
        Ok(done(out, vec![rect], node))
    }
}
