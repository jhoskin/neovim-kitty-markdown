//! `shapes/erBox.ts`: the entity table of ER diagrams.

use super::draw_rect::{RectOptions, draw_rect};
use super::util::{
    BBox, build_label_group, calculate_text_width, get_node_classes, label_fill, measure_text,
    node_group, node_style, paint_style, path_element, rect_element, translate,
};
use super::{ErAttribute, Intersector, Node, NodeExtra, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Color, Element, Group, Paint, PathData, Point};

/// `addText(shapeSvg, labelText, config, translateX, translateY, classes,
/// style)`: a left-aligned, top-anchored text block.
fn add_text(ctx: &ShapeCtx, node: &Node, fill: Color, text: &str, class: &str) -> (Group, BBox) {
    let width = calculate_text_width(ctx, node, text) + 100.0;
    let measured = measure_text(ctx, node, text, Some(width), true);
    let group = build_label_group(
        &measured,
        ctx,
        fill,
        &format!("label {class}"),
        translate(0.0, 0.0),
    );
    (
        group,
        BBox {
            x: 0.0,
            y: 0.0,
            width: measured.width,
            height: measured.height,
        },
    )
}

/// `lineToPolygon(x1, y1, x2, y2, thickness)`: a divider as a hairline
/// polygon.
fn line_to_polygon(x1: f64, y1: f64, x2: f64, y2: f64, thickness: f64) -> PathData {
    let points = if x1 == x2 {
        vec![
            Point::new(x1 - thickness / 2.0, y1),
            Point::new(x1 + thickness / 2.0, y1),
            Point::new(x2 + thickness / 2.0, y2),
            Point::new(x2 - thickness / 2.0, y2),
        ]
    } else {
        vec![
            Point::new(x1, y1 - thickness / 2.0),
            Point::new(x1, y1 + thickness / 2.0),
            Point::new(x2, y2 + thickness / 2.0),
            Point::new(x2, y2 - thickness / 2.0),
        ]
    };
    super::util::create_path_from_points(&points)
}

struct Row {
    y_offset: f64,
    row_height: f64,
}

pub struct ErBox;

impl Shape for ErBox {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let (alias, attributes): (Option<String>, Vec<ErAttribute>) = match &node.extra {
            NodeExtra::Entity { alias, attributes } => (alias.clone(), attributes.clone()),
            _ => (None, Vec::new()),
        };
        if let Some(alias) = alias {
            node.label = Some(alias);
        }
        let mut padding = ctx.er_diagram_padding;
        let mut text_padding = ctx.er_entity_padding;
        let fill = label_fill(ctx, node);

        if attributes.is_empty() && node.label.as_deref().is_some_and(|l| !l.is_empty()) {
            let options = RectOptions {
                rx: 0.0,
                ry: 0.0,
                label_padding_x: padding,
                label_padding_y: padding * 1.5,
            };
            let label = node.label.clone().unwrap_or_default();
            if calculate_text_width(ctx, node, &label) + options.label_padding_x * 2.0
                < ctx.er_min_entity_width
            {
                node.width = Some(ctx.er_min_entity_width);
            }
            return Ok(draw_rect(node, ctx, options));
        }

        if !ctx.html_labels {
            padding *= 1.25;
            text_padding *= 1.25;
        }
        let mut shape_svg = node_group(node, &get_node_classes(node, None));
        let label = node.label.clone().unwrap_or_default();
        let (mut name_group, mut name_bbox) = add_text(ctx, node, fill, &label, "name");
        name_bbox.height += text_padding;

