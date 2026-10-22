//! `shapes/stateEnd.ts`: a ring with a filled dot. The ring takes
//! `lineColor` at width 2 over `mainBkg`; the dot takes `stateBorder ??
//! nodeBorder`.

use super::state_start::fix_state_dot_size;
use super::util::{apply_declarations, circle_element, node_group, styles2string};
use super::{Intersector, Node, Shape, ShapeCtx, ShapeOut};
use crate::error::HelperError;
use crate::mermaid::scene::{Common, Element, Group, Paint, Style};

pub struct StateEnd;

impl Shape for StateEnd {
    fn draw(&self, node: &mut Node, ctx: &ShapeCtx) -> Result<ShapeOut, HelperError> {
        fix_state_dot_size(node);
        let mut shape_svg = node_group(node, "node default");
        let node_styles = styles2string(node).node_styles;
        let user_fill = node_styles
            .get("fill")
            .and_then(|f| super::util::parse_css_color(f));
        let outer_r = node.width() / 2.0;
        let mut outer = Style {
            fill: Some(user_fill.unwrap_or(Paint::Color(ctx.vars.main_bkg))),
            stroke: Some(Paint::Color(ctx.vars.line_color)),
            stroke_width: Some(2.0),
            ..Style::default()
        };
        apply_declarations(&mut outer, &node_styles);
        let inner_fill = ctx.vars.state_border.unwrap_or(ctx.vars.node_border);
        // `rc.circle(0, 0, innerNodeRadius)` takes a diameter.
        let inner_r = ((node.width() * 5.0) / 14.0) / 2.0;
        let mut inner = Style {
            fill: Some(Paint::Color(inner_fill)),
            stroke: Some(Paint::Color(inner_fill)),
            stroke_width: Some(2.0),
            ..Style::default()
        };
        apply_declarations(&mut inner, &node_styles);
        shape_svg.children.push(Element::Group(Group {
            common: Common {
                class: Some("outer-path".to_string()),
                ..Common::default()
            },
            children: vec![
                circle_element(0.0, 0.0, outer_r, "", outer),
                circle_element(0.0, 0.0, inner_r, "", inner),
            ],
        }));
        node.width = Some(outer_r * 2.0);
        node.height = Some(outer_r * 2.0);
        node.intersector = Intersector::Circle(outer_r);
        Ok(ShapeOut {
            group: shape_svg,
            width: outer_r * 2.0,
            height: outer_r * 2.0,
        })
    }
}
