//! `shapes/crossedCircle.ts`: the summary shape, label-less.

use super::util::{
    circle_element, get_node_classes, group_element, node_group, node_style, path_element,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::PathData;

/// `createLine(r)`: the two diagonals through the circle.
fn create_line(r: f64) -> PathData {
    let x_axis_45 = (std::f64::consts::PI / 4.0).cos();
    let y_axis_45 = (std::f64::consts::PI / 4.0).sin();
    let line_length = r * 2.0;
    let half = line_length / 2.0;
    let q1 = (half * x_axis_45, half * y_axis_45);
    let q2 = (-half * x_axis_45, half * y_axis_45);
    let q3 = (-half * x_axis_45, -half * y_axis_45);
    let q4 = (half * x_axis_45, -half * y_axis_45);
    let mut d = PathData::new();
    d.move_to(q2.0, q2.1)
        .line_to(q4.0, q4.1)
        .move_to(q1.0, q1.1)
        .line_to(q3.0, q3.1);
    d
}

pub struct CrossedCircle;

impl Shape for CrossedCircle {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        node.label = Some(String::new());
        let mut shape_svg = node_group(node, &get_node_classes(node, None));
        let radius = node.width().max(30.0);
        let style = node_style(ctx, node);
        shape_svg.children.push(group_element(
            "outer-path",
            Vec::new(),
            vec![
                circle_element(0.0, 0.0, radius, "", style.clone()),
                path_element(create_line(radius), "", style, Vec::new()),
            ],
        ));
        node.width = Some(radius * 2.0);
        node.height = Some(radius * 2.0);
        node.intersector = Intersector::Circle(radius);
        Ok(ShapeOut {
            group: shape_svg,
            width: radius * 2.0,
            height: radius * 2.0,
        })
    }
}
