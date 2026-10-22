//! `shapes/console.ts`: a rounded terminal window with a `>_` glyph.

use super::util::{
    done, get_node_classes, group_element, label_helper, node_style, rect_element, translate,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Common, Element, FontSpec, FontWeight, Paint, Style, Text, TextRun};

pub struct ConsoleWindow;

impl Shape for ConsoleWindow {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let accent = ctx.vars.node_border;
        let padding = node.padding.unwrap_or(12.0);
        let glyph_band = 20.0;
        let radius = 12.0;
        let w = (out.bbox.width + padding * 2.0).max(node.width()).max(90.0);
        let h = (out.bbox.height + padding * 2.0 + glyph_band).max(node.height());
        let top = -h / 2.0;
        let glyph = Element::Text(Text {
            common: Common {
                class: Some("console-glyph".to_string()),
                style: Style {
                    fill: Some(Paint::Color(accent)),
                    ..Style::default()
                },
                ..Common::default()
            },
            x: -w / 2.0 + 12.0,
            y: top + 16.0,
            anchor: None,
            baseline: None,
            font: FontSpec {
                family: Some("monospace".to_string()),
                size: Some(14.0),
                weight: Some(FontWeight::Bold),
                style: None,
            },
            runs: vec![TextRun::new(">_")],
        });
        let group = group_element(
            "basic label-container",
            Vec::new(),
            vec![
                rect_element(
                    -w / 2.0,
                    top,
                    w,
                    h,
                    Some(radius),
                    Some(radius),
                    "",
                    node_style(ctx, node),
                ),
                glyph,
            ],
        );
        node.width = Some(w);
        node.height = Some(h);
        let body_center_y = top + glyph_band + (h - glyph_band) / 2.0;
        out.label.common.transform = translate(
            -(out.bbox.width / 2.0) - out.bbox.x,
            body_center_y - out.bbox.height / 2.0 - out.bbox.y,
        );
        node.intersector = Intersector::Rect;
        Ok(done(out, vec![group], node))
    }
}
