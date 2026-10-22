//! `shapes/rectWithTitle.ts`: the state with a title row over a
//! description.

use super::util::{
    build_label_group, label_fill, line_element, measure_text, node_style, rect_element, translate,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Common, Element, Group, Paint, Style};

pub struct RectWithTitle;

impl Shape for RectWithTitle {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let classes = if node.css_classes.is_empty() {
            "node default".to_string()
        } else {
            format!("node {}", node.css_classes)
        };
        let mut shape_svg = Group {
            common: Common {
                id: Some(node.dom_id().to_string()),
                class: Some(classes),
                ..Common::default()
            },
            children: Vec::new(),
        };
        let fill = label_fill(ctx, node);
        let title = node.label.clone().unwrap_or_default();
        let title_box = measure_text(ctx, node, &title, None, false);
        let description = node.description.join("<br/>");
        let descr_box = measure_text(ctx, node, &description, None, false);
        let half_padding = node.padding() / 2.0;

        let descr_x = if descr_box.width > title_box.width {
            0.0
        } else {
            (title_box.width - descr_box.width) / 2.0
        };
        let descr_y = title_box.height + half_padding + 5.0;
        let title_x = if descr_box.width < title_box.width {
            0.0
        } else {
            -(title_box.width - descr_box.width) / 2.0
        };
        let title_group = build_label_group(&title_box, ctx, fill, "", translate(title_x, 0.0));
        let descr_group = build_label_group(&descr_box, ctx, fill, "", translate(descr_x, descr_y));

        // `label.getBBox()`: the union of both text boxes.
        let min_x = title_x.min(descr_x);
        let max_x = (title_x + title_box.width).max(descr_x + descr_box.width);
        let bbox_w = max_x - min_x;
        let bbox_h = if descr_box.height > 0.0 {
            descr_y + descr_box.height
        } else {
            title_box.height
        };
        let label = Group {
            common: Common {
                class: Some("label".to_string()),
                transform: translate(-bbox_w / 2.0, -bbox_h / 2.0 - half_padding + 3.0),
                ..Common::default()
            },
            children: vec![Element::Group(title_group), Element::Group(descr_group)],
        };

        let total_width = bbox_w + node.padding();
        let total_height = bbox_h + node.padding();
        let x = -bbox_w / 2.0 - half_padding;
        let y = -bbox_h / 2.0 - half_padding;
        let divider_y = y + title_box.height + half_padding;
        let g = Group {
            common: Common::default(),
            children: vec![
                rect_element(
                    x,
                    y,
                    total_width,
                    total_height,
                    None,
                    None,
                    "outer title-state",
                    node_style(ctx, node),
                ),
                line_element(
                    x,
                    divider_y,
                    bbox_w / 2.0 + half_padding,
                    divider_y,
                    "divider",
                    Style {
                        stroke: Some(Paint::Color(ctx.vars.node_border)),
                        stroke_width: Some(ctx.vars.stroke_width),
                        ..Style::default()
                    },
                ),
            ],
        };
        shape_svg.children.push(Element::Group(g));
        shape_svg.children.push(Element::Group(label));
        node.width = Some(total_width);
        node.height = Some(total_height);
        node.intersector = Intersector::Rect;
        Ok(ShapeOut {
            group: shape_svg,
            width: total_width,
            height: total_height,
        })
    }
}
