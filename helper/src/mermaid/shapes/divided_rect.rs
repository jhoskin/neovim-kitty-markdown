//! `shapes/dividedRect.ts`.

use super::util::{
    create_path_from_points, done, get_node_classes, label_helper, node_style, path_element,
    points_bbox, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct DividedRectangle;

impl Shape for DividedRectangle {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let padding_x = node.padding();
        let padding_y = node.padding();
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let w = out.bbox.width + padding_x;
        let h = out.bbox.height + padding_y;
        let rect_offset = h * 0.2;
        let x = -w / 2.0;
        let y = -h / 2.0 - rect_offset / 2.0;
        let pts = vec![
            Point::new(x, y + rect_offset),
            Point::new(-x, y + rect_offset),
            Point::new(-x, -y),
            Point::new(x, -y),
            Point::new(x, y),
            Point::new(-x, y),
            Point::new(-x, y + rect_offset),
        ];
        let path = path_element(
            create_path_from_points(&pts),
            "basic label-container outer-path",
            node_style(ctx, node),
            Vec::new(),
        );
        out.label.common.transform = translate(
            x + node.padding() / 2.0 - out.bbox.x,
            y + rect_offset + node.padding() / 2.0 - out.bbox.y,
        );
        update_node_bounds(node, points_bbox(&pts));
        node.intersector = Intersector::Rect;
        Ok(done(out, vec![path], node))
    }
}
