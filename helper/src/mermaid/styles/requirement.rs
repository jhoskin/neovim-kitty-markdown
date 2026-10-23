//! `diagrams/requirement/styles.js` (mermaid 12.2.0) resolved to attribute
//! values.
//!
//! The unified renderer draws requirement boxes with `requirementBox`,
//! whose rough.js paths carry their own fill and stroke attributes
//! (`mainBkg`, `nodeBorder`), so the `.reqBox`, `.reqTitle`, and
//! `.req-title-line` rules of the old renderer match nothing. The rules
//! that do reach the unified output are:
//!
//! - `marker { fill: relationColor; stroke: relationColor }`,
//! - `.relationshipLine { stroke: relationColor; stroke-width: 1px }` on
//!   the edge paths,
//! - `.label text, span { color: nodeTextColor || textColor }` for the node
//!   and edge label text,
//! - `.labelBkg { background-color: requirementEdgeLabelBackground ??
//!   edgeLabelBackground }`, the HTML edge label background,
//! - `.edgeLabel .label text { fill: relationLabelColor }` and `.edgeLabel
//!   .label rect { fill: edgeLabelBackground }` for SVG labels.
//!
//! The per-node colour cycle (`genColor`) only applies to the color
//! themes, which the helper does not render (R-175).

use crate::mermaid::scene::Color;
use crate::mermaid::shapes::NodeStyleVars;
use crate::mermaid::shapes::vars::parse_color;
use crate::mermaid::theme::ThemeVariables;

/// `.relationshipLine { stroke-width: 1px }` (classic look).
pub const RELATIONSHIP_STROKE_WIDTH: f64 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub struct RequirementStyles {
    /// `relationColor`: edge strokes and markers.
    pub relation_color: Color,
    /// `relationLabelColor`: SVG edge label text.
    pub relation_label_color: Color,
    /// `requirementEdgeLabelBackground ?? edgeLabelBackground`.
    pub label_bkg: Color,
    /// `textColor`, the root rule's fill.
    pub text_color: Color,
}

impl RequirementStyles {
    /// The values of `getStyles(options)` for the resolved theme; `vars`
    /// supplies the fallbacks the shapes already derived. Each fallback is
    /// the derivation every built-in theme uses (`relationColor =
    /// lineColor`, `relationLabelColor = actorTextColor`).
    pub fn resolve(tv: &ThemeVariables, vars: &NodeStyleVars) -> RequirementStyles {
        let color =
            |name: &str, fallback: Color| tv.str(name).and_then(parse_color).unwrap_or(fallback);
        RequirementStyles {
            relation_color: color("relationColor", vars.line_color),
            relation_label_color: color(
                "relationLabelColor",
                color("actorTextColor", vars.text_color),
            ),
            label_bkg: color("requirementEdgeLabelBackground", vars.edge_label_background),
            text_color: vars.text_color,
        }
    }

    /// Writes the rules the shapes and edges read into their variables:
    /// the HTML edge label background.
    pub fn apply_to(&self, vars: &mut NodeStyleVars) {
        vars.edge_label_background = self.label_bkg;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::config::defaults::MermaidConfig;
    use crate::mermaid::theme;

    #[test]
    fn default_and_dark_theme_values() {
        let tv = theme::resolve("default", None).expect("default theme");
        let vars = NodeStyleVars::from_theme(&tv, &MermaidConfig::default());
        let s = RequirementStyles::resolve(&tv, &vars);
        assert_eq!(s.relation_color, Color::rgb(0x33, 0x33, 0x33));
        assert_eq!(s.relation_label_color, Color::BLACK);
        assert_eq!(s.label_bkg, Color::rgba(232, 232, 232, 0.8));

        let tv = theme::resolve("dark", None).expect("dark theme");
        let vars = NodeStyleVars::from_theme(&tv, &MermaidConfig::default());
        let s = RequirementStyles::resolve(&tv, &vars);
        assert_eq!(s.relation_color, Color::rgb(211, 211, 211));
    }
}
