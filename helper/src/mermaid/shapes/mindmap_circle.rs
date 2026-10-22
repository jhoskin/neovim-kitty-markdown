//! `shapes/mindmapCircle.ts`: `circle` with the node's own padding.

use super::circle::draw_circle;
use super::{Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;

pub struct MindmapCircle;

impl Shape for MindmapCircle {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        Ok(draw_circle(node, ctx, Some(node.padding())))
    }
}
