//! khroma port and the five built-in themes with themeVariables derivation.
//!
//! `resolve` is `theme[name].getThemeVariables(overrides)` from mermaid's
//! `themes/index.js` for the themes the helper ships (R-104). The six other
//! names mermaid 12.2.0 registers (`neo`, `neo-dark`, `redux`, `redux-dark`,
//! `redux-color`, `redux-dark-color`) are known so the config layer can tell
//! "a theme the helper does not render" (R-175) from "a name mermaid would
//! ignore".

pub mod base;
pub mod dark;
pub mod default;
pub mod forest;
pub mod khroma;
pub mod neutral;
pub mod vars;

use indexmap::IndexMap;
use serde_json::{Map, Value};

pub use vars::{ThemeError, Vars};

/// The themes the helper renders, in the order of mermaid's registry.
pub const BUILTIN_THEMES: &[&str] = &["base", "dark", "default", "forest", "neutral"];

/// Every name `themes/index.js` registers, built-in or not. `initialize`
/// and `resolveAppearance` accept exactly these (plus the `'null'`
/// sentinel).
pub const MERMAID_THEMES: &[&str] = &[
    "base",
    "dark",
    "default",
    "forest",
    "neutral",
    "neo",
    "neo-dark",
    "redux",
    "redux-dark",
    "redux-color",
    "redux-dark-color",
];

pub fn is_builtin(name: &str) -> bool {
    BUILTIN_THEMES.contains(&name)
}

pub fn is_mermaid_theme(name: &str) -> bool {
    MERMAID_THEMES.contains(&name)
}

/// The derived theme variables: every `this.<name>` of the mermaid `Theme`
/// object, in assignment order. Values are strings for colors and sizes,
/// numbers and booleans for the few non-color variables, objects for
/// `cynefin`, `radar`, `wardley`, `xyChart`, and `packet`, and `Null` where
/// mermaid holds `undefined`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThemeVariables {
    vars: IndexMap<String, Value>,
}

impl ThemeVariables {
    pub fn from_vars(vars: Vars) -> ThemeVariables {
        ThemeVariables {
            vars: vars.into_map(),
        }
    }

    /// From a JSON object, for variables that come back out of the config
    /// merge rather than from a theme.
    pub fn from_json(map: &Map<String, Value>) -> ThemeVariables {
        ThemeVariables {
            vars: map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        }
    }

    pub fn get(&self, name: &str) -> &Value {
        self.vars.get(name).unwrap_or(&Value::Null)
    }

    /// A string variable, or `None` when it is absent or not a string.
    pub fn str(&self, name: &str) -> Option<&str> {
        self.get(name).as_str()
    }

    /// A variable the SVG needs as a color, with a fallback for the odd
    /// override that is not a string.
    pub fn color(&self, name: &str, fallback: &str) -> String {
        self.str(name).unwrap_or(fallback).to_string()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.vars.iter()
    }

    pub fn len(&self) -> usize {
        self.vars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }

    /// The variables as `JSON.stringify` would print the mermaid object:
    /// keys whose value is `undefined` are left out.
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        for (k, v) in &self.vars {
            if !v.is_null() {
                map.insert(k.clone(), v.clone());
            }
        }
        Value::Object(map)
    }
}

/// `theme[name].getThemeVariables(overrides)` for a built-in theme.
/// `overrides` is the `themeVariables` object after mermaid's merging; the
/// theme code sees it exactly as JS does, so a number or an object override
/// behaves the same way.
pub fn resolve(
    name: &str,
    overrides: Option<&Map<String, Value>>,
) -> Result<ThemeVariables, ThemeError> {
    let vars = match name {
        "base" => base::get_theme_variables(overrides)?,
        "dark" => dark::get_theme_variables(overrides)?,
        "default" => default::get_theme_variables(overrides)?,
        "forest" => forest::get_theme_variables(overrides)?,
        "neutral" => neutral::get_theme_variables(overrides)?,
        other => return Err(ThemeError(format!("unknown theme: {other}"))),
    };
    Ok(ThemeVariables::from_vars(vars))
}

/// `getThemeVariables(value)` with the `themeVariables` value as mermaid
/// receives it: an object is the override map, anything else (including
/// `undefined`) means "no overrides" because `typeof overrides !== 'object'`
/// skips the override passes. A JSON array is an object in JS; its indices
/// become variables, which no renderer reads, so it is treated as empty.
pub fn resolve_value(name: &str, overrides: &Value) -> Result<ThemeVariables, ThemeError> {
    match overrides {
        Value::Object(map) => resolve(name, Some(map)),
        Value::Array(_) => resolve(name, Some(&Map::new())),
        _ => resolve(name, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_theme_resolves() {
        for name in BUILTIN_THEMES {
            let tv = resolve(name, None).unwrap();
            assert!(tv.str("primaryColor").is_some(), "{name}");
            assert!(tv.len() > 250, "{name}: {}", tv.len());
        }
    }

    #[test]
    fn override_wins_over_derived_value() {
        let mut o = Map::new();
        o.insert("primaryBorderColor".into(), Value::from("#123456"));
        let tv = resolve("base", Some(&o)).unwrap();
        assert_eq!(tv.str("primaryBorderColor"), Some("#123456"));
        assert_eq!(tv.str("nodeBorder"), Some("#123456"));
    }

    #[test]
    fn unknown_theme_is_an_error() {
        assert!(resolve("redux-color", None).is_err());
    }
}
