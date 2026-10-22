//! `shapes/multiWaveEdgedRectangle.ts`: stacked documents.

use super::util::{
    create_path_from_points, done, generate_full_sine_wave_points, get_node_classes, group_element,
    label_helper, node_style, path_element, points_bbox, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct MultiWaveEdgedRectangle;

impl Shape for MultiWaveEdgedRectangle {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        let mut adjust_final_height = true;
        if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
            adjust_final_height = false;
            node.width = Some(node.width() - label_padding_x * 2.0);
            node.height = Some(node.height() - label_padding_y * 3.0);
        }
        let w = out.bbox.width.max(node.width()) + label_padding_x * 2.0;
        let h = out.bbox.height.max(node.height()) + label_padding_y * 3.0;
        let wave_amplitude = h / 8.0;
        let final_h = h + if adjust_final_height {
            wave_amplitude / 2.0
        } else {
            -wave_amplitude / 2.0
        };
        let x = -w / 2.0;
        let y = -final_h / 2.0;
        let rect_offset = 10.0;

        let wave_points = generate_full_sine_wave_points(
            x - rect_offset,
            y + final_h + rect_offset,
            x + w - rect_offset,
            y + final_h + rect_offset,
            wave_amplitude,
            0.8,
        );
        let last = wave_points.last().copied().unwrap_or_default();
        let mut outer = vec![
            Point::new(x - rect_offset, y + rect_offset),
            Point::new(x - rect_offset, y + final_h + rect_offset),
        ];
        outer.extend(wave_points.iter().copied());
        outer.extend([
            Point::new(x + w - rect_offset, last.y - rect_offset),
            Point::new(x + w, last.y - rect_offset),
            Point::new(x + w, last.y - 2.0 * rect_offset),
            Point::new(x + w + rect_offset, last.y - 2.0 * rect_offset),
            Point::new(x + w + rect_offset, y - rect_offset),
            Point::new(x + rect_offset, y - rect_offset),
            Point::new(x + rect_offset, y),
            Point::new(x, y),
            Point::new(x, y + rect_offset),
        ]);
        let inner = vec![
            Point::new(x, y + rect_offset),
            Point::new(x + w - rect_offset, y + rect_offset),
            Point::new(x + w - rect_offset, last.y - rect_offset),
            Point::new(x + w, last.y - rect_offset),
            Point::new(x + w, y),
            Point::new(x, y),
        ];
        let style = node_style(ctx, node);
        let shape = group_element(
            "basic label-container outer-path",
            translate(0.0, -wave_amplitude / 2.0),
            vec![
                path_element(
                    create_path_from_points(&outer),
                    "",
                    style.clone(),
                    Vec::new(),
                ),
                path_element(create_path_from_points(&inner), "", style, Vec::new()),
            ],
        );
        out.label.common.transform = translate(
            -(out.bbox.width / 2.0) - rect_offset - out.bbox.x,
            -(out.bbox.height / 2.0) + rect_offset - wave_amplitude - out.bbox.y,
        );
        update_node_bounds(node, points_bbox(&outer));
        node.intersector = Intersector::Polygon(outer);
        Ok(done(out, vec![shape], node))
    }
}
