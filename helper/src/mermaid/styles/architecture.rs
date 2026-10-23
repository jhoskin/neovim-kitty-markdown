//! `architectureStyles.ts` (`getStyles`) and the root rule of the global
//! stylesheet, resolved to the values the architecture renderer sets.
//!
//! The rules that reach the picture: `.edge` (`stroke-width:
//! archEdgeWidth`, `stroke: archEdgeColor`, `fill: none`), `.arrow`
//! (`fill: archEdgeArrowColor`), `.node-bkg` (`fill: none`, `stroke:
//! archGroupBorderColor`, `stroke-width: archGroupBorderWidth`,
//! `stroke-dasharray: 8`), and the root `font-family`, `font-size: 16px`,
//! and `fill: textColor` every label inherits. A value the browser rejects
//! drops its declaration: the stroke then keeps `none`, a width its
//! initial 1, a fill its initial black.

use serde_json::Value;

use crate::mermaid::scene::{Color, Paint};
use crate::mermaid::styles::pie::{css_color, css_length};
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::khroma;

/// The SVG `stroke-width` initial value.
const DEFAULT_STROKE_WIDTH: f64 = 1.0;
/// The root rule's `font-size`.
pub const ROOT_FONT_SIZE: f64 = 16.0;

#[derive(Debug, Clone, PartialEq)]
pub struct ArchitectureStyles {
    pub font_family: String,
    /// The root `fill: textColor`.
    pub text_color: Color,
    pub edge_color: Paint,
    pub edge_width: f64,
    pub arrow_color: Paint,
    pub group_border_color: Paint,
    pub group_border_width: f64,
}

/// The text a value interpolates into the stylesheet.
fn css_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => n.as_f64().map(khroma::js_number),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

impl ArchitectureStyles {
    /// `getStyles(themeVariables)`. `fallback_font` is the config
    /// `fontFamily` for a theme without a `fontFamily` variable.
    pub fn resolve(tv: &ThemeVariables, fallback_font: &str) -> ArchitectureStyles {
        let text = |name: &str| css_text(tv.get(name));
        let color = |name: &str| text(name).and_then(|v| css_color(&v));
        let width = |name: &str| {
            text(name)
                .and_then(|v| css_length(&v))
                .unwrap_or(DEFAULT_STROKE_WIDTH)
        };
        let stroke = |name: &str| color(name).map_or(Paint::None, Paint::Color);
        ArchitectureStyles {
            font_family: tv
                .str("fontFamily")
                .filter(|f| !f.trim().is_empty())
                .unwrap_or(fallback_font)
                .to_string(),
            text_color: color("textColor").unwrap_or(Color::rgb(0x33, 0x33, 0x33)),
            edge_color: stroke("archEdgeColor"),
            edge_width: width("archEdgeWidth"),
            arrow_color: color("archEdgeArrowColor")
                .map_or(Paint::Color(Color::BLACK), Paint::Color),
            group_border_color: stroke("archGroupBorderColor"),
            group_border_width: width("archGroupBorderWidth"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::theme;

    #[test]
    fn default_theme_matches_the_oracle_stylesheet() {
        let tv = theme::resolve("default", None).unwrap();
        let s = ArchitectureStyles::resolve(&tv, "serif");
        assert_eq!(s.edge_color, Paint::Color(Color::rgb(0x33, 0x33, 0x33)));
        assert_eq!(s.edge_width, 3.0);
        assert_eq!(s.arrow_color, Paint::Color(Color::rgb(0x33, 0x33, 0x33)));
        // `hsl(240, 60%, 86.2745098039%)`.
        assert_eq!(
            s.group_border_color,
            Paint::Color(Color::rgb(0xc7, 0xc7, 0xf1))
        );
        assert_eq!(s.group_border_width, 2.0);
        assert_eq!(s.text_color, Color::rgb(0x33, 0x33, 0x33));
    }
}
