//! `shapes/choice.ts`: the state diagram choice diamond, always 28 wide.

use super::util::{
    create_path_from_points, get_node_classes, node_group, node_style, path_element,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct Choice;

impl Shape for Choice {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        node.label = Some(String::new());
        let mut shape_svg = node_group(node, &get_node_classes(node, None));
        let s = node.width().max(28.0);
        let points = vec![
            Point::new(0.0, s / 2.0),
            Point::new(s / 2.0, 0.0),
            Point::new(0.0, -s / 2.0),
            Point::new(-s / 2.0, 0.0),
        ];
        shape_svg.children.push(path_element(
            create_path_from_points(&points),
            "",
            node_style(ctx, node),
            Vec::new(),
        ));
        node.width = Some(28.0);
        node.height = Some(28.0);
        node.intersector = Intersector::Polygon(points);
        Ok(ShapeOut {
            group: shape_svg,
            width: 28.0,
            height: 28.0,
        })
    }
}
