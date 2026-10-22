//! Config merging and sanitizing, ported from mermaid 12.2.0:
//! `assignWithDepth` (`assignWithDepth.ts`), `sanitize` (`config.ts`),
//! `sanitizeDirective`, `sanitizeCss`, and `sanitizeDictionaryConfig`
//! (`utils/sanitizeDirective.ts`), and the `configKeys` set of
//! `defaultConfig.ts`.
//!
//! JSON `null` is JS `null`; an absent key is `undefined`. The two differ in
//! `assignWithDepth`: `null` in the source is skipped, `null` in the target
//! is an object that a primitive does not overwrite.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use super::defaults;
use crate::mermaid::theme;

/// `typeof v === 'object'`, which is true for `null`.
fn is_js_object(v: &Value) -> bool {
    matches!(v, Value::Object(_) | Value::Array(_) | Value::Null)
}

/// `Array.prototype.includes` with SameValueZero: primitives by value,
/// objects by identity (never equal to a fresh value).
fn includes(arr: &[Value], v: &Value) -> bool {
    !matches!(v, Value::Object(_) | Value::Array(_)) && arr.contains(v)
}

/// `Object.assign(dst, src)` for the shapes that reach it: own enumerable
/// keys of an object or the indices of an array are copied onto `dst`.
fn object_assign(dst: &mut Map<String, Value>, src: &Value) {
    match src {
        Value::Object(s) => {
            for (k, v) in s {
                dst.insert(k.clone(), v.clone());
            }
        }
        Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                dst.insert(i.to_string(), v.clone());
            }
        }
        _ => {}
    }
}

/// `assignWithDepth(dst, src, { depth })`. `dst` is taken by value and
/// returned, since JS returns either the mutated `dst` or `src` itself.
/// `None` stands for `undefined`.
pub fn assign_with_depth_opt(dst: Option<Value>, src: &Value, depth: i32) -> Value {
    if let Value::Array(items) = src {
        match dst {
            Some(Value::Array(mut d)) => {
                for s in items {
                    if !includes(&d, s) {
                        d.push(s.clone());
                    }
                }
                return Value::Array(d);
            }
            other => {
                let mut acc = other;
                for s in items {
                    acc = Some(assign_with_depth_opt(acc, s, depth));
                }
                return acc.unwrap_or(Value::Null);
            }
        }
    }
    let dst_is_nullish = matches!(dst, None | Some(Value::Null));
    if dst_is_nullish || depth <= 0 {
        return match dst {
            Some(Value::Object(mut d)) if is_js_object(src) => {
                object_assign(&mut d, src);
                Value::Object(d)
            }
            Some(Value::Array(d)) if is_js_object(src) => {
                // `Object.assign` onto an array adds string keys that JSON
                // never prints, so the array is unchanged.
                Value::Array(d)
            }
            _ => src.clone(),
        };
    }
    let Some(mut dst) = dst else {
        return src.clone();
    };
    if src.is_null() || !is_js_object(src) {
        return dst;
    }
    let Value::Object(d) = &mut dst else {
        // An array target with an object source gains string keys JSON
        // never prints.
        return dst;
    };
    let entries: Vec<(String, &Value)> = match src {
        Value::Object(s) => s.iter().map(|(k, v)| (k.clone(), v)).collect(),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
        _ => Vec::new(),
    };
    for (key, src_value) in entries {
        if is_js_object(src_value) {
            if src_value.is_null() {
                continue;
            }
            let current = d.remove(&key);
            let current = match current {
                // `undefined` (absent) becomes `[]` or `{}` first.
                None => {
                    if src_value.is_array() {
                        Value::Array(Vec::new())
                    } else {
                        Value::Object(Map::new())
                    }
                }
                Some(v) => v,
            };
            if is_js_object(&current) {
                let merged = assign_with_depth_opt(Some(current), src_value, depth - 1);
                d.insert(key, merged);
            } else {
                d.insert(key, current);
            }
        } else {
            let keep = d.get(&key).is_some_and(is_js_object);
            if !keep {
                d.insert(key, src_value.clone());
            }
        }
    }
    dst
}

/// `assignWithDepth` with a defined target.
pub fn assign_with_depth(dst: Value, src: &Value, depth: i32) -> Value {
    assign_with_depth_opt(Some(dst), src, depth)
}

/// `assignWithDepth({}, src)`: a depth-2 copy.
pub fn copy_with_depth(src: &Value) -> Value {
    assign_with_depth(Value::Object(Map::new()), src, 2)
}

