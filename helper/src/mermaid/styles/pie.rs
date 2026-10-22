//! `pieStyles.ts` (`getStyles`) resolved to attribute values.
//!
//! mermaid writes the pie theme variables into a stylesheet:
//! `.pieCircle { stroke; stroke-width; opacity }`, `.pieOuterCircle {
//! stroke; stroke-width; fill: none }`, `.pieTitleText`, `.slice`, and
//! `.legend text` with a fill, a font size, and the theme font family. The
//! scene carries no CSS, so the same values are resolved here once and set
//! on each element by the renderer. A value the browser would reject (a
//! color it cannot parse, a length without a number) makes the browser drop
//! that declaration, so the same fallback applies here.

use serde_json::Value;

use crate::mermaid::scene::Color;
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::khroma::{self, Channel};

/// The declarations of `getStyles` as the renderer needs them.
#[derive(Debug, Clone, PartialEq)]
pub struct PieStyles {
    /// `options.fontFamily`, the theme's `fontFamily`.
    pub font_family: String,
    /// `pie1` to `pie12`, the slice fill cycle. A color the browser cannot
    /// parse paints black, which is what an unset `fill` attribute does.
    pub colors: [Color; 12],
    pub title_size: f64,
    pub title_color: Color,
    pub section_size: f64,
    pub section_color: Color,
    pub legend_size: f64,
    pub legend_color: Color,
    /// `.pieCircle` stroke; `None` when the declaration is invalid, which
    /// leaves the SVG default of no stroke.
    pub stroke_color: Option<Color>,
    pub stroke_width: f64,
    pub outer_stroke_color: Option<Color>,
    pub outer_stroke_width: f64,
    pub opacity: f64,
}

/// The `font-size` the root style of the SVG sets, inherited when a text
/// declaration is invalid.
const ROOT_FONT_SIZE: f64 = 16.0;
/// The SVG `stroke-width` initial value.
const DEFAULT_STROKE_WIDTH: f64 = 1.0;

impl PieStyles {
    /// `getStyles(themeVariables)` for the pie diagram. `fallback_font` is
    /// the config `fontFamily` for a theme without a `fontFamily` variable.
    pub fn resolve(tv: &ThemeVariables, fallback_font: &str) -> PieStyles {
        // The root rule `fill: #333` (mermaid's `getStyles` in `styles.ts`
        // uses `textColor`) is what an invalid text fill falls back to.
        let root_fill = tv
            .str("textColor")
            .and_then(css_color)
            .unwrap_or(Color::rgb(0x33, 0x33, 0x33));
        let text_color = |name: &str| {
            css_value(tv, name)
                .and_then(|v| css_color(&v))
                .unwrap_or(root_fill)
        };
        let text_size = |name: &str| {
            css_value(tv, name)
                .and_then(|v| css_length(&v))
                .unwrap_or(ROOT_FONT_SIZE)
        };
        let stroke_color = |name: &str| css_value(tv, name).and_then(|v| css_color(&v));
        let stroke_width = |name: &str| {
            css_value(tv, name)
                .and_then(|v| css_length(&v))
                .unwrap_or(DEFAULT_STROKE_WIDTH)
        };
        let mut colors = [Color::BLACK; 12];
        for (i, slot) in colors.iter_mut().enumerate() {
            if let Some(c) = css_value(tv, &format!("pie{}", i + 1)).and_then(|v| css_color(&v)) {
                *slot = c;
            }
        }
        PieStyles {
            font_family: tv
                .str("fontFamily")
                .filter(|f| !f.trim().is_empty())
                .unwrap_or(fallback_font)
                .to_string(),
            colors,
            title_size: text_size("pieTitleTextSize"),
            title_color: text_color("pieTitleTextColor"),
            section_size: text_size("pieSectionTextSize"),
            section_color: text_color("pieSectionTextColor"),
            legend_size: text_size("pieLegendTextSize"),
            legend_color: text_color("pieLegendTextColor"),
            stroke_color: stroke_color("pieStrokeColor"),
            stroke_width: stroke_width("pieStrokeWidth"),
            outer_stroke_color: stroke_color("pieOuterStrokeColor"),
            outer_stroke_width: stroke_width("pieOuterStrokeWidth"),
            opacity: css_value(tv, "pieOpacity")
                .and_then(|s| s.trim().parse::<f64>().ok())
                .map(|o| o.clamp(0.0, 1.0))
                .unwrap_or(1.0),
        }
    }
}

