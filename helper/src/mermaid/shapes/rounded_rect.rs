//! `shapes/roundedRect.ts`.

use super::draw_rect::{RectOptions, draw_rect};
use super::{Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;

pub struct RoundedRect;

impl Shape for RoundedRect {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        // `themeVariables?.radius ?? 5`.
        let radius = ctx.vars.radius;
        let options = RectOptions {
            rx: radius,
            ry: radius,
            label_padding_x: node.padding(),
            label_padding_y: node.padding(),
        };
        Ok(draw_rect(node, ctx, options))
    }
}
