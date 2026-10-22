//! `shapes/cylinder.ts`.

use super::intersect::{self, Bounds};
use super::util::{
    done, get_node_classes, label_helper, node_style, path_bbox, path_element, translate,
    update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{PathData, Point};

const MIN_HEIGHT: f64 = 8.0;
const MIN_WIDTH: f64 = 8.0;

/// `createCylinderPathD(x, y, width, height, rx, ry)`: relative arcs and
/// lines written as absolute commands.
pub fn cylinder_path(x: f64, y: f64, width: f64, height: f64, rx: f64, ry: f64) -> PathData {
    let mut d = PathData::new();
    d.move_to(x, y + ry)
        .arc_to(rx, ry, 0.0, false, false, x + width, y + ry)
        .arc_to(rx, ry, 0.0, false, false, x, y + ry)
        .line_to(x, y + ry + height)
        .arc_to(rx, ry, 0.0, false, false, x + width, y + ry + height)
        .line_to(x + width, y + ry);
    d
}

/// The `node.intersect` closure of `cylinder.ts`: a rect intersection
/// pulled onto the top or bottom cap.
pub fn intersect_cylinder(node: &Bounds, rx: f64, ry: f64, point: Point) -> Point {
    let mut pos = intersect::rect(node, point);
    let x = pos.x - node.x;
    if rx != 0.0
        && (x.abs() < node.width / 2.0
            || (x.abs() == node.width / 2.0 && (pos.y - node.y).abs() > node.height / 2.0 - ry))
    {
        let mut y = ry * ry * (1.0 - (x * x) / (rx * rx));
        if y > 0.0 {
            y = y.sqrt();
        }
        y = ry - y;
        if point.y - node.y > 0.0 {
            y = -y;
        }
        pos.y += y;
    }
    pos
}

pub struct Cylinder;

impl Shape for Cylinder {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        let original_width = node.width();
        if node.width.is_some_and(|w| w != 0.0) {
            node.width = Some((node.width() - label_padding_y).max(MIN_WIDTH));
        }
        if node.height.is_some_and(|h| h != 0.0) {
            let rx = original_width / 2.0;
            let ry = rx / (2.5 + original_width / 50.0);
            node.height = Some((node.height() - label_padding_x - ry * 3.0).max(MIN_HEIGHT));
        }
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let w = node.width().max(out.bbox.width) + label_padding_y;
        let rx = w / 2.0;
        let ry = rx / (2.5 + w / 50.0);
        let h = node.height().max(out.bbox.height) + label_padding_x + ry;

        let d = cylinder_path(0.0, 0.0, w, h, rx, ry);
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
            -(out.bbox.height / 2.0) + node.padding() / 1.5 - out.bbox.y,
        );
        node.intersector = Intersector::Cylinder { rx, ry };
        Ok(done(out, vec![path], node))
    }
}
