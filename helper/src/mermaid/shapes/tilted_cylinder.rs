//! `shapes/tiltedCylinder.ts`: the horizontal cylinder.

use super::intersect::{self, Bounds};
use super::util::{
    done, get_node_classes, label_helper, node_style, path_bbox, path_element, translate,
    update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{PathData, Point};

const MIN_HEIGHT: f64 = 5.0;
const MIN_WIDTH: f64 = 10.0;

/// `createCylinderPathD` of `tiltedCylinder.ts` at `(0, 0)`.
pub fn tilted_cylinder_path(w: f64, h: f64, rx: f64, ry: f64) -> PathData {
    let mut d = PathData::new();
    d.move_to(0.0, 0.0)
        .arc_to(rx, ry, 0.0, false, true, 0.0, -h)
        .line_to(w, -h)
        .arc_to(rx, ry, 0.0, false, true, w, 0.0)
        .move_to(w, -h)
        .arc_to(rx, ry, 0.0, false, false, w, 0.0)
        .line_to(0.0, 0.0);
    d
}

/// The `node.intersect` closure of `tiltedCylinder.ts`.
pub fn intersect_tilted(node: &Bounds, rx: f64, ry: f64, point: Point) -> Point {
    let mut pos = intersect::rect(node, point);
    let y = pos.y - node.y;
    if ry != 0.0
        && (y.abs() < node.height / 2.0
            || (y.abs() == node.height / 2.0 && (pos.x - node.x).abs() > node.width / 2.0 - rx))
    {
        let mut x = rx * rx * (1.0 - (y * y) / (ry * ry));
        if x != 0.0 {
            x = x.abs().sqrt();
        }
        x = rx - x;
        if point.x - node.x > 0.0 {
            x = -x;
        }
        pos.x += x;
    }
    pos
}

pub struct TiltedCylinder;

impl Shape for TiltedCylinder {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding = node_padding / 2.0;
        let original_height = node.height();
        if node.height.is_some_and(|h| h != 0.0) {
            node.height = Some((node.height() - label_padding).max(MIN_HEIGHT));
        }
        if node.width.is_some_and(|w| w != 0.0) {
            let ry = original_height / 2.0;
            let rx = ry / (2.5 + original_height / 50.0);
            node.width = Some((node.width() - label_padding - rx * 3.0).max(MIN_WIDTH));
        }
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let h = node.height().max(out.bbox.height) + label_padding;
        let ry = h / 2.0;
        let rx = ry / (2.5 + h / 50.0);
        let w = node.width().max(out.bbox.width) + rx + label_padding;

        let d = tilted_cylinder_path(w, h, rx, ry);
        let bbox = path_bbox(&d);
        let path = path_element(
            d,
            "basic label-container outer-path",
            node_style(ctx, node),
            translate(-w / 2.0, h / 2.0),
        );
        out.label.common.transform = translate(
            -(out.bbox.width / 2.0) - rx - out.bbox.x,
            -(out.bbox.height / 2.0) - out.bbox.y,
        );
        update_node_bounds(node, bbox);
        node.intersector = Intersector::TiltedCylinder { rx, ry };
        Ok(done(out, vec![path], node))
    }
}
