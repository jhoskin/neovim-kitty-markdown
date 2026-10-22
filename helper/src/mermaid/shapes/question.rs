//! `shapes/question.ts`: the decision diamond.

use super::util::{
    done, get_node_classes, insert_polygon_shape, label_helper, node_style, translate,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Element, Point};

/// The diamond's points for side `s`, shared with `calcIntersect`.
pub fn question_points(s: f64) -> Vec<Point> {
    vec![
        Point::new(s / 2.0, 0.0),
        Point::new(s, -s / 2.0),
        Point::new(s / 2.0, -s),
        Point::new(0.0, -s / 2.0),
    ]
}

pub struct Question;

impl Shape for Question {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let w = out.bbox.width + node.padding();
        let h = out.bbox.height + node.padding();
        let s = w + h;
        let adjustment = 0.5;
        let points = question_points(s);
        let mut polygon = insert_polygon_shape(s, s, &points, node_style(ctx, node));
        if let Element::Polygon(p) = &mut polygon {
            p.common.transform = translate(-s / 2.0 + adjustment, s / 2.0);
        }
        node.width = Some(s);
        node.height = Some(s);
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![polygon], node))
    }
}
