//! The event modeling colors, resolved to attribute values.
//!
//! `diagrams/eventmodeling/styles.js` returns an empty stylesheet: the
//! renderer and `calculateEntityVisualProps` write every color as an
//! attribute, read from the `em*` theme variables with a literal fallback
//! (`themeVariables.emUiFill ?? 'white'`). Only the global stylesheet
//! applies: the root `font-family`, `font-size`, and `fill: textColor`
//! that the swimlane labels inherit. The box text is HTML in a
//! `foreignObject`, and HTML text paints with `color`, which no rule sets,
//! so it stays the page's black.
//!
//! A color attribute the browser cannot parse is dropped: a dropped `fill`
//! inherits the root `fill`, a dropped `stroke` leaves none. `None` marks
//! such a value here.

use crate::mermaid::scene::Color;
use crate::mermaid::styles::pie::{css_color, css_length};
use crate::mermaid::theme::ThemeVariables;

use crate::mermaid::diagrams::eventmodeling::db::EntityKind;

/// The global stylesheet's `font-size` when the theme has none.
const DEFAULT_FONT_SIZE: f64 = 16.0;

/// The fill and stroke of one box kind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxColors {
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventModelingStyles {
    /// The root `font-family`; the box HTML and the lane labels inherit it.
    pub font_family: String,
    /// The root `font-size`.
    pub font_size: f64,
    /// The root `fill: textColor`.
    pub text_color: Option<Color>,
    /// HTML text color: the page default.
    pub html_text_color: Color,
    pub ui: BoxColors,
    pub processor: BoxColors,
    pub read_model: BoxColors,
    pub command: BoxColors,
    pub event: BoxColors,
    /// `emSwimlaneBackgroundOdd ?? 'rgb(250,250,250)'`.
    pub swimlane_fill: Option<Color>,
    /// `emSwimlaneBackgroundStroke ?? 'rgb(240,240,240)'`.
    pub swimlane_stroke: Option<Color>,
    /// `emArrowhead ?? '#000000'`.
    pub arrowhead: Option<Color>,
    /// `emRelationStroke ?? relation.visual.stroke` (`#000`).
    pub relation_stroke: Option<Color>,
}

impl EventModelingStyles {
    /// `fallback_font` is the config `fontFamily` for a theme without a
    /// `fontFamily` variable.
    pub fn resolve(tv: &ThemeVariables, fallback_font: &str) -> EventModelingStyles {
        // `themeVariables.x ?? fallback`: only a missing value falls back.
        let var = |name: &str, fallback: &str| -> Option<Color> {
            css_color(tv.str(name).unwrap_or(fallback))
        };
        let colors = |fill: (&str, &str), stroke: (&str, &str)| BoxColors {
            fill: var(fill.0, fill.1),
            stroke: var(stroke.0, stroke.1),
        };
        EventModelingStyles {
            font_family: tv
                .str("fontFamily")
                .filter(|f| !f.is_empty())
                .unwrap_or(fallback_font)
                .to_string(),
            font_size: tv
                .str("fontSize")
                .and_then(css_length)
                .unwrap_or(DEFAULT_FONT_SIZE),
            text_color: tv.str("textColor").and_then(css_color),
            html_text_color: Color::BLACK,
            ui: colors(("emUiFill", "white"), ("emUiStroke", "#dbdada")),
            processor: colors(
                ("emProcessorFill", "#edb3f6"),
                ("emProcessorStroke", "#b88cbf"),
            ),
            read_model: colors(
                ("emReadModelFill", "#d3f1a2"),
                ("emReadModelStroke", "#a3b732"),
            ),
            command: colors(("emCommandFill", "#bcd6fe"), ("emCommandStroke", "#679ac3")),
            event: colors(("emEventFill", "#ffb778"), ("emEventStroke", "#c19a0f")),
            swimlane_fill: var("emSwimlaneBackgroundOdd", "rgb(250,250,250)"),
            swimlane_stroke: var("emSwimlaneBackgroundStroke", "rgb(240,240,240)"),
            arrowhead: var("emArrowhead", "#000000"),
            relation_stroke: var("emRelationStroke", "#000"),
        }
    }

    pub fn box_colors(&self, kind: EntityKind) -> BoxColors {
        match kind {
            EntityKind::Ui => self.ui,
            EntityKind::Processor => self.processor,
            EntityKind::ReadModel => self.read_model,
            EntityKind::Command => self.command,
            EntityKind::Event => self.event,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::theme;

    #[test]
    fn default_theme_colors() {
        let tv = theme::resolve("default", None).expect("default theme");
        let s = EventModelingStyles::resolve(&tv, "x");
        assert_eq!(s.ui.fill, Some(Color::WHITE));
        assert_eq!(s.command.fill, Color::from_hex("#bcd6fe"));
        assert_eq!(s.swimlane_fill, Some(Color::rgb(250, 250, 250)));
        assert_eq!(s.arrowhead, Color::from_hex("#333333"));
    }
}
