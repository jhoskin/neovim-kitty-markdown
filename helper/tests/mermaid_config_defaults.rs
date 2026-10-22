//! The Rust config defaults against the oracle dumps in
//! `tests/goldens/themes`: `default-config.json` is `mermaidAPI.defaultConfig`
//! (the schema defaults), `site-config.json` is `getConfig()` after
//! `initialize` with the base config (R-174), and `config-keys.json` is the
//! `configKeys` set that `sanitizeDirective` filters directives through.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use kitty_markdown_helper::mermaid::config::merge::config_keys;
use kitty_markdown_helper::mermaid::config::{BaseConfig, MermaidConfig, site};

fn golden(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/goldens/themes")
        .join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).expect("golden is JSON")
}

/// Lists every path where two JSON values differ.
fn diff(path: &str, a: &Value, b: &Value, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in x {
                match y.get(k) {
                    Some(w) => diff(&format!("{path}.{k}"), v, w, out),
                    None => out.push(format!("{path}.{k}: only in ours ({v})")),
                }
            }
            for k in y.keys() {
                if !x.contains_key(k) {
                    out.push(format!("{path}.{k}: only in oracle ({})", y[k]));
                }
            }
        }
        _ if a != b => out.push(format!("{path}: ours {a}, oracle {b}")),
        _ => {}
    }
}

fn assert_same(label: &str, ours: &Value, oracle: &Value) {
    let mut problems = Vec::new();
    diff("", ours, oracle, &mut problems);
    assert!(
        problems.is_empty(),
        "{label}: {} differences\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn defaults_match_default_config_json() {
    let ours = MermaidConfig::default().to_value();
    assert_same("defaultConfig", &ours, &golden("default-config.json"));
}

#[test]
fn site_config_matches_site_config_json() {
    let site = site(&BaseConfig::new("default")).expect("site config");
    let mut ours = site.config.clone();
    if let Value::Object(map) = &mut ours {
        map.remove("themeVariables");
    }
    assert_same("siteConfig", &ours, &golden("site-config.json"));
}

#[test]
fn default_round_trips_through_the_typed_struct() {
    let value = MermaidConfig::default().to_value();
    let back: MermaidConfig = serde_json::from_value(value.clone()).expect("deserialize");
    assert_eq!(back.to_value(), value);
}

#[test]
fn config_keys_match_config_keys_json() {
    let oracle: BTreeSet<String> = match golden("config-keys.json") {
        Value::Array(items) => items
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        other => panic!("expected an array, got {other}"),
    };
    let ours: BTreeSet<String> = config_keys().iter().map(|k| k.to_string()).collect();
    let missing: Vec<_> = oracle.difference(&ours).collect();
    let extra: Vec<_> = ours.difference(&oracle).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "missing {missing:?}\nextra {extra:?}"
    );
}
