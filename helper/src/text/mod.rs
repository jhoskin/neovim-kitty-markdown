//! Fonts and text measurement. Mermaid measures labels in a browser; these
//! modules reproduce `getBBox` and `getBoundingClientRect` with rustybuzz.

#[cfg(target_os = "macos")]
mod coretext;
pub mod fonts;
mod instances;
pub mod markdown;
pub mod measure;
pub mod open_sans;
pub mod wrap;

use std::sync::{Arc, OnceLock};

use usvg::fontdb;

pub use fonts::FontSystem;
pub use measure::{FontSpec, FontStyle, TextMetrics};

static FONTS: OnceLock<FontSystem> = OnceLock::new();

/// The process-wide font database. Loading the system fonts takes long
/// enough that every caller shares one.
pub fn font_system() -> &'static FontSystem {
    FONTS.get_or_init(FontSystem::new)
}

/// The font database only if some caller already loaded it. The probe uses
/// this so its answer never waits on font discovery (R-025).
pub fn font_system_if_loaded() -> Option<&'static FontSystem> {
    FONTS.get()
}

/// The database resvg rasterizes with: the measurer's faces plus one alias
/// face per family spelled in lower case. resvg matches `font-family`
/// case-sensitively, and mermaid writes its chain as `"trebuchet ms",
/// verdana, arial, sans-serif`, so without the aliases every label would
/// fall back to the serif default while the measurer (case-insensitive)
/// kept the right widths.
pub fn fontdb_arc() -> Arc<fontdb::Database> {
    static DB: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = font_system().db().clone();
        let aliases: Vec<fontdb::FaceInfo> = db
            .faces()
            .filter_map(|face| {
                let lower: Vec<(String, fontdb::Language)> = face
                    .families
                    .iter()
                    .map(|(name, lang)| (name.to_lowercase(), *lang))
                    .collect();
                if lower.iter().zip(&face.families).all(|(a, b)| a.0 == b.0) {
                    return None;
                }
                let mut alias = face.clone();
                alias.families = lower;
                Some(alias)
            })
            .collect();
        for alias in aliases {
            db.push_face_info(alias);
        }
        Arc::new(db)
    })
    .clone()
}

#[cfg(test)]
mod fontdb_tests {
    use super::*;

    #[test]
    fn lower_case_family_names_resolve() {
        let db = fontdb_arc();
        let mut checked = 0;
        for face in db.faces() {
            let Some((name, _)) = face.families.first() else {
                continue;
            };
            if name.chars().all(|c| !c.is_uppercase()) {
                continue;
            }
            let alias = name.to_lowercase();
            let query = fontdb::Query {
                families: &[fontdb::Family::Name(&alias)],
                weight: face.weight,
                stretch: face.stretch,
                style: face.style,
            };
            assert!(db.query(&query).is_some(), "no alias for {name}");
            checked += 1;
            if checked == 20 {
                break;
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TextError {
    #[error("unknown measure mode `{0}`; use `svg` or `html`")]
    UnknownMode(String),
    #[error("no font found for `{0}`")]
    NoFont(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl TextError {
    /// The protocol error code for this failure.
    pub fn code(&self) -> &'static str {
        match self {
            TextError::UnknownMode(_) => "invalid_params",
            TextError::NoFont(_) | TextError::Json(_) => "internal",
        }
    }
}

/// The `measure` subcommand: one JSON object with `width`, `height`, and
/// `lines`. In `svg` mode the numbers are `getBBox` of one `<text>` holding
/// the whole string, with each `<br>`-separated line also measured alone.
/// In `html` mode they are `getBoundingClientRect` of mermaid's label div
/// around `markdownToHTML(text)` without a width limit.
pub fn run_measure(text: &str, font: &str, size: f32, mode: &str) -> Result<String, TextError> {
    let fonts = font_system();
    let metrics = TextMetrics::new(fonts);
    let spec = FontSpec::new(font, size);
    let face = fonts
        .resolve(font, fonts::Weight(spec.weight), fonts::Style::Normal)
        .ok_or_else(|| TextError::NoFont(font.to_string()))?;
    let resolved = fonts.family_name(face).unwrap_or_default();
    let value = match mode {
        "svg" => {
            let whole = metrics.svg_bbox(text, &spec);
            let lines: Vec<serde_json::Value> = markdown::split_breaks(text)
                .into_iter()
                .map(|line| {
                    let bbox = metrics.svg_bbox(line, &spec);
                    serde_json::json!({ "text": line, "width": bbox.width, "height": bbox.height })
                })
                .collect();
            let dims = metrics.calculate_text_dimensions(text, &spec);
            serde_json::json!({
                "mode": "svg",
                "font": resolved,
                "width": whole.width,
                "height": whole.height,
                "lines": lines,
                "calculate_text_dimensions": {
                    "width": dims.width,
                    "height": dims.height,
                    "line_height": dims.line_height,
                },
            })
        }
        "html" => {
            let html = markdown::markdown_to_html(text);
            let lines = markdown::html_to_lines(&html);
            let label = metrics.html_label(&lines, &spec, None);
            let line_values: Vec<serde_json::Value> = label
                .lines
                .iter()
                .map(|line| {
                    let text: String = line.iter().map(|w| w.content.as_str()).collect();
                    serde_json::json!({ "text": text, "width": metrics.words_width(line, &spec) })
                })
                .collect();
            serde_json::json!({
                "mode": "html",
                "font": resolved,
                "width": label.width,
                "height": label.height,
                "lines": line_values,
            })
        }
        other => return Err(TextError::UnknownMode(other.to_string())),
    };
    Ok(serde_json::to_string(&value)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_measure_reports_both_modes() {
        let svg =
            run_measure("Hello<br>world", "\"Open Sans\", sans-serif", 16.0, "svg").expect("svg");
        let value: serde_json::Value = serde_json::from_str(&svg).expect("json");
        assert_eq!(value["height"], 22.0);
        assert_eq!(value["lines"].as_array().map(Vec::len), Some(2));
        let html = run_measure("Hello", "\"Open Sans\", sans-serif", 16.0, "html").expect("html");
        let value: serde_json::Value = serde_json::from_str(&html).expect("json");
        assert_eq!(value["height"], 24.0);
    }

    #[test]
    fn run_measure_rejects_unknown_mode_and_missing_font() {
        let err = run_measure("x", "Open Sans", 16.0, "pdf").expect_err("mode");
        assert_eq!(err.code(), "invalid_params");
        let err = run_measure("x", "No Such Font Family", 16.0, "svg").expect_err("font");
        assert_eq!(err.code(), "internal");
    }
}
