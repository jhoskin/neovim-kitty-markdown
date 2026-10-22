//! `diagrams/timeline/styles.js` (mermaid 12.2.0) resolved to attributes.
//!
//! `getStyles` emits, for every color slot `i` in `0..colorSlotCount`, the
//! rules for the class `section-${i - 1}`: `path { fill: cScale<i> }`,
//! `text { fill: cScaleLabel<i> }`, `line { stroke: cScaleInv<i>;
//! stroke-width: 3 }`. Every iteration also emits `.lineWrapper line {
//! stroke: cScaleLabel<i> }` for the same selector, so the last slot wins
//! and the activity line and the dashed task lines take `cScaleLabel11`
//! over their `stroke="black"` attribute. `.eventWrapper { filter:
//! brightness(120%) }` is folded into the event colors, since the scene has
//! no filter element and a brightness filter on opaque solid paint is a
//! per-channel multiply in sRGB (Chrome applies CSS filter functions in
//! sRGB). The root rule of `styles.ts` gives the svg `font-family`,
//! `font-size`, and `fill: textColor`; the title and the arrowhead inherit
//! that fill.

use serde_json::Value;

use crate::mermaid::scene::{Color, Paint, Style};
use crate::mermaid::theme::ThemeVariables;

use super::sequence::css_paint;

/// `DEFAULT_COLOR_SLOTS` and `MAX_COLOR_SLOTS` of `colorThemeGate.ts`.
const DEFAULT_COLOR_SLOTS: usize = 12;
const MAX_COLOR_SLOTS: usize = 64;

/// `colorSlotCount(THEME_COLOR_LIMIT)`: the limit when it is a positive
/// integer no larger than 64, else 12.
pub fn color_slot_count(limit: &Value) -> usize {
    match limit.as_f64() {
        Some(v) if v.fract() == 0.0 && v > 0.0 && v <= MAX_COLOR_SLOTS as f64 => v as usize,
        _ => DEFAULT_COLOR_SLOTS,
    }
}

/// `filter: brightness(120%)` on an opaque color: each channel times 1.2,
/// clamped. `Paint::None` and references stay as they are.
pub fn brighten(paint: &Paint, factor: f64) -> Paint {
    match paint {
        Paint::Color(c) => {
            let scale = |v: u8| (f64::from(v) * factor).round().clamp(0.0, 255.0) as u8;
            Paint::Color(Color::rgba(scale(c.r), scale(c.g), scale(c.b), c.a))
        }
        other => other.clone(),
    }
}

/// The theme variables `getStyles` reads, as paints.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineStyles {
    /// `cScale0..` through `cScale<slots - 1>`.
    pub c_scale: Vec<Paint>,
    pub c_scale_label: Vec<Paint>,
    pub c_scale_inv: Vec<Paint>,
    /// `colorSlotCount(THEME_COLOR_LIMIT)`: how many `.section-N` rule sets
    /// the stylesheet holds.
    pub slots: usize,
    /// The root `fill`, from the theme's `textColor`.
    pub text_color: Paint,
    /// The root `font-family`, from the theme's `fontFamily`.
    pub font_family: String,
    /// The root `font-size` in pixels, from the theme's `fontSize`.
    pub font_size: f64,
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

impl TimelineStyles {
    /// `getStyles(themeVariables)` with the root rule of `styles.ts`.
    pub fn resolve(vars: &ThemeVariables) -> TimelineStyles {
        let slots = color_slot_count(vars.get("THEME_COLOR_LIMIT"));
        let series = |prefix: &str| -> Vec<Paint> {
            (0..slots)
                .map(|i| css_paint(vars.str(&format!("{prefix}{i}")).unwrap_or("")))
                .collect()
        };
        TimelineStyles {
            c_scale: series("cScale"),
            c_scale_label: series("cScaleLabel"),
            c_scale_inv: series("cScaleInv"),
            slots,
            text_color: css_paint(vars.str("textColor").unwrap_or("#333")),
            font_family: vars
                .str("fontFamily")
                .unwrap_or("\"trebuchet ms\", verdana, arial, sans-serif")
                .to_string(),
            font_size: css_px(vars.str("fontSize").unwrap_or("16px"), 16.0),
        }
    }

