//! `shapes/state.ts`: `drawRect` with radius 5 and no label padding.
//!
//! The TS passes no `labelPaddingX`/`labelPaddingY`, so the box is `NaN`
//! wide in a browser; mermaid never selects this shape (state diagrams use
//! `rect` and `roundedRect`). The padding reads as `0` here so the port
//! stays total.

use super::draw_rect::{RectOptions, draw_rect};
use super::{Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;

pub struct State;

impl Shape for State {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let options = RectOptions {
            rx: 5.0,
            ry: 5.0,
            label_padding_x: 0.0,
            label_padding_y: 0.0,
        };
        Ok(draw_rect(node, ctx, options))
    }
}