        let mut y_offset = 0.0;
        let mut rows: Vec<Row> = Vec::new();
        let mut cells: Vec<(Group, BBox, usize, f64)> = Vec::new();
        let mut max_type_width = 0.0_f64;
        let mut max_name_width = 0.0_f64;
        let mut max_keys_width = 0.0_f64;
        let mut max_comment_width = 0.0_f64;
        let mut shape_width = name_bbox.width;
        for attribute in &attributes {
            let (tg, tb) = add_text(ctx, node, fill, &attribute.type_name, "attribute-type");
            max_type_width = max_type_width.max(tb.width + padding);
            let (ng, nb) = add_text(ctx, node, fill, &attribute.name, "attribute-name");
            max_name_width = max_name_width.max(nb.width + padding);
            let (kg, kb) = add_text(ctx, node, fill, &attribute.keys.join(","), "attribute-keys");
            max_keys_width = max_keys_width.max(kb.width + padding);
            let (cg, cb) = add_text(ctx, node, fill, &attribute.comment, "attribute-comment");
            max_comment_width = max_comment_width.max(cb.width + padding);
            let row_height = tb.height.max(nb.height).max(kb.height).max(cb.height) + text_padding;
            for b in [&tb, &nb, &kb, &cb] {
                shape_width = shape_width.max(b.width);
            }
            cells.push((tg, tb, 0, y_offset));
            cells.push((ng, nb, 1, y_offset));
            cells.push((kg, kb, 2, y_offset));
            cells.push((cg, cb, 3, y_offset));
            rows.push(Row {
                y_offset,
                row_height,
            });
            y_offset += row_height;
        }
        let mut total_width_sections = 4.0;
        let mut keys_present = true;
        let mut comment_present = true;
        if max_keys_width <= padding {
            keys_present = false;
            max_keys_width = 0.0;
            total_width_sections -= 1.0;
        }
        if max_comment_width <= padding {
            comment_present = false;
            max_comment_width = 0.0;
            total_width_sections -= 1.0;
        }
        let columns_sum = max_type_width + max_name_width + max_keys_width + max_comment_width;
        if name_bbox.width + padding * 2.0 - columns_sum > 0.0 {
            let difference = name_bbox.width + padding * 2.0 - columns_sum;
            max_type_width += difference / total_width_sections;
            max_name_width += difference / total_width_sections;
            if max_keys_width > 0.0 {
                max_keys_width += difference / total_width_sections;
            }
            if max_comment_width > 0.0 {
                max_comment_width += difference / total_width_sections;
            }
        }
        let max_width = max_type_width + max_name_width + max_keys_width + max_comment_width;
        let total_rows_height: f64 = rows.iter().map(|r| r.row_height).sum();
        let w = (shape_width + padding * 2.0)
            .max(node.width())
            .max(max_width);
        let h = (total_rows_height + name_bbox.height).max(node.height());
        let x = -w / 2.0;
        let y = -h / 2.0;

        for (group, _bbox, column, ty) in &mut cells {
            let column_x = match column {
                1 => max_type_width,
                2 => max_type_width + max_name_width,
                3 => max_type_width + max_name_width + max_keys_width,
                _ => 0.0,
            };
            group.common.transform = translate(
                x + padding / 2.0 + column_x,
                *ty + y + name_bbox.height + text_padding / 2.0,
            );
        }
        name_group.common.transform = translate(-name_bbox.width / 2.0, y + text_padding / 2.0);

        let style = node_style(ctx, node);
        shape_svg
            .children
            .push(rect_element(x, y, w, h, None, None, "outer-path", style));
        for (i, row) in rows.iter().enumerate() {
            let content_row_index = i + 1;
            let is_even = content_row_index % 2 == 0 && row.y_offset != 0.0;
            let fill = if is_even {
                ctx.vars.row_even
            } else {
                ctx.vars.row_odd
            };
            shape_svg.children.push(rect_element(
                x,
                name_bbox.height + y + row.y_offset,
                w,
                row.row_height,
                None,
                None,
                if is_even {
                    "row-rect-even"
                } else {
                    "row-rect-odd"
                },
                paint_style(
                    &ctx.vars,
                    Paint::Color(fill),
                    Paint::Color(ctx.vars.node_border),
                ),
            ));
        }
        shape_svg.children.push(Element::Group(name_group));
        for (group, _, _, _) in cells {
            shape_svg.children.push(Element::Group(group));
        }

        let thickness = 0.0001;
        let divider_style = node_style(ctx, node);
        let mut dividers = vec![line_to_polygon(
            x,
            name_bbox.height + y,
            w + x,
            name_bbox.height + y,
            thickness,
        )];
        dividers.push(line_to_polygon(
            max_type_width + x,
            name_bbox.height + y,
            max_type_width + x,
            h + y,
            thickness,
        ));
        if keys_present {
            let xc = max_type_width + max_name_width + x;
            dividers.push(line_to_polygon(
                xc,
                name_bbox.height + y,
                xc,
                h + y,
                thickness,
            ));
        }
        if comment_present {
            let xc = max_type_width + max_name_width + max_keys_width + x;
            dividers.push(line_to_polygon(
                xc,
                name_bbox.height + y,
                xc,
                h + y,
                thickness,
            ));
        }
        // `yOffsets` only ever holds `0`, so this repeats the header line.
        dividers.push(line_to_polygon(
            x,
            name_bbox.height + y,
            w + x,
            name_bbox.height + y,
            thickness,
        ));
        for d in dividers {
            shape_svg.children.push(path_element(
                d,
                "divider",
                divider_style.clone(),
                Vec::new(),
            ));
        }
        node.width = Some(w);
        node.height = Some(h);
        node.intersector = Intersector::Rect;
        Ok(ShapeOut {
            group: shape_svg,
            width: w,
            height: h,
        })
    }
}
