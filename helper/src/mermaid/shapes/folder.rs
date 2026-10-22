//! `shapes/folder.ts`: a rectangle with a raised tab.

use super::util::{
    create_path_from_points, done, get_node_classes, label_helper, node_style, path_element,
    translate,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::Point;

pub struct Folder;

impl Shape for Folder {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let padding = node.padding.unwrap_or(12.0);
        let w = (out.bbox.width + padding * 2.0).max(node.width()).max(90.0);
        let content_height = out.bbox.height + padding * 2.0;
        let tab_height = (content_height * 0.16).clamp(8.0, 14.0);
        let total_height = (content_height + tab_height).max(node.height());
        let body_height = total_height - tab_height;
        let tab_width = (w * 0.38).max(28.0);
        let top = -total_height / 2.0;
        let points = vec![
            Point::new(-w / 2.0, top),
            Point::new(-w / 2.0 + tab_width, top),
            Point::new(-w / 2.0 + tab_width, top + tab_height),
            Point::new(w / 2.0, top + tab_height),
            Point::new(w / 2.0, total_height / 2.0),
            Point::new(-w / 2.0, total_height / 2.0),
        ];
        let path = path_element(
            create_path_from_points(&points),
            "basic label-container",
            node_style(ctx, node),
            Vec::new(),
        );
        node.width = Some(w);
        node.height = Some(total_height);
        let body_center_y = top + tab_height + body_height / 2.0;
        out.label.common.transform = translate(
            -(out.bbox.width / 2.0) - out.bbox.x,
            body_center_y - out.bbox.height / 2.0 - out.bbox.y,
        );
        node.intersector = Intersector::Polygon(points);
        Ok(done(out, vec![path], node))
    }
}
