//! `theme`, `look`, and `layout` after the merge: `resolveAppearance` and
//! `readAppearance` from mermaid 12.2.0's `config.ts`, then the helper's own
//! check that the result is something it renders (R-175).
//!
//! mermaid resolves each key from the first layer that has a usable value,
//! diagram section first, top level second: the directives and front
//! matter, then `initialize()`'s config (the base config), then the schema
//! defaults. With the base config always naming all three keys, the schema's
//! per-diagram `redux-color`/`neo`/`elk` defaults never win; only a diagram
//! that asks for them does.

use serde_json::Value;

use crate::error::HelperError;
use crate::mermaid::theme;

pub const APPEARANCE_KEYS: [&str; 3] = ["theme", "look", "layout"];

/// `LOOKS` in `config.ts`.
pub const LOOKS: [&str; 3] = ["classic", "handDrawn", "neo"];

/// The one look the helper renders.
pub const SUPPORTED_LOOK: &str = "classic";
/// The one layout the helper renders.
pub const SUPPORTED_LAYOUT: &str = "dagre";

/// `isUsableAppearance`: a theme must be registered (or the `'null'`
/// sentinel), a look must be one of `LOOKS`, a layout is anything.
fn is_usable(key: &str, value: &Value) -> bool {
    match key {
        "theme" => matches!(value, Value::String(s) if s == "null" || theme::is_mermaid_theme(s)),
        "look" => matches!(value, Value::String(s) if LOOKS.contains(&s.as_str())),
        _ => true,
    }
}

/// `readAppearance(layer, diagramConfigKey, key)`: the section's value, then
/// the top-level one, the first that is defined and usable.
fn read_appearance<'a>(layer: &'a Value, config_key: &str, key: &str) -> Option<&'a Value> {
    let section = layer.get(config_key).and_then(|s| s.get(key));
    let top = layer.get(key);
    [section, top]
        .into_iter()
        .flatten()
        .find(|v| !v.is_null() && is_usable(key, v))
}

/// `resolveAppearance(cfg, sumOfDirectives)`. `config_key` is the diagram's
/// config section; without one (no diagram scope) mermaid leaves `cfg` as
/// merged. `layers` is highest priority first: the directive sum, the
/// directives newest first, the `initialize()` delta, the defaults.
pub fn resolve_appearance(cfg: &mut Value, config_key: Option<&str>, layers: &[&Value]) {
    let Some(config_key) = config_key else {
        return;
    };
    for key in APPEARANCE_KEYS {
        let found = layers
            .iter()
            .find_map(|layer| read_appearance(layer, config_key, key).cloned());
        let Some(value) = found else {
            continue;
        };
        if let Value::Object(top) = cfg {
            top.insert(key.to_string(), value.clone());
            // Keep the section in step, so `cfg.flowchart.look` cannot
            // contradict `cfg.look`.
            if let Some(Value::Object(section)) = top.get_mut(config_key)
                && section.get(key).is_some_and(|v| !v.is_null())
            {
                section.insert(key.to_string(), value);
            }
        }
    }
}

/// The resolved appearance of a config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appearance {
    pub theme: String,
    pub look: String,
    pub layout: String,
}

fn string_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

impl Appearance {
    pub fn of(cfg: &Value) -> Appearance {
        let get = |k: &str| cfg.get(k).map(string_of).unwrap_or_default();
        Appearance {
            theme: get("theme"),
            look: get("look"),
            layout: get("layout"),
        }
    }
}

/// R-175: a `look` other than `classic`, a `layout` other than `dagre`, or
/// a theme outside the five built-in ones is `unsupported_feature`, spelled
/// `look: neo`, `layout: elk`, `theme: redux-color`. A theme name mermaid
/// does not register never gets here: `resolve_appearance` passes over it,
/// as `readAppearance` does, so the theme below it applies (R-234).
pub fn check_supported(resolved: &Appearance) -> Result<(), HelperError> {
    if !theme::is_builtin(&resolved.theme) {
        return Err(HelperError::UnsupportedFeature {
            feature: format!("theme: {}", resolved.theme),
        });
    }
    if resolved.look != SUPPORTED_LOOK {
        return Err(HelperError::UnsupportedFeature {
            feature: format!("look: {}", resolved.look),
        });
    }
    if resolved.layout != SUPPORTED_LAYOUT {
        return Err(HelperError::UnsupportedFeature {
            feature: format!("layout: {}", resolved.layout),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn section_value_wins_over_top_level_within_a_layer() {
        let mut cfg = json!({"theme": "default", "look": "classic", "layout": "dagre", "flowchart": {"theme": "redux-color", "look": "neo"}});
        let sum = json!({"flowchart": {"look": "handDrawn"}});
        let delta = json!({"theme": "default", "look": "classic", "layout": "dagre"});
        let defaults = json!({"theme": "default", "look": "classic", "layout": "elk", "flowchart": {"theme": "redux-color", "look": "neo"}});
        resolve_appearance(
            &mut cfg,
            Some("flowchart"),
            &[&sum, &sum, &delta, &defaults],
        );
        assert_eq!(cfg["look"], "handDrawn");
        assert_eq!(cfg["flowchart"]["look"], "handDrawn");
        assert_eq!(cfg["theme"], "default");
        assert_eq!(cfg["flowchart"]["theme"], "default");
        assert_eq!(cfg["layout"], "dagre");
    }

    #[test]
    fn unusable_values_are_passed_over() {
        let mut cfg = json!({"theme": "bogus", "look": "sketchy", "layout": "dagre", "pie": {}});
        let sum = json!({"theme": "bogus", "look": "sketchy"});
        let delta = json!({"theme": "forest", "look": "classic", "layout": "dagre"});
        resolve_appearance(&mut cfg, Some("pie"), &[&sum, &delta]);
        assert_eq!(cfg["theme"], "forest");
        assert_eq!(cfg["look"], "classic");
    }

    #[test]
    fn no_scope_leaves_the_merge_alone() {
        let mut cfg = json!({"theme": "bogus"});
        resolve_appearance(&mut cfg, None, &[&json!({"theme": "dark"})]);
        assert_eq!(cfg["theme"], "bogus");
    }

    #[test]
    fn unsupported_features_are_spelled_out() {
        let ok = Appearance {
            theme: "dark".into(),
            look: "classic".into(),
            layout: "dagre".into(),
        };
        assert!(check_supported(&ok).is_ok());
        let neo = Appearance {
            look: "neo".into(),
            ..ok.clone()
        };
        assert_eq!(
            check_supported(&neo).unwrap_err().to_string(),
            "unsupported feature: look: neo"
        );
        let elk = Appearance {
            layout: "elk".into(),
            ..ok.clone()
        };
        assert_eq!(
            check_supported(&elk).unwrap_err().to_string(),
            "unsupported feature: layout: elk"
        );
        let redux = Appearance {
            theme: "redux-color".into(),
            ..ok.clone()
        };
        assert_eq!(
            check_supported(&redux).unwrap_err().to_string(),
            "unsupported feature: theme: redux-color"
        );
    }
}
