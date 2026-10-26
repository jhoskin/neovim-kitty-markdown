//! `shapes/linedWaveEdgedRect.ts`: the lined document.

use super::util::{
    create_path_from_points, done, generate_full_sine_wave_points, get_node_classes, label_helper,
    node_style, points_bbox, rough_path_element, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct LinedWaveEdgedRect;

impl Shape for LinedWaveEdgedRect {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
            let original_width = node.width();
            node.width = Some(((original_width * 10.0) / 11.0 - label_padding_x * 2.0).max(10.0));
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
        let final_h = h + wave_amplitude;
        let side = (w / 2.0) * 0.1;

        let mut points = vec![
            Point::new(-w / 2.0 - side, -final_h / 2.0),
            Point::new(-w / 2.0 - side, final_h / 2.0),
        ];
        points.extend(generate_full_sine_wave_points(
            -w / 2.0 - side,
            final_h / 2.0,
            w / 2.0 + side,
            final_h / 2.0,
            wave_amplitude,
            0.8,
        ));
        points.extend([
            Point::new(w / 2.0 + side, -final_h / 2.0),
            Point::new(-w / 2.0 - side, -final_h / 2.0),
            Point::new(-w / 2.0, -final_h / 2.0),
            Point::new(-w / 2.0, (final_h / 2.0) * 1.1),
            Point::new(-w / 2.0, -final_h / 2.0),
        ]);
        let path = rough_path_element(
            create_path_from_points(&points),
            "basic label-container outer-path",
            node_style(ctx, node),
            translate(0.0, -wave_amplitude / 2.0),
            &ctx.vars,
        );
        out.label.common.transform = translate(
            -w / 2.0 + node.padding() + side / 2.0 - out.bbox.x,
            -h / 2.0 + node.padding() - wave_amplitude - out.bbox.y,
        );
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![path], node))
    }
}
