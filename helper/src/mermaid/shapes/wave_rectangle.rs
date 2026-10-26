//! `shapes/waveRectangle.ts`: the paper tape flag.

use super::util::{
    create_path_from_points, done, generate_full_sine_wave_points, get_node_classes, label_helper,
    node_style, points_bbox, rough_path_element, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct WaveRectangle;

impl Shape for WaveRectangle {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let node_padding = node.padding();
        let label_padding_x = node_padding;
        let label_padding_y = node_padding;
        if node.width.is_some_and(|w| w != 0.0) || node.height.is_some_and(|h| h != 0.0) {
            let mut width = node.width();
            if width < 20.0 {
                width = 20.0;
            }
            let mut height = node.height();
            if height < 10.0 {
                height = 10.0;
            }
            let wave_amplitude = (height * 0.2).min(height / 4.0);
            node.height = Some((height - label_padding_y - wave_amplitude * (20.0 / 9.0)).ceil());
            node.width = Some(width - label_padding_x * 2.0);
        }
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let w = if node.width.is_some_and(|w| w != 0.0) {
            node.width()
        } else {
            out.bbox.width
        } + label_padding_x * 2.0;
        let h = if node.height.is_some_and(|h| h != 0.0) {
            node.height()
        } else {
            out.bbox.height
        } + label_padding_y;
        let wave_amplitude = h / 8.0;
        let final_h = h + wave_amplitude * 2.0;

        let mut points = vec![Point::new(-w / 2.0, final_h / 2.0)];
        points.extend(generate_full_sine_wave_points(
            -w / 2.0,
            final_h / 2.0,
            w / 2.0,
            final_h / 2.0,
            wave_amplitude,
            1.0,
        ));
        points.push(Point::new(w / 2.0, -final_h / 2.0));
        points.extend(generate_full_sine_wave_points(
            w / 2.0,
            -final_h / 2.0,
            -w / 2.0,
            -final_h / 2.0,
            wave_amplitude,
            -1.0,
        ));
        let path = rough_path_element(
            create_path_from_points(&points),
            "basic label-container",
            node_style(ctx, node),
            Vec::new(),
            &ctx.vars,
        );
        update_node_bounds(node, points_bbox(&points));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![path], node))
    }
}
