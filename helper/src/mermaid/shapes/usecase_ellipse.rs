//! `shapes/usecaseEllipse.ts`.

use super::util::{done, get_node_classes, label_helper, node_style};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Common, Element, Ellipse};

pub struct UsecaseEllipse;

impl Shape for UsecaseEllipse {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let padding = out.half_padding;
        let radius_x = out.bbox.width / 2.0 + padding * 2.0;
        let radius_y = out.bbox.height / 2.0 + padding * 2.0;
        let ellipse = Element::Ellipse(Ellipse {
            common: Common {
                class: Some("basic label-container".to_string()),
                style: node_style(ctx, node),
                ..Common::default()
            },
            cx: 0.0,
            cy: 0.0,
            rx: radius_x,
            ry: radius_y,
        });
        node.width = Some(radius_x * 2.0);
        node.height = Some(radius_y * 2.0);
        node.intersector = Intersector::Ellipse {
            rx: radius_x,
            ry: radius_y,
        };
        Ok(done(out, vec![ellipse], node))
    }
}
