//! `diagrams/radar/styles.ts` (`styles`) resolved to attribute values.
//!
//! mermaid writes `themeVariables.radar` (`buildRadarStyleOptions` merges
//! the active theme variables over the default theme's, which leaves the
//! active ones) into a stylesheet: `.radarTitle`, `.radarAxisLine`,
//! `.radarAxisLabel`, `.radarGraticule`, `.radarLegendText`, and per index
//! `i < THEME_COLOR_LIMIT` a `.radarCurve-i` and `.radarLegendBox-i` rule in
//! the color `cScale<i>`. The text rules set `color`, which SVG text does
//! not paint with, so text fills inherit the root `fill: textColor`. The
//! scene carries no CSS, so the values are resolved here once and set on
//! each element by the renderer. A value the browser would reject makes it
//! drop that declaration, and the element keeps the inherited or initial
//! value instead.

use serde_json::Value;

use crate::mermaid::scene::Color;
use crate::mermaid::styles::pie::{css_color, css_length};
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::khroma;

/// `THEME_COLOR_LIMIT`: curves past this index get no style rule.
pub const THEME_COLOR_LIMIT: usize = 12;

/// The font size of the root style, inherited when a text declaration is
/// invalid.
const ROOT_FONT_SIZE: f64 = 16.0;
/// The SVG `stroke-width` initial value.
const DEFAULT_STROKE_WIDTH: f64 = 1.0;
/// The SVG `fill-opacity` initial value.
const DEFAULT_OPACITY: f64 = 1.0;

/// The declarations of `styles` as the renderer needs them. A color is
/// `None` when the declaration is invalid and the browser would drop it.
#[derive(Debug, Clone, PartialEq)]
pub struct RadarStyles {
    /// `options.fontFamily` of the global stylesheet, the theme's
    /// `fontFamily`.
    pub font_family: String,
    /// The root `fill: textColor` every text inherits.
    pub text_color: Color,
    /// `.radarTitle { font-size: fontSize }`.
    pub title_font_size: f64,
    pub axis_color: Option<Color>,
    pub axis_stroke_width: f64,
    pub axis_label_font_size: f64,
    pub graticule_color: Option<Color>,
    pub graticule_opacity: f64,
    pub graticule_stroke_width: f64,
    pub legend_font_size: f64,
    pub curve_opacity: f64,
    pub curve_stroke_width: f64,
    /// `cScale0` to `cScale11`; `None` for a color the browser rejects.
    pub colors: [Option<Color>; THEME_COLOR_LIMIT],
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

/// A CSS `<number>` such as an opacity.
fn css_number(value: &str) -> Option<f64> {
    let v: f64 = value.trim().parse().ok()?;
    v.is_finite().then_some(v)
}

impl RadarStyles {
    /// `styles({ radar: themeVariables.radar })`. `fallback_font` is the
    /// config `fontFamily` for a theme without a `fontFamily` variable.
    pub fn resolve(tv: &ThemeVariables, fallback_font: &str) -> RadarStyles {
        let radar = tv.get("radar");
        let option = |name: &str| radar.get(name).and_then(css_text);
        let text_color = tv
            .str("textColor")
            .and_then(css_color)
            .unwrap_or(Color::rgb(0x33, 0x33, 0x33));
        let color = |name: &str| option(name).and_then(|v| css_color(&v));
        let width = |name: &str| {
            option(name)
                .and_then(|v| css_length(&v))
                .unwrap_or(DEFAULT_STROKE_WIDTH)
        };
        // `font-size: ${value}px`: a value that already has a unit makes
        // the declaration invalid.
        let px_size = |name: &str| {
            option(name)
                .and_then(|v| css_length(&format!("{v}px")))
                .unwrap_or(ROOT_FONT_SIZE)
        };
        let opacity = |name: &str| {
            option(name)
                .and_then(|v| css_number(&v))
                .map(|o| o.clamp(0.0, 1.0))
                .unwrap_or(DEFAULT_OPACITY)
        };
        let mut colors = [None; THEME_COLOR_LIMIT];
        for (i, slot) in colors.iter_mut().enumerate() {
            *slot = css_text(tv.get(&format!("cScale{i}"))).and_then(|v| css_color(&v));
        }
        RadarStyles {
            font_family: tv
                .str("fontFamily")
                .filter(|f| !f.trim().is_empty())
                .unwrap_or(fallback_font)
                .to_string(),
            text_color,
            title_font_size: css_text(tv.get("fontSize"))
                .and_then(|v| css_length(&v))
                .unwrap_or(ROOT_FONT_SIZE),
            axis_color: color("axisColor"),
            axis_stroke_width: width("axisStrokeWidth"),
            axis_label_font_size: px_size("axisLabelFontSize"),
            graticule_color: color("graticuleColor"),
            graticule_opacity: opacity("graticuleOpacity"),
            graticule_stroke_width: width("graticuleStrokeWidth"),
            legend_font_size: px_size("legendFontSize"),
            curve_opacity: opacity("curveOpacity"),
            curve_stroke_width: width("curveStrokeWidth"),
            colors,
        }
    }

    /// The `cScale` color of curve `index`, or `None` past the last rule
    /// or for a rejected color.
    pub fn curve_color(&self, index: usize) -> Option<Color> {
        self.colors.get(index).copied().flatten()
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
        let s = RadarStyles::resolve(&tv, "serif");
        assert_eq!(
            s.font_family,
            "\"trebuchet ms\", verdana, arial, sans-serif"
        );
        assert_eq!(s.text_color, Color::rgb(0x33, 0x33, 0x33));
        assert_eq!(s.title_font_size, 16.0);
        assert_eq!(s.axis_color, Some(Color::rgb(0x33, 0x33, 0x33)));
        assert_eq!(s.axis_stroke_width, 2.0);
        assert_eq!(s.axis_label_font_size, 12.0);
        assert_eq!(s.graticule_color, Some(Color::rgb(0xDE, 0xDE, 0xDE)));
        assert_eq!(s.graticule_opacity, 0.3);
        assert_eq!(s.graticule_stroke_width, 1.0);
        assert_eq!(s.legend_font_size, 12.0);
        assert_eq!(s.curve_opacity, 0.5);
        assert_eq!(s.curve_stroke_width, 2.0);
        // `hsl(240, 100%, 76.2745098039%)`.
        assert_eq!(s.curve_color(0), Some(Color::rgb(0x86, 0x86, 0xFF)));
        assert_eq!(s.curve_color(12), None);
    }

    #[test]
    fn overrides_and_rejected_values() {
        let tv = theme::resolve_value(
            "base",
            &json!({
                "cScale0": "#FF0000",
                "radar": { "curveOpacity": 0, "axisLabelFontSize": "14px", "axisColor": "nope" }
            }),
        )
        .unwrap();
        let s = RadarStyles::resolve(&tv, "serif");
        assert_eq!(s.curve_color(0), Some(Color::rgb(255, 0, 0)));
        assert_eq!(s.curve_opacity, 0.0);
        assert_eq!(s.axis_label_font_size, ROOT_FONT_SIZE);
        assert_eq!(s.axis_color, None);
    }
}
