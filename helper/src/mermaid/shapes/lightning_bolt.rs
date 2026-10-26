//! `shapes/lightningBolt.ts`: the com-link shape, label-less.

use super::util::{
    create_path_from_points, get_node_classes, node_group, node_style, points_bbox,
    rough_path_element, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct LightningBolt;

impl Shape for LightningBolt {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        node.label = Some(String::new());
        let mut shape_svg = node_group(node, &get_node_classes(node, None));
        let width = node.width().max(35.0);
        let height = node.height().max(35.0);
        let gap = 7.0;
        let points = vec![
            Point::new(width, 0.0),
            Point::new(0.0, height + gap / 2.0),
            Point::new(width - 2.0 * gap, height + gap / 2.0),
            Point::new(0.0, 2.0 * height),
            Point::new(width, height - gap / 2.0),
            Point::new(2.0 * gap, height - gap / 2.0),
        ];
        shape_svg.children.push(rough_path_element(
            create_path_from_points(&points),
            "outer-path",
            node_style(ctx, node),
            translate(-width / 2.0, -height),
            &ctx.vars,
        ));
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(ShapeOut {
            group: shape_svg,
            width: node.width(),
            height: node.height(),
        })
    }
}
