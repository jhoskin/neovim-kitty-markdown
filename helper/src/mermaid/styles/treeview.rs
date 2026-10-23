//! `diagrams/treeView/styles.ts` (`styles`) resolved to attribute values.
//!
//! mermaid merges `defaultTreeViewDiagramStyles` with
//! `themeVariables.treeView` (`cleanAndMerge`, a lodash `merge` that skips
//! `undefined`) and writes the result into a stylesheet:
//! `.treeView-node-label { font-size; fill; white-space: pre }`,
//! `.treeView-node-dir { font-weight: bold }`, `.treeView-node-line {
//! stroke }`, `.treeView-node-icon { color }`, `.treeView-node-description
//! { font-size; fill; font-style: italic; white-space: pre }`, and
//! `.treeView-highlight-bg { fill; stroke; stroke-width: 1 }`. No built-in
//! theme derives a `treeView` object, so every theme draws with the
//! defaults below unless the diagram sets `themeVariables.treeView`.
//!
//! A declaration the browser rejects is dropped: a rejected label or
//! description `fill` inherits the root `fill` (the theme's `textColor`), a
//! rejected `stroke` or highlight `fill` leaves none, a rejected `color`
//! inherits the initial black, and a rejected `font-size` inherits the
//! root 16 px.

use serde_json::Value;

use crate::mermaid::scene::Color;
use crate::mermaid::styles::pie::css_color;
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::khroma;

/// The declarations of `styles` as the renderer needs them.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeViewStyles {
    /// `options.fontFamily` of the global stylesheet, the theme's
    /// `fontFamily`.
    pub font_family: String,
    pub label_font_size: f64,
    pub label_color: Color,
    /// `None` when the declaration is invalid: the lines get no stroke.
    pub line_color: Option<Color>,
    pub icon_color: Color,
    pub description_color: Color,
    pub highlight_bg: Option<Color>,
    pub highlight_stroke: Option<Color>,
}

/// `defaultTreeViewDiagramStyles` in `styles.ts`.
const DEFAULTS: [(&str, &str); 7] = [
    ("labelFontSize", "16px"),
    ("labelColor", "black"),
    ("lineColor", "black"),
    ("iconColor", "#546e7a"),
    ("descriptionColor", "#6a9955"),
    ("highlightBg", "rgba(255, 193, 7, 0.15)"),
    ("highlightStroke", "#ffc107"),
];

/// The `font-size` of the root style, inherited when a declaration is
/// invalid.
const ROOT_FONT_SIZE: f64 = 16.0;

/// A CSS `font-size` in a stylesheet: a `px` length or `0`. A bare number
/// is invalid CSS, and other units never come out of a diagram config in
/// practice; both fall back to the inherited size.
fn css_font_size(value: &str) -> Option<f64> {
    let s = value.trim();
    let v: f64 = match s.strip_suffix("px") {
        Some(digits) => digits.parse().ok()?,
        None if s == "0" => 0.0,
        None => return None,
    };
    (v.is_finite() && v >= 0.0).then_some(v)
}

impl TreeViewStyles {
    /// `styles({ treeView: themeVariables.treeView })`. `fallback_font` is
    /// the config `fontFamily` for a theme without a `fontFamily` variable.
    pub fn resolve(tv: &ThemeVariables, fallback_font: &str) -> TreeViewStyles {
        let overrides = tv.get("treeView");
        let option = |name: &str| -> Option<String> {
            let default = DEFAULTS
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (*v).to_string());
            match overrides.get(name) {
                Some(Value::String(s)) => Some(s.clone()),
                Some(Value::Number(n)) => n.as_f64().map(khroma::js_number),
                Some(Value::Bool(b)) => Some(b.to_string()),
                // `undefined` and `null` are skipped by the merge; an object
                // or array interpolates as text the browser rejects.
                Some(Value::Null) | None => default,
                Some(_) => None,
            }
        };
        let color = |name: &str| option(name).and_then(|v| css_color(&v));
        let text_color = tv
            .str("textColor")
            .and_then(css_color)
            .unwrap_or(Color::BLACK);
        TreeViewStyles {
            font_family: tv
                .str("fontFamily")
                .filter(|f| !f.trim().is_empty())
                .unwrap_or(fallback_font)
                .to_string(),
            label_font_size: option("labelFontSize")
                .and_then(|v| css_font_size(&v))
                .unwrap_or(ROOT_FONT_SIZE),
            label_color: color("labelColor").unwrap_or(text_color),
            line_color: color("lineColor"),
            icon_color: color("iconColor").unwrap_or(Color::BLACK),
            description_color: color("descriptionColor").unwrap_or(text_color),
            highlight_bg: color("highlightBg"),
            highlight_stroke: color("highlightStroke"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::theme;
    use serde_json::json;

    #[test]
    fn every_theme_uses_the_stylesheet_defaults() {
        for name in ["default", "dark", "neutral", "forest", "base"] {
            let tv = theme::resolve(name, None).unwrap();
            let s = TreeViewStyles::resolve(&tv, "serif");
            assert_eq!(
                s.font_family, "\"trebuchet ms\", verdana, arial, sans-serif",
                "{name}"
            );
            assert_eq!(s.label_font_size, 16.0, "{name}");
            assert_eq!(s.label_color, Color::BLACK, "{name}");
            assert_eq!(s.line_color, Some(Color::BLACK), "{name}");
            assert_eq!(s.icon_color, Color::rgb(0x54, 0x6e, 0x7a), "{name}");
            assert_eq!(s.description_color, Color::rgb(0x6a, 0x99, 0x55), "{name}");
            assert_eq!(s.highlight_bg, Some(Color::rgba(255, 193, 7, 0.15)));
            assert_eq!(s.highlight_stroke, Some(Color::rgb(0xff, 0xc1, 0x07)));
        }
    }

    #[test]
    fn overrides_merge_over_the_defaults() {
        let tv = theme::resolve_value(
            "default",
            &json!({ "treeView": { "labelFontSize": "20px", "labelColor": "#FF0000", "lineColor": "nope" } }),
        )
        .unwrap();
        let s = TreeViewStyles::resolve(&tv, "serif");
        assert_eq!(s.label_font_size, 20.0);
        assert_eq!(s.label_color, Color::rgb(255, 0, 0));
        assert_eq!(s.line_color, None);
        assert_eq!(s.icon_color, Color::rgb(0x54, 0x6e, 0x7a));
    }

    #[test]
    fn a_unitless_font_size_is_dropped() {
        assert_eq!(css_font_size("20"), None);
        assert_eq!(css_font_size("0"), Some(0.0));
        assert_eq!(css_font_size(" 12.5px "), Some(12.5));
    }
}
