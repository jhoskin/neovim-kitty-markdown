//! MermaidConfig defaults from `config.schema.yaml`, merge, appearance resolution.
//!
//! [`resolve`] is the config side of mermaid 12.2.0's render path for one
//! diagram, as mermaid-cli drives it:
//!
//! 1. `initialize(baseConfig)`: `setSiteConfig` merges the base config
//!    (R-174: `layout: dagre`, `look: classic`, the selected theme) over the
//!    schema defaults and derives the site theme variables.
//! 2. `preprocessDiagram(text)`: front matter and directives become one
//!    config object (R-102, R-103).
//! 3. `detectType` names the diagram, which scopes the appearance defaults.
//! 4. `addDirective(config)`: `sanitizeDirective`, the `fontFamily` to
//!    `themeVariables.fontFamily` copy, then `updateCurrentConfig`: site
//!    config plus the directive sum, `resolveAppearance`, and a fresh theme
//!    derivation when the theme changed. Without a theme change the
//!    directive's `themeVariables` are merged onto the site's derived ones,
//!    so a `primaryColor` override alone recolors nothing derived; that is
//!    mermaid's behavior and the reason its docs ask for `theme: base`.
//!
//! The result is then checked against what the helper renders (R-175).

pub mod appearance;
pub mod defaults;
pub mod merge;

use serde_json::{Map, Value, json};

pub use appearance::Appearance;
pub use defaults::{
    ClassConfig, ErConfig, FlowchartConfig, GanttConfig, MermaidConfig, Num, PieConfig,
    QuadrantChartConfig, SequenceConfig, StateConfig, XyChartAxisConfig, XyChartConfig,
};

use crate::error::HelperError;
use crate::mermaid::detect::{self, DetectError, FRONT_MATTER_ERROR, FRONT_MATTER_TYPE};
use crate::mermaid::preprocess::{self, PreprocessError, Preprocessed};
use crate::mermaid::theme::{self, ThemeVariables};
use merge::{assign_with_depth, copy_with_depth, sanitize, sanitize_directive};

/// The base config (SPEC.md Definitions): what `helper/oracle/mermaid-config.json`
/// holds, with the theme the plugin selects. mermaid-cli adds
/// `startOnLoad: false`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseConfig {
    pub theme: String,
}

impl BaseConfig {
    pub fn new(theme: &str) -> BaseConfig {
        BaseConfig {
            theme: theme.to_string(),
        }
    }

    /// The object `mermaid.initialize` receives.
    pub fn to_value(&self) -> Value {
        json!({
            "startOnLoad": false,
            "layout": "dagre",
            "look": "classic",
            "theme": self.theme,
            "handDrawnSeed": 1,
        })
    }
}

/// Everything a diagram renderer needs from the config layer.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    /// The detector id, for example `flowchart-v2`.
    pub diagram_type: &'static str,
    /// The config section the diagram reads, for example `flowchart`.
    pub config_key: String,
    pub config: MermaidConfig,
    /// The effective config as JSON, `getConfig()` without `themeVariables`,
    /// for sections the typed struct leaves as JSON.
    pub raw: Value,
    pub theme_variables: ThemeVariables,
    pub appearance: Appearance,
    pub preprocessed: Preprocessed,
}

impl Resolved {
    /// The front matter title, if any.
    pub fn title(&self) -> Option<&str> {
        self.preprocessed.title.as_deref()
    }

    /// The cleaned diagram text for the parser.
    pub fn code(&self) -> &str {
        &self.preprocessed.code
    }
}

/// The site config and the pieces `updateCurrentConfig` reads besides it.
#[derive(Debug, Clone)]
pub struct Site {
    /// `siteConfig`: defaults, base config, derived theme variables.
    pub config: Value,
    /// `siteConfigDelta`: the base config alone.
    pub delta: Value,
    /// `configFromInitialize`: the base config as handed to `initialize`.
    pub from_initialize: Value,
    /// `defaultConfig`.
    pub defaults: Value,
}