/// `sanitize(options)` in `config.ts`: strips the secure keys, `__` keys,
/// and string values that could carry markup, recursively.
pub fn sanitize(options: &mut Value, secure: &[String]) {
    let Value::Object(map) = options else {
        if let Value::Array(items) = options {
            for item in items.iter_mut() {
                if is_js_object(item) {
                    sanitize(item, secure);
                }
            }
        }
        return;
    };
    map.remove("secure");
    for key in secure {
        map.remove(key);
    }
    map.retain(|k, _| !k.starts_with("__"));
    let keys: Vec<String> = map.keys().cloned().collect();
    for key in keys {
        let remove = matches!(
            map.get(&key),
            Some(Value::String(s)) if s.contains('<') || s.contains('>') || s.contains("url(data:")
        );
        if remove {
            map.remove(&key);
            continue;
        }
        if let Some(v) = map.get_mut(&key) {
            if is_js_object(v) {
                sanitize(v, secure);
            }
        }
    }
}

/// `sanitizeCss`: an unbalanced brace pair is replaced with an error
/// comment, because the value ends up in a `<style>` element.
pub fn sanitize_css(text: &str) -> String {
    let mut start = 0usize;
    let mut end = 0usize;
    for c in text.chars() {
        if start < end {
            return "{ /* ERROR: Unbalanced CSS */ }".to_string();
        }
        if c == '{' {
            start += 1;
        } else if c == '}' {
            end += 1;
        }
    }
    if start != end {
        return "{ /* ERROR: Unbalanced CSS */ }".to_string();
    }
    text.to_string()
}

static NODE_COLORS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^#[0-9a-f]{3,8}$|^rgb\([0-9\s%,.]+\)$|^hsl\([0-9\s%,.]+\)$|^[a-z]+$")
        .expect("valid regex")
});
static ICON_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_-]+(?::[A-Za-z0-9_-]+)?$").expect("valid regex"));
static THEME_VARIABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r##"^[0-9 "#%(),.;A-Za-z]+$"##).expect("valid regex"));

/// `DICTIONARY_CONFIG_PATTERNS`: configs with user-defined keys whose
/// values are checked against a pattern instead.
fn dictionary_pattern(key: &str) -> Option<&'static Regex> {
    match key {
        "nodeColors" => Some(&NODE_COLORS_RE),
        "filenameIcons" | "extensionIcons" => Some(&ICON_RE),
        _ => None,
    }
}

fn suspicious_key(key: &str) -> bool {
    key.starts_with("__") || key.contains("proto") || key.contains("constr")
}

fn sanitize_dictionary_config(dict: &mut Value, pattern: &Regex) {
    let Value::Object(map) = dict else {
        return;
    };
    map.retain(|k, v| !suspicious_key(k) && matches!(v, Value::String(s) if pattern.is_match(s)));
}

