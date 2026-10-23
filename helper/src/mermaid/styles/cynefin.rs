//! `diagrams/cynefin/styles.ts` (`styles`) resolved to attribute values.
//!
//! mermaid writes `themeVariables.cynefin` (`getCynefinTheme` merges the
//! active theme variables over the default theme's, which leaves the active
//! ones) into one rule per class. The scene carries no CSS, so the values
//! are resolved here once and set on each element by the renderer. A value
//! the browser would reject makes it drop that declaration: a text then
//! inherits the root `fill: textColor` and `font-size: 16px`, a stroke
//! keeps its initial `none`, and a width its initial 1.

use serde_json::Value;

use crate::mermaid::scene::Color;
use crate::mermaid::styles::pie::{css_color, css_length};
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::khroma;

/// The root rule's `font-size`, inherited when a text declaration is
/// invalid.
const ROOT_FONT_SIZE: f64 = 16.0;
/// The SVG `stroke-width` initial value.
const DEFAULT_STROKE_WIDTH: f64 = 1.0;

/// The background colors the renderer sets as `fill` attributes
/// (`getCynefinDomainColors`). `None` is a color the browser rejects; the
/// shape then inherits the root fill.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainColors {
    pub complex: Option<Color>,
    pub complicated: Option<Color>,
    pub clear: Option<Color>,
    pub chaotic: Option<Color>,
    pub confusion: Option<Color>,
}

/// The declarations of `styles` as the renderer needs them.
#[derive(Debug, Clone, PartialEq)]
pub struct CynefinStyles {
    /// `options.fontFamily` of the global stylesheet, the theme's
    /// `fontFamily`.
    pub font_family: String,
    /// The root `fill: textColor` every text inherits.
    pub root_text_color: Color,
    /// `.cynefinDomainLabel { font-size: domainFontSize px }`.
    pub domain_font_size: f64,
    /// `.cynefinDomainLabel` and `.cynefinTitle { fill: labelColor }`.
    pub label_color: Color,
    /// `.cynefinSubtitle` and `.cynefinArrowLabel`: `itemFontSize - 1` px.
    pub subtitle_font_size: f64,
    /// `.cynefinItemText { font-size: itemFontSize px }`.
    pub item_font_size: f64,
    /// `.cynefinTitle { font-size: domainFontSize + 2 px }`.
    pub title_font_size: f64,
    /// `fill: textColor` of the subtitle, item, and arrow label rules.
    pub text_color: Color,
    /// `stroke: boundaryColor` of the boundary, confusion, and item rules.
    pub boundary_color: Option<Color>,
    pub boundary_width: f64,
    pub cliff_color: Option<Color>,
    pub cliff_width: f64,
    /// `.cynefinArrowLine { stroke }` and `.cynefinArrowHead { fill }`.
    pub arrow_color: Option<Color>,
    pub arrow_width: f64,
    pub domains: DomainColors,
}

/// The text a value interpolates into the stylesheet: a string as it is, a
/// number the way JS prints it, nothing for anything else.
fn css_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => n.as_f64().map(khroma::js_number),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// JS `ToNumber` for the `itemFontSize - 1` subtraction.
fn js_to_number(value: &Value) -> f64 {
    match value {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                0.0
            } else {
                t.parse().unwrap_or(f64::NAN)
            }
        }
        Value::Bool(b) => f64::from(u8::from(*b)),
        _ => f64::NAN,
    }
}

/// `${value}px` as a `font-size`: a value that already has a unit, or
/// `NaN`, makes the declaration invalid and the text keeps the root size.
fn px_font_size(text: &str) -> f64 {
    css_length(&format!("{text}px")).unwrap_or(ROOT_FONT_SIZE)
}

