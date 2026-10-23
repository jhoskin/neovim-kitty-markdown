//! `diagrams/treemap/styles.ts` (`getStyles`) and the `styles2String`
//! split the treemap renderer applies to `classDef` styles, resolved to
//! attribute values.
//!
//! Of the stylesheet rules only `.treemapTitle` reaches an element: the
//! section rects carry `treemapSection section<i>` and the leaf groups
//! `treemapNode treemapLeafGroup leaf<i>`, so `.treemapNode.section` and
//! `.treemapNode.leaf` never match, and the label and value texts set
//! their `fill` and `font-size` inline, which beats `.treemapLabel` and
//! `.treemapValue`. For the same reason the `classDef` rules mermaid adds
//! to the stylesheet never match: the renderer appends an `x` to the class
//! name of a leaf group (`leaf0 importantx`). The class styles arrive
//! through the inline `style` attributes only.

use indexmap::IndexMap;
use serde_json::Value;

use crate::mermaid::scene::{Color, FontStyle, FontWeight, Paint};
use crate::mermaid::shapes::util::{Declarations, is_label_style, parse_css_color};
use crate::mermaid::styles::pie::{css_color, css_length};
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::khroma::js_number;

/// The SVG root rule's `font-size`.
const ROOT_FONT_SIZE: f64 = 16.0;
/// `defaultTreemapStyleOptions.titleFontSize`.
const TITLE_FONT_SIZE: f64 = 14.0;

/// The values of `getStyles` the renderer needs.
#[derive(Debug, Clone, PartialEq)]
pub struct TreemapStyles {
    /// The root `font-family`: the theme's `fontFamily`.
    pub font_family: String,
    /// The root `fill`, `textColor`, which a text without its own fill
    /// inherits.
    pub text_color: Option<Color>,
    /// `.treemapTitle { fill: titleColor; font-size: 14px }`.
    pub title_color: Option<Color>,
    pub title_font_size: f64,
    pub root_font_size: f64,
}

impl TreemapStyles {
    /// `getStyles({ treemap })`: the theme variables of the active theme
    /// (`cleanAndMerge(defaultThemeVariables, config.themeVariables)`,
    /// where the config already holds the full theme).
    pub fn resolve(tv: &ThemeVariables, fallback_font: &str) -> TreemapStyles {
        let text_color = tv.str("textColor").and_then(css_color);
        TreemapStyles {
            font_family: tv
                .str("fontFamily")
                .filter(|f| !f.trim().is_empty())
                .unwrap_or(fallback_font)
                .to_string(),
            text_color,
            // An invalid `titleColor` drops the declaration and the title
            // inherits the root fill.
            title_color: tv.str("titleColor").and_then(css_color).or(text_color),
            title_font_size: TITLE_FONT_SIZE,
            root_font_size: ROOT_FONT_SIZE,
        }
    }
}

/// A theme color as an SVG paint: `transparent` paints nothing, anything
/// khroma cannot read leaves the attribute invalid (`None`).
pub fn theme_paint(value: &str) -> Option<Paint> {
    let v = value.trim();
    if v.eq_ignore_ascii_case("transparent") || v.eq_ignore_ascii_case("none") {
        return Some(Paint::None);
    }
    css_color(v)
        .map(Paint::Color)
        .or_else(|| parse_css_color(v))
}

/// The ordinal color scales of `renderer.ts`: `scaleOrdinal().range(...)`
/// with an implicit domain, so a name takes the next range slot the first
/// time it is asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct OrdinalScale {
    domain: IndexMap<String, usize>,
    range: Vec<String>,
}

impl OrdinalScale {
    pub fn new(range: Vec<String>) -> OrdinalScale {
        OrdinalScale {
            domain: IndexMap::new(),
            range,
        }
    }

    /// `colorScale`: `transparent`, then `cScale0` to `cScale11`.
    pub fn sections(tv: &ThemeVariables) -> OrdinalScale {
        OrdinalScale::with_prefix(tv, "cScale", true)
    }

    /// `colorScalePeer`: `transparent`, then `cScalePeer0` to
    /// `cScalePeer11`.
    pub fn peers(tv: &ThemeVariables) -> OrdinalScale {
        OrdinalScale::with_prefix(tv, "cScalePeer", true)
    }

    /// `colorScaleLabel`: `cScaleLabel0` to `cScaleLabel11`.
    pub fn labels(tv: &ThemeVariables) -> OrdinalScale {
        OrdinalScale::with_prefix(tv, "cScaleLabel", false)
    }

    fn with_prefix(tv: &ThemeVariables, prefix: &str, transparent: bool) -> OrdinalScale {
        let mut range = Vec::new();
        if transparent {
            range.push("transparent".to_string());
        }
        for i in 0..12 {
            // `undefined` in the range prints as the string "undefined"
            // in the attribute, which no browser paints.
            range.push(match tv.get(&format!("{prefix}{i}")) {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.as_f64().map(js_number).unwrap_or_default(),
                _ => "undefined".to_string(),
            });
        }
        OrdinalScale::new(range)
    }