/// The text a theme variable interpolates into the stylesheet: a string as
/// it is, a number the way JS prints it (a user override such as
/// `pieStrokeWidth: 3`), nothing for anything else.
fn css_value(tv: &ThemeVariables, name: &str) -> Option<String> {
    match tv.get(name) {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => n.as_f64().map(khroma::js_number),
        _ => None,
    }
}

/// A CSS color as khroma reads it (hex, `rgb()`, `hsl()`, keywords); the
/// theme code writes nothing else.
pub fn css_color(value: &str) -> Option<Color> {
    let mut ch = khroma::parse(value.trim()).ok()?;
    let byte = |v: f64| v.round().clamp(0.0, 255.0) as u8;
    Some(Color::rgba(
        byte(ch.get(Channel::R)),
        byte(ch.get(Channel::G)),
        byte(ch.get(Channel::B)),
        ch.get(Channel::A).clamp(0.0, 1.0),
    ))
}

/// A CSS `<length>` in user units: a bare number or a `px` value. Other
/// units never come out of the themes; the browser would resolve them
/// against the font, which the scene cannot express, so they are dropped.
pub fn css_length(value: &str) -> Option<f64> {
    let s = value.trim();
    let digits = s.strip_suffix("px").unwrap_or(s).trim_end();
    let v: f64 = digits.parse().ok()?;
    v.is_finite().then_some(v)
}

/// `parseFontSize` (`utils.ts`) as `pieRenderer.ts` uses it for the outer
/// circle radius: `parseInt(value, 10)`, so `2.5px` reads as 2 and a value
/// without a leading integer is `undefined`.
pub fn parse_int_prefix(value: &str) -> Option<f64> {
    let s = value.trim_start_matches(|c: char| c.is_whitespace());
    let (sign, rest) = match s.strip_prefix('-') {
        Some(r) => (-1.0, r),
        None => (1.0, s.strip_prefix('+').unwrap_or(s)),
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<f64>().ok().map(|v| sign * v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::theme;

    #[test]
    fn default_theme_resolves_to_the_oracle_stylesheet() {
        let tv = theme::resolve("default", None).unwrap();
        let s = PieStyles::resolve(&tv, "serif");
        assert_eq!(
            s.font_family,
            "\"trebuchet ms\", verdana, arial, sans-serif"
        );
        assert_eq!(s.colors[0], Color::rgb(0xEC, 0xEC, 0xFF));
        assert_eq!(s.title_size, 25.0);
        assert_eq!(s.title_color, Color::BLACK);
        assert_eq!(s.section_size, 17.0);
        assert_eq!(s.section_color, Color::rgb(0x33, 0x33, 0x33));
        assert_eq!(s.legend_size, 17.0);
        assert_eq!(s.stroke_color, Some(Color::BLACK));
        assert_eq!(s.stroke_width, 2.0);
        assert_eq!(s.outer_stroke_width, 2.0);
        assert_eq!(s.opacity, 0.7);
    }

    #[test]
    fn css_values_parse_like_a_browser() {
        assert_eq!(css_length("17px"), Some(17.0));
        assert_eq!(css_length("2.5"), Some(2.5));
        assert_eq!(css_length("big"), None);
        assert_eq!(css_color("black"), Some(Color::BLACK));
        assert_eq!(css_color("#ff0000"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(css_color("nope"), None);
        assert_eq!(parse_int_prefix("2.5px"), Some(2.0));
        assert_eq!(parse_int_prefix("5px"), Some(5.0));
        assert_eq!(parse_int_prefix("px"), None);
    }
}
