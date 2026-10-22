//! `shapes/defaultMindmapNode.ts`: the rounded mindmap box with a base
//! line.

use super::util::{
    done, get_node_classes, label_helper, line_element, node_style, path_bbox, translate,
    update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Common, Element, Path, PathData, Style};

pub struct DefaultMindmapNode;

impl Shape for DefaultMindmapNode {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let w = out.bbox.width + 8.0 * out.half_padding;
        let h = out.bbox.height + 2.0 * out.half_padding;
        let rd = 5.0;
        // The classic path: relative `v`, `q`, `h` commands made absolute.
        let mut d = PathData::new();
        let (x0, y0) = (-w / 2.0, h / 2.0 - rd);
        d.move_to(x0, y0);
        let y1 = y0 - h + 2.0 * rd;
        d.line_to(x0, y1);
        d.quad_to(x0, y1 - rd, x0 + rd, y1 - rd);
        let x2 = x0 + rd + (w - 2.0 * rd);
        d.line_to(x2, y1 - rd);
        d.quad_to(x2 + rd, y1 - rd, x2 + rd, y1);
        let y3 = y1 + h - 2.0 * rd;
        d.line_to(x2 + rd, y3);
        d.quad_to(x2 + rd, y3 + rd, x2, y3 + rd);
        d.line_to(x2 - (w - 2.0 * rd), y3 + rd);
        d.quad_to(x0, y3 + rd, x0, y3);
        d.close();
        let bbox = path_bbox(&d);
        let bg = Element::Path(Path {
            common: Common {
                id: Some(node.dom_id().to_string()),
                class: Some("node-bkg node-".to_string()),
                style: node_style(ctx, node),
                ..Common::default()
            },
            d,
        });
        let line = line_element(
            -w / 2.0,
            h / 2.0,
            w / 2.0,
            h / 2.0,
            "node-line-",
            Style::default(),
        );
        out.label.common.transform = translate(-out.bbox.width / 2.0, -out.bbox.height / 2.0);
        update_node_bounds(node, bbox);
        node.intersector = Intersector::Rect;
        Ok(done(out, vec![bg, line], node))
    }
}