    /// `scale(name)`.
    pub fn get(&mut self, name: &str) -> String {
        let next = self.domain.len();
        let index = *self.domain.entry(name.to_string()).or_insert(next);
        self.range
            .get(index % self.range.len().max(1))
            .cloned()
            .unwrap_or_default()
    }
}

/// `styles2String({ cssCompiledStyles })` for a treemap node.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClassStyles {
    /// `nodeStyles`: every declaration that is not a label style.
    pub node: Declarations,
    /// `borderStyles`: the node declarations whose key contains `stroke`.
    pub border: Declarations,
    /// `labelStyles` after `.replace('color:', 'fill:')`, which renames
    /// the first `color` declaration only.
    pub label: Declarations,
}

impl ClassStyles {
    /// `styles2Map`: `const [key, value] = style.split(':')`, so a value
    /// stops at its first colon, and a later key replaces an earlier one in
    /// place.
    pub fn from_compiled(compiled: Option<&[String]>) -> ClassStyles {
        let mut map = Declarations::new();
        for style in compiled.unwrap_or(&[]) {
            let mut parts = style.split(':');
            let key = parts.next().unwrap_or("").trim().to_string();
            let value = parts.next().map(|v| v.trim().to_string());
            // `[key, undefined].join(':')` is `key:`, a declaration the
            // browser drops.
            map.insert(key, value.unwrap_or_default());
        }
        let mut out = ClassStyles::default();
        let mut renamed = false;
        for (key, value) in map {
            if is_label_style(&key) {
                let key = if key == "color" && !renamed {
                    renamed = true;
                    "fill".to_string()
                } else {
                    key
                };
                out.label.insert(key, value);
            } else {
                if key.contains("stroke") {
                    out.border.insert(key.clone(), value.clone());
                }
                out.node.insert(key, value);
            }
        }
        out
    }

    /// The label declarations the scene can carry on a text: the fill, and
    /// the font size, weight, style, and family.
    pub fn label_fill(&self) -> Option<Paint> {
        self.label.get("fill").and_then(|v| parse_css_color(v))
    }

    pub fn label_font_size(&self) -> Option<f64> {
        self.label.get("font-size").and_then(|v| css_length(v))
    }

    pub fn label_font_weight(&self) -> Option<FontWeight> {
        self.label.get("font-weight").and_then(|v| match v.trim() {
            "bold" | "bolder" => Some(FontWeight::Bold),
            "normal" | "lighter" => Some(FontWeight::Normal),
            n => n.parse::<u16>().ok().map(FontWeight::Number),
        })
    }

    pub fn label_font_style(&self) -> Option<FontStyle> {
        self.label.get("font-style").and_then(|v| match v.trim() {
            "italic" => Some(FontStyle::Italic),
            "oblique" => Some(FontStyle::Oblique),
            "normal" => Some(FontStyle::Normal),
            _ => None,
        })
    }

    pub fn label_font_family(&self) -> Option<String> {
        self.label
            .get("font-family")
            .filter(|v| !v.trim().is_empty())
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::theme;

    #[test]
    fn the_title_rule_uses_the_theme() {
        let tv = theme::resolve("default", None).unwrap();
        let s = TreemapStyles::resolve(&tv, "serif");
        assert_eq!(
            s.font_family,
            "\"trebuchet ms\", verdana, arial, sans-serif"
        );
        assert_eq!(s.title_font_size, 14.0);
        assert_eq!(s.title_color, Some(Color::rgb(0x33, 0x33, 0x33)));
    }

    #[test]
    fn ordinal_scales_assign_slots_on_first_use() {
        let mut scale = OrdinalScale::new(vec!["a".into(), "b".into()]);
        assert_eq!(scale.get("x"), "a");
        assert_eq!(scale.get("y"), "b");
        assert_eq!(scale.get("z"), "a");
        assert_eq!(scale.get("y"), "b");
        let tv = theme::resolve("default", None).unwrap();
        assert_eq!(OrdinalScale::sections(&tv).get(""), "transparent");
    }

    #[test]
    fn class_styles_split_like_styles2string() {
        let compiled = [
            "fill:red".to_string(),
            "color:blue".to_string(),
            "stroke:#FFD600".to_string(),
            "stroke-width:2px".to_string(),
            "font-weight:bold".to_string(),
        ];
        let s = ClassStyles::from_compiled(Some(&compiled));
        assert_eq!(
            s.node.keys().collect::<Vec<_>>(),
            ["fill", "stroke", "stroke-width"]
        );
        assert_eq!(
            s.border.keys().collect::<Vec<_>>(),
            ["stroke", "stroke-width"]
        );
        assert_eq!(s.label["fill"], "blue");
        assert_eq!(s.label_font_weight(), Some(FontWeight::Bold));
    }
}
