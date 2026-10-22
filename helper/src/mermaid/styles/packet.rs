//! `diagrams/packet/styles.ts` (`styles`) resolved to attribute values.
//!
//! mermaid merges `defaultPacketStyleOptions` with `themeVariables.packet`
//! (`cleanAndMerge`, a lodash `merge` that skips `undefined`) and writes the
//! result into a stylesheet: `.packetByte { font-size }`, `.packetByte.start
//! { fill }`, `.packetByte.end { fill }`, `.packetLabel { fill; font-size }`,
//! `.packetTitle { fill; font-size }`, and `.packetBlock { stroke;
//! stroke-width; fill }`. The scene carries no CSS, so the same values are
//! resolved here once and set on each element by the renderer. A value the
//! browser would reject makes it drop that declaration, so a rejected fill
//! inherits the root `fill` (textColor), a rejected stroke leaves no stroke,
//! and a rejected size inherits the root 16 px.
//!
//! Only the dark and forest themes derive a `packet` object; the default,
//! neutral, and base themes carry none, so their diagrams use the defaults
//! below unless the diagram sets `themeVariables.packet`.

use serde_json::Value;

use crate::mermaid::scene::Color;
use crate::mermaid::styles::pie::{css_color, css_length};
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::khroma;

/// The declarations of `styles` as the renderer needs them. A color is
/// `None` when the declaration is invalid and the browser would drop it.
#[derive(Debug, Clone, PartialEq)]
pub struct PacketStyles {
    /// `options.fontFamily` of the global stylesheet, the theme's
    /// `fontFamily`.
    pub font_family: String,
    pub byte_font_size: f64,
    pub start_byte_color: Option<Color>,
    pub end_byte_color: Option<Color>,
    pub label_color: Option<Color>,
    pub label_font_size: f64,
    pub title_color: Option<Color>,
    pub title_font_size: f64,
    pub block_stroke_color: Option<Color>,
    pub block_stroke_width: f64,
    pub block_fill_color: Option<Color>,
}

/// `defaultPacketStyleOptions` in `styles.ts`.
const DEFAULTS: [(&str, &str); 10] = [
    ("byteFontSize", "10px"),
    ("startByteColor", "black"),
    ("endByteColor", "black"),
    ("labelColor", "black"),
    ("labelFontSize", "12px"),
    ("titleColor", "black"),
    ("titleFontSize", "14px"),
    ("blockStrokeColor", "black"),
    ("blockStrokeWidth", "1"),
    ("blockFillColor", "#efefef"),
];

/// The `font-size` the root style of the SVG sets, inherited when a text
/// declaration is invalid.
const ROOT_FONT_SIZE: f64 = 16.0;
/// The SVG `stroke-width` initial value.
const DEFAULT_STROKE_WIDTH: f64 = 1.0;

impl PacketStyles {
    /// `styles({ packet: themeVariables.packet })`. `fallback_font` is the
    /// config `fontFamily` for a theme without a `fontFamily` variable.
    pub fn resolve(tv: &ThemeVariables, fallback_font: &str) -> PacketStyles {
        let overrides = tv.get("packet");
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
        let size = |name: &str| {
            option(name)
                .and_then(|v| css_length(&v))
                .unwrap_or(ROOT_FONT_SIZE)
        };
        PacketStyles {
            font_family: tv
                .str("fontFamily")
                .filter(|f| !f.trim().is_empty())
                .unwrap_or(fallback_font)
                .to_string(),
            byte_font_size: size("byteFontSize"),
            start_byte_color: color("startByteColor"),
            end_byte_color: color("endByteColor"),
            label_color: color("labelColor"),
            label_font_size: size("labelFontSize"),
            title_color: color("titleColor"),
            title_font_size: size("titleFontSize"),
            block_stroke_color: color("blockStrokeColor"),
            block_stroke_width: option("blockStrokeWidth")
                .and_then(|v| css_length(&v))
                .unwrap_or(DEFAULT_STROKE_WIDTH),
            block_fill_color: color("blockFillColor"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::theme;
    use serde_json::json;

    #[test]
    fn default_theme_uses_the_stylesheet_defaults() {
        let tv = theme::resolve("default", None).unwrap();
        let s = PacketStyles::resolve(&tv, "serif");
        assert_eq!(
            s.font_family,
            "\"trebuchet ms\", verdana, arial, sans-serif"
        );
        assert_eq!(s.byte_font_size, 10.0);
        assert_eq!(s.label_font_size, 12.0);
        assert_eq!(s.title_font_size, 14.0);
        assert_eq!(s.start_byte_color, Some(Color::BLACK));
        assert_eq!(s.end_byte_color, Some(Color::BLACK));
        assert_eq!(s.label_color, Some(Color::BLACK));
        assert_eq!(s.title_color, Some(Color::BLACK));
        assert_eq!(s.block_stroke_color, Some(Color::BLACK));
        assert_eq!(s.block_stroke_width, 1.0);
        assert_eq!(s.block_fill_color, Some(Color::rgb(0xEF, 0xEF, 0xEF)));
    }

    #[test]
    fn dark_theme_derives_its_packet_object() {
        let tv = theme::resolve("dark", None).unwrap();
        let s = PacketStyles::resolve(&tv, "serif");
        let text = css_color(tv.str("primaryTextColor").unwrap());
        let background = css_color(tv.str("background").unwrap());
        assert_eq!(s.label_color, text);
        assert_eq!(s.block_stroke_color, text);
        assert_eq!(s.block_fill_color, background);
        // Sizes are not part of the theme object.
        assert_eq!(s.byte_font_size, 10.0);
    }

    #[test]
    fn overrides_merge_over_the_defaults() {
        let tv = theme::resolve_value(
            "default",
            &json!({ "packet": { "startByteColor": "red", "blockStrokeWidth": 2, "labelFontSize": "big" } }),
        )
        .unwrap();
        let s = PacketStyles::resolve(&tv, "serif");
        assert_eq!(s.start_byte_color, Some(Color::rgb(255, 0, 0)));
        assert_eq!(s.end_byte_color, Some(Color::BLACK));
        assert_eq!(s.block_stroke_width, 2.0);
        assert_eq!(s.label_font_size, ROOT_FONT_SIZE);
    }
}
