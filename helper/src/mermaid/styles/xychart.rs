//! The xychart theme: `getChartDefaultThemeConfig` in `xychartDb.ts`.
//!
//! mermaid merges the default theme's `xyChart` object with the effective
//! `themeVariables.xyChart` (`cleanAndMerge`), so a diagram that overrides
//! one key through `themeVariables` keeps the default theme's value for the
//! others, not the selected theme's. The xychart has no `getStyles`; its
//! colors are attributes on every element, which is what the scene wants.

use serde_json::Value;

use crate::mermaid::scene::Color;
use crate::mermaid::theme::khroma::{self, Channel};
use crate::mermaid::theme::{self, ThemeVariables};

/// `XYChartThemeConfig` with the palette already split.
#[derive(Debug, Clone, PartialEq)]
pub struct XyChartTheme {
    pub background_color: Color,
    pub title_color: Color,
    pub data_label_color: Color,
    pub legend_text_color: Color,
    pub x_axis_label_color: Color,
    pub x_axis_title_color: Color,
    pub x_axis_tick_color: Color,
    pub x_axis_line_color: Color,
    pub y_axis_label_color: Color,
    pub y_axis_title_color: Color,
    pub y_axis_tick_color: Color,
    pub y_axis_line_color: Color,
    /// `plotColorPalette` split on `,` and trimmed.
    pub plot_color_palette: Vec<Color>,
}

/// The colors of one axis as `getAxis` receives them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisTheme {
    pub title_color: Color,
    pub label_color: Color,
    pub tick_color: Color,
    pub axis_line_color: Color,
}

impl XyChartTheme {
    pub fn x_axis(&self) -> AxisTheme {
        AxisTheme {
            title_color: self.x_axis_title_color,
            label_color: self.x_axis_label_color,
            tick_color: self.x_axis_tick_color,
            axis_line_color: self.x_axis_line_color,
        }
    }

    pub fn y_axis(&self) -> AxisTheme {
        AxisTheme {
            title_color: self.y_axis_title_color,
            label_color: self.y_axis_label_color,
            tick_color: self.y_axis_tick_color,
            axis_line_color: self.y_axis_line_color,
        }
    }
}

/// A CSS color string as an sRGB color. Anything khroma cannot parse is
/// black, which is also what a browser paints for an invalid `fill`.
pub fn css_color(text: &str) -> Color {
    match khroma::parse(text.trim()) {
        Ok(mut ch) => Color::rgba(
            ch.get(Channel::R).round().clamp(0.0, 255.0) as u8,
            ch.get(Channel::G).round().clamp(0.0, 255.0) as u8,
            ch.get(Channel::B).round().clamp(0.0, 255.0) as u8,
            ch.get(Channel::A).clamp(0.0, 1.0),
        ),
        Err(_) => Color::BLACK,
    }
}

/// `cleanAndMerge(defaultTheme.xyChart, themeVariables.xyChart)`: the key
/// from the diagram's variables when it is a string, else the default
/// theme's.
fn merged_key(defaults: &Value, current: &Value, key: &str) -> String {
    let pick = |v: &Value| v.get(key).and_then(Value::as_str).map(str::to_string);
    pick(current).or_else(|| pick(defaults)).unwrap_or_default()
}

pub fn theme_config(tv: &ThemeVariables) -> XyChartTheme {
    let defaults = theme::resolve("default", None)
        .map(|vars| vars.get("xyChart").clone())
        .unwrap_or(Value::Null);
    let current = tv.get("xyChart").clone();
    let color = |key: &str| css_color(&merged_key(&defaults, &current, key));
    let palette = merged_key(&defaults, &current, "plotColorPalette");
    XyChartTheme {
        background_color: color("backgroundColor"),
        title_color: color("titleColor"),
        data_label_color: color("dataLabelColor"),
        legend_text_color: color("legendTextColor"),
        x_axis_label_color: color("xAxisLabelColor"),
        x_axis_title_color: color("xAxisTitleColor"),
        x_axis_tick_color: color("xAxisTickColor"),
        x_axis_line_color: color("xAxisLineColor"),
        y_axis_label_color: color("yAxisLabelColor"),
        y_axis_title_color: color("yAxisTitleColor"),
        y_axis_tick_color: color("yAxisTickColor"),
        y_axis_line_color: color("yAxisLineColor"),
        plot_color_palette: palette.split(',').map(css_color).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_palette_and_colors() {
        let tv = theme::resolve("default", None).unwrap();
        let t = theme_config(&tv);
        assert_eq!(t.plot_color_palette.len(), 10);
        assert_eq!(t.plot_color_palette[0], Color::rgb(0xEC, 0xEC, 0xFF));
        assert_eq!(t.background_color, Color::WHITE);
        assert_eq!(t.title_color, Color::rgb(0x13, 0x13, 0x00));
    }

    #[test]
    fn a_partial_override_keeps_the_default_theme_for_the_rest() {
        let json = serde_json::json!({
            "xyChart": { "plotColorPalette": "#000000, #0000FF" }
        });
        let tv = ThemeVariables::from_json(json.as_object().unwrap());
        let t = theme_config(&tv);
        assert_eq!(
            t.plot_color_palette,
            vec![Color::BLACK, Color::rgb(0, 0, 255)]
        );
        assert_eq!(t.background_color, Color::WHITE);
    }

    #[test]
    fn css_colors() {
        assert_eq!(css_color("red"), Color::rgb(255, 0, 0));
        assert_eq!(css_color(" #0F0 "), Color::rgb(0, 255, 0));
        assert_eq!(
            css_color("rgba(0, 0, 255, 0.5)"),
            Color::rgba(0, 0, 255, 0.5)
        );
        assert_eq!(css_color("nope"), Color::BLACK);
    }
}
