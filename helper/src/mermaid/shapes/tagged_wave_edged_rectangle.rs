//! `shapes/taggedWaveEdgedRectangle.ts`: the tagged document.

use super::util::{
    create_path_from_points, done, generate_full_sine_wave_points, get_node_classes, group_element,
    label_helper, node_style, points_bbox, rough_path_element, translate, update_node_bounds,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct TaggedWaveEdgedRectangle;

impl Shape for TaggedWaveEdgedRectangle {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let w = (out.bbox.width + node.padding() * 2.0).max(node.width());
        let h = (out.bbox.height + node.padding() * 2.0).max(node.height());
        let wave_amplitude = h / 8.0;
        let tag_width = 0.2 * w;
        let tag_height = 0.2 * h;
        let final_h = h + wave_amplitude;
        let side = (w / 2.0) * 0.1;

        let mut points = vec![Point::new(-w / 2.0 - side, final_h / 2.0)];
        points.extend(generate_full_sine_wave_points(
            -w / 2.0 - side,
            final_h / 2.0,
            w / 2.0 + side,
            final_h / 2.0,
            wave_amplitude,
            0.8,
        ));
        points.push(Point::new(w / 2.0 + side, -final_h / 2.0));
        points.push(Point::new(-w / 2.0 - side, -final_h / 2.0));

        let x = -w / 2.0 + side;
        let y = -final_h / 2.0 - tag_height * 0.4;
        let mut tag_points = vec![
            Point::new(x + w - tag_width, (y + h) * 1.3),
            Point::new(x + w, y + h - tag_height),
            Point::new(x + w, (y + h) * 0.9),
        ];
        tag_points.extend(generate_full_sine_wave_points(
            x + w,
            (y + h) * 1.25,
            x + w - tag_width,
            (y + h) * 1.3,
            -h * 0.02,
            0.5,
        ));

        let style = node_style(ctx, node);
        let shape = group_element(
            "basic label-container outer-path",
            translate(0.0, -wave_amplitude / 2.0),
            vec![
                rough_path_element(
                    create_path_from_points(&points),
                    "",
                    style.clone(),
                    Vec::new(),
                    &ctx.vars,
                ),
                rough_path_element(
                    create_path_from_points(&tag_points),
                    "",
                    style,
                    Vec::new(),
                    &ctx.vars,
                ),
            ],
        );
        out.label.common.transform = translate(
            -w / 2.0 + node.padding() - out.bbox.x,
            -h / 2.0 + node.padding() - wave_amplitude / 2.0 - out.bbox.y,
        );
        update_node_bounds(node, points_bbox(&points).union(points_bbox(&tag_points)));
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![shape], node))
    }
}
