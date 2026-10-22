//! `shapes/usecaseBusiness.ts`: an ellipse with a business marker stroke.

use super::util::{
    assemble, get_node_classes, insert_label, label_helper, node_style, path_element, translate,
};
use super::{Intersector, Node, NodeExtra, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Common, Element, Ellipse, Paint, PathData};

pub struct UsecaseBusiness;

impl Shape for UsecaseBusiness {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let stereotype = match &node.extra {
            NodeExtra::Usecase { stereotype, .. } => stereotype.clone(),
            _ => None,
        };
        let mut out = label_helper(
            node,
            ctx,
            Some(&get_node_classes(node, Some("usecase-business-shape"))),
            false,
        );
        out.label.common.class = Some("label usecase-label".to_string());
        let stereo = stereotype.map(|s| {
            let (mut g, b) = insert_label(&format!("«{s}»"), ctx, node, node.wrapping_width, true);
            g.common.class = Some("label usecase-stereotype".to_string());
            (g, b)
        });
        let label_gap = if stereo.is_some() { 2.0 } else { 0.0 };
        let stereo_h = stereo.as_ref().map_or(0.0, |(_, b)| b.height);
        let stereo_w = stereo.as_ref().map_or(0.0, |(_, b)| b.width);
        let label_height = out.bbox.height + stereo_h + label_gap;
        let label_width = out.bbox.width.max(stereo_w);
        let padding = out.half_padding;
        let radius_x = label_width / 2.0 + padding * 2.0;
        let radius_y = label_height / 2.0 + padding * 2.0;
        let marker_inset = (padding / 5.0).max(1.0).min(padding / 2.0);
        let marker_start_x = label_width / 2.0 + marker_inset;
        let marker_end_x = radius_x - marker_inset;
        let ns = marker_start_x / radius_x;
        let ne = marker_end_x / radius_x;
        let marker_start_y = radius_y * (1.0 - ns * ns).max(0.0).sqrt();
        let marker_end_y = -radius_y * (1.0 - ne * ne).max(0.0).sqrt();

        let mut children = Vec::new();
        let style = node_style(ctx, node);
        children.push(Element::Ellipse(Ellipse {
            common: Common {
                class: Some("basic label-container usecase-business-ellipse".to_string()),
                style: style.clone(),
                ..Common::default()
            },
            cx: 0.0,
            cy: 0.0,
            rx: radius_x,
            ry: radius_y,
        }));
        out.label.common.transform = translate(
            -out.bbox.width / 2.0 - out.bbox.x,
            label_height / 2.0 - out.bbox.height / 2.0 - out.bbox.height / 2.0 - out.bbox.y,
        );
        let label = std::mem::take(&mut out.label);
        let mut group = assemble(out.shape_svg, children, label);
        if let Some((mut g, b)) = stereo {
            g.common.transform = translate(
                -b.width / 2.0 - b.x,
                -label_height / 2.0 + stereo_h / 2.0 - b.height / 2.0 - b.y,
            );
            group.children.push(Element::Group(g));
        }
        let mut marker = PathData::new();
        marker
            .move_to(marker_start_x, marker_start_y)
            .line_to(marker_end_x, marker_end_y);
        let mut marker_style = style;
        marker_style.fill = Some(Paint::None);
        group.children.push(path_element(
            marker,
            "usecase-business-marker",
            marker_style,
            Vec::new(),
        ));
        node.width = Some(radius_x * 2.0);
        node.height = Some(radius_y * 2.0);
        node.intersector = Intersector::Ellipse {
            rx: radius_x,
            ry: radius_y,
        };
        Ok(ShapeOut {
            group,
            width: radius_x * 2.0,
            height: radius_y * 2.0,
        })
    }
}