/// `initialize(baseConfig)` followed by `setSiteConfig`.
pub fn site(base: &BaseConfig) -> Result<Site, HelperError> {
    let defaults = defaults::default_config_value();
    let base_value = base.to_value();
    // `initialize` derives the theme once, `setSiteConfig` once more with
    // the first result as the overrides; the second pass changes nothing,
    // but it is what the oracle does.
    let first = theme::resolve(&base.theme, None).map_err(theme_error)?;
    let second = theme::resolve_value(&base.theme, &first.to_json()).map_err(theme_error)?;
    let mut config = assign_with_depth(copy_with_depth(&defaults), &base_value, 2);
    if let Value::Object(map) = &mut config {
        map.insert("themeVariables".to_string(), second.to_json());
    }
    Ok(Site {
        config,
        delta: copy_with_depth(&base_value),
        from_initialize: copy_with_depth(&base_value),
        defaults,
    })
}

fn theme_error(err: theme::ThemeError) -> HelperError {
    HelperError::internal(format!("theme: {err}"))
}

fn preprocess_error(err: PreprocessError) -> HelperError {
    match err {
        // The YAML error and the unknown-type error both stop mermaid before
        // any parser runs; the oracle prints them as the diagram's error.
        PreprocessError::FrontMatter(message) => HelperError::ParseError {
            line: 1,
            column: 0,
            diagram_type: FRONT_MATTER_TYPE.to_string(),
            message,
        },
        PreprocessError::Detect(err) => detect_error(err),
    }
}

fn detect_error(err: DetectError) -> HelperError {
    HelperError::ParseError {
        line: 1,
        column: 0,
        diagram_type: "unknown".to_string(),
        message: err.to_string(),
    }
}

/// `addDirective(directive)` plus `updateCurrentConfig(siteConfig, directives)`
/// for one directive, with the diagram config scope set to `config_key`.
pub fn apply_directive(
    site: &Site,
    directive: Value,
    config_key: Option<&str>,
) -> Result<Value, HelperError> {
    let mut directive = directive;
    sanitize_directive(&mut directive);
    if let Value::Object(map) = &mut directive {
        // A directive `fontFamily` reaches the theme unless the directive
        // already names `themeVariables.fontFamily`.
        let font = map.get("fontFamily").cloned();
        if let Some(font) = font.filter(theme::vars::js_truthy) {
            let has_tv_font = map
                .get("themeVariables")
                .and_then(|tv| tv.get("fontFamily"))
                .is_some_and(theme::vars::js_truthy);
            if !has_tv_font {
                let tv = map
                    .entry("themeVariables")
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Value::Object(tv) = tv {
                    tv.insert("fontFamily".to_string(), font);
                }
            }
        }
    }
    let secure = secure_keys(&site.config);
    let mut cfg = copy_with_depth(&site.config);
    let mut sum = Value::Object(Map::new());
    let mut sanitized = directive;
    sanitize(&mut sanitized, &secure);
    sum = assign_with_depth(sum, &sanitized, 2);
    cfg = assign_with_depth(cfg, &sum, 2);

    appearance::resolve_appearance(
        &mut cfg,
        config_key,
        &[&sum, &sanitized, &site.delta, &site.defaults],
    );

    let theme_was_overridden = sum.get("theme").is_some_and(theme::vars::js_truthy)
        || cfg.get("theme") != site.config.get("theme");
    let cfg_theme = cfg.get("theme").and_then(Value::as_str).map(str::to_string);
    if theme_was_overridden {
        if let Some(name) = cfg_theme.filter(|t| theme::is_mermaid_theme(t)) {
            if theme::is_builtin(&name) {
                let base_tv = site
                    .from_initialize
                    .get("themeVariables")
                    .cloned()
                    .unwrap_or_else(|| Value::Object(Map::new()));
                let empty = Value::Null;
                let overrides = assign_with_depth(
                    copy_with_depth(&base_tv),
                    sum.get("themeVariables").unwrap_or(&empty),
                    2,
                );
                let tv = theme::resolve_value(&name, &overrides).map_err(theme_error)?;
                if let Value::Object(map) = &mut cfg {
                    map.insert("themeVariables".to_string(), tv.to_json());
                }
            }
            // A registered theme the helper lacks is reported by the
            // appearance check, with the variables left as merged.
        }
    }
    Ok(cfg)
}

