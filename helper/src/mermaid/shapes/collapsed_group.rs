//! `shapes/collapsedGroup.ts`: a collapsed subgraph (`@{ view: collapsed }`).
//!
//! The rect sets `fill`/`stroke` attributes to the cluster colors, but it
//! sits under `.node rect { fill: mainBkg; stroke: nodeBorder }`, which
//! wins; the separator and the dots take `clusterBorder` from their own
//! rules.

use super::util::{
    circle_element, done, get_node_classes, label_helper, line_element, node_style, rect_element,
    translate,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Paint, Style};

const INDICATOR_ROW_HEIGHT: f64 = 20.0;
const SEPARATOR_GAP: f64 = 8.0;
const MIN_WIDTH: f64 = 80.0;
const RADIUS: f64 = 8.0;

pub struct CollapsedGroup;

impl Shape for CollapsedGroup {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let stroke = ctx.vars.cluster_border;
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let padding = node.padding.unwrap_or(8.0);
        let title_height = out.bbox.height;
        let total_width = (out.bbox.width + padding * 2.0)
            .max(MIN_WIDTH)
            .max(node.width());
        let total_height = (title_height + SEPARATOR_GAP + INDICATOR_ROW_HEIGHT + padding * 2.0)
            .max(node.height());
        let x = -total_width / 2.0;
        let y = -total_height / 2.0;
        let label_shift_y = -(SEPARATOR_GAP + INDICATOR_ROW_HEIGHT) / 2.0;
        out.label.common.transform = if ctx.use_html_labels(node) {
            translate(
                -out.bbox.width / 2.0,
                -out.bbox.height / 2.0 + label_shift_y,
            )
        } else {
            translate(0.0, -out.bbox.height / 2.0 + label_shift_y)
        };
        let mut elements = vec![rect_element(
            x,
            y,
            total_width,
            total_height,
            Some(RADIUS),
            Some(RADIUS),
            "basic label-container collapsed-group",
            node_style(ctx, node),
        )];
        let separator_y = y + padding + title_height + SEPARATOR_GAP;
        elements.push(line_element(
            x + 8.0,
            separator_y,
            x + total_width - 8.0,
            separator_y,
            "collapsed-separator",
            Style {
                stroke: Some(Paint::Color(stroke)),
                stroke_width: Some(0.75),
                stroke_dasharray: Some(vec![3.0, 3.0]),
                ..Style::default()
            },
        ));
        let dot_y = separator_y + INDICATOR_ROW_HEIGHT / 2.0;
        for i in -1..=1 {
            elements.push(circle_element(
                f64::from(i) * 10.0,
                dot_y,
                2.5,
                "collapsed-indicator",
                Style {
                    fill: Some(Paint::Color(stroke)),
                    stroke: Some(Paint::None),
                    opacity: Some(0.6),
                    ..Style::default()
                },
            ));
        }
        node.width = Some(total_width);
        node.height = Some(total_height);
        node.intersector = Intersector::Rect;
        Ok(done(out, elements, node))
    }
}