impl CynefinStyles {
    /// `styles()` and `getCynefinDomainColors()`. `fallback_font` is the
    /// config `fontFamily` for a theme without a `fontFamily` variable.
    pub fn resolve(tv: &ThemeVariables, fallback_font: &str) -> CynefinStyles {
        let cynefin = tv.get("cynefin");
        let raw = |name: &str| cynefin.get(name).unwrap_or(&Value::Null);
        let option = |name: &str| css_text(raw(name));
        let color = |name: &str| option(name).and_then(|v| css_color(&v));
        let width = |name: &str| {
            option(name)
                .and_then(|v| css_length(&v))
                .unwrap_or(DEFAULT_STROKE_WIDTH)
        };
        let root_text_color = tv
            .str("textColor")
            .and_then(css_color)
            .unwrap_or(Color::rgb(0x33, 0x33, 0x33));
        let text_fill = |name: &str| color(name).unwrap_or(root_text_color);
        let item_size = raw("itemFontSize");
        let domain_size = raw("domainFontSize");
        // `${t.domainFontSize + 2}px`: a string concatenates, a number adds.
        let title_text = match domain_size {
            Value::String(s) => Some(format!("{s}2")),
            Value::Null => None,
            other => Some(khroma::js_number(js_to_number(other) + 2.0)),
        };
        CynefinStyles {
            font_family: tv
                .str("fontFamily")
                .filter(|f| !f.trim().is_empty())
                .unwrap_or(fallback_font)
                .to_string(),
            root_text_color,
            domain_font_size: option("domainFontSize").map_or(ROOT_FONT_SIZE, |v| px_font_size(&v)),
            label_color: text_fill("labelColor"),
            subtitle_font_size: px_font_size(&khroma::js_number(js_to_number(item_size) - 1.0)),
            item_font_size: option("itemFontSize").map_or(ROOT_FONT_SIZE, |v| px_font_size(&v)),
            title_font_size: title_text.map_or(ROOT_FONT_SIZE, |v| px_font_size(&v)),
            text_color: text_fill("textColor"),
            boundary_color: color("boundaryColor"),
            boundary_width: width("boundaryWidth"),
            cliff_color: color("cliffColor"),
            cliff_width: width("cliffWidth"),
            arrow_color: color("arrowColor"),
            arrow_width: width("arrowWidth"),
            domains: DomainColors {
                complex: color("complexBg"),
                complicated: color("complicatedBg"),
                clear: color("clearBg"),
                chaotic: color("chaoticBg"),
                confusion: color("confusionBg"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::theme;
    use serde_json::json;

    #[test]
    fn default_theme_resolves_to_the_oracle_stylesheet() {
        let tv = theme::resolve("default", None).unwrap();
        let s = CynefinStyles::resolve(&tv, "serif");
        assert_eq!(
            s.font_family,
            "\"trebuchet ms\", verdana, arial, sans-serif"
        );
        assert_eq!(s.domain_font_size, 16.0);
        assert_eq!(s.subtitle_font_size, 11.0);
        assert_eq!(s.item_font_size, 12.0);
        assert_eq!(s.title_font_size, 18.0);
        assert_eq!(s.label_color, Color::rgb(0x13, 0x13, 0x00));
        assert_eq!(s.text_color, Color::rgb(0x33, 0x33, 0x33));
        assert_eq!(s.boundary_color, Some(Color::rgb(0x33, 0x33, 0x33)));
        assert_eq!(s.boundary_width, 2.0);
        assert_eq!(s.cliff_color, Some(Color::rgb(0x8B, 0, 0)));
        assert_eq!(s.cliff_width, 4.0);
        assert_eq!(s.arrow_width, 2.0);
        assert_eq!(s.domains.complex, Some(Color::rgb(0xE8, 0xF5, 0xE9)));
        assert_eq!(s.domains.confusion, Some(Color::rgb(0xF3, 0xE5, 0xF5)));
    }

    #[test]
    fn dark_theme_matches_the_oracle_stylesheet() {
        let tv = theme::resolve("dark", None).unwrap();
        let s = CynefinStyles::resolve(&tv, "serif");
        assert_eq!(s.label_color, Color::rgb(0xe0, 0xdf, 0xdf));
        assert_eq!(s.text_color, Color::rgb(0xcc, 0xcc, 0xcc));
        assert_eq!(s.boundary_color, Some(Color::rgb(0xd3, 0xd3, 0xd3)));
        assert_eq!(s.cliff_color, Some(Color::rgb(0xFF, 0x6B, 0x6B)));
        assert_eq!(s.domains.complex, Some(Color::rgb(0x1B, 0x5E, 0x20)));
    }

    #[test]
    fn overrides_follow_the_js_arithmetic() {
        let tv = theme::resolve_value(
            "base",
            &json!({
                "cynefin": {
                    "itemFontSize": "14",
                    "domainFontSize": "20",
                    "boundaryColor": "nope",
                    "labelColor": "nope"
                }
            }),
        )
        .unwrap();
        let s = CynefinStyles::resolve(&tv, "serif");
        assert_eq!(s.item_font_size, 14.0);
        assert_eq!(s.subtitle_font_size, 13.0);
        // `"20" + 2` is `"202"`.
        assert_eq!(s.title_font_size, 202.0);
        assert_eq!(s.boundary_color, None);
        assert_eq!(s.label_color, s.root_text_color);
    }
}
