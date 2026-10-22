//! `shapes/browser.ts`: a window with a chrome bar of dots and an address
//! bar hint.

use super::util::{
    circle_element, done, get_node_classes, group_element, label_helper, line_element, node_style,
    rect_element, translate,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Paint, Style};

pub struct Browser;

impl Shape for Browser {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let accent = ctx.vars.node_border;
        let padding = node.padding.unwrap_or(12.0);
        let bar_height = 18.0;
        let radius = 12.0;
        let w = (out.bbox.width + padding * 2.0).max(node.width()).max(90.0);
        let h = (out.bbox.height + padding * 2.0 + bar_height).max(node.height());
        let top = -h / 2.0;
        let accent_stroke = Style {
            stroke: Some(Paint::Color(accent)),
            stroke_width: Some(1.0),
            ..Style::default()
        };
        let mut children = vec![
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
            line_element(
                -w / 2.0,
                top + bar_height,
                w / 2.0,
                top + bar_height,
                "",
                accent_stroke.clone(),
            ),
        ];
        for i in 0..3 {
            children.push(circle_element(
                -w / 2.0 + 12.0 + f64::from(i) * 9.0,
                top + bar_height / 2.0,
                2.5,
                "",
                Style {
                    fill: Some(Paint::Color(accent)),
                    stroke: Some(Paint::None),
                    ..Style::default()
                },
            ));
        }
        children.push(rect_element(
            -w / 2.0 + 44.0,
            top + 4.0,
            (w - 56.0).max(10.0),
            bar_height - 8.0,
            Some(3.0),
            Some(3.0),
            "browser-address-bar",
            Style {
                fill: Some(Paint::None),
                opacity: Some(0.6),
                ..accent_stroke
            },
        ));
        let group = group_element("basic label-container", Vec::new(), children);
        node.width = Some(w);
        node.height = Some(h);
        let body_center_y = top + bar_height + (h - bar_height) / 2.0;
        out.label.common.transform = translate(
            -(out.bbox.width / 2.0) - out.bbox.x,
            body_center_y - out.bbox.height / 2.0 - out.bbox.y,
        );
        node.intersector = Intersector::Rect;
        Ok(done(out, vec![group], node))
    }
}
