//! `diagrams/block/styles.ts` (mermaid 12.2.0) resolved to attributes.
//!
//! The rules that paint something in the classic look: `.node rect, .node
//! circle, .node ellipse, .node polygon, .node path { fill: mainBkg;
//! stroke: nodeBorder; stroke-width: strokeWidth }`, `.node .cluster {
//! fill: fade(clusterBkg, 0.5); stroke: fade(clusterBorder, 0.2);
//! stroke-width: 1px }` (two classes, so it beats `.node rect` on the
//! composite's rectangle), `.label { font-family; color: nodeTextColor ||
//! textColor }`, `.flowchart-link { stroke: lineColor; fill: none }`, and
//! `.edgeLabel { background-color: edgeLabelBackground }`. The root
//! stylesheet (`src/styles.ts`) adds `.marker { fill: lineColor; stroke:
//! lineColor }` and the root font. `genColor` emits nothing for the five
//! built-in themes, since only the `redux-color` themes carry a palette.

use crate::mermaid::scene::{Color, Paint, Style};
use crate::mermaid::shapes::NodeStyleVars;
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::khroma::{self, Channel};

/// The theme variables the block diagram reads, as colors.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockStyles {
    /// The shape stylesheet rules as the shared shapes read them.
    pub vars: NodeStyleVars,
    /// `.node .cluster { fill }`: `fade(clusterBkg, 0.5)`.
    pub composite_fill: Color,
    /// `.node .cluster { stroke }`: `fade(clusterBorder, 0.2)`.
    pub composite_stroke: Color,
    /// `.marker { fill; stroke }` from the root stylesheet.
    pub line_color: Color,
    pub font_family: String,
    pub font_size: f64,
}

/// A CSS color as khroma reads it (hex, `rgb()`, `hsl()`, named).
pub fn css_color(value: &str) -> Option<Color> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("transparent") || value == "none" {
        return None;
    }
    let mut ch = khroma::parse(value).ok()?;
    let byte = |v: f64| v.round().clamp(0.0, 255.0) as u8;
    Some(Color::rgba(
        byte(ch.get(Channel::R)),
        byte(ch.get(Channel::G)),
        byte(ch.get(Channel::B)),
        ch.get(Channel::A).clamp(0.0, 1.0),
    ))
}

fn var_color(vars: &ThemeVariables, name: &str, fallback: Color) -> Color {
    vars.str(name).and_then(css_color).unwrap_or(fallback)
}

/// `fade(color, opacity)`: the color's channels with a new alpha.
pub fn fade(color: Color, opacity: f64) -> Color {
    Color::rgba(color.r, color.g, color.b, opacity)
}

/// CSS `font-size` such as `16px` to pixels; `16` without a unit too.
fn css_px(value: &str, fallback: f64) -> f64 {
    value
        .trim()
        .trim_end_matches("px")
        .trim()
        .parse::<f64>()
        .unwrap_or(fallback)
}

impl BlockStyles {
    /// `getStyles(options)` over the derived theme variables.
    pub fn resolve(vars: &ThemeVariables) -> BlockStyles {
        let defaults = NodeStyleVars::default();
        let text_color = var_color(vars, "textColor", defaults.text_color);
        // `.label { color: nodeTextColor || textColor }`.
        let node_text_color = vars
            .str("nodeTextColor")
            .filter(|s| !s.is_empty())
            .and_then(css_color)
            .unwrap_or(text_color);
        let cluster_bkg = var_color(vars, "clusterBkg", defaults.cluster_bkg);
        let cluster_border = var_color(vars, "clusterBorder", defaults.cluster_border);
        let line_color = var_color(vars, "lineColor", defaults.line_color);
        let font_family = vars
            .str("fontFamily")
            .filter(|s| !s.is_empty())
            .unwrap_or(&defaults.font_family)
            .to_string();
        let font_size = css_px(vars.str("fontSize").unwrap_or("16px"), 16.0);
        let stroke_width = match vars.get("strokeWidth") {
            serde_json::Value::Number(n) => n.as_f64().unwrap_or(1.0),
            serde_json::Value::String(s) => css_px(s, 1.0),
            _ => 1.0,
        };
        let node_vars = NodeStyleVars {
            main_bkg: var_color(vars, "mainBkg", defaults.main_bkg),
            node_border: var_color(vars, "nodeBorder", defaults.node_border),
            line_color,
            text_color,
            node_text_color,
            title_color: var_color(vars, "titleColor", defaults.title_color),
            cluster_bkg,
            cluster_border,
            edge_label_background: var_color(
                vars,
                "edgeLabelBackground",
                defaults.edge_label_background,
            ),
            stroke_width,
            font_family: font_family.clone(),
            font_size,
            ..defaults
        };
        BlockStyles {
            vars: node_vars,
            composite_fill: fade(cluster_bkg, 0.5),
            composite_stroke: fade(cluster_border, 0.2),
            line_color,
            font_family,
            font_size,
        }
    }

    /// `.node .cluster { fill; stroke; stroke-width: 1px }`.
    pub fn composite_style(&self) -> Style {
        Style {
            fill: Some(Paint::Color(self.composite_fill)),
            stroke: Some(Paint::Color(self.composite_stroke)),
            stroke_width: Some(1.0),
            ..Style::default()
        }
    }

    /// `.marker { fill: lineColor; stroke: lineColor }`.
    pub fn marker_style(&self) -> Style {
        Style {
            fill: Some(Paint::Color(self.line_color)),
            stroke: Some(Paint::Color(self.line_color)),
            ..Style::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::theme;

    #[test]
    fn default_theme_resolves_to_the_shape_defaults() {
        let vars = theme::resolve("default", None).expect("default theme");
        let styles = BlockStyles::resolve(&vars);
        let defaults = NodeStyleVars::default();
        assert_eq!(styles.vars.main_bkg, defaults.main_bkg);
        assert_eq!(styles.vars.node_border, defaults.node_border);
        assert_eq!(styles.vars.line_color, defaults.line_color);
        assert_eq!(styles.font_size, 16.0);
        assert_eq!(styles.composite_fill.a, 0.5);
        assert_eq!(styles.composite_stroke.a, 0.2);
    }

    #[test]
    fn dark_theme_changes_the_node_colors() {
        let vars = theme::resolve("dark", None).expect("dark theme");
        let styles = BlockStyles::resolve(&vars);
        assert_ne!(styles.vars.main_bkg, NodeStyleVars::default().main_bkg);
        assert_eq!(css_color("transparent"), None);
        assert_eq!(css_color("#fff"), Some(Color::WHITE));
    }
}
