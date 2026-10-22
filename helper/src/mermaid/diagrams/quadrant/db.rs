//! Port of `quadrantDb.ts` (mermaid 12.2.0): the `yy` object the grammar
//! actions call, the point style parser from `utils.ts`, and
//! `getQuadrantData`, which hands the builder the config and the theme.

use super::builder::{Build, Builder, Config, Data, PointInput, StylesObject, ThemeConfig};
use crate::mermaid::config::QuadrantChartConfig;
use crate::mermaid::theme::ThemeVariables;

/// `LexTextObj`: a `text` production value. The `markdown` flag records a
/// `"`...`"` string; the renderer prints it as plain text, as mermaid does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextObj {
    pub text: String,
    pub markdown: bool,
}

/// `InvalidStyleError` and the unknown-style `Error` from `parseStyles`,
/// with the message mermaid throws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleError(pub String);

impl StyleError {
    fn invalid(style: &str, value: &str, kind: &str) -> StyleError {
        StyleError(format!(
            "value for {style} {value} is invalid, please use a valid {kind}"
        ))
    }
}

impl std::fmt::Display for StyleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// `validateHexCode`: `#?` then three or six hex digits. The `#` is
/// optional, so `ff0000` passes and reaches the SVG as an invalid color,
/// exactly as in mermaid.
fn is_hex_code(value: &str) -> bool {
    let digits = value.strip_prefix('#').unwrap_or(value);
    (digits.len() == 3 || digits.len() == 6) && digits.chars().all(|c| c.is_ascii_hexdigit())
}

/// `validateNumber`: `^\d+$`.
fn is_number(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_digit())
}

/// `validateSizeInPixels`: `^\d+px$`.
fn is_size_in_pixels(value: &str) -> bool {
    value
        .strip_suffix("px")
        .is_some_and(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
}

/// `parseStyles(styles)`: each entry is `key: value`, split on the first
/// colon with the surrounding whitespace. A missing value reads as the
/// string `undefined`, which fails every validator the way JS does.
pub fn parse_styles(styles: &[String]) -> Result<StylesObject, StyleError> {
    let mut out = StylesObject::default();
    for style in styles {
        let style = style.trim();
        let (key, value) = match style.find(':') {
            Some(i) => (style[..i].trim_end(), style[i + 1..].trim_start()),
            None => (style, "undefined"),
        };
        // `split(/\s*:\s*/)` stops nowhere, so a second colon starts a third
        // field that is dropped; the value is everything up to it.
        let value = match value.find(':') {
            Some(i) => value[..i].trim_end(),
            None => value,
        };
        match key {
            "radius" => {
                if !is_number(value) {
                    return Err(StyleError::invalid(key, value, "number"));
                }
                out.radius = value.parse::<f64>().ok();
            }
            "color" => {
                if !is_hex_code(value) {
                    return Err(StyleError::invalid(key, value, "hex code"));
                }
                out.color = Some(value.to_string());
            }
            "stroke-color" => {
                if !is_hex_code(value) {
                    return Err(StyleError::invalid(key, value, "hex code"));
                }
                out.stroke_color = Some(value.to_string());
            }
            "stroke-width" => {
                if !is_size_in_pixels(value) {
                    return Err(StyleError::invalid(
                        key,
                        value,
                        "number of pixels (eg. 10px)",
                    ));
                }
                out.stroke_width = Some(value.to_string());
            }
            other => return Err(StyleError(format!("style named {other} is not supported."))),
        }
    }
    Ok(out)
}

/// `textSanitizer`: `sanitizeText(text.trim(), config)`. DOMPurify parses
/// the text as HTML and serializes it again, so a bare `&` comes back as
/// `&amp;` and d3's `.text()` then shows those five characters. An entity
/// the parser knows survives the round trip unchanged. `<` and `>` cannot
/// reach here: the lexer has no token for them.
pub fn sanitize_text(text: &str) -> String {
    let text = text.trim();
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        if is_known_entity(after) {
            out.push('&');
        } else {
            out.push_str("&amp;");
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Whether `after` (the text following an `&`) starts with an entity the
/// HTML parser decodes: a numeric reference or one of the common names.
fn is_known_entity(after: &str) -> bool {
    let Some(end) = after.find(';') else {
        return false;
    };
    let name = &after[..end];
    if let Some(digits) = name.strip_prefix('#') {
        let hex = digits.strip_prefix(['x', 'X']);
        return match hex {
            Some(h) => !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit()),
            None => !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()),
        };
    }
    matches!(name, "amp" | "lt" | "gt" | "quot" | "apos" | "nbsp")
}

/// The diagram database: everything the grammar actions store.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Db {
    data: Data,
    classes: Vec<(String, StylesObject)>,
    /// `commonDb` fields; parsed and kept, drawn by nothing.
    pub acc_title: String,
    pub acc_description: String,
}

impl Db {
    pub fn new() -> Db {
        Db::default()
    }

    pub fn set_quadrant1_text(&mut self, t: &TextObj) {
        self.data.quadrant1_text = sanitize_text(&t.text);
    }

    pub fn set_quadrant2_text(&mut self, t: &TextObj) {
        self.data.quadrant2_text = sanitize_text(&t.text);
    }

    pub fn set_quadrant3_text(&mut self, t: &TextObj) {
        self.data.quadrant3_text = sanitize_text(&t.text);
    }

    pub fn set_quadrant4_text(&mut self, t: &TextObj) {
        self.data.quadrant4_text = sanitize_text(&t.text);
    }

