//! `shapes/note.ts`: the state and class diagram note, painted with
//! `noteBkgColor` and `noteBorderColor`.

use super::util::{
    apply_declarations, done, get_node_classes, label_helper, paint_style, rect_element,
    styles2string, translate,
};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Element, Paint};

pub struct Note;

impl Shape for Note {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        if !ctx.use_html_labels(node) {
            node.center_label = true;
        }
        let mut out = label_helper(node, ctx, Some(&get_node_classes(node, None)), false);
        let total_width = (out.bbox.width + node.padding() * 2.0).max(node.width());
        let total_height = (out.bbox.height + node.padding() * 2.0).max(node.height());
        let x = -total_width / 2.0;
        let y = -total_height / 2.0;
        let mut style = paint_style(
            &ctx.vars,
            Paint::Color(ctx.vars.note_bkg_color),
            Paint::Color(ctx.vars.note_border_color),
        );
        apply_declarations(&mut style, &styles2string(node).node_styles);
        let rect = rect_element(
            x,
            y,
            total_width,
            total_height,
            None,
            None,
            "basic label-container outer-path",
            style,
        );
        out.label.common.class = Some("label noteLabel".to_string());
        // A stylesheet rule such as the class diagram's `.noteLabel
        // .nodeLabel { color: noteTextColor }` colors the label span; an
        // inline `color` label style still wins over it.
        if let Some(color) = ctx.vars.note_label_color
            && !styles2string(node).label_styles.contains_key("color")
        {
            for child in &mut out.label.children {
                if let Element::Text(text) = child {
                    text.common.style.fill = Some(Paint::Color(color));
                }
            }
        }
        out.label.common.transform = translate(
            -out.bbox.width / 2.0 - out.bbox.x,
            -(out.bbox.height / 2.0) - out.bbox.y,
        );
        node.width = Some(total_width);
        node.height = Some(total_height);
        node.intersector = Intersector::Rect;
        Ok(done(out, vec![rect], node))
    }
}