/// `sanitizeDirective(args)`: drops keys outside `configKeys`, suspicious
/// keys, and `null` values; recurses into objects; runs `sanitizeCss` on
/// `themeCSS`, `fontFamily`, and `altFontFamily` keys; and blanks theme
/// variable strings outside `[\d "#%(),.;A-Za-z]`. A `sans-serif` in
/// `themeVariables.fontFamily` becomes `''` here, while the top-level
/// `fontFamily` keeps its hyphen.
pub fn sanitize_directive(args: &mut Value) {
    match args {
        Value::Array(items) => {
            for item in items.iter_mut() {
                sanitize_directive(item);
            }
        }
        Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                let drop = suspicious_key(&key)
                    || !config_keys().contains(key.as_str())
                    || matches!(map.get(&key), None | Some(Value::Null));
                if drop {
                    map.remove(&key);
                    continue;
                }
                let Some(value) = map.get_mut(&key) else {
                    continue;
                };
                if is_js_object(value) {
                    match dictionary_pattern(&key) {
                        Some(pattern) => sanitize_dictionary_config(value, pattern),
                        None => sanitize_directive(value),
                    }
                    continue;
                }
                for css_key in ["themeCSS", "fontFamily", "altFontFamily"] {
                    if key.contains(css_key) {
                        if let Value::String(s) = value {
                            *s = sanitize_css(s);
                        }
                    }
                }
            }
            if let Some(Value::Object(tv)) = map.get_mut("themeVariables") {
                for v in tv.values_mut() {
                    if let Value::String(s) = v {
                        if !THEME_VARIABLE_RE.is_match(s) {
                            *s = String::new();
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// `keyify`: every key at every depth of an object, arrays skipped (the
/// key of an array value is skipped too), function- and undefined-valued
/// keys included.
pub fn keyify(value: &Value, out: &mut HashSet<String>) {
    if let Value::Object(map) = value {
        for (k, v) in map {
            match v {
                Value::Array(_) => {}
                Value::Object(_) => {
                    out.insert(k.clone());
                    keyify(v, out);
                }
                _ => {
                    out.insert(k.clone());
                }
            }
        }
    }
}

/// `configKeys` in `defaultConfig.ts`: the keys of the default config,
/// including `themeVariables` and every default-theme variable, plus the
/// function- and `undefined`-valued keys that `JSON.stringify` would drop.
pub fn config_keys() -> &'static HashSet<&'static str> {
    static KEYS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
        let mut set = HashSet::new();
        keyify(&defaults::default_config_value(), &mut set);
        set.insert("themeVariables".to_string());
        if let Ok(vars) = theme::resolve("default", None) {
            keyify(&vars.to_json(), &mut set);
        }
        set.into_iter()
            .map(|k| &*Box::leak(k.into_boxed_str()))
            .chain(defaults::UNSERIALIZED_KEYS.iter().copied())
            .collect()
    });
    &KEYS
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn assign_with_depth_merges_two_levels_and_keeps_dissimilar_types() {
        let dst = json!({"foo": {"bar": "bar"}, "bar": "foo"});
        let src = json!({"foo": "foo", "bar": "bar"});
        assert_eq!(
            assign_with_depth(dst, &src, 2),
            json!({"foo": {"bar": "bar"}, "bar": "bar"})
        );
        let dst = json!({"a": {"b": {"c": 1, "d": 2}}});
        let src = json!({"a": {"b": {"c": 9}}});
        // Depth 2 reaches `a.b`, where `Object.assign` replaces keys but
        // keeps the ones the source lacks.
        assert_eq!(
            assign_with_depth(dst, &src, 2),
            json!({"a": {"b": {"c": 9, "d": 2}}})
        );
    }

    #[test]
    fn assign_with_depth_creates_missing_objects_and_skips_null() {
        let dst = json!({});
        let src = json!({"flowchart": {"padding": 1}, "x": null, "y": {"z": null}});
        assert_eq!(
            assign_with_depth(dst, &src, 2),
            json!({"flowchart": {"padding": 1}, "y": {}})
        );
    }

    #[test]
    fn assign_with_depth_arrays() {
        assert_eq!(
            assign_with_depth(json!({"a": [1, 2]}), &json!({"a": [2, 3]}), 2),
            json!({"a": [1, 2, 3]})
        );
        // An array source onto an object target applies each element.
        assert_eq!(
            assign_with_depth(json!({}), &json!([{"a": 1}, {"b": 2}]), 2),
            json!({"a": 1, "b": 2})
        );
        assert_eq!(assign_with_depth(json!({}), &json!(["str"]), 2), json!({}));
    }

    #[test]
    fn primitive_does_not_clobber_object_or_null() {
        let dst = json!({"a": {"x": 1}, "n": null});
        let src = json!({"a": 5, "n": 7});
        assert_eq!(
            assign_with_depth(dst, &src, 2),
            json!({"a": {"x": 1}, "n": null})
        );
    }

    #[test]
    fn sanitize_strips_secure_and_markup() {
        let mut v = json!({"secure": [], "maxTextSize": 1, "theme": "<b>", "__proto__": 1, "flowchart": {"curve": "a>b", "padding": 2}});
        sanitize(&mut v, &["maxTextSize".to_string()]);
        assert_eq!(v, json!({"flowchart": {"padding": 2}}));
    }

    #[test]
    fn sanitize_directive_drops_unknown_keys_and_blanks_bad_theme_variables() {
        let mut v = json!({
            "theme": "base",
            "bogus": 1,
            "gantt": {"barHeight": 3, "nope": 4, "displayMode": null},
            "themeVariables": {"primaryColor": "#ff0000", "fontFamily": "sans-serif", "lineColor": "<b>", "darkMode": true, "notAVariable": "x"},
            "fontFamily": "sans-serif",
            "themeCSS": "a { b }"
        });
        sanitize_directive(&mut v);
        assert_eq!(
            v,
            json!({
                "theme": "base",
                "gantt": {"barHeight": 3},
                "themeVariables": {"primaryColor": "#ff0000", "fontFamily": "", "lineColor": "", "darkMode": true},
                "fontFamily": "sans-serif",
                "themeCSS": "a { b }"
            })
        );
        let mut css = json!({"themeCSS": "a { b"});
        sanitize_directive(&mut css);
        assert_eq!(css["themeCSS"], "{ /* ERROR: Unbalanced CSS */ }");
    }

    #[test]
    fn dictionary_configs_check_values_not_keys() {
        let mut v =
            json!({"sankey": {"nodeColors": {"a": "#fff", "b": "not a color!", "__x": "#000"}}});
        sanitize_directive(&mut v);
        assert_eq!(v, json!({"sankey": {"nodeColors": {"a": "#fff"}}}));
    }

    #[test]
    fn config_keys_include_theme_variables_and_undefined_keys() {
        let keys = config_keys();
        assert!(keys.contains("primaryColor"));
        assert!(keys.contains("plotColorPalette"));
        assert!(keys.contains("useWidth"));
        assert!(keys.contains("messageFont"));
        assert!(keys.contains("nodeColors"));
        assert!(!keys.contains("secure"));
        assert!(!keys.contains("actorColours"));
    }
}
