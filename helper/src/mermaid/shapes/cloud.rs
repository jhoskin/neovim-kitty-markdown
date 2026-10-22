//! `shapes/cloud.ts`: the mindmap cloud, a chain of relative arcs.

use super::util::{
    done, get_node_classes, label_helper, node_style, path_bbox, path_element, translate,
    update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::PathData;

/// Appends the relative arc `a rx,ry rot large,sweep dx,dy` as an absolute
/// command and returns the new current point.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rel_arc(
    d: &mut PathData,
    cur: (f64, f64),
    rx: f64,
    ry: f64,
    rot: f64,
    large: bool,
    sweep: bool,
    dx: f64,
    dy: f64,
) -> (f64, f64) {
    let to = (cur.0 + dx, cur.1 + dy);
    d.arc_to(rx, ry, rot, large, sweep, to.0, to.1);
    to
}

pub struct Cloud;

impl Shape for Cloud {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let w = out.bbox.width + 2.0 * out.half_padding;
        let h = out.bbox.height + 2.0 * out.half_padding;
        let r1 = 0.15 * w;
        let r2 = 0.25 * w;
        let r3 = 0.35 * w;
        let r4 = 0.2 * w;
        let mut d = PathData::new();
        d.move_to(0.0, 0.0);
        let mut c = (0.0, 0.0);
        c = rel_arc(&mut d, c, r1, r1, 0.0, false, true, w * 0.25, -w * 0.1);
        c = rel_arc(&mut d, c, r3, r3, 1.0, false, true, w * 0.4, -w * 0.1);
        c = rel_arc(&mut d, c, r2, r2, 1.0, false, true, w * 0.35, w * 0.2);
        c = rel_arc(&mut d, c, r1, r1, 1.0, false, true, w * 0.15, h * 0.35);
        c = rel_arc(&mut d, c, r4, r4, 1.0, false, true, -w * 0.15, h * 0.65);
        c = rel_arc(&mut d, c, r2, r1, 1.0, false, true, -w * 0.25, w * 0.15);
        c = rel_arc(&mut d, c, r3, r3, 1.0, false, true, -w * 0.5, 0.0);
        c = rel_arc(&mut d, c, r1, r1, 1.0, false, true, -w * 0.25, -w * 0.15);
        c = rel_arc(&mut d, c, r1, r1, 1.0, false, true, -w * 0.1, -h * 0.35);
        let _ = rel_arc(&mut d, c, r4, r4, 1.0, false, true, w * 0.1, -h * 0.65);
        d.line_to(0.0, c.1 - h * 0.65).line_to(0.0, 0.0).close();
        let bbox = path_bbox(&d);
        let path = path_element(
            d,
            "basic label-container",
            node_style(ctx, node),
            translate(-w / 2.0, -h / 2.0),
        );
        out.label.common.transform = translate(-out.bbox.width / 2.0, -out.bbox.height / 2.0);
        update_node_bounds(node, bbox);
        node.intersector = Intersector::Rect;
        Ok(done(out, vec![path], node))
    }
}
