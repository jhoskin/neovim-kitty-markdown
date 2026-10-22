//! `shapes/usecaseJsonTable.ts`: a titled key/value table.

use super::util::{
    get_node_classes, group_element, insert_label, label_helper, node_style, rect_element,
    styles2string, translate,
};
use super::{Intersector, Node, NodeExtra, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Element, Group};

const CELL_PADDING_X: f64 = 8.0;
const CELL_PADDING_Y: f64 = 4.0;
const DEFAULT_BORDER_WIDTH: f64 = 1.0;

/// `positionLabel(label, bbox, centerX, centerY)`.
fn position_label(label: &mut Group, bbox: &super::LabelBox, cx: f64, cy: f64) {
    label.common.transform = translate(
        cx - bbox.width / 2.0 - bbox.x,
        cy - bbox.height / 2.0 - bbox.y,
    );
}

pub struct UsecaseJsonTable;

impl Shape for UsecaseJsonTable {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let rows = match &node.extra {
            NodeExtra::Usecase { rows, .. } => rows.clone(),
            _ => Vec::new(),
        };
        let mut out = label_helper(
            node,
            ctx,
            Some(&get_node_classes(node, Some("usecase-json-table"))),
            false,
        );
        out.label.common.class = Some("label usecase-json-title".to_string());
        let measured: Vec<_> = rows
            .iter()
            .map(|(k, v)| {
                let (mut kg, kb) = insert_label(k, ctx, node, None, true);
                kg.common.class = Some("label usecase-json-key".to_string());
                let (mut vg, vb) = insert_label(v, ctx, node, None, true);
                vg.common.class = Some("label usecase-json-value".to_string());
                (kg, kb, vg, vb)
            })
            .collect();
        let key_width = measured
            .iter()
            .map(|(_, kb, _, _)| kb.width)
            .fold(0.0, f64::max)
            + CELL_PADDING_X * 2.0;
        let measured_value_width = measured
            .iter()
            .map(|(_, _, _, vb)| vb.width)
            .fold(0.0, f64::max)
            + CELL_PADDING_X * 2.0;
        let title_width = out.bbox.width + CELL_PADDING_X * 2.0;
        let inner_width = title_width.max(key_width + measured_value_width);
        let value_width =
            measured_value_width + (inner_width - key_width - measured_value_width).max(0.0);
        let title_height = out.bbox.height + CELL_PADDING_Y * 2.0;
        let row_heights: Vec<f64> = measured
            .iter()
            .map(|(_, kb, _, vb)| kb.height.max(vb.height) + CELL_PADDING_Y * 2.0)
            .collect();
        let inner_height = title_height + row_heights.iter().sum::<f64>();
        let border_width = styles2string(node)
            .node_styles
            .get("stroke-width")
            .and_then(|s| s.trim_end_matches("px").parse::<f64>().ok())
            .filter(|v| *v >= 0.0)
            .unwrap_or(DEFAULT_BORDER_WIDTH);
        let total_width = inner_width + border_width * 2.0;
        let total_height = inner_height + border_width * 2.0;
        let left = -inner_width / 2.0;
        let top = -inner_height / 2.0;
        let style = node_style(ctx, node);

        let outer = rect_element(
            -total_width / 2.0,
            -total_height / 2.0,
            total_width,
            total_height,
            None,
            None,
            "label-container usecase-json-border",
            style.clone(),
        );
        let mut table_children = vec![rect_element(
            left,
            top,
            inner_width,
            title_height,
            None,
            None,
            "usecase-json-cell usecase-json-title-cell",
            style.clone(),
        )];
        let mut title = std::mem::take(&mut out.label);
        position_label(&mut title, &out.bbox, 0.0, top + title_height / 2.0);
        table_children.push(Element::Group(title));
        let mut row_top = top + title_height;
        for ((mut kg, kb, mut vg, vb), row_height) in measured.into_iter().zip(row_heights) {
            position_label(&mut kg, &kb, left + key_width / 2.0, row_height / 2.0);
            position_label(
                &mut vg,
                &vb,
                left + key_width + value_width / 2.0,
                row_height / 2.0,
            );
            let row = group_element(
                "usecase-json-row",
                translate(0.0, row_top),
                vec![
                    rect_element(
                        left + key_width,
                        0.0,
                        value_width,
                        row_height,
                        None,
                        None,
                        "usecase-json-cell usecase-json-value-cell",
                        style.clone(),
                    ),
                    rect_element(
                        left,
                        0.0,
                        key_width,
                        row_height,
                        None,
                        None,
                        "usecase-json-cell usecase-json-key-cell",
                        style.clone(),
                    ),
                    Element::Group(kg),
                    Element::Group(vg),
                ],
            );
            table_children.push(row);
            row_top += row_height;
        }
        let mut group = out.shape_svg;
        group.children.push(outer);
        group.children.push(group_element(
            "usecase-json-table-grid",
            Vec::new(),
            table_children,
        ));
        node.width = Some(total_width);
        node.height = Some(total_height);
        node.intersector = Intersector::Rect;
        Ok(ShapeOut {
            group,
            width: total_width,
            height: total_height,
        })
    }
}
