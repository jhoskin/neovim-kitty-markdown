//! `diagrams/sequence/styles.js` (mermaid 12.2.0) resolved to attributes.
//!
//! mermaid wraps `getStyles` in `#<svgId>{ ... }` (stylis nesting), so every
//! rule outranks the presentation attributes `svgDraw` sets: the actor rect
//! gets `.actor`'s fill and stroke, not its `#eaeaea`/`#666` attributes.
//! Two rules reach further than they look: `.actor-man circle, line`
//! becomes `#id .actor-man circle, #id line`, so every `<line>` in the
//! diagram gets `stroke-width: 2px` unless a class rule beats it
//! (`.messageLine0` does at 1.5, the lifeline's `stroke-width="0.5px"`
//! attribute does not); and the root `& { fill: textColor }` is what
//! paints the title, the sequence-number digits' siblings and the marker
//! paths that set no fill of their own.

use crate::mermaid::scene::{Color, LineCap, Paint, Style};
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::khroma::{self, Channel};

/// `1pt` in CSS pixels, the crosshead's stroke width.
const PT: f64 = 4.0 / 3.0;

/// The theme variables `getStyles` reads, as colors.
#[derive(Debug, Clone, PartialEq)]
pub struct SequenceStyles {
    pub actor_border: Paint,
    pub actor_bkg: Paint,
    pub actor_text_color: Paint,
    pub actor_line_color: Paint,
    pub signal_color: Paint,
    pub signal_text_color: Paint,
    pub label_box_bkg_color: Paint,
    pub label_box_border_color: Paint,
    pub label_text_color: Paint,
    pub loop_text_color: Paint,
    pub note_border_color: Paint,
    pub note_bkg_color: Paint,
    pub note_text_color: Paint,
    pub activation_border_color: Paint,
    pub activation_bkg_color: Paint,
    pub sequence_number_color: Paint,
    /// The root `fill`, from the theme's `textColor`.
    pub text_color: Paint,
    /// The fill a `rect` block gets when it names no color:
    /// `rectBkgColor || actorBkg || rgba(128, 128, 128, 0.5)`.
    pub rect_fallback_fill: Paint,
    /// The root `font-family`, from the theme's `fontFamily`.
    pub font_family: String,
    /// The root `font-size` in pixels, from the theme's `fontSize`.
    pub font_size: f64,
}

/// A CSS color as khroma reads it (hex, `rgb()`, `hsl()`, named), or
/// `Paint::None` for `transparent` and anything unparseable.
pub fn css_paint(value: &str) -> Paint {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("transparent") || value == "none" {
        return Paint::None;
    }
    match khroma::parse(value) {
        Ok(mut ch) => {
            let byte = |v: f64| v.round().clamp(0.0, 255.0) as u8;
            Paint::Color(Color::rgba(
                byte(ch.get(Channel::R)),
                byte(ch.get(Channel::G)),
                byte(ch.get(Channel::B)),
                ch.get(Channel::A).clamp(0.0, 1.0),
            ))
        }
        Err(_) => Paint::None,
    }
}