/// `['secure', ...siteConfig.secure]`.
fn secure_keys(site_config: &Value) -> Vec<String> {
    site_config
        .get("secure")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Replaces values whose JSON type differs from the default's, so a
/// directive like `padding: "big"` falls back to the default instead of
/// failing the typed deserialization. mermaid would carry the string into
/// arithmetic and render garbage; the default is the closest sane reading.
fn coerce_types(value: &mut Value, defaults: &Value) {
    let (Value::Object(map), Value::Object(d)) = (value, defaults) else {
        return;
    };
    for (k, dv) in d {
        let Some(v) = map.get_mut(k) else {
            continue;
        };
        match (&mut *v, dv) {
            (Value::Object(_), Value::Object(_)) => coerce_types(v, dv),
            (Value::Null, _) => {}
            (Value::String(_), Value::String(_))
            | (Value::Number(_), Value::Number(_))
            | (Value::Bool(_), Value::Bool(_))
            | (Value::Array(_), Value::Array(_)) => {}
            (_, Value::Null) => {}
            _ => {
                log::warn!(
                    "config {k}: expected {}, got {v}; using the default",
                    type_name(dv)
                );
                *v = dv.clone();
            }
        }
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// The typed config from the effective JSON; `themeVariables` is left out.
pub fn typed_config(raw: &Value) -> Result<(MermaidConfig, Value), HelperError> {
    let mut without_tv = raw.clone();
    if let Value::Object(map) = &mut without_tv {
        map.remove("themeVariables");
    }
    coerce_types(&mut without_tv, &defaults::default_config_value());
    let config: MermaidConfig = serde_json::from_value(without_tv.clone())
        .map_err(|e| HelperError::internal(format!("config: {e}")))?;
    Ok((config, without_tv))
}

/// The full config path for one diagram: preprocess, detect, apply the
/// directive, check the appearance, and type the result.
pub fn resolve(source: &str, base: &BaseConfig) -> Result<Resolved, HelperError> {
    let site = site(base)?;
    let preprocessed = preprocess::preprocess(source).map_err(preprocess_error)?;
    let diagram_type = detect::detect_type(&preprocessed.code).map_err(detect_error)?;
    if diagram_type == FRONT_MATTER_TYPE {
        return Err(HelperError::ParseError {
            line: 1,
            column: 0,
            diagram_type: FRONT_MATTER_TYPE.to_string(),
            message: FRONT_MATTER_ERROR.to_string(),
        });
    }
    let config_key = detect::config_key(diagram_type).to_string();
    let requested_theme = preprocessed.config.get("theme").cloned();
    let cfg = apply_directive(&site, preprocessed.config.clone(), Some(&config_key))?;

    let mut appearance = Appearance::of(&cfg);
    if diagram_type == "flowchart-elk" {
        // The `flowchart-elk` keyword asks for ELK by name; mermaid's
        // detector sets `layout: 'elk'` for it.
        appearance.layout = "elk".to_string();
    }
    appearance::check_supported(&appearance, requested_theme.as_ref())?;

    let theme_variables = match cfg.get("themeVariables") {
        Some(Value::Object(map)) => ThemeVariables::from_json(map),
        _ => ThemeVariables::default(),
    };
    let (config, raw) = typed_config(&cfg)?;
    Ok(Resolved {
        diagram_type,
        config_key,
        config,
        raw,
        theme_variables,
        appearance,
        preprocessed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> BaseConfig {
        BaseConfig::new("default")
    }

    #[test]
    fn base_config_wins_over_per_diagram_defaults() {
        let r = resolve("graph TD\n  A-->B", &base()).unwrap();
        assert_eq!(r.diagram_type, "flowchart-v2");
        assert_eq!(r.config_key, "flowchart");
        assert_eq!(r.appearance.theme, "default");
        assert_eq!(r.appearance.look, "classic");
        assert_eq!(r.appearance.layout, "dagre");
        assert_eq!(r.config.flowchart.look.as_deref(), Some("classic"));
        assert_eq!(r.config.flowchart.theme.as_deref(), Some("default"));
        assert_eq!(*r.config.hand_drawn_seed, 1.0);
        assert_eq!(r.theme_variables.str("primaryColor"), Some("#ECECFF"));
        // `state.layout: elk` in the schema never wins against the base.
        let s = resolve("stateDiagram-v2\n  [*] --> A", &base()).unwrap();
        assert_eq!(s.appearance.layout, "dagre");
        assert_eq!(s.config.state.layout.as_deref(), Some("dagre"));
    }

    #[test]
    fn dark_base_theme_and_directive_theme() {
        let r = resolve("graph TD\n  A-->B", &BaseConfig::new("dark")).unwrap();
        assert_eq!(r.theme_variables.str("primaryColor"), Some("#1f2020"));
        let r = resolve(
            "%%{init: {'theme': 'forest'}}%%\ngraph TD\n  A-->B",
            &BaseConfig::new("dark"),
        )
        .unwrap();
        assert_eq!(r.appearance.theme, "forest");
        assert_eq!(r.theme_variables.str("primaryColor"), Some("#cde498"));
    }

    #[test]
    fn unsupported_look_layout_and_theme_via_directive_and_front_matter() {
        let cases = [
            (
                "%%{init: {'look': 'neo'}}%%\ngraph TD\n  A-->B",
                "look: neo",
            ),
            (
                "%%{init: {'look': 'handDrawn'}}%%\ngraph TD\n  A-->B",
                "look: handDrawn",
            ),
            (
                "%%{init: {'layout': 'elk'}}%%\ngraph TD\n  A-->B",
                "layout: elk",
            ),
            (
                "%%{init: {'theme': 'redux-color'}}%%\ngraph TD\n  A-->B",
                "theme: redux-color",
            ),
            (
                "---\nconfig:\n  look: neo\n---\ngraph TD\n  A-->B",
                "look: neo",
            ),
            (
                "---\nconfig:\n  layout: elk\n---\ngraph TD\n  A-->B",
                "layout: elk",
            ),
            (
                "---\nconfig:\n  theme: redux-color\n---\ngraph TD\n  A-->B",
                "theme: redux-color",
            ),
            (
                "%%{init: {'flowchart': {'look': 'neo'}}}%%\ngraph TD\n  A-->B",
                "look: neo",
            ),
            (
                "%%{init: {'theme': 'nope'}}%%\ngraph TD\n  A-->B",
                "theme: nope",
            ),
            ("flowchart-elk TD\n  A-->B", "layout: elk"),
        ];
        for (text, feature) in cases {
            let err = resolve(text, &base()).unwrap_err();
            match err {
                HelperError::UnsupportedFeature { feature: f } => assert_eq!(f, feature, "{text}"),
                other => panic!("{text}: {other:?}"),
            }
        }
    }

    #[test]
    fn unclosed_front_matter_and_unknown_text_are_parse_errors() {
        let err = resolve("---\ntitle: x\n\ngraph TD\n  A-->B", &base()).unwrap_err();
        match err {
            HelperError::ParseError {
                message,
                diagram_type,
                ..
            } => {
                assert_eq!(message, FRONT_MATTER_ERROR);
                assert_eq!(diagram_type, "---");
            }
            other => panic!("{other:?}"),
        }
        let err = resolve("not a diagram", &base()).unwrap_err();
        match err {
            HelperError::ParseError { message, .. } => {
                assert!(message.starts_with(
                    "No diagram type detected matching given configuration for text: "
                ))
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn theme_variables_without_a_theme_switch_are_merged_not_derived() {
        let r = resolve(
            "%%{init: {'themeVariables': {'primaryColor': '#00ff00'}}}%%\ngraph TD\n  A-->B",
            &base(),
        )
        .unwrap();
        assert_eq!(r.theme_variables.str("primaryColor"), Some("#00ff00"));
        // Derived from the default theme's `#ECECFF`, untouched.
        assert_eq!(r.theme_variables.str("nodeBkg"), Some("#ECECFF"));
    }

    #[test]
    fn secure_keys_and_unknown_keys_are_dropped() {
        let r = resolve(
            "%%{init: {'maxTextSize': 1, 'securityLevel': 'loose', 'bogus': 1, 'gantt': {'barHeight': 33, 'nope': 2}}}%%\ngantt\n  title x",
            &base(),
        )
        .unwrap();
        assert_eq!(*r.config.max_text_size, 50000.0);
        assert_eq!(r.config.security_level, "strict");
        assert_eq!(*r.config.gantt.bar_height, 33.0);
        assert!(r.raw.get("bogus").is_none());
        assert!(r.raw["gantt"].get("nope").is_none());
    }

    #[test]
    fn wrong_typed_values_fall_back_to_defaults() {
        let r = resolve(
            "%%{init: {'flowchart': {'nodeSpacing': 'big', 'curve': 'linear'}}}%%\ngraph TD\n  A-->B",
            &base(),
        )
        .unwrap();
        assert_eq!(*r.config.flowchart.node_spacing, 50.0);
        assert_eq!(r.config.flowchart.curve, "linear");
    }
}
