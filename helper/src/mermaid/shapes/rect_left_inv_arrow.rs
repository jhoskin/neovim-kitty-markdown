//! `shapes/rectLeftInvArrow.ts`: the `odd` shape.

use super::util::{
    create_path_from_points, done, get_node_classes, label_helper, node_style, path_element,
    points_bbox, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct RectLeftInvArrow;

impl Shape for RectLeftInvArrow {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let label_width = out.bbox.width + label_padding_x;
        let h = (out.bbox.height + label_padding_y).max(node.height());
        let notch_width = h / 4.0;
        let w = label_width.max(node.width() - notch_width);
        let x = -w / 2.0;
        let y = -h / 2.0;
        let notch = y / 2.0;
        let points = vec![
            Point::new(x + notch, y),
            Point::new(x, 0.0),
            Point::new(x + notch, -y),
            Point::new(-x, -y),
            Point::new(-x, y),
        ];
        let path = path_element(
            create_path_from_points(&points),
            "basic label-container outer-path",
            node_style(ctx, node),
            translate(-notch / 2.0, 0.0),
        );
        out.label.common.transform = translate(
            -notch / 2.0 - out.bbox.width / 2.0 - out.bbox.x,
            -(out.bbox.height / 2.0) - out.bbox.y,
        );
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![path], node))
    }
}
