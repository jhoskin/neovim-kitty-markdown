//! `shapes/classBox.ts` with `diagrams/class/shapeUtil.ts` `textHelper`
//! and `addText`: the UML class box with annotation, title, members, and
//! methods compartments.
//!
//! `getBBox()` calls on the text groups are replaced by the union of the
//! measured text boxes at their transforms; the JS quirks that feed the
//! geometry (`0 - PADDING / 2 || 0` staying `-6`, the `translate(x)` regex
//! miss on the annotation group) are reproduced where they change a number.

use super::util::{
    BBox, build_label_group, calculate_text_width, get_node_classes, label_fill, line_element,
    measure_text, node_group, node_style, rect_element, translate,
};
use super::{ClassMember, Classifier, Intersector, Node, NodeExtra, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{
    Color, Common, Element, FontSpec, FontStyle, FontWeight, Group, Paint, Style,
};

/// One text block placed by `addText`: the measured box and the group's
/// `translate(0, -height / (2 * lines) + yOffset)`.
struct Block {
    group: Group,
    bbox: BBox,
}

fn add_text(
    ctx: &ShapeCtx,
    node: &Node,
    fill: Color,
    text: &str,
    y_offset: f64,
    bold: bool,
    classifier: Classifier,
) -> Block {
    let width = calculate_text_width(ctx, node, text) + 50.0;
    let mut measured = measure_text(ctx, node, text, Some(width), true);
    let lines = measured.lines.len().max(1) as f64;
    if bold {
        for line in &mut measured.lines {
            for run in &mut line.runs {
                run.bold = true;
            }
        }
    }
    if classifier == Classifier::Abstract {
        for line in &mut measured.lines {
            for run in &mut line.runs {
                run.italic = true;
            }
        }
    }
    let ty = -measured.height / (2.0 * lines) + y_offset;
    let mut group = build_label_group(&measured, ctx, fill, "label", translate(0.0, ty));
    if bold || classifier != Classifier::None {
        group.common.style.font = FontSpec {
            weight: bold.then_some(FontWeight::Number(900)),
            style: (classifier == Classifier::Abstract).then_some(FontStyle::Italic),
            ..FontSpec::default()
        };
    }
    Block {
        group,
        bbox: BBox {
            x: 0.0,
            y: ty,
            width: measured.width,
            height: measured.height,
        },
    }
}

fn union(blocks: &[Block]) -> BBox {
    blocks.iter().fold(BBox::EMPTY, |acc, b| acc.union(b.bbox))
}

/// A `g.<class> text` compartment with its blocks.
fn compartment(class: &str, blocks: Vec<Block>, transform: (f64, f64)) -> (Element, BBox) {
    let bbox = union(&blocks);
    let group = Group {
        common: Common {
            class: Some(format!("{class} text")),
            transform: translate(transform.0, transform.1),
            ..Common::default()
        },
        children: blocks
            .into_iter()
            .map(|b| Element::Group(b.group))
            .collect(),
    };
    (Element::Group(group), bbox)
}

pub struct ClassBox;

impl Shape for ClassBox {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let (annotations, members, methods): (Vec<String>, Vec<ClassMember>, Vec<ClassMember>) =
            match &node.extra {
                NodeExtra::Class {
                    annotations,
                    members,
                    methods,
                } => (annotations.clone(), members.clone(), methods.clone()),
                _ => (Vec::new(), Vec::new(), Vec::new()),
            };
        let padding = ctx.class_padding;
        let gap = padding;
        let use_html_labels = node.use_html_labels.unwrap_or(ctx.html_labels);
        let text_padding = if use_html_labels { 0.0 } else { 3.0 };
        let fill = label_fill(ctx, node);

        // textHelper
        let mut shape_svg = node_group(node, &get_node_classes(node, None));
        let mut annotation_blocks = Vec::new();
        let mut annotation_group_height = 0.0;
        if let Some(annotation) = annotations.first() {
            let b = add_text(
                ctx,
                node,
                fill,
                &format!("«{annotation}»"),
                0.0,
                false,
                Classifier::None,
            );
            annotation_group_height = b.bbox.height;
            annotation_blocks.push(b);
        }
        let label = node.label.clone().unwrap_or_default();
        let label_block = add_text(ctx, node, fill, &label, 0.0, true, Classifier::None);
        let label_group_height = label_block.bbox.height;
        let label_group_width = label_block.bbox.width;

        let mut member_blocks = Vec::new();
        let mut y_offset = 0.0;
        for member in &members {
            let b = add_text(
                ctx,
                node,
                fill,
                &member.text,
                y_offset,
                false,
                member.classifier,
            );
            y_offset += b.bbox.height + text_padding;
            member_blocks.push(b);
        }
        let mut members_group_height = union(&member_blocks).height;
        if members_group_height <= 0.0 {
            members_group_height = gap / 2.0;
        }
        let mut method_blocks = Vec::new();
        let mut methods_y_offset = 0.0;
        for method in &methods {
            let b = add_text(
                ctx,
                node,
                fill,
                &method.text,
                methods_y_offset,
                false,
                method.classifier,
            );
            methods_y_offset += b.bbox.height + text_padding;
            method_blocks.push(b);
        }

        let annotation_width = union(&annotation_blocks).width;
        let ann_t = (-annotation_width / 2.0, 0.0);
        let label_t = (-label_group_width / 2.0, annotation_group_height);
        let members_t = (
            0.0,
            annotation_group_height + label_group_height + gap * 2.0,
        );
        let methods_t = (
            0.0,
            annotation_group_height + label_group_height + members_group_height + gap * 4.0,
        );
        let (ann_el, ann_bbox) = compartment("annotation-group", annotation_blocks, ann_t);
        let (label_el, label_bbox) = compartment("label-group", vec![label_block], label_t);
        let (members_el, members_bbox) = compartment("members-group", member_blocks, members_t);
        let (methods_el, methods_bbox) = compartment("methods-group", method_blocks, methods_t);
        let shape_bbox = ann_bbox
            .translated(ann_t.0, ann_t.1)
            .union(label_bbox.translated(label_t.0, label_t.1))
            .union(members_bbox.translated(members_t.0, members_t.1))
            .union(methods_bbox.translated(methods_t.0, methods_t.1));

        // classBox
        let render_extra_box =
            members.is_empty() && methods.is_empty() && !ctx.hide_empty_members_box;
        let empty = members.is_empty() && methods.is_empty();
        let w = node.width().max(shape_bbox.width);
        let mut h = node.height().max(shape_bbox.height);
        let node_height_greater = node.height() > shape_bbox.height;
        if empty {
            h += gap;
        } else if !members.is_empty() && methods.is_empty() {
            h += gap * 2.0;
        }
        let x = -w / 2.0;
        let y = -h / 2.0;
        let mut extra_height = if render_extra_box {
            padding * 2.0
        } else if empty {
            -padding
        } else {
            0.0
        };
        if node_height_greater {
            extra_height = padding * 2.0;
        }
        let top_adjust = if render_extra_box {
            padding
        } else if empty {
            -padding / 2.0
        } else {
            0.0
        };
        let rect_x = x - padding;
        let rect_y = y - padding - top_adjust;
        let rect_w = w + 2.0 * padding;
        let rect_h = h + 2.0 * padding + extra_height;
        let style = node_style(ctx, node);
        let rect = rect_element(
            rect_x,
            rect_y,
            rect_w,
            rect_h,
            None,
            None,
            "basic label-container outer-path",
            style.clone(),
        );

        // `bbox.height - (renderExtraBox ? PADDING / 2 : 0) || 0`: a `0 - 6`
        // stays `-6`, only an exact `0` falls back to `0`.
        let adj = |height: f64| -> f64 {
            let v = height - if render_extra_box { padding / 2.0 } else { 0.0 };
            if v == 0.0 { 0.0 } else { v }
        };
        let ann_h = adj(ann_bbox.height);
        let label_h = adj(label_bbox.height);
        let members_h = adj(members_bbox.height);
        let methods_area_placement = (ann_h + label_h + y + padding - rect_y) / 2.0;

        let mut compartments = vec![ann_el, label_el, members_el, methods_el];
        let translate_ys = [0.0, label_t.1, members_t.1, methods_t.1];
        let widths = [ann_bbox.width, label_bbox.width, 0.0, 0.0];
        for (i, el) in compartments.iter_mut().enumerate() {
            let Element::Group(g) = el else { continue };
            let translate_y = translate_ys[i];
            let mut new_translate_y = translate_y + y + padding - top_adjust;
            if i == 3 {
                let members_for_methods = members_h.max(gap / 2.0);
                new_translate_y = if node_height_greater {
                    methods_area_placement
                        .max(ann_h + label_h + members_for_methods + y + gap * 2.0 + padding)
                        + gap * 2.0
                } else {
                    ann_h + label_h + members_for_methods + y + gap * 4.0 + padding
                };
            }
            if empty && ctx.hide_empty_members_box {
                new_translate_y = if !annotations.is_empty() {
                    translate_y - gap
                } else {
                    translate_y
                };
            }
            if !use_html_labels {
                new_translate_y -= 4.0;
            }
            let mut new_translate_x = x;
            if i < 2 {
                // HTML labels have no `text` element with `text-anchor:
                // middle`, so the group is centred by its own width.
                new_translate_x = if use_html_labels {
                    -widths[i] / 2.0
                } else {
                    0.0
                };
            }
            g.common.transform = translate(new_translate_x, new_translate_y);
        }
        shape_svg.children.push(rect);
        shape_svg.children.extend(compartments);

        let divider_style = Style {
            stroke: Some(Paint::Color(ctx.vars.node_border)),
            stroke_width: Some(ctx.vars.stroke_width),
            fill: style.fill.clone(),
            ..Style::default()
        };
        if !members.is_empty() || !methods.is_empty() || render_extra_box {
            let first_line_y = ann_h + label_h + y + padding;
            shape_svg.children.push(line_element(
                rect_x,
                first_line_y,
                rect_x + rect_w,
                first_line_y + 0.001,
                "divider",
                divider_style.clone(),
            ));
            let second_line_y = ann_h + label_h + members_h + y + gap * 2.0 + padding;
            let ly = if node_height_greater {
                methods_area_placement.max(second_line_y)
            } else {
                second_line_y
            };
            shape_svg.children.push(line_element(
                rect_x,
                ly,
                rect_x + rect_w,
                ly + 0.001,
                "divider",
                divider_style,
            ));
        }
        node.width = Some(rect_w);
        node.height = Some(rect_h);
        node.intersector = Intersector::Rect;
        Ok(ShapeOut {
            group: shape_svg,
            width: rect_w,
            height: rect_h,
        })
    }
}
