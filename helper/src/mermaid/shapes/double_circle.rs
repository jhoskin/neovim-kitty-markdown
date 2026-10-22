//! `shapes/doubleCircle.ts`.

use super::util::{circle_element, done, get_node_classes, label_helper, node_style};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Common, Element, Group};

pub struct DoubleCircle;

impl Shape for DoubleCircle {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let gap = 5.0;
        let padding = node.padding();
        let label_padding = padding;
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), true);
        let label_radius = (out.bbox.width.powi(2) + out.bbox.height.powi(2)).sqrt() / 2.0;
        let inner_radius = if node.width.is_some_and(|w| w != 0.0) {
            node.width() / 2.0
        } else {
            label_radius
        } + label_padding;
        let outer_radius = inner_radius + gap;
        let style = node_style(ctx, node);
        let group = Element::Group(Group {
            common: Common {
                class: Some("basic label-container".to_string()),
                style: style.clone(),
                ..Common::default()
            },
            children: vec![
                circle_element(0.0, 0.0, outer_radius, "outer-circle", style.clone()),
                circle_element(0.0, 0.0, inner_radius, "inner-circle", style),
            ],
        });
        node.width = Some(outer_radius * 2.0);
        node.height = Some(outer_radius * 2.0);
        node.intersector = Intersector::Circle(outer_radius);
        Ok(done(out, vec![group], node))
    }
}
