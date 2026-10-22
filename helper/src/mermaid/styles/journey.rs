//! `diagrams/user-journey/styles.js` (mermaid 12.2.0) resolved to
//! attributes.
//!
//! mermaid wraps `getStyles` in `#<svgId>{ ... }`, so every class rule
//! outranks the presentation attributes `svgDraw.js` sets. Three rules
//! decide what the user sees: `.task-type-N, .section-type-N { fill:
//! fillTypeN }` replaces the `sectionFills` colors on every box when the
//! theme sets `fillType0` (every built-in theme does), `line { stroke:
//! textColor }` recolors the dashed task lines and the activity line, and
//! `.mouth { stroke: #666 }` beats that `line` rule on the flat mouth. The
//! root rule of `styles.ts` (`fill: textColor; font-family; font-size`)
//! paints the smile paths and the arrowhead, which set no fill.

use crate::mermaid::scene::{FontSpec, FontWeight, Paint, Style};
use crate::mermaid::theme::ThemeVariables;

use super::sequence::css_paint;

/// `.face { fill: #FFF8DC }` when the theme has no `faceColor`.
const FACE_FILL: &str = "#FFF8DC";
/// `.mouth { stroke: #666 }`, also the eyes' inline color.
const FEATURE_COLOR: &str = "#666";
/// `.face { stroke: #999 }`.
const FACE_STROKE: &str = "#999";
/// `getNoteRect().stroke`, which no class rule overrides.
const NOTE_STROKE: &str = "#666";
/// `drawCircle` gets `stroke: '#000'` from both callers.
const ACTOR_STROKE: &str = "#000";

/// The theme variables `getStyles` reads.
#[derive(Debug, Clone, PartialEq)]
pub struct JourneyStyles {
    /// The root `fill` and the `.label` color, from `textColor`.
    pub text_color: Paint,
    /// `fillType0` to `fillType7`; `None` when the theme leaves one unset.
    pub fill_types: [Option<Paint>; 8],
    /// `actor0` to `actor5`; no built-in theme sets them.
    pub actors: [Option<Paint>; 6],
    pub face_fill: Paint,
    /// The root `font-family`, from the theme's `fontFamily`.
    pub font_family: String,
    /// The root `font-size` in pixels, from the theme's `fontSize`.
    pub font_size: f64,
}

fn optional_paint(vars: &ThemeVariables, name: &str) -> Option<Paint> {
    vars.str(name)
        .filter(|v| !v.trim().is_empty())
        .map(css_paint)
}

/// CSS `font-size` such as `16px` to pixels; `16` without a unit too.
fn css_px(value: &str, fallback: f64) -> f64 {
    value
        .trim()
        .trim_end_matches("px")
        .trim()
        .parse::<f64>()
        .unwrap_or(fallback)
}

fn solid(color: &str) -> Paint {
    css_paint(color)
}

impl JourneyStyles {
    /// `getStyles(themeVariables)` with the root rule of `styles.ts`.
    pub fn resolve(vars: &ThemeVariables) -> JourneyStyles {
        let fill_types = std::array::from_fn(|i| optional_paint(vars, &format!("fillType{i}")));
        let actors = std::array::from_fn(|i| optional_paint(vars, &format!("actor{i}")));
        JourneyStyles {
            text_color: css_paint(vars.str("textColor").unwrap_or("#333")),
            fill_types,
            actors,
            face_fill: optional_paint(vars, "faceColor").unwrap_or_else(|| solid(FACE_FILL)),
            font_family: vars
                .str("fontFamily")
                .unwrap_or("\"trebuchet ms\", verdana, arial, sans-serif")
                .to_string(),
            font_size: css_px(vars.str("fontSize").unwrap_or("16px"), 16.0),
        }
    }

    /// The root `& { fill: textColor; font-size }`.
    pub fn root(&self) -> Style {
        Style {
            fill: Some(self.text_color.clone()),
            font: FontSpec {
                size: Some(self.font_size),
                ..FontSpec::default()
            },
            ..Style::default()
        }
    }

    /// `.task-type-N, .section-type-N { fill: fillTypeN }`: written only
    /// when `fillType0` is set, and then a missing `fillTypeN` is an
    /// invalid declaration that leaves the inline fill in place.
    fn box_fill(&self, num: usize, inline: &str) -> Paint {
        let themed = self
            .fill_types
            .first()
            .and_then(Option::as_ref)
            .and_then(|_| self.fill_types.get(num))
            .and_then(Option::clone);
        themed.unwrap_or_else(|| css_paint(inline))
    }

    /// The section and task rects: `getNoteRect` with the class fill.
    pub fn note_rect(&self, num: usize, inline_fill: &str) -> Style {
        Style {
            fill: Some(self.box_fill(num, inline_fill)),
            stroke: Some(solid(NOTE_STROKE)),
            ..Style::default()
        }
    }

