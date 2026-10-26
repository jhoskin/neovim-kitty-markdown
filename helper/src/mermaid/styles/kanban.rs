//! `diagrams/kanban/styles.ts` (mermaid 12.2.0) resolved to attributes.
//!
//! The sheet has two parts the kanban renderer paints with: the
//! `genSections` loop, which colors `.section-N rect` from the `cScale`
//! palette, and the `.node rect { fill: background; stroke: nodeBorder }`
//! rule that replaces the flowchart `mainBkg` fill for the cards. HTML
//! labels take `.cluster-label, .label { color: textColor }`; SVG text
//! labels (`htmlLabels: false`) take its `fill`, except that a section
//! title also matches `.section-N text { fill: cScaleLabel<N + 1> }`.

use serde_json::Value;

use crate::mermaid::scene::{Color, DEFAULT_FONT_FAMILY, Paint, Style};
use crate::mermaid::shapes::NodeStyleVars;
use crate::mermaid::styles::gantt::css_color;
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::vars::{darken, js_truthy, lighten};

/// `THEME_COLOR_LIMIT` of every built-in theme.
const THEME_COLOR_LIMIT: usize = 12;

#[derive(Debug, Clone, PartialEq)]
pub struct KanbanStyles {
    pub font_family: String,
    /// The root `font-size` in pixels (`themeVariables.fontSize`).
    pub font_size: f64,
    pub text_color: Color,
    pub background: Color,
    pub node_border: Color,
    /// `adjuster(cScale<i>, 10)` for `i` in `0..THEME_COLOR_LIMIT`: the
    /// fill of `.section-${i - 1} rect`.
    section_fills: Vec<Color>,
    /// `cScaleLabel<i>` for `i` in `0..THEME_COLOR_LIMIT`: the fill of
    /// `.section-${i - 1} text`.
    section_text_fills: Vec<Color>,
}

impl KanbanStyles {
    pub fn from_theme(tv: &ThemeVariables) -> KanbanStyles {
        let color = |name: &str| css_color(tv.str(name).unwrap_or("black"));
        // `const adjuster = (color, level) => options.darkMode ?
        // darken(color, level) : lighten(color, level)`.
        let dark_mode = js_truthy(tv.get("darkMode"));
        let section_fills = (0..THEME_COLOR_LIMIT)
            .map(|i| {
                let scale = tv.get(&format!("cScale{i}"));
                let adjusted = if dark_mode {
                    darken(scale, 10.0)
                } else {
                    lighten(scale, 10.0)
                };
                match adjusted {
                    Ok(Value::String(c)) => css_color(&c),
                    _ => Color::BLACK,
                }
            })
            .collect();
        let section_text_fills = (0..THEME_COLOR_LIMIT)
            .map(|i| color(&format!("cScaleLabel{i}")))
            .collect();
        let font_size = tv
            .str("fontSize")
            .and_then(|s| s.trim().trim_end_matches("px").parse::<f64>().ok())
            .unwrap_or(16.0);
        KanbanStyles {
            font_family: tv
                .str("fontFamily")
                .unwrap_or(DEFAULT_FONT_FAMILY)
                .to_string(),
            font_size,
            text_color: color("textColor"),
            background: color("background"),
            node_border: color("nodeBorder"),
            section_fills,
            section_text_fills,
        }
    }

    /// The fill of an SVG text section title: `.section-N text { fill:
    /// cScaleLabel<N + 1> }` is more specific than the `fill: textColor`
    /// the text would inherit from `.cluster-label`. Past `section-10` only
    /// the inherited fill is left. HTML titles never match `text`.
    pub fn section_text_fill(&self, n: usize) -> Color {
        self.section_text_fills
            .get(n + 1)
            .copied()
            .unwrap_or(self.text_color)
    }

    /// `.section-N rect { fill: adjuster(cScale<N + 1>, 10); stroke: the
    /// same }` for the renderer's 1-based section count `n`, with the SVG
    /// default stroke width. The sheet stops at `section-10`; a later
    /// section inherits the root `fill: textColor` and has no stroke.
    pub fn section_rect(&self, n: usize) -> Style {
        match self.section_fills.get(n + 1) {
            Some(fill) => Style {
                fill: Some(Paint::Color(*fill)),
                stroke: Some(Paint::Color(*fill)),
                stroke_width: Some(1.0),
                ..Style::default()
            },
            None => Style {
                fill: Some(Paint::Color(self.text_color)),
                stroke: Some(Paint::None),
                ..Style::default()
            },
        }
    }

    /// The variables the `kanbanItem` shape reads, with this sheet's rules
    /// in place of the flowchart ones: `.node rect { fill: background;
    /// stroke: nodeBorder; stroke-width: 1px }` and `.label { color:
    /// textColor }`.
    pub fn node_vars(&self) -> NodeStyleVars {
        NodeStyleVars {
            main_bkg: self.background,
            node_border: self.node_border,
            text_color: self.text_color,
            node_text_color: self.text_color,
            title_color: self.text_color,
            stroke_width: 1.0,
            font_family: self.font_family.clone(),
            font_size: self.font_size,
            ..NodeStyleVars::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::theme;

    #[test]
    fn default_theme_sections_are_lightened_scale_colors() {
        let tv = theme::resolve("default", None).expect("theme");
        let styles = KanbanStyles::from_theme(&tv);
        assert_eq!(styles.font_size, 16.0);
        assert_eq!(styles.background, Color::WHITE);
        // `section-1` takes `cScale2` (`tertiaryColor`) lightened by 10.
        let s1 = styles.section_rect(1);
        assert_eq!(s1.fill, s1.stroke);
        assert_ne!(s1.fill, Some(Paint::Color(styles.text_color)));
        // Past the sheet: the root fill, no stroke.
        let s11 = styles.section_rect(11);
        assert_eq!(s11.fill, Some(Paint::Color(styles.text_color)));
        assert_eq!(s11.stroke, Some(Paint::None));
        // SVG titles: `section-2` takes `cScaleLabel3`, which is white in
        // the default theme (the oracle's `.section-2 text{fill:#ffffff}`),
        // and past the sheet the inherited text color.
        assert_eq!(styles.section_text_fill(1), Color::BLACK);
        assert_eq!(styles.section_text_fill(2), Color::WHITE);
        assert_eq!(styles.section_text_fill(11), styles.text_color);
    }

    #[test]
    fn dark_theme_darkens_the_scale() {
        let tv = theme::resolve("dark", None).expect("theme");
        let styles = KanbanStyles::from_theme(&tv);
        assert_eq!(styles.background, Color::from_hex("#333").unwrap());
        let vars = styles.node_vars();
        assert_eq!(vars.main_bkg, styles.background);
        assert_eq!(vars.node_text_color, styles.text_color);
    }
}
