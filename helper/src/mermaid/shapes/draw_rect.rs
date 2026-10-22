//! `shapes/drawRect.ts`: the rectangle every `*Rect` shape and `state`
//! delegate to.

use super::util::{done, get_node_classes, label_helper, node_style, rect_element};
use super::{Intersector, Node, ShapeCtx, ShapeOut};

/// `RectOptions` of `types.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RectOptions {
    pub rx: f64,
    pub ry: f64,
    pub label_padding_x: f64,
    pub label_padding_y: f64,
}

/// `drawRect(parent, node, options)`.
pub fn draw_rect(node: &mut Node, ctx: &ShapeCtx, options: RectOptions) -> ShapeOut {
    let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
    let total_width = (out.bbox.width + options.label_padding_x * 2.0).max(node.width());
    let total_height = (out.bbox.height + options.label_padding_y * 2.0).max(node.height());
    let x = -total_width / 2.0;
    let y = -total_height / 2.0;

    // `if (options?.rx && options.ry)`: both radii truthy override the node's.
    let (rx, ry) = if options.rx != 0.0 && options.ry != 0.0 {
        (Some(options.rx), Some(options.ry))
    } else {
        (node.rx, node.ry)
    };
    let rect = rect_element(
        x,
        y,
        total_width,
        total_height,
        rx,
        ry,
        "basic label-container",
        node_style(ctx, node),
    );
    node.width = Some(total_width);
    node.height = Some(total_height);
    node.intersector = Intersector::Rect;
    done(out, vec![rect], node)
}
