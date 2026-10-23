//! The `conf` object of `c4Renderer.ts`: `getRequiredConfig('c4')`, the
//! `c4` section after the defaults, the base config, and the diagram's
//! directives merged.
//!
//! The typed config keeps that section as JSON; the accessors read it with
//! the schema defaults (`config.schema.yaml`, `C4DiagramConfig`) as the
//! fallback for a key a directive removed or mistyped. `setConf` copies a
//! root `fontFamily` only when the section has one, which the schema never
//! gives it, so it changes nothing here.

use serde_json::Value;

use crate::mermaid::config::Resolved;
use crate::text::FontSpec as TextFont;

/// One `*Font()` result of the c4 config: `{ fontFamily, fontSize,
/// fontWeight }`.
#[derive(Debug, Clone, PartialEq)]
pub struct C4Font {
    pub family: String,
    pub size: f64,
    /// The CSS `font-weight` value as configured, such as `normal`.
    pub weight: String,
}

impl C4Font {
    /// The numeric weight Chrome resolves the CSS value to.
    pub fn weight_number(&self) -> u16 {
        css_weight(&self.weight)
    }

    /// The font the text package measures with.
    pub fn text_font(&self) -> TextFont {
        TextFont::new(self.family.clone(), self.size as f32).weight(self.weight_number())
    }

    pub fn bold(mut self) -> C4Font {
        self.weight = "bold".to_string();
        self
    }

    pub fn sized(mut self, size: f64) -> C4Font {
        self.size = size;
        self
    }
}

/// CSS `font-weight` keywords and numbers.
pub fn css_weight(value: &str) -> u16 {
    match value.trim() {
        "bold" | "bolder" => 700,
        "lighter" => 100,
        other => other.parse::<u16>().unwrap_or(400),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Conf {
    pub use_max_width: bool,
    pub diagram_margin_x: f64,
    pub diagram_margin_y: f64,
    pub c4_shape_margin: f64,
    pub c4_shape_padding: f64,
    pub width: f64,
    pub height: f64,
    pub next_line_padding_x: f64,
    /// `c4.wrap`: whether the element labels wrap.
    pub wrap: bool,
    section: Value,
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

/// The schema's per-shape font defaults: Open Sans, normal, 14 px (12 px
/// for `message`).
const DEFAULT_FONT_FAMILY: &str = "\"Open Sans\", sans-serif";

impl Conf {
    pub fn from_resolved(resolved: &Resolved) -> Conf {
        let s = resolved.config.c4.clone();
        Conf {
            use_max_width: s
                .get("useMaxWidth")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            diagram_margin_x: num(&s, "diagramMarginX", 50.0),
            diagram_margin_y: num(&s, "diagramMarginY", 10.0),
            c4_shape_margin: num(&s, "c4ShapeMargin", 50.0),
            c4_shape_padding: num(&s, "c4ShapePadding", 20.0),
            width: num(&s, "width", 216.0),
            height: num(&s, "height", 60.0),
            next_line_padding_x: num(&s, "nextLinePaddingX", 0.0),
            wrap: s.get("wrap").and_then(Value::as_bool).unwrap_or(true),
            section: s,
        }
    }

    /// `conf[<prefix>Font]()`: `boundary`, `message`, or an element type.
    pub fn font(&self, prefix: &str) -> C4Font {
        let default_size = if prefix == "message" { 12.0 } else { 14.0 };
        C4Font {
            family: text(
                &self.section,
                &format!("{prefix}FontFamily"),
                DEFAULT_FONT_FAMILY,
            ),
            size: num(&self.section, &format!("{prefix}FontSize"), default_size),
            weight: text(&self.section, &format!("{prefix}FontWeight"), "normal"),
        }
    }

    /// `config[<type>_bg_color]` and `config[<type>_border_color]`.
    pub fn color(&self, key: &str) -> Option<String> {
        match self.section.get(key) {
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        }
    }

    /// The raw value of a font key, for the stylesheet: `elementFontStyles`
    /// skips a key that is unset or empty.
    pub fn raw(&self, key: &str) -> Option<&Value> {
        self.section.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::config::{self, BaseConfig};

    #[test]
    fn defaults_match_the_schema() {
        let resolved = config::resolve(
            "C4Context\n  Person(a, \"A\")\n",
            &BaseConfig::new("default"),
        )
        .expect("resolves");
        let conf = Conf::from_resolved(&resolved);
        assert_eq!(conf.width, 216.0);
        assert_eq!(conf.c4_shape_margin, 50.0);
        assert!(conf.wrap);
        let message = conf.font("message");
        assert_eq!(message.size, 12.0);
        assert_eq!(message.family, "\"Open Sans\", sans-serif");
        assert_eq!(conf.font("person").size, 14.0);
        assert_eq!(conf.color("person_bg_color").as_deref(), Some("#08427B"));
    }

    #[test]
    fn a_directive_changes_a_font_size() {
        let source =
            "%%{init: {'c4': {'boundaryFontSize': 20}}}%%\nC4Context\n  Person(a, \"A\")\n";
        let resolved = config::resolve(source, &BaseConfig::new("default")).expect("resolves");
        assert_eq!(Conf::from_resolved(&resolved).font("boundary").size, 20.0);
    }
}
