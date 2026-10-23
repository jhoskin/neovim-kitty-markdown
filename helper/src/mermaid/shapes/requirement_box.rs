//! `shapes/requirementBox.ts`: requirement and element boxes.

use super::measure::LabelOpts;
use super::util::{
    BBox, build_label_group, calculate_text_width, get_node_classes, label_fill, label_font,
    line_element, node_group, node_style, rect_element, styles2string, translate,
};
use super::{Intersector, Node, NodeExtra, RequirementData, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Color, Element, FontSpec, FontWeight, Group, Style};

/// `userNodeOverrides`: rough.js strokes the box and the divider at
/// `stroke-width` from the node styles, else 1.3. No requirement stylesheet
/// rule overrides the attribute (`.divider` sets the group's stroke, which
/// the path's own attribute beats).
const ROUGH_STROKE_WIDTH: f64 = 1.3;

/// `addText(parentGroup, inputText, yOffset, style, alignment)`: a text
/// block centred on `(0, yOffset)`. Returns `None` for an empty string
/// (the JS returns `0` without inserting anything).
fn add_text(
    ctx: &ShapeCtx,
    node: &Node,
    fill: Color,
    text: &str,
    y_offset: f64,
    bold: bool,
) -> Option<(Group, BBox)> {
    if text.is_empty() {
        return None;
    }
    let html = ctx.use_html_labels(node);
    let width = calculate_text_width(ctx, text) + 50.0;
    // `node.labelStyle + '; font-weight: bold;'` reaches the div, so the
    // name is measured bold.
    let font = FontSpec {
        weight: bold.then_some(FontWeight::Bold),
        ..label_font(ctx, node)
    };
    let opts = LabelOpts {
        font,
        html,
        width: Some(width),
        min_width: 0.0,
        markdown: true,
    };
    let mut measured = ctx.measurer.measure(text, &opts);
    if !html {
        measured.height += 6.0;
    }
    if bold {
        for line in &mut measured.lines {
            for run in &mut line.runs {
                run.bold = true;
            }
        }
    }
    let tx = -measured.width / 2.0;
    let ty = -measured.height / 2.0 + y_offset;
    let mut group = build_label_group(&measured, ctx, fill, "label", translate(tx, ty));
    if !html {
        // `createFormattedText(..., centerText = !isNode)`: node labels keep
        // the start anchor, and no requirement rule centres them.
        for child in &mut group.children {
            if let Element::Text(t) = child {
                t.anchor = None;
            }
        }
    }
    if bold {
        group.common.style.font = FontSpec {
            weight: Some(FontWeight::Bold),
            ..FontSpec::default()
        };
    }
    Some((
        group,
        BBox {
            x: tx,
            y: ty,
            width: measured.width,
            height: measured.height,
        },
    ))
}

pub struct RequirementBox;

impl Shape for RequirementBox {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let data = match &node.extra {
            NodeExtra::Requirement(r) => r.clone(),
            _ => RequirementData::Element {
                name: node.label.clone().unwrap_or_default(),
                type_name: String::new(),
                doc_ref: String::new(),
            },
        };
        let padding = 20.0;
        let gap = 20.0;
        let fill = label_fill(ctx, node);
        let mut shape_svg = node_group(node, &get_node_classes(node, None));
        let mut blocks: Vec<(Group, BBox)> = Vec::new();

        let type_label = match &data {
            // The entities stay encoded so the markdown pass reads them as
            // text, not as an HTML tag.
            RequirementData::Requirement { type_name, .. } => {
                format!("&lt;&lt;{type_name}&gt;&gt;")
            }
            RequirementData::Element { .. } => "&lt;&lt;Element&gt;&gt;".to_string(),
        };
        let type_height = match add_text(ctx, node, fill, &type_label, 0.0, false) {
            Some((g, b)) => {
                blocks.push((g, b));
                b.height
            }
            None => 0.0,
        };
        let mut accumulative_height = type_height;
        let name = match &data {
            RequirementData::Requirement { name, .. } | RequirementData::Element { name, .. } => {
                name.clone()
            }
        };
        let name_height = match add_text(ctx, node, fill, &name, accumulative_height, true) {
            Some((g, b)) => {
                blocks.push((g, b));
                b.height
            }
            None => 0.0,
        };
        accumulative_height += name_height + gap;

        let body: Vec<String> = match &data {
            RequirementData::Requirement {
                requirement_id,
                text,
                risk,
                verify_method,
                ..
            } => vec![
                prefixed("ID: ", requirement_id),
                prefixed("Text: ", text),
                prefixed("Risk: ", risk),
                prefixed("Verification: ", verify_method),
            ],
            RequirementData::Element {
                type_name, doc_ref, ..
            } => vec![
                prefixed("Type: ", type_name),
                prefixed("Doc Ref: ", doc_ref),
            ],
        };
        for (i, line) in body.iter().enumerate() {
            let height = match add_text(ctx, node, fill, line, accumulative_height, false) {
                Some((g, b)) => {
                    blocks.push((g, b));
                    b.height
                }
                None => 0.0,
            };
            // The last line's height is never accumulated in the JS.
            if i + 1 < body.len() {
                accumulative_height += height;
            }
        }

        let bbox = blocks.iter().fold(BBox::EMPTY, |acc, (_, b)| acc.union(*b));
        let total_width = bbox.width + padding;
        let total_height = bbox.height + padding;
        let x = -total_width / 2.0;
        let y = -total_height / 2.0;
        let mut style = node_style(ctx, node);
        if !styles2string(node).node_styles.contains_key("stroke-width") {
            style.stroke_width = Some(ROUGH_STROKE_WIDTH);
        }
        // The divider is a rough line with the same options: the box's
        // stroke and width, no fill.
        let divider_style = Style {
            stroke: style.stroke.clone(),
            stroke_width: style.stroke_width,
            stroke_dasharray: style.stroke_dasharray.clone(),
            ..Style::default()
        };
        shape_svg.children.push(rect_element(
            x,
            y,
            total_width,
            total_height,
            None,
            None,
            "basic label-container outer-path",
            style,
        ));
        for (i, (mut group, b)) in blocks.into_iter().enumerate() {
            let new_translate_y = b.y - total_height / 2.0;
            let new_translate_x = if i == 0 || i == 1 {
                b.x
            } else {
                x + padding / 2.0
            };
            group.common.transform = translate(new_translate_x, new_translate_y + padding);
            shape_svg.children.push(Element::Group(group));
        }
        if accumulative_height > type_height + name_height + gap {
            let line_y = y + type_height + name_height + gap;
            shape_svg.children.push(line_element(
                x,
                line_y,
                x + total_width,
                line_y,
                "divider",
                divider_style,
            ));
        }
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

fn prefixed(prefix: &str, value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        format!("{prefix}{value}")
    }
}
