//! `shapes/kanbanItem.ts`: a kanban card with title, ticket, and assignee.

use super::util::{
    PAGE_TEXT_COLOR, build_label_group, done, get_node_classes, insert_label, label_fill,
    label_helper, line_element, node_style, rect_element, translate,
};
use super::{Intersector, Node, NodeExtra, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Color, Common, Element, Group, Paint, Style};

/// `colorFromPriority(priority)`; `Medium` draws no stroke.
fn color_from_priority(priority: &str) -> Option<Color> {
    match priority {
        "Very High" => Some(Color::rgb(255, 0, 0)),
        "High" => Some(Color::rgb(255, 165, 0)),
        "Low" => Some(Color::rgb(0, 0, 255)),
        "Very Low" => Some(Color::rgb(173, 216, 230)),
        _ => None,
    }
}

pub struct KanbanItem;

impl Shape for KanbanItem {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let data = match &node.extra {
            NodeExtra::Kanban(k) => k.clone(),
            _ => Default::default(),
        };
        let label_padding_x = 10.0;
        let org_width = node.width;
        node.width = Some(node.width.unwrap_or(200.0) - 10.0);
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let padding = node.padding.filter(|p| *p != 0.0).unwrap_or(10.0);
        // `insertLabel` gets `useHtmlLabels: kanbanNode.useHtmlLabels`,
        // which `getData` never sets, so the ticket and the assignee follow
        // `getEffectiveHtmlLabels` alone; only the title went through
        // `labelHelper`'s `node.useHtmlLabels || evaluate(htmlLabels)`.
        let meta_node = Node {
            use_html_labels: None,
            ..node.clone()
        };
        let (mut label_el, mut bbox2) = insert_label(
            data.ticket.as_deref().unwrap_or(""),
            ctx,
            &meta_node,
            node.width,
            false,
        );
        // With `ticketBaseUrl` the ticket label goes into an `<a
        // class="kanban-ticket-link">`, whose `text-decoration: underline`
        // the label inherits. The scene has no link, so a group stands in.
        let link = data.ticket.as_deref().is_some_and(|t| !t.is_empty())
            && ctx.kanban_ticket_base_url.is_some();
        if link {
            bbox2.underline = Some(PAGE_TEXT_COLOR);
            label_el = build_label_group(
                &bbox2,
                ctx,
                label_fill(ctx, node),
                "label",
                label_el.common.transform.clone(),
            );
        }
        let (mut label_el_assigned, bbox_assigned) = insert_label(
            data.assigned.as_deref().unwrap_or(""),
            ctx,
            &meta_node,
            node.width,
            false,
        );
        node.width = org_width;
        let label_padding_y = 10.0;
        let total_width = node.width();
        let height_adj = bbox2.height.max(bbox_assigned.height) / 2.0;
        let total_height =
            (out.bbox.height + label_padding_y * 2.0).max(node.height()) + height_adj;
        let x = -total_width / 2.0;
        let y = -total_height / 2.0;
        out.label.common.transform = translate(
            padding - total_width / 2.0,
            -height_adj - out.bbox.height / 2.0,
        );
        label_el.common.transform = translate(
            padding - total_width / 2.0,
            -height_adj + out.bbox.height / 2.0,
        );
        label_el_assigned.common.transform = translate(
            padding + total_width / 2.0 - bbox_assigned.width - 2.0 * label_padding_x,
            -height_adj + out.bbox.height / 2.0,
        );
        let rx = node.rx;
        let ry = node.ry;
        let elements = vec![rect_element(
            x,
            y,
            total_width,
            total_height,
            Some(rx.unwrap_or(5.0)),
            Some(ry.unwrap_or(5.0)),
            "basic label-container __APA__",
            node_style(ctx, node),
        )];
        let mut tail = Vec::new();
        if let Some(priority) = data.priority.as_deref()
            && let Some(color) = color_from_priority(priority)
        {
            let line_x = x + 2.0;
            let y1 = y + (rx.unwrap_or(0.0) / 2.0).floor();
            let y2 = y + total_height - (rx.unwrap_or(0.0) / 2.0).floor();
            tail.push(line_element(
                line_x,
                y1,
                line_x,
                y2,
                "",
                Style {
                    stroke: Some(Paint::Color(color)),
                    stroke_width: Some(4.0),
                    ..Style::default()
                },
            ));
        }
        node.width = Some(total_width);
        node.height = Some(total_height);
        node.intersector = Intersector::Rect;
        let mut result = done(out, elements, node);
        if link {
            // `shapeSvg.insert('svg:a', ':first-child')` after the title,
            // then the rect in front of it: rect, link, title, assigned.
            let anchor = Group {
                common: Common {
                    class: Some("kanban-ticket-link".to_string()),
                    ..Common::default()
                },
                children: vec![Element::Group(label_el)],
            };
            result.group.children.insert(1, Element::Group(anchor));
        } else {
            result.group.children.push(Element::Group(label_el));
        }
        result
            .group
            .children
            .push(Element::Group(label_el_assigned));
        result.group.children.extend(tail);
        Ok(result)
    }
}
