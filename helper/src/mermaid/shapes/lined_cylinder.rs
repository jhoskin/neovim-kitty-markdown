//! `shapes/linedCylinder.ts`: disk storage.

use super::cylinder::cylinder_path;
use super::util::{
    done, get_node_classes, label_helper, node_style, path_bbox, path_element, translate,
    update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;

const MIN_HEIGHT: f64 = 10.0;
const MIN_WIDTH: f64 = 10.0;

pub struct LinedCylinder;

impl Shape for LinedCylinder {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
            let original_width = node.width();
            node.width = Some((node.width() - label_padding_x).max(MIN_WIDTH));
            let rx = original_width / 2.0;
            let ry = rx / (2.5 + original_width / 50.0);
            node.height = Some((node.height() - label_padding_y - ry * 3.0).max(MIN_HEIGHT));
        }
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let w = if node.width.is_some_and(|w| w != 0.0) {
            node.width()
        } else {
            out.bbox.width
        } + label_padding_x * 2.0;
        let rx = w / 2.0;
        let ry = rx / (2.5 + w / 50.0);
        let h = if node.height.is_some_and(|h| h != 0.0) {
            node.height()
        } else {
            out.bbox.height
        } + ry
            + label_padding_y * 2.0;
        let outer_offset = h * 0.1;

        let mut d = cylinder_path(0.0, 0.0, w, h, rx, ry);
        d.move_to(0.0, ry + outer_offset)
            .arc_to(rx, ry, 0.0, false, false, w, ry + outer_offset);
        let bbox = path_bbox(&d);
        let path = path_element(
            d,
            "basic label-container outer-path",
            node_style(ctx, node),
            translate(-w / 2.0, -(h / 2.0 + ry)),
        );
        update_node_bounds(node, bbox);
        out.label.common.transform = translate(
            -(out.bbox.width / 2.0) - out.bbox.x,
            -(out.bbox.height / 2.0) + ry - out.bbox.y,
        );
        node.intersector = Intersector::Cylinder { rx, ry };
        Ok(done(out, vec![path], node))
    }
}
