//! `diagrams/venn/styles.ts` (mermaid 12.2.0) resolved to attributes, with
//! the theme variables `vennRenderer.ts` reads for its inline styles.
//!
//! The sheet sets `.venn-title { font-size: 32px; fill:
//! vennTitleTextColor }`, `.venn-circle text` and `.venn-intersection
//! text { font-size: 48px }` (the intersection rule also fills with
//! `vennSetTextColor`), and `.venn-text-node { color: vennSetTextColor }`,
//! all with the theme's `fontFamily`. The renderer's inline styles win
//! over every rule but the title size: the title's `font-size` is a
//! presentation attribute, which any CSS rule beats, so the title stays at
//! 32 px while the labels take the inline `48 * scale` px.

use crate::mermaid::scene::{Color, DEFAULT_FONT_FAMILY, Paint};
use crate::mermaid::styles::gantt::css_color;
use crate::mermaid::styles::sequence::css_paint;
use crate::mermaid::theme::ThemeVariables;

/// `.venn-title { font-size: 32px }`.
pub const TITLE_FONT_SIZE: f64 = 32.0;

#[derive(Debug, Clone, PartialEq)]
pub struct VennStyles {
    pub font_family: String,
    /// `themeVariables.venn1` to `venn8`, the truthy ones in order.
    pub palette: Vec<String>,
    /// `themeVariables.primaryColor`, the circle color without a palette.
    pub primary_color: String,
    /// `isDark(themeVariables.background || '#f4f4f4')`, as its input.
    pub background: String,
    /// `themeVariables.vennTitleTextColor || themeVariables.titleColor`.
    pub title_fill: Paint,
    /// `themeVariables.vennSetTextColor ?? defaultTextColor`, where
    /// `defaultTextColor` is `primaryTextColor ?? textColor`: the label
    /// fill of a region.
    pub region_text_fill: String,
    /// `.venn-text-node { color: vennSetTextColor }`; an unset variable
    /// leaves the HTML default, black.
    pub text_node_color: Color,
}

impl VennStyles {
    pub fn resolve(vars: &ThemeVariables) -> VennStyles {
        let truthy = |name: &str| vars.str(name).filter(|s| !s.is_empty());
        let palette = (1..=8)
            .filter_map(|i| truthy(&format!("venn{i}")).map(str::to_string))
            .collect();
        let default_text = vars
            .str("primaryTextColor")
            .or_else(|| vars.str("textColor"))
            .unwrap_or("black");
        let title = truthy("vennTitleTextColor")
            .or_else(|| vars.str("titleColor"))
            .unwrap_or("black");
        let set_text = vars.str("vennSetTextColor");
        VennStyles {
            font_family: vars
                .str("fontFamily")
                .unwrap_or(DEFAULT_FONT_FAMILY)
                .to_string(),
            palette,
            primary_color: vars.str("primaryColor").unwrap_or("#ECECFF").to_string(),
            background: truthy("background").unwrap_or("#f4f4f4").to_string(),
            title_fill: css_paint(title),
            region_text_fill: set_text.unwrap_or(default_text).to_string(),
            text_node_color: set_text.map(css_color).unwrap_or(Color::BLACK),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::theme;
    use serde_json::Value;

    #[test]
    fn default_theme_has_a_palette_and_dark_grey_text() {
        let vars = theme::resolve_value("default", &Value::Null).expect("theme");
        let s = VennStyles::resolve(&vars);
        assert_eq!(s.palette.len(), 8);
        assert_eq!(s.title_fill, Paint::Color(Color::rgb(51, 51, 51)));
        assert_eq!(s.text_node_color, Color::rgb(51, 51, 51));
    }
}
