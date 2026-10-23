//! `diagrams/sankey/styles.js` (mermaid 12.2.0) resolved to attributes,
//! with the root rule of `styles.ts` (`font-family`, `fill: textColor`).
//!
//! The rules and where they land:
//!
//! - `.node-labels { font-family }`: the label group's family. The root
//!   rule sets the same family, so an invalid value (a stray `;`) drops
//!   both and the page default applies; the renderer handles that.
//! - `.sankey-label-bg { stroke: mainBkg || background || '#fff';
//!   stroke-width: 4px; stroke-linejoin: round; paint-order: stroke }`:
//!   the halo of an `outlined` label. Its fill is the inherited root fill.
//!   The foreground copy is drawn over it with the same fill, so drawing
//!   the stroke last instead of first gives the same pixels and the scene
//!   needs no `paint-order`.
//! - `.sankey-label-fg { fill: textColor }`.
//! - `.link { fill: none; stroke-opacity: 0.5; mix-blend-mode: multiply }`:
//!   the renderer writes the same values as attributes, so the rule adds
//!   nothing.
//! - `.node rect { shape-rendering: crispEdges }`: the scene has no
//!   `shape-rendering`, so node edges are antialiased; a node edge is at
//!   most one pixel column or row off.

use crate::mermaid::scene::{Color, Paint};
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::vars::js_truthy;

use super::sequence::css_paint;

/// The `stroke-width: 4px` of `.sankey-label-bg`.
pub const LABEL_HALO_WIDTH: f64 = 4.0;

#[derive(Debug, Clone, PartialEq)]
pub struct SankeyStyles {
    /// `options.fontFamily`, for the root and `.node-labels`.
    pub font_family: String,
    /// The root `fill: textColor`, which the labels inherit.
    pub text_color: Paint,
    /// `.sankey-label-bg` stroke.
    pub label_halo: Paint,
    /// `.sankey-label-fg` fill. It is the root fill too: an invalid
    /// `textColor` drops the rule and the label inherits that fill.
    pub label_fill: Paint,
}

impl SankeyStyles {
    pub fn resolve(vars: &ThemeVariables) -> SankeyStyles {
        let text = vars.str("textColor").unwrap_or("");
        // `options.mainBkg || options.background || '#fff'`.
        let halo = ["mainBkg", "background"]
            .iter()
            .map(|name| vars.get(name))
            .find(|v| js_truthy(v))
            .and_then(|v| v.as_str())
            .unwrap_or("#fff");
        // An invalid root fill leaves the SVG initial fill, black.
        let text_color = match css_paint(text) {
            Paint::None => Paint::Color(Color::BLACK),
            other => other,
        };
        SankeyStyles {
            font_family: vars
                .str("fontFamily")
                .unwrap_or(crate::mermaid::scene::DEFAULT_FONT_FAMILY)
                .to_string(),
            label_fill: text_color.clone(),
            text_color,
            label_halo: css_paint(halo),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_matches_the_oracle_stylesheet() {
        let vars = crate::mermaid::theme::resolve("default", None).expect("theme");
        let s = SankeyStyles::resolve(&vars);
        assert_eq!(s.text_color, Paint::Color(Color::rgb(0x33, 0x33, 0x33)));
        assert_eq!(s.label_halo, Paint::Color(Color::rgb(0xEC, 0xEC, 0xFF)));
        assert_eq!(
            s.font_family,
            "\"trebuchet ms\", verdana, arial, sans-serif"
        );
    }
}
