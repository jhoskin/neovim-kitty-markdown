//! `shapes/waveEdgedRectangle.ts`: the document shape.

use super::util::{
    create_path_from_points, done, generate_full_sine_wave_points, get_node_classes, label_helper,
    node_style, path_element, points_bbox, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct WaveEdgedRectangle;

impl Shape for WaveEdgedRectangle {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        let mut adjust_final_height = true;
        if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
            adjust_final_height = false;
            node.width = Some((node.width() - label_padding_x * 2.0).max(10.0));
            node.height = Some((node.height() - label_padding_y * 2.0).max(10.0));
        }
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let w = if node.width.is_some_and(|w| w != 0.0) {
            node.width()
        } else {
            out.bbox.width
        } + label_padding_x * 2.0;
        let h = if node.height.is_some_and(|h| h != 0.0) {
            node.height()
        } else {
            out.bbox.height
        } + label_padding_y * 2.0;
        let wave_amplitude = h / 8.0;
        let final_h = h + if adjust_final_height {
            wave_amplitude
        } else {
            -wave_amplitude
        };
        let min_width = 14.0;
        let width_dif = min_width - w;
        let extra_w = if width_dif > 0.0 {
            width_dif / 2.0
        } else {
            0.0
        };

        let mut points = vec![Point::new(-w / 2.0 - extra_w, final_h / 2.0)];
        points.extend(generate_full_sine_wave_points(
            -w / 2.0 - extra_w,
            final_h / 2.0,
            w / 2.0 + extra_w,
            final_h / 2.0,
            wave_amplitude,
            0.8,
        ));
        points.push(Point::new(w / 2.0 + extra_w, -final_h / 2.0));
        points.push(Point::new(-w / 2.0 - extra_w, -final_h / 2.0));

        let path = path_element(
            create_path_from_points(&points),
            "basic label-container outer-path",
            node_style(ctx, node),
            translate(0.0, -wave_amplitude / 2.0),
        );
        out.label.common.transform = translate(
            -w / 2.0 + node.padding() - out.bbox.x,
            -h / 2.0 + node.padding() - wave_amplitude - out.bbox.y,
        );
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![path], node))
    }
}
