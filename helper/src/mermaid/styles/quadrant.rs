//! Quadrant chart styles resolved to attributes.
//!
//! `quadrantDiagram.ts` (mermaid 12.2.0) registers `styles: () => ''`: the
//! chart has no stylesheet. Every color and size is written as an attribute
//! by `quadrantRenderer.ts` from the builder output, and the only inherited
//! value is the root `font-family`, which the scene carries. These helpers
//! turn the builder's color strings and CSS lengths into scene paints.

use crate::mermaid::scene::{Color, FontSpec, Paint, Style};
use crate::mermaid::theme::khroma::{self, Channel};

/// A theme or user color as a scene paint. `None` stands for a value the
/// SVG would not carry (an undefined theme variable, which `d3.attr`
/// drops) or one a browser rejects (a hex code without `#`, which
/// `validateHexCode` lets through); both leave the attribute unwritten.
pub fn paint(color: Option<&str>) -> Option<Paint> {
    let color = color?;
    if let Some(c) = Color::from_hex(color) {
        return Some(Paint::Color(c));
    }
    let mut ch = khroma::parse(color).ok()?;
    let channels = [
        ch.get(Channel::R),
        ch.get(Channel::G),
        ch.get(Channel::B),
        ch.get(Channel::A),
    ];
    // The theme's `quadrantPointFill` is `hsl(h, s%, NaN%)` (see
    // `theme::base::quadrant`); a browser rejects it and the point takes
    // the root `fill`.
    if channels.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let byte = |v: f64| v.round().clamp(0.0, 255.0) as u8;
    Some(Paint::Color(Color::rgba(
        byte(channels[0]),
        byte(channels[1]),
        byte(channels[2]),
        channels[3],
    )))
}

/// The root `fill` of every mermaid SVG: `#id { fill: textColor }` from
/// the global stylesheet, which an element with a rejected `fill` inherits.
pub fn root_style(text_color: Option<&str>) -> Style {
    Style {
        fill: paint(text_color),
        ..Style::default()
    }
}

/// A `stroke-width` as `parseStyles` validated it (`^\d+px$`) or the
/// builder's `0px` default, in user units.
pub fn px_length(value: &str) -> f64 {
    value
        .strip_suffix("px")
        .unwrap_or(value)
        .trim()
        .parse::<f64>()
        .unwrap_or(0.0)
}

/// `fill` and `font-size` of a quadrant text element.
pub fn text_style(fill: Option<&str>) -> Style {
    Style {
        fill: paint(fill),
        ..Style::default()
    }
}

pub fn text_font(font_size: f64) -> FontSpec {
    FontSpec {
        size: Some(font_size),
        ..FontSpec::default()
    }
}

/// `style="stroke: ...; stroke-width: ..."` of a border line.
pub fn line_style(stroke: Option<&str>, width: f64) -> Style {
    Style {
        stroke: paint(stroke),
        stroke_width: Some(width),
        ..Style::default()
    }
}

/// `fill`, `stroke`, `stroke-width` of a data point circle.
pub fn point_style(fill: Option<&str>, stroke: Option<&str>, stroke_width: &str) -> Style {
    Style {
        fill: paint(fill),
        stroke: paint(stroke),
        stroke_width: Some(px_length(stroke_width)),
        ..Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paints_hex_rgb_and_keywords_and_drops_the_rest() {
        assert_eq!(
            paint(Some("#ECECFF")),
            Some(Paint::Color(Color::rgb(0xEC, 0xEC, 0xFF)))
        );
        assert_eq!(
            paint(Some("rgb(255, 0, 0)")),
            Some(Paint::Color(Color::rgb(255, 0, 0)))
        );
        assert_eq!(paint(Some("white")), Some(Paint::Color(Color::WHITE)));
        assert_eq!(paint(Some("ff0000")), None);
        assert_eq!(paint(Some("hsl(240, 100%, NaN%)")), None);
        assert_eq!(paint(None), None);
    }

    #[test]
    fn px_lengths() {
        assert_eq!(px_length("0px"), 0.0);
        assert_eq!(px_length("10px"), 10.0);
        assert_eq!(px_length("nope"), 0.0);
    }
}
