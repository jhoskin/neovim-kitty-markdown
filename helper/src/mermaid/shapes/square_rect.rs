//! `shapes/squareRect.ts`.

use super::draw_rect::{RectOptions, draw_rect};
use super::{Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;

pub struct SquareRect;

impl Shape for SquareRect {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        // Classic: twice the padding on x, once on y (neo uses 16 and 12).
        let label_padding_x = node_padding * 2.0;
        let label_padding_y = node_padding;
        let options = RectOptions {
            rx: 0.0,
            ry: 0.0,
            label_padding_x: node.label_padding_x.unwrap_or(label_padding_x),
            label_padding_y,
        };
        Ok(draw_rect(node, ctx, options))
    }
}
