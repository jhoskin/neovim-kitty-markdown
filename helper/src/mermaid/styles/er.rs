//! `diagrams/er/styles.ts` (mermaid 12.2.0) resolved to attribute values.
//!
//! The shapes and edges paint themselves from [`NodeStyleVars`];
//! [`ErStyles::apply_to`] writes the ER values into them before the
//! render:
//!
//! - `.label { color: nodeTextColor || textColor }`: the entity text.
//! - `.labelBkg { background-color: fade(tertiaryColor, 0.5) }`: the
//!   relationship label background. The `.edgeLabel` span also has a
//!   background, but the label's block `p` is outside the span's inline
//!   box, so only the `div.labelBkg` paints. SVG labels draw
//!   `.edgeLabel .label rect { fill: edgeLabelBackground }` instead.
//! - `.node rect, .node polygon { stroke-width: 1px }`, `.cluster rect`.
//!
//! The rules no shape knows about are applied to the rendered tree by the
//! ER renderer with the values here:
//!
//! - `.edgeLabel .label { font-size: 14px }`: the relationship labels.
//! - `.edge-pattern-dashed { stroke-dasharray: 8,8 }`: non-identifying
//!   relationships, after the shared `stroke-dasharray: 3` rule.
//! - `.relationshipLine { stroke: lineColor; stroke-width: 1px }`.
//! - `.marker { fill: none !important; stroke: lineColor !important;
//!   stroke-width: 1 }`: the cardinality markers. The circles keep their
//!   own `fill="white"`, which the marker's inherited `fill` does not
//!   override.
//!
//! The palette rules of `genColor` only exist for the `redux-color` themes
//! and the `neo` look rules only for `look: neo`; the helper renders
//! neither (R-175), so they are not ported.

use crate::mermaid::scene::Color;
use crate::mermaid::shapes::NodeStyleVars;
use crate::mermaid::shapes::vars::parse_color;
use crate::mermaid::theme::ThemeVariables;

/// `.edgeLabel .label { font-size: 14px }`.
pub const EDGE_LABEL_FONT_SIZE: f64 = 14.0;
/// `.edge-pattern-dashed { stroke-dasharray: 8,8 }`.
pub const DASHED: [f64; 2] = [8.0, 8.0];

/// The ER stylesheet values the shared shapes and edges do not cover.
#[derive(Debug, Clone, PartialEq)]
pub struct ErStyles {
    /// `nodeTextColor || textColor`.
    pub label_color: Color,
    /// `fade(tertiaryColor, 0.5)`.
    pub label_bkg: Color,
    /// `edgeLabelBackground`.
    pub edge_label_background: Color,
    /// `lineColor`.
    pub line_color: Color,
    /// `textColor`, the title text.
    pub text_color: Color,
}

impl ErStyles {
    /// The values of `getStyles(options)` for the resolved theme. `vars`
    /// supplies the fallbacks the shapes already derived.
    pub fn resolve(tv: &ThemeVariables, vars: &NodeStyleVars) -> ErStyles {
        let color =
            |name: &str, fallback: Color| tv.str(name).and_then(parse_color).unwrap_or(fallback);
        let tertiary = color("tertiaryColor", vars.cluster_bkg);
        ErStyles {
            label_color: color("nodeTextColor", vars.text_color),
            // `fade`: `khroma.rgba(r, g, b, 0.5)` of the channels.
            label_bkg: Color { a: 0.5, ..tertiary },
            edge_label_background: vars.edge_label_background,
            line_color: vars.line_color,
            text_color: vars.text_color,
        }
    }

    /// Writes the ER rules into the shapes' variables: the label color and
    /// the relationship label background for the label mode in use.
    pub fn apply_to(&self, vars: &mut NodeStyleVars, html_labels: bool) {
        vars.node_text_color = self.label_color;
        vars.edge_label_background = if html_labels {
            self.label_bkg
        } else {
            self.edge_label_background
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::config::defaults::MermaidConfig;
    use crate::mermaid::theme;

    #[test]
    fn default_theme_values() {
        let tv = theme::resolve("default", None).expect("default theme");
        let vars = NodeStyleVars::from_theme(&tv, &MermaidConfig::default());
        let s = ErStyles::resolve(&tv, &vars);
        assert_eq!(s.line_color, Color::rgb(0x33, 0x33, 0x33));
        // hsl(80, 100%, 96.27%) at half opacity.
        assert_eq!(
            (s.label_bkg.r, s.label_bkg.g, s.label_bkg.b),
            (249, 255, 236)
        );
        assert_eq!(s.label_bkg.a, 0.5);
        let mut v = vars.clone();
        s.apply_to(&mut v, true);
        assert_eq!(v.edge_label_background, s.label_bkg);
        s.apply_to(&mut v, false);
        assert_eq!(v.edge_label_background, Color::rgba(232, 232, 232, 0.8));
    }
}
