//! `shapes/forkJoin.ts`: the fork/join bar, perpendicular to the flow.

use super::util::{apply_declarations, get_node_classes, node_group, rect_element, styles2string};
use super::{Dir, Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Paint, Style};

pub struct ForkJoin;

impl Shape for ForkJoin {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        node.label = Some(String::new());
        let mut shape_svg = node_group(node, &get_node_classes(node, None));
        let mut width = node.width().max(70.0);
        let mut height = node.height().max(10.0);
        if matches!(ctx.dir, Some(Dir::LR) | Some(Dir::RL)) {
            width = node.width().max(10.0);
            height = node.height().max(70.0);
        }
        let x = -width / 2.0;
        let y = -height / 2.0;
        let mut style = Style {
            fill: Some(Paint::Color(ctx.vars.line_color)),
            stroke: Some(Paint::Color(ctx.vars.line_color)),
            stroke_width: Some(ctx.vars.stroke_width),
            ..Style::default()
        };
        apply_declarations(&mut style, &styles2string(node).node_styles);
        shape_svg
            .children
            .push(rect_element(x, y, width, height, None, None, "", style));
        node.width = Some(width);
        node.height = Some(height);
        let padding = ctx.state_padding;
        if node.width() != 0.0 && node.height() != 0.0 {
            node.width = Some(node.width() + padding / 2.0);
            node.height = Some(node.height() + padding / 2.0);
        }
        node.intersector = Intersector::Rect;
        Ok(ShapeOut {
            group: shape_svg,
            width: node.width(),
            height: node.height(),
        })
    }
}
