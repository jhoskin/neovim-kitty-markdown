//! `diagrams/ishikawa/ishikawaStyles.ts` (mermaid 12.2.0) resolved to
//! attributes.
//!
//! The sheet strokes the spine, the branches, and the sub-branches with
//! `lineColor` (2 px, the sub-branches 1 px), fills the arrow marker with
//! `lineColor`, fills the head and the cause label boxes with `mainBkg`
//! under a 2 px `lineColor` stroke, and sets every `text` to the theme's
//! `fontFamily`, `fontSize`, and `textColor`. The head label overrides
//! that with `font-weight: 600` and `font-size: 14px`. The class rules
//! also set `text-anchor` and `dominant-baseline`; a CSS rule beats the
//! renderer's `text-anchor` presentation attribute, so the head label is
//! centred although the renderer writes `start`.

use crate::mermaid::scene::{DEFAULT_FONT_FAMILY, Paint, Style};
use crate::mermaid::styles::pie::css_length;
use crate::mermaid::styles::sequence::css_paint;
use crate::mermaid::theme::ThemeVariables;

/// `.ishikawa .ishikawa-head-label { font-size: 14px }`.
pub const HEAD_LABEL_FONT_SIZE: f64 = 14.0;
/// `.ishikawa .ishikawa-head-label { font-weight: 600 }`.
pub const HEAD_LABEL_FONT_WEIGHT: u16 = 600;
/// The browser default `font-size`, what an invalid `fontSize` leaves.
const DEFAULT_FONT_SIZE: f64 = 16.0;

#[derive(Debug, Clone, PartialEq)]
pub struct IshikawaStyles {
    pub font_family: String,
    /// `.ishikawa text { font-size: ${fontSize} }` in pixels.
    pub font_size: f64,
    pub line_color: Paint,
    pub main_bkg: Paint,
    pub text_color: Paint,
}

impl IshikawaStyles {
    pub fn resolve(vars: &ThemeVariables) -> IshikawaStyles {
        let paint = |name: &str| css_paint(vars.str(name).unwrap_or("black"));
        // A unitless number is not a CSS length, so the declaration drops
        // and the text keeps the page default.
        let font_size = vars
            .str("fontSize")
            .filter(|s| s.trim().ends_with("px"))
            .and_then(css_length)
            .unwrap_or(DEFAULT_FONT_SIZE);
        IshikawaStyles {
            font_family: vars
                .str("fontFamily")
                .unwrap_or(DEFAULT_FONT_FAMILY)
                .to_string(),
            font_size,
            line_color: paint("lineColor"),
            main_bkg: paint("mainBkg"),
            text_color: paint("textColor"),
        }
    }

    /// `.ishikawa-spine` and `.ishikawa-branch`: 2 px, no fill.
    pub fn branch(&self) -> Style {
        Style {
            fill: Some(Paint::None),
            stroke: Some(self.line_color.clone()),
            stroke_width: Some(2.0),
            ..Style::default()
        }
    }

    /// `.ishikawa-sub-branch`: 1 px, no fill.
    pub fn sub_branch(&self) -> Style {
        Style {
            stroke_width: Some(1.0),
            ..self.branch()
        }
    }

    /// `.ishikawa-arrow`: the marker triangle.
    pub fn arrow(&self) -> Style {
        Style {
            fill: Some(self.line_color.clone()),
            ..Style::default()
        }
    }

    /// `.ishikawa-head` and `.ishikawa-label-box`.
    pub fn boxed(&self) -> Style {
        Style {
            fill: Some(self.main_bkg.clone()),
            stroke: Some(self.line_color.clone()),
            stroke_width: Some(2.0),
            ..Style::default()
        }
    }

    /// `.ishikawa text`: the fill; the font goes on the text element.
    pub fn text(&self) -> Style {
        Style {
            fill: Some(self.text_color.clone()),
            ..Style::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::config::{self, BaseConfig};
    use crate::mermaid::scene::Color;

    fn styles(theme: &str) -> IshikawaStyles {
        let resolved =
            config::resolve("ishikawa\n  A\n", &BaseConfig::new(theme)).expect("resolves");
        IshikawaStyles::resolve(&resolved.theme_variables)
    }

    #[test]
    fn default_theme_matches_the_oracle_stylesheet() {
        let s = styles("default");
        assert_eq!(
            s.line_color,
            Paint::Color(Color::from_hex("#333333").unwrap())
        );
        assert_eq!(
            s.main_bkg,
            Paint::Color(Color::from_hex("#ECECFF").unwrap())
        );
        assert_eq!(s.text_color, Paint::Color(Color::from_hex("#333").unwrap()));
        assert_eq!(s.font_size, 16.0);
        assert_eq!(s.font_family, DEFAULT_FONT_FAMILY);
    }
}
