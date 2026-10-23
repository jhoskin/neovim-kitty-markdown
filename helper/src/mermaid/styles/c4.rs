//! `diagrams/c4/styles.js` (mermaid 12.2.0) resolved to attributes, plus
//! the element palette of `c4ShapeAdapter.ts`.
//!
//! The rules that decide what the user sees:
//!
//! - The root rule of `styles.ts` (`fill: textColor; font-family;
//!   font-size`) paints the title and the arrowheads, whose paths set no
//!   fill and inherit from the `svg` through their `marker`.
//! - `elementFontStyles()` gives each `.c4-shape.c4-<type> .label` the
//!   `<type>FontFamily`, `<type>FontSize`, and `<type>FontWeight` of the c4
//!   config; a key that is unset or empty leaves the root font.
//! - `.c4-name { font-weight: bold }`, `.c4-type { font-size: 0.75em }`,
//!   and `.c4-descr { font-size: 0.82em }` style the label sections.
//! - `.c4-shape .basic, rect, path, circle, ellipse, line { stroke-width:
//!   2px }` thickens the element outlines; the `polygon` of a framed
//!   rectangle keeps the default 1 px.
//!
//! The `.person { stroke: personBorder; fill: personBkg }` rule matches no
//! element: the person shape's classes are `c4-shape c4-person`.

use serde_json::Value;

use crate::mermaid::diagrams::c4::conf::{C4Font, Conf};
use crate::mermaid::scene::{FontSpec, Paint, Style};
use crate::mermaid::theme::ThemeVariables;

use super::sequence::css_paint;

/// `C4_ELEMENT_TYPES` of `c4ShapeAdapter.ts`: every type with per-type
/// config, internal and external.
pub const C4_ELEMENT_TYPES: [&str; 20] = [
    "person",
    "external_person",
    "system",
    "external_system",
    "system_db",
    "external_system_db",
    "system_queue",
    "external_system_queue",
    "container",
    "external_container",
    "container_db",
    "external_container_db",
    "container_queue",
    "external_container_queue",
    "component",
    "external_component",
    "component_db",
    "external_component_db",
    "component_queue",
    "external_component_queue",
];

/// `.c4-shape .basic, rect, path, circle, ...  { stroke-width: 2px }`.
pub const SHAPE_STROKE_WIDTH: f64 = 2.0;
/// `.c4-type { font-size: 0.75em }`.
pub const TYPE_FONT_SCALE: f64 = 0.75;
/// `.c4-descr { font-size: 0.82em }`.
pub const DESCR_FONT_SCALE: f64 = 0.82;
/// The element text color when `$fontColor` is unset.
pub const DEFAULT_FONT_COLOR: &str = "#FFFFFF";
/// `svgDraw.ts`: boundary text and the default boundary and rel stroke.
pub const BOUNDARY_COLOR: &str = "#444444";

#[derive(Debug, Clone, PartialEq)]
pub struct C4Styles {
    /// The root `fill`, from `textColor`.
    pub text_color: Paint,
    /// The root `font-family`, from the theme's `fontFamily`.
    pub font_family: String,
    /// The root `font-size` in pixels, from the theme's `fontSize`.
    pub font_size: f64,
}

/// CSS `font-size` such as `16px` to pixels; `16` without a unit too.
fn css_px(value: &str) -> Option<f64> {
    value
        .trim()
        .trim_end_matches("px")
        .trim()
        .parse::<f64>()
        .ok()
}

impl C4Styles {
    pub fn resolve(vars: &ThemeVariables) -> C4Styles {
        C4Styles {
            text_color: css_paint(vars.str("textColor").unwrap_or("#333")),
            font_family: vars
                .str("fontFamily")
                .unwrap_or("\"trebuchet ms\", verdana, arial, sans-serif")
                .to_string(),
            font_size: vars.str("fontSize").and_then(css_px).unwrap_or(16.0),
        }
    }

    /// The root rule as the root group's style.
    pub fn root(&self) -> Style {
        Style {
            fill: Some(self.text_color.clone()),
            font: FontSpec {
                family: Some(self.font_family.clone()),
                size: Some(self.font_size),
                ..FontSpec::default()
            },
            ..Style::default()
        }
    }

    /// The `.label` font of an element of `type_c4_shape`: the
    /// `elementFontStyles()` rule over the root font.
    pub fn element_font(&self, conf: &Conf, type_c4_shape: &str) -> C4Font {
        let mut font = C4Font {
            family: self.font_family.clone(),
            size: self.font_size,
            weight: "normal".to_string(),
        };
        if !C4_ELEMENT_TYPES.contains(&type_c4_shape) {
            return font;
        }
        if let Some(Value::String(family)) = conf.raw(&format!("{type_c4_shape}FontFamily"))
            && !family.is_empty()
        {
            font.family = family.clone();
        }
        match conf.raw(&format!("{type_c4_shape}FontSize")) {
            Some(Value::Number(n)) => {
                if let Some(size) = n.as_f64().filter(|s| *s != 0.0) {
                    font.size = size;
                }
            }
            Some(Value::String(s)) => {
                if let Some(size) = css_px(s) {
                    font.size = size;
                }
            }
            _ => {}
        }
        match conf.raw(&format!("{type_c4_shape}FontWeight")) {
            Some(Value::String(w)) if !w.is_empty() => font.weight = w.clone(),
            Some(Value::Number(n)) => font.weight = n.to_string(),
            _ => {}
        }
        font
    }

    /// The `fill` and `stroke` `elementCssStyles` puts inline: the
    /// `$bgColor`/`$borderColor` override, else the per-type palette.
    pub fn element_colors(
        conf: &Conf,
        type_c4_shape: &str,
        bg_color: Option<&str>,
        border_color: Option<&str>,
    ) -> (Option<Paint>, Option<Paint>) {
        let known = C4_ELEMENT_TYPES.contains(&type_c4_shape);
        let pick = |explicit: Option<&str>, suffix: &str| -> Option<Paint> {
            let value = match explicit {
                Some(v) => Some(v.to_string()),
                None if known => conf.color(&format!("{type_c4_shape}{suffix}")),
                None => None,
            };
            value.filter(|v| !v.is_empty()).map(|v| css_paint(&v))
        };
        (
            pick(bg_color, "_bg_color"),
            pick(border_color, "_border_color"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::config::{self, BaseConfig};
    use crate::mermaid::scene::Color;

    #[test]
    fn the_palette_and_the_fonts_come_from_the_c4_config() {
        let resolved = config::resolve(
            "C4Context\n  Person(a, \"A\")\n",
            &BaseConfig::new("default"),
        )
        .expect("resolves");
        let conf = Conf::from_resolved(&resolved);
        let styles = C4Styles::resolve(&resolved.theme_variables);
        let (fill, stroke) = C4Styles::element_colors(&conf, "system", None, Some("red"));
        assert_eq!(fill, Some(Paint::Color(Color::rgb(0x11, 0x68, 0xBD))));
        assert_eq!(stroke, Some(Paint::Color(Color::rgb(255, 0, 0))));
        let font = styles.element_font(&conf, "person");
        assert_eq!(font.family, "\"Open Sans\", sans-serif");
        assert_eq!(font.size, 14.0);
        assert_eq!(
            styles.text_color,
            Paint::Color(Color::rgb(0x33, 0x33, 0x33))
        );
    }
}
