//! `diagrams/class/styles.js` (mermaid 12.2.0) resolved to attribute values.
//!
//! The unified shapes and edges already resolve the rules the class
//! stylesheet shares with the flowchart one (`.node rect, .node path`,
//! `.divider`, `.cluster rect`, `.relation`, the edge pattern classes).
//! This module holds the rules that differ, and [`ClassStyles::apply_to`]
//! writes them into the shapes' [`NodeStyleVars`] before the render:
//!
//! - `.nodeLabel, .edgeLabel { color: classText }`: the text of class boxes
//!   and edge labels.
//! - `.edgeLabel .label span, .labelBkg { background: mainBkg }`: the edge
//!   label background, `mainBkg` instead of `edgeLabelBackground`.
//! - `.noteLabel .nodeLabel { color: noteTextColor }`: the note text.
//! - The marker rules `[id$="-extensionStart"], .extension { fill:
//!   transparent !important; stroke: lineColor !important; stroke-width: 1 }`
//!   and their siblings: composition and dependency markers fill with
//!   `lineColor`, extension and aggregation markers stay hollow, and
//!   lollipop markers fill with `mainBkg` (the circle's own
//!   `fill="transparent"` attribute still wins over that).
//! - `.edgeTerminals { font-size: 11px }`: the cardinality labels.
//!
//! The per-class colour cycle (`genColor`) only applies to the color
//! themes, which the helper does not render (R-175).

use crate::mermaid::scene::Color;
use crate::mermaid::shapes::NodeStyleVars;
use crate::mermaid::shapes::vars::parse_color;
use crate::mermaid::theme::ThemeVariables;

/// `.edgeTerminals { font-size: 11px }`.
pub const EDGE_TERMINALS_FONT_SIZE: f64 = 11.0;

/// The class stylesheet values the shared unified styles do not cover.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassStyles {
    /// `classText`.
    pub class_text: Color,
    /// `mainBkg`.
    pub main_bkg: Color,
    /// `lineColor`.
    pub line_color: Color,
    /// `noteTextColor`.
    pub note_text_color: Color,
}

impl ClassStyles {
    /// The values of `getStyles(options)` for the resolved theme. `vars`
    /// supplies the fallbacks the shapes already derived.
    pub fn resolve(tv: &ThemeVariables, vars: &NodeStyleVars) -> ClassStyles {
        let color =
            |name: &str, fallback: Color| tv.str(name).and_then(parse_color).unwrap_or(fallback);
        ClassStyles {
            // Every built-in theme derives `classText` from
            // `primaryTextColor`, the same value as `nodeTextColor`.
            class_text: color("classText", vars.node_text_color),
            main_bkg: vars.main_bkg,
            line_color: vars.line_color,
            note_text_color: vars.note_text_color,
        }
    }

    /// Writes the class rules into the shapes' variables: the label text
    /// color and the edge label background.
    pub fn apply_to(&self, vars: &mut NodeStyleVars) {
        vars.node_text_color = self.class_text;
        vars.edge_label_background = self.main_bkg;
    }

    /// The marker fill for a class marker name (`aggregationStart`,
    /// `extensionEnd-margin`, ...), or `None` for a hollow marker.
    pub fn marker_fill(&self, marker: &str) -> Option<Color> {
        if marker.starts_with("composition") || marker.starts_with("dependency") {
            Some(self.line_color)
        } else if marker.starts_with("lollipop") {
            Some(self.main_bkg)
        } else {
            None
        }
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
        let s = ClassStyles::resolve(&tv, &vars);
        assert_eq!(s.class_text, Color::rgb(0x13, 0x13, 0x00));
        assert_eq!(s.main_bkg, Color::rgb(0xEC, 0xEC, 0xFF));
        assert_eq!(s.marker_fill("extensionEnd"), None);
        assert_eq!(s.marker_fill("compositionStart"), Some(s.line_color));
        assert_eq!(s.marker_fill("lollipopEnd"), Some(s.main_bkg));
        let mut v = vars.clone();
        s.apply_to(&mut v);
        assert_eq!(v.edge_label_background, s.main_bkg);
    }
}
