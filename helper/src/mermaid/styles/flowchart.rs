//! `diagrams/flowchart/styles.ts` (mermaid 12.2.0) resolved to attribute
//! values.
//!
//! The stylesheet's node, cluster, edge, and label rules (`.node rect`,
//! `.cluster rect`, `.flowchart-link`, `.edgeLabel`, `.label`) are the
//! variables of [`NodeStyleVars`], which the shapes, clusters, and edges of
//! the unified core apply themselves; a `classDef` or `style` declaration
//! goes on top of them there, as its `!important` would. What is left for
//! the flowchart renderer is the root rule of the global stylesheet and
//! `.flowchartTitleText`.
//!
//! The per-subgraph palette of `genColor` only applies to the
//! `redux-color` themes, which the helper does not render (R-175).

use crate::mermaid::config::defaults::MermaidConfig;
use crate::mermaid::scene::Color;
use crate::mermaid::shapes::NodeStyleVars;
use crate::mermaid::theme::ThemeVariables;

/// `.flowchartTitleText { font-size: 18px }`.
pub const TITLE_FONT_SIZE: f64 = 18.0;

#[derive(Debug, Clone, PartialEq)]
pub struct FlowchartStyles {
    /// The variables the unified core reads.
    pub vars: NodeStyleVars,
    /// `#id { fill: textColor }` and `.flowchartTitleText { fill:
    /// textColor }`.
    pub text_color: Color,
    /// `#id { font-family: fontFamily; font-size: fontSize }`.
    pub font_family: String,
    pub font_size: f64,
}

impl FlowchartStyles {
    pub fn resolve(tv: &ThemeVariables, config: &MermaidConfig) -> FlowchartStyles {
        let vars = NodeStyleVars {
            node_rule: true,
            ..NodeStyleVars::from_theme(tv, config)
        };
        FlowchartStyles {
            text_color: vars.text_color,
            font_family: vars.font_family.clone(),
            font_size: vars.font_size,
            vars,
        }
    }
}