    /// The stylesheet slot of a node whose class is `section-<section>`,
    /// where `section` is `drawNode`'s `(fullSection % limit) - 1`: slot
    /// `section + 1` when a rule set exists for it.
    fn slot(&self, section: f64) -> Option<usize> {
        let i = section + 1.0;
        (i.fract() == 0.0 && i >= 0.0 && i < self.slots as f64).then_some(i as usize)
    }

    /// `.section-N path { fill: cScale<N+1> }`; a node without a rule set
    /// inherits the root fill.
    pub fn node_fill(&self, section: f64) -> Paint {
        self.slot(section)
            .map_or_else(|| self.text_color.clone(), |i| self.c_scale[i].clone())
    }

    /// `.section-N text { fill: cScaleLabel<N+1> }`.
    pub fn node_text_fill(&self, section: f64) -> Paint {
        self.slot(section).map_or_else(
            || self.text_color.clone(),
            |i| self.c_scale_label[i].clone(),
        )
    }

    /// `.section-N line { stroke: cScaleInv<N+1>; stroke-width: 3 }`. A
    /// node without a rule set keeps the line's attributes, which set no
    /// stroke, so the line is invisible.
    pub fn node_line(&self, section: f64) -> Style {
        match self.slot(section) {
            Some(i) => Style {
                stroke: Some(self.c_scale_inv[i].clone()),
                stroke_width: Some(3.0),
                ..Style::default()
            },
            None => Style::default(),
        }
    }

    /// `.lineWrapper line { stroke: cScaleLabel<slots - 1> }`, the last
    /// emitted rule for that selector.
    pub fn line_wrapper_stroke(&self) -> Paint {
        self.c_scale_label
            .last()
            .cloned()
            .unwrap_or_else(|| self.text_color.clone())
    }

    /// The root rule: `fill: textColor` and the theme `font-size`.
    pub fn root(&self) -> Style {
        Style {
            fill: Some(self.text_color.clone()),
            font: crate::mermaid::scene::FontSpec {
                size: Some(self.font_size),
                ..Default::default()
            },
            ..Style::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn slot_count_follows_color_theme_gate() {
        assert_eq!(color_slot_count(&json!(12)), 12);
        assert_eq!(color_slot_count(&json!(3)), 3);
        assert_eq!(color_slot_count(&json!(64)), 64);
        assert_eq!(color_slot_count(&json!(65)), 12);
        assert_eq!(color_slot_count(&json!(0)), 12);
        assert_eq!(color_slot_count(&json!(2.5)), 12);
        assert_eq!(color_slot_count(&json!("12")), 12);
        assert_eq!(color_slot_count(&Value::Null), 12);
    }

    #[test]
    fn brightness_multiplies_and_clamps() {
        let p = Paint::Color(Color::rgb(100, 200, 250));
        assert_eq!(brighten(&p, 1.2), Paint::Color(Color::rgb(120, 240, 255)));
        assert_eq!(brighten(&Paint::None, 1.2), Paint::None);
    }

    #[test]
    fn default_theme_resolves_twelve_slots() {
        let vars = crate::mermaid::theme::resolve("default", None).expect("theme");
        let styles = TimelineStyles::resolve(&vars);
        assert_eq!(styles.slots, 12);
        assert_eq!(styles.c_scale.len(), 12);
        assert!(matches!(styles.node_fill(-1.0), Paint::Color(_)));
        assert_eq!(styles.node_fill(-1.0), styles.c_scale[0]);
        assert_eq!(styles.node_fill(10.0), styles.c_scale[11]);
        assert_eq!(styles.node_fill(11.0), styles.text_color);
        assert_eq!(styles.line_wrapper_stroke(), styles.c_scale_label[11]);
        assert_eq!(styles.font_size, 16.0);
    }
}
