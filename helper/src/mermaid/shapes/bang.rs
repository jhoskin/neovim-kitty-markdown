//! `shapes/bang.ts`: the mindmap bang, a chain of relative arcs.

use super::cloud::rel_arc;
use super::util::{
    done, get_node_classes, label_helper, node_style, path_bbox, path_element, translate,
    update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::PathData;

pub struct Bang;

impl Shape for Bang {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let w = out.bbox.width + 10.0 * out.half_padding;
        let h = out.bbox.height + 8.0 * out.half_padding;
        let r = 0.15 * w;
        let min_width = out.bbox.width + 20.0;
        let min_height = out.bbox.height + 20.0;
        let ew = w.max(min_width);
        let eh = h.max(min_height);
        out.label.common.transform = translate(-out.bbox.width / 2.0, -out.bbox.height / 2.0);
        let mut d = PathData::new();
        d.move_to(0.0, 0.0);
        let mut c = (0.0, 0.0);
        c = rel_arc(&mut d, c, r, r, 1.0, false, false, ew * 0.25, -eh * 0.1);
        c = rel_arc(&mut d, c, r, r, 1.0, false, false, ew * 0.25, 0.0);
        c = rel_arc(&mut d, c, r, r, 1.0, false, false, ew * 0.25, 0.0);
        c = rel_arc(&mut d, c, r, r, 1.0, false, false, ew * 0.25, eh * 0.1);
        c = rel_arc(&mut d, c, r, r, 1.0, false, false, ew * 0.15, eh * 0.33);
        c = rel_arc(
            &mut d,
            c,
            r * 0.8,
            r * 0.8,
            1.0,
            false,
            false,
            0.0,
            eh * 0.34,
        );
        c = rel_arc(&mut d, c, r, r, 1.0, false, false, -ew * 0.15, eh * 0.33);
        c = rel_arc(&mut d, c, r, r, 1.0, false, false, -ew * 0.25, eh * 0.15);
        c = rel_arc(&mut d, c, r, r, 1.0, false, false, -ew * 0.25, 0.0);
        c = rel_arc(&mut d, c, r, r, 1.0, false, false, -ew * 0.25, 0.0);
        c = rel_arc(&mut d, c, r, r, 1.0, false, false, -ew * 0.25, -eh * 0.15);
        c = rel_arc(&mut d, c, r, r, 1.0, false, false, -ew * 0.1, -eh * 0.33);
        c = rel_arc(
            &mut d,
            c,
            r * 0.8,
            r * 0.8,
            1.0,
            false,
            false,
            0.0,
            -eh * 0.34,
        );
        let end = rel_arc(&mut d, c, r, r, 1.0, false, false, ew * 0.1, -eh * 0.33);
        d.line_to(0.0, end.1).line_to(0.0, 0.0).close();
        let bbox = path_bbox(&d);
        let path = path_element(
            d,
            "basic label-container",
            node_style(ctx, node),
            translate(-ew / 2.0, -eh / 2.0),
        );
        update_node_bounds(node, bbox);
        node.intersector = Intersector::Rect;
        Ok(done(out, vec![path], node))
    }
}