    /// `drawCircle`: the inline `actorColours` entry unless the theme sets
    /// `.actor-N { fill: actorN }`.
    pub fn actor_circle(&self, pos: usize, inline_fill: &str) -> Style {
        let fill = self
            .actors
            .get(pos)
            .and_then(Option::clone)
            .unwrap_or_else(|| css_paint(inline_fill));
        Style {
            fill: Some(fill),
            stroke: Some(solid(ACTOR_STROKE)),
            ..Style::default()
        }
    }

    /// `.face { fill; stroke: #999 }` with the inline `stroke-width: 2`.
    pub fn face(&self) -> Style {
        Style {
            fill: Some(self.face_fill.clone()),
            stroke: Some(solid(FACE_STROKE)),
            stroke_width: Some(2.0),
            ..Style::default()
        }
    }

    /// The eyes: inline `fill` and `stroke` of `#666`, `stroke-width: 2`.
    pub fn eye(&self) -> Style {
        Style {
            fill: Some(solid(FEATURE_COLOR)),
            stroke: Some(solid(FEATURE_COLOR)),
            stroke_width: Some(2.0),
            ..Style::default()
        }
    }

    /// The smile and sad arcs: `.mouth { stroke: #666 }` over the root fill.
    pub fn mouth_path(&self) -> Style {
        Style {
            fill: Some(self.text_color.clone()),
            stroke: Some(solid(FEATURE_COLOR)),
            ..Style::default()
        }
    }

    /// The flat mouth: `.mouth` outranks `line { stroke: textColor }`.
    pub fn mouth_line(&self) -> Style {
        Style {
            stroke: Some(solid(FEATURE_COLOR)),
            stroke_width: Some(1.0),
            ..Style::default()
        }
    }

    /// `.task-line`: `line { stroke: textColor }` beats the inline `#666`.
    pub fn task_line(&self) -> Style {
        Style {
            stroke: Some(self.text_color.clone()),
            stroke_width: Some(1.0),
            stroke_dasharray: Some(vec![4.0, 2.0]),
            ..Style::default()
        }
    }

    /// The activity line: `line { stroke: textColor }` beats the inline
    /// `black`; the arrowhead marker is set by the caller.
    pub fn activity_line(&self) -> Style {
        Style {
            stroke: Some(self.text_color.clone()),
            stroke_width: Some(4.0),
            ..Style::default()
        }
    }

    /// The arrowhead path inherits the root fill through `defs`.
    pub fn arrowhead(&self) -> Style {
        Style {
            fill: Some(self.text_color.clone()),
            ..Style::default()
        }
    }

    /// `.legend { fill: textColor; font-family }` on the actor names.
    pub fn legend_text(&self) -> Style {
        Style {
            fill: Some(self.text_color.clone()),
            font: FontSpec {
                family: Some(self.font_family.clone()),
                ..FontSpec::default()
            },
            ..Style::default()
        }
    }

    /// `.label { font-family; color: textColor }` on the HTML label divs,
    /// drawn as text.
    pub fn label_text(&self) -> Style {
        self.legend_text()
    }

    /// The title: `fill` from `titleColor` when it parses, else the root
    /// fill; bold at the resolved `titleFontSize`.
    pub fn title_text(&self, title_color: &str, size_px: f64, family: Option<String>) -> Style {
        let fill = match css_paint(title_color) {
            Paint::None => self.text_color.clone(),
            paint => paint,
        };
        Style {
            fill: Some(fill),
            font: FontSpec {
                family,
                size: Some(size_px),
                weight: Some(FontWeight::Bold),
                style: None,
            },
            ..Style::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::scene::Color;
    use crate::mermaid::theme;

    fn default_styles() -> JourneyStyles {
        let vars = theme::resolve("default", None).expect("default theme");
        JourneyStyles::resolve(&vars)
    }

    #[test]
    fn the_theme_fill_replaces_the_section_fill() {
        let styles = default_styles();
        // `fillType0` is `primaryColor`, `#ECECFF` in the default theme.
        assert_eq!(
            styles.note_rect(0, "#191970").fill,
            Some(Paint::Color(Color::rgb(0xEC, 0xEC, 0xFF)))
        );
        // Index 8 has no `fillTypeN`, so the inline color stays.
        assert_eq!(
            styles.note_rect(8, "#191970").fill,
            Some(Paint::Color(Color::rgb(0x19, 0x19, 0x70)))
        );
    }

    #[test]
    fn no_builtin_theme_recolors_the_actors() {
        let styles = default_styles();
        assert_eq!(
            styles.actor_circle(0, "#8FBC8F").fill,
            Some(Paint::Color(Color::rgb(0x8F, 0xBC, 0x8F)))
        );
    }

    #[test]
    fn an_empty_title_color_inherits_the_text_color() {
        let styles = default_styles();
        let style = styles.title_text("", 32.0, None);
        assert_eq!(style.fill, styles.root().fill);
        assert_eq!(style.font.weight, Some(FontWeight::Bold));
    }
}