    pub fn set_x_axis_left_text(&mut self, t: &TextObj) {
        self.data.x_axis_left_text = sanitize_text(&t.text);
    }

    pub fn set_x_axis_right_text(&mut self, t: &TextObj) {
        self.data.x_axis_right_text = sanitize_text(&t.text);
    }

    pub fn set_y_axis_top_text(&mut self, t: &TextObj) {
        self.data.y_axis_top_text = sanitize_text(&t.text);
    }

    pub fn set_y_axis_bottom_text(&mut self, t: &TextObj) {
        self.data.y_axis_bottom_text = sanitize_text(&t.text);
    }

    /// `commonDb.setDiagramTitle`: sanitized, not trimmed. The grammar
    /// trims its `title_value`; a front matter title arrives as written.
    pub fn set_diagram_title(&mut self, title: &str) {
        self.data.title_text = sanitize_text_no_trim(title);
    }

    pub fn diagram_title(&self) -> &str {
        &self.data.title_text
    }

    /// `addPoint`: styles are validated first, then the point goes to the
    /// front of the list (`addPoints` prepends), so the first point in the
    /// source is drawn last and sits on top.
    pub fn add_point(
        &mut self,
        t: &TextObj,
        class_name: &str,
        x: f64,
        y: f64,
        styles: &[String],
    ) -> Result<(), StyleError> {
        let styles = parse_styles(styles)?;
        self.data.points.insert(
            0,
            PointInput {
                x,
                y,
                text: sanitize_text(&t.text),
                class_name: class_name.to_string(),
                styles,
            },
        );
        Ok(())
    }

    /// `addClass`: a later `classDef` of the same name replaces the first.
    pub fn add_class(&mut self, class_name: &str, styles: &[String]) -> Result<(), StyleError> {
        let styles = parse_styles(styles)?;
        self.classes.push((class_name.to_string(), styles));
        Ok(())
    }

    /// `getQuadrantData`: the builder with the effective `quadrantChart`
    /// section, the fifteen theme variables, and the title.
    pub fn quadrant_data(&self, config: &QuadrantChartConfig, theme: &ThemeVariables) -> Build {
        let mut builder = Builder::new(
            Config::from_config(config),
            ThemeConfig::from_theme(theme),
            self.data.clone(),
        );
        for (name, styles) in &self.classes {
            builder.add_class(name, styles.clone());
        }
        builder.build()
    }
}

/// `sanitizeText` without the `trim()` of `textSanitizer`.
fn sanitize_text_no_trim(text: &str) -> String {
    let trimmed = sanitize_text(text);
    let start = text.len() - text.trim_start().len();
    let end = text.trim_end().len();
    format!("{}{}{}", &text[..start], trimmed, &text[end..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_styles_accepts_the_documented_forms() {
        let styles: Vec<String> = [
            "radius: 12",
            "color: #ff3300",
            "stroke-color: #10f0f0",
            "stroke-width: 5px",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let s = parse_styles(&styles).unwrap();
        assert_eq!(s.radius, Some(12.0));
        assert_eq!(s.color.as_deref(), Some("#ff3300"));
        assert_eq!(s.stroke_color.as_deref(), Some("#10f0f0"));
        assert_eq!(s.stroke_width.as_deref(), Some("5px"));
        let s = parse_styles(&["radius : 10".to_string(), "color: f00".to_string()]).unwrap();
        assert_eq!(s.radius, Some(10.0));
        assert_eq!(s.color.as_deref(), Some("f00"));
    }

    #[test]
    fn parse_styles_throws_mermaids_messages() {
        let err = |s: &str| parse_styles(&[s.to_string()]).unwrap_err().0;
        assert_eq!(
            err("radius: 12px"),
            "value for radius 12px is invalid, please use a valid number"
        );
        assert_eq!(
            err("color: red"),
            "value for color red is invalid, please use a valid hex code"
        );
        assert_eq!(
            err("stroke-width: 5"),
            "value for stroke-width 5 is invalid, please use a valid number of pixels (eg. 10px)"
        );
        assert_eq!(
            err("radius"),
            "value for radius undefined is invalid, please use a valid number"
        );
        assert_eq!(err("size: 3"), "style named size is not supported.");
    }

    #[test]
    fn sanitize_text_escapes_a_bare_ampersand() {
        assert_eq!(sanitize_text("  A & B "), "A &amp; B");
        assert_eq!(sanitize_text("A &amp; B"), "A &amp; B");
        assert_eq!(sanitize_text("A &#38; B"), "A &#38; B");
        assert_eq!(sanitize_text("A &foo; B"), "A &amp;foo; B");
        assert_eq!(sanitize_text_no_trim(" x & y "), " x &amp; y ");
    }

    #[test]
    fn points_are_prepended_and_title_overrides() {
        let mut db = Db::new();
        db.set_diagram_title("front");
        db.set_diagram_title("inline");
        let t = |s: &str| TextObj {
            text: s.into(),
            markdown: false,
        };
        db.add_point(&t("first"), "", 0.1, 0.2, &[]).unwrap();
        db.add_point(&t("second"), "", 0.3, 0.4, &[]).unwrap();
        let build = db.quadrant_data(&QuadrantChartConfig::default(), &ThemeVariables::default());
        assert_eq!(build.points[0].text.text, "second");
        assert_eq!(build.points[1].text.text, "first");
        assert_eq!(
            build.title.as_ref().map(|t| t.text.as_str()),
            Some("inline")
        );
        assert!(build.points[0].fill.is_none());
    }
}
