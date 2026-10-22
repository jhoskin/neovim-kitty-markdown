//! `shapes/datastore.ts`: a rect whose dash array `width height` draws only
//! the top and bottom edges.

use super::draw_rect::{RectOptions, draw_rect};
use super::{Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Element;

pub struct Datastore;

impl Shape for Datastore {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let padding = node.padding();
        let options = RectOptions {
            rx: 0.0,
            ry: 0.0,
            label_padding_x: node.label_padding_x.unwrap_or(padding * 2.0),
            label_padding_y: node.label_padding_y.unwrap_or(padding),
        };
        let mut out = draw_rect(node, ctx, options);
        if let Some(Element::Rect(rect)) = out.group.children.first_mut()
            && rect.width > 0.0
            && rect.height > 0.0
        {
            rect.common.style.stroke_dasharray = Some(vec![rect.width, rect.height]);
        }
        Ok(out)
    }
}