fn var_paint(vars: &ThemeVariables, name: &str, fallback: &str) -> Paint {
    css_paint(vars.str(name).unwrap_or(fallback))
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

impl SequenceStyles {
    /// `getStyles(themeVariables)` with the root rule of `styles.ts`.
    pub fn resolve(vars: &ThemeVariables) -> SequenceStyles {
        let actor_bkg = var_paint(vars, "actorBkg", "#ECECFF");
        let rect_fallback_fill = match vars.str("rectBkgColor") {
            Some(c) if !c.is_empty() => css_paint(c),
            _ => match &actor_bkg {
                Paint::None => css_paint("rgba(128, 128, 128, 0.5)"),
                other => other.clone(),
            },
        };
        SequenceStyles {
            actor_border: var_paint(vars, "actorBorder", "#9370DB"),
            actor_bkg,
            actor_text_color: var_paint(vars, "actorTextColor", "black"),
            actor_line_color: var_paint(vars, "actorLineColor", "#9370DB"),
            signal_color: var_paint(vars, "signalColor", "#333"),
            signal_text_color: var_paint(vars, "signalTextColor", "#333"),
            label_box_bkg_color: var_paint(vars, "labelBoxBkgColor", "#ECECFF"),
            label_box_border_color: var_paint(vars, "labelBoxBorderColor", "#9370DB"),
            label_text_color: var_paint(vars, "labelTextColor", "black"),
            loop_text_color: var_paint(vars, "loopTextColor", "black"),
            note_border_color: var_paint(vars, "noteBorderColor", "#aaaa33"),
            note_bkg_color: var_paint(vars, "noteBkgColor", "#fff5ad"),
            note_text_color: var_paint(vars, "noteTextColor", "black"),
            activation_border_color: var_paint(vars, "activationBorderColor", "#666"),
            activation_bkg_color: var_paint(vars, "activationBkgColor", "#f4f4f4"),
            sequence_number_color: var_paint(vars, "sequenceNumberColor", "white"),
            text_color: var_paint(vars, "textColor", "#333"),
            rect_fallback_fill,
            font_family: vars
                .str("fontFamily")
                .unwrap_or("\"trebuchet ms\", verdana, arial, sans-serif")
                .to_string(),
            font_size: css_px(vars.str("fontSize").unwrap_or("16px"), 16.0),
        }
    }

    /// `.actor { stroke; fill; stroke-width: 1 }` on the participant rect.
    pub fn actor_rect(&self) -> Style {
        Style {
            fill: Some(self.actor_bkg.clone()),
            stroke: Some(self.actor_border.clone()),
            stroke_width: Some(1.0),
            ..Style::default()
        }
    }

    /// `text.actor > tspan { fill: actorTextColor; stroke: none }`.
    pub fn actor_text(&self) -> Style {
        text_style(self.actor_text_color.clone())
    }

    /// `.actor-line { stroke }` plus `line { stroke-width: 2px }`, which
    /// beats the `0.5px` presentation attribute.
    pub fn actor_line(&self) -> Style {
        Style {
            fill: Some(self.actor_bkg.clone()),
            stroke: Some(self.actor_line_color.clone()),
            stroke_width: Some(2.0),
            ..Style::default()
        }
    }

    /// The stick figure's strokes: the group's inline `stroke: actorBorder`
    /// with `.actor-man circle, line { fill: actorBkg; stroke-width: 2px }`.
    pub fn actor_man(&self) -> Style {
        Style {
            fill: Some(self.actor_bkg.clone()),
            stroke: Some(self.actor_border.clone()),
            stroke_width: Some(2.0),
            ..Style::default()
        }
    }

    /// `.messageLine0` / `.messageLine1` with the inline `fill: none` and,
    /// for the dotted line, the inline `stroke-dasharray: 3, 3` that beats
    /// the stylesheet's `2, 2`.
    pub fn message_line(&self, dotted: bool) -> Style {
        Style {
            fill: Some(Paint::None),
            stroke: Some(self.signal_color.clone()),
            stroke_width: Some(1.5),
            stroke_dasharray: dotted.then(|| vec![3.0, 3.0]),
            ..Style::default()
        }
    }

    pub fn message_text(&self) -> Style {
        text_style(self.signal_text_color.clone())
    }

    /// `.labelBox { stroke; fill }`.
    pub fn label_box(&self) -> Style {
        Style {
            fill: Some(self.label_box_bkg_color.clone()),
            stroke: Some(self.label_box_border_color.clone()),
            ..Style::default()
        }
    }

    pub fn label_text(&self) -> Style {
        text_style(self.label_text_color.clone())
    }

    /// `.loopText` and `.sectionTitle` share `loopTextColor`.
    pub fn loop_text(&self) -> Style {
        text_style(self.loop_text_color.clone())
    }

    /// `.loopLine { stroke-width: 2px; stroke-dasharray: 2, 2; stroke;
    /// fill }`; a section divider carries the inline `3, 3`.
    pub fn loop_line(&self, section: bool) -> Style {
        Style {
            fill: Some(self.label_box_border_color.clone()),
            stroke: Some(self.label_box_border_color.clone()),
            stroke_width: Some(2.0),
            stroke_dasharray: Some(if section {
                vec![3.0, 3.0]
            } else {
                vec![2.0, 2.0]
            }),
            ..Style::default()
        }
    }

    /// `.note { stroke; fill }`; the rect keeps the default stroke width.
    pub fn note_rect(&self) -> Style {
        Style {
            fill: Some(self.note_bkg_color.clone()),
            stroke: Some(self.note_border_color.clone()),
            ..Style::default()
        }
    }

    pub fn note_text(&self) -> Style {
        text_style(self.note_text_color.clone())
    }

    /// `.activation0..2 { fill; stroke }`.
    pub fn activation(&self) -> Style {
        Style {
            fill: Some(self.activation_bkg_color.clone()),
            stroke: Some(self.activation_border_color.clone()),
            ..Style::default()
        }
    }

    /// `.sequenceNumber { fill }` on the digits.
    pub fn sequence_number_text(&self) -> Style {
        text_style(self.sequence_number_color.clone())
    }

    /// `[id$="-sequencenumber"] { fill: signalColor }` on the marker; the
    /// circle inherits it.
    pub fn sequence_number_marker(&self) -> Style {
        Style {
            fill: Some(self.signal_color.clone()),
            ..Style::default()
        }
    }

    /// `[id$="-arrowhead"] path { fill; stroke }` with the default stroke
    /// width of 1.
    pub fn arrowhead(&self) -> Style {
        Style {
            fill: Some(self.signal_color.clone()),
            stroke: Some(self.signal_color.clone()),
            ..Style::default()
        }
    }

    /// `[id$="-crosshead"] path { fill; stroke }` over the path's own
    /// `stroke-width="1pt"` and inline `stroke-dasharray: 0, 0`.
    pub fn crosshead(&self) -> Style {
        Style {
            fill: Some(self.signal_color.clone()),
            stroke: Some(self.signal_color.clone()),
            stroke_width: Some(PT),
            ..Style::default()
        }
    }

    /// A marker path with no rule of its own inherits the root fill.
    pub fn root_fill(&self) -> Style {
        Style {
            fill: Some(self.text_color.clone()),
            ..Style::default()
        }
    }

    /// The stick heads: `stroke="black" stroke-width="1.5" fill="none"`.
    pub fn stick_head(&self) -> Style {
        Style {
            fill: Some(Paint::None),
            stroke: Some(Paint::Color(Color::BLACK)),
            stroke_width: Some(1.5),
            ..Style::default()
        }
    }

    /// The zero-length line that carries the sequence-number marker:
    /// `line { stroke-width: 2px }` beats its `stroke-width="0"`, which
    /// is what scales the marker.
    pub fn sequence_number_line(&self) -> Style {
        Style {
            fill: Some(self.actor_bkg.clone()),
            stroke: Some(Paint::None),
            stroke_width: Some(2.0),
            stroke_linecap: Some(LineCap::Butt),
            ..Style::default()
        }
    }

    /// `drawBackgroundRect` for a `rect` block: the parsed color, no
    /// stroke (the loop model has none), class `rect`.
    pub fn background_rect(&self, fill: &str) -> Style {
        let paint = match css_paint(fill) {
            Paint::None if fill.trim().is_empty() => self.rect_fallback_fill.clone(),
            other => other,
        };
        Style {
            fill: Some(paint),
            ..Style::default()
        }
    }

    /// A box's background: its color and `stroke: rgb(0,0,0, 0.5)`.
    pub fn box_rect(&self, fill: &str) -> Style {
        Style {
            fill: Some(css_paint(fill)),
            stroke: Some(Paint::Color(Color::rgba(0, 0, 0, 0.5))),
            ..Style::default()
        }
    }

    /// The box title inherits the root fill.
    pub fn box_text(&self) -> Style {
        text_style(self.text_color.clone())
    }
}

fn text_style(fill: Paint) -> Style {
    Style {
        fill: Some(fill),
        stroke: Some(Paint::None),
        ..Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_paint_reads_named_hex_and_functional_colors() {
        assert_eq!(css_paint("black"), Paint::Color(Color::rgb(0, 0, 0)));
        assert_eq!(
            css_paint("#ECECFF"),
            Paint::Color(Color::rgb(0xec, 0xec, 0xff))
        );
        assert_eq!(
            css_paint("rgb(0, 255, 0)"),
            Paint::Color(Color::rgb(0, 255, 0))
        );
        assert_eq!(css_paint("transparent"), Paint::None);
        match css_paint("rgba(0, 0, 255, 0.1)") {
            Paint::Color(c) => {
                assert_eq!((c.r, c.g, c.b), (0, 0, 255));
                assert!((c.a - 0.1).abs() < 1e-9);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn default_theme_values() {
        let vars = crate::mermaid::theme::resolve_value("default", &serde_json::Value::Null)
            .expect("default theme");
        let s = SequenceStyles::resolve(&vars);
        assert_eq!(s.actor_border, Paint::Color(Color::rgb(0x93, 0x70, 0xdb)));
        assert_eq!(s.note_bkg_color, Paint::Color(Color::rgb(0xff, 0xf5, 0xad)));
        assert_eq!(s.font_size, 16.0);
        assert_eq!(
            s.font_family,
            "\"trebuchet ms\", verdana, arial, sans-serif"
        );
    }
}
