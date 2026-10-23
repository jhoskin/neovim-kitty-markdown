//! The wardley colors: `getTheme` in `wardleyRenderer.ts`.
//!
//! `diagrams/wardley/styles.ts` never reaches the picture. mermaid inserts
//! the diagram stylesheet into the `<svg>` before it calls `draw`, and the
//! wardley `draw` starts with `svg.selectAll('*').remove()`, which removes
//! the `<style>` too. So the renderer's presentation attributes are all
//! that apply, with the values `getTheme` reads from `themeVariables`:
//! the `wardley` object each theme builds, then a few top-level variables,
//! then fixed defaults.

use serde_json::Value;

use crate::mermaid::scene::{Color, Paint};
use crate::mermaid::styles::pie::css_color;
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::khroma;

/// The `WardleyTheme` of `getTheme`, each value as the attribute paints
/// it. `None` is an attribute value the browser ignores, which leaves the
/// SVG initial value.
#[derive(Debug, Clone, PartialEq)]
pub struct WardleyStyles {
    pub background_color: Option<Paint>,
    pub axis_color: Option<Paint>,
    pub axis_text_color: Option<Paint>,
    pub grid_color: Option<Paint>,
    pub component_fill: Option<Paint>,
    pub component_stroke: Option<Paint>,
    pub component_label_color: Option<Paint>,
    pub link_stroke: Option<Paint>,
    pub evolution_stroke: Option<Paint>,
    pub annotation_stroke: Option<Paint>,
    pub annotation_text_color: Option<Paint>,
    pub annotation_fill: Option<Paint>,
}

/// The text `d3.attr` writes for a value: a string as it is, a number the
/// way JS prints it, nothing for `null` and `undefined`.
fn attr_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => n.as_f64().map(khroma::js_number),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

/// A `fill` or `stroke` attribute value.
pub fn attr_paint(value: &str) -> Option<Paint> {
    if value.trim().eq_ignore_ascii_case("none") {
        return Some(Paint::None);
    }
    css_color(value).map(Paint::Color)
}

/// A fixed color the renderer writes, such as `#000` or `white`.
pub fn fixed(value: &str) -> Paint {
    attr_paint(value).unwrap_or(Paint::Color(Color::BLACK))
}

impl WardleyStyles {
    /// `getTheme()`: `themeVariables.wardley?.<key> ?? <fallbacks>`, where
    /// `??` passes over `null` and `undefined` only.
    pub fn resolve(tv: &ThemeVariables) -> WardleyStyles {
        let wardley = tv.get("wardley");
        let pick = |key: &str, top: Option<&str>, default: &str| -> Option<Paint> {
            let text = wardley
                .get(key)
                .and_then(attr_text)
                .or_else(|| top.and_then(|name| attr_text(tv.get(name))))
                .unwrap_or_else(|| default.to_string());
            attr_paint(&text)
        };
        WardleyStyles {
            background_color: pick("backgroundColor", Some("background"), "#fff"),
            axis_color: pick("axisColor", None, "#000"),
            axis_text_color: pick("axisTextColor", Some("primaryTextColor"), "#222"),
            grid_color: pick("gridColor", None, "rgba(100, 100, 100, 0.2)"),
            component_fill: pick("componentFill", None, "#fff"),
            component_stroke: pick("componentStroke", None, "#000"),
            component_label_color: pick("componentLabelColor", Some("primaryTextColor"), "#222"),
            link_stroke: pick("linkStroke", None, "#000"),
            evolution_stroke: pick("evolutionStroke", None, "#dc3545"),
            annotation_stroke: pick("annotationStroke", None, "#000"),
            annotation_text_color: pick("annotationTextColor", Some("primaryTextColor"), "#222"),
            annotation_fill: pick("annotationFill", Some("background"), "#fff"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::theme;

    #[test]
    fn default_and_dark_themes() {
        let tv = theme::resolve("default", None).unwrap();
        let s = WardleyStyles::resolve(&tv);
        assert_eq!(
            s.background_color,
            Some(Paint::Color(Color::rgb(255, 255, 255)))
        );
        assert_eq!(
            s.axis_color,
            Some(Paint::Color(Color::rgb(0x33, 0x33, 0x33)))
        );
        assert_eq!(
            s.axis_text_color,
            Some(Paint::Color(Color::rgb(0x13, 0x13, 0x00)))
        );
        assert_eq!(
            s.evolution_stroke,
            Some(Paint::Color(Color::rgb(0xdc, 0x35, 0x45)))
        );
        let dark = WardleyStyles::resolve(&theme::resolve("dark", None).unwrap());
        assert_ne!(dark.background_color, s.background_color);
    }

    #[test]
    fn paints_follow_the_attribute_rules() {
        assert_eq!(attr_paint("none"), Some(Paint::None));
        assert_eq!(attr_paint("not a color"), None);
        assert_eq!(fixed("#ccc"), Paint::Color(Color::rgb(0xcc, 0xcc, 0xcc)));
    }
}
