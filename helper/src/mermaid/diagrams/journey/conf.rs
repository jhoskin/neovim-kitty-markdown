//! The `conf` object of `journeyRenderer.ts`: `getConfig().journey` after
//! the defaults, the base config, and the diagram's directives merged.
//!
//! `journeyDiagram.ts` `init` calls `renderer.setConf(cnf.journey)`, which
//! copies the section key by key and nothing else, so every value here is
//! the `journey` section as the resolved config holds it. The typed config
//! keeps that section as JSON; the accessors below read it with the
//! schema defaults (`config.schema.yaml`, `JourneyDiagramConfig`) as the
//! fallback for a key a directive removed or mistyped.

use serde_json::Value;

use crate::mermaid::config::Resolved;

/// `JourneyDiagramConfig.textPlacement`: how a box label is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextPlacement {
    /// `fo`: an HTML `div` in a `foreignObject`, centered as a table cell.
    ForeignObject,
    /// `old`: one `<text>` 5 px below the box center.
    Old,
    /// Anything else: one `<text>` per `<br>` line with `taskFont*`.
    Tspan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Conf {
    pub use_max_width: bool,
    pub diagram_margin_x: f64,
    pub diagram_margin_y: f64,
    pub left_margin: f64,
    pub max_label_width: f64,
    pub width: f64,
    pub height: f64,
    pub box_text_margin: f64,
    pub task_font_size: f64,
    pub task_font_family: String,
    pub task_margin: f64,
    pub text_placement: TextPlacement,
    pub actor_colours: Vec<String>,
    pub section_fills: Vec<String>,
    pub section_colours: Vec<String>,
    pub title_color: String,
    pub title_font_family: String,
    /// The raw `titleFontSize` value, a CSS length such as `4ex`.
    pub title_font_size: String,
}

fn num(section: &Value, key: &str, default: f64) -> f64 {
    match section.get(key) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(default),
        Some(Value::String(s)) => s.trim().parse::<f64>().unwrap_or(default),
        _ => default,
    }
}

fn text(section: &Value, key: &str, default: &str) -> String {
    match section.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => default.to_string(),
    }
}

fn flag(section: &Value, key: &str, default: bool) -> bool {
    section.get(key).and_then(Value::as_bool).unwrap_or(default)
}

/// A color list; an empty or missing list falls back to the default so the
/// modulo in the renderer never divides by zero.
fn colors(section: &Value, key: &str, default: &[&str]) -> Vec<String> {
    let list: Vec<String> = section
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if list.is_empty() {
        default.iter().map(|s| s.to_string()).collect()
    } else {
        list
    }
}

const DEFAULT_ACTOR_COLOURS: [&str; 6] = [
    "#8FBC8F", "#7CFC00", "#00FFFF", "#20B2AA", "#B0E0E6", "#FFFFE0",
];
const DEFAULT_SECTION_FILLS: [&str; 7] = [
    "#191970", "#8B008B", "#4B0082", "#2F4F4F", "#800000", "#8B4513", "#00008B",
];
const DEFAULT_SECTION_COLOURS: [&str; 1] = ["#fff"];

impl Conf {
    pub fn from_resolved(resolved: &Resolved) -> Conf {
        let s = &resolved.config.journey;
        let text_placement = match text(s, "textPlacement", "fo").as_str() {
            "fo" => TextPlacement::ForeignObject,
            "old" => TextPlacement::Old,
            _ => TextPlacement::Tspan,
        };
        Conf {
            use_max_width: flag(s, "useMaxWidth", true),
            diagram_margin_x: num(s, "diagramMarginX", 50.0),
            diagram_margin_y: num(s, "diagramMarginY", 10.0),
            left_margin: num(s, "leftMargin", 150.0),
            max_label_width: num(s, "maxLabelWidth", 360.0),
            width: num(s, "width", 150.0),
            height: num(s, "height", 50.0),
            box_text_margin: num(s, "boxTextMargin", 5.0),
            task_font_size: num(s, "taskFontSize", 14.0),
            task_font_family: text(s, "taskFontFamily", "\"Open Sans\", sans-serif"),
            task_margin: num(s, "taskMargin", 50.0),
            text_placement,
            actor_colours: colors(s, "actorColours", &DEFAULT_ACTOR_COLOURS),
            section_fills: colors(s, "sectionFills", &DEFAULT_SECTION_FILLS),
            section_colours: colors(s, "sectionColours", &DEFAULT_SECTION_COLOURS),
            title_color: text(s, "titleColor", ""),
            title_font_family: text(
                s,
                "titleFontFamily",
                "\"trebuchet ms\", verdana, arial, sans-serif",
            ),
            title_font_size: text(s, "titleFontSize", "4ex"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::config::{self, BaseConfig};

    #[test]
    fn defaults_match_the_schema() {
        let resolved = config::resolve("journey\n  a: 5: Me\n", &BaseConfig::new("default"))
            .expect("resolves");
        let conf = Conf::from_resolved(&resolved);
        assert_eq!(conf.left_margin, 150.0);
        assert_eq!(conf.width, 150.0);
        assert_eq!(conf.height, 50.0);
        assert_eq!(conf.task_margin, 50.0);
        assert_eq!(conf.text_placement, TextPlacement::ForeignObject);
        assert_eq!(conf.section_fills.len(), 7);
        assert_eq!(conf.actor_colours[0], "#8FBC8F");
        assert_eq!(conf.title_font_size, "4ex");
    }

    /// `configKeys` in `defaultConfig.ts` skips array-valued keys, so
    /// `sanitizeDirective` drops `sectionFills` (and `actorColours`,
    /// `sectionColours`) from a directive or front matter while a scalar
    /// key such as `leftMargin` goes through.
    #[test]
    fn front_matter_sets_scalars_and_loses_color_lists() {
        let source = "---\nconfig:\n  journey:\n    leftMargin: 200\n    sectionFills: ['#111111', '#222222']\n---\njourney\n  a: 5: Me\n";
        let resolved = config::resolve(source, &BaseConfig::new("default")).expect("resolves");
        let conf = Conf::from_resolved(&resolved);
        assert_eq!(conf.left_margin, 200.0);
        assert_eq!(conf.section_fills.len(), 7);
        assert_eq!(conf.section_fills[0], "#191970");
    }
}
