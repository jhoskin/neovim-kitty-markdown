//! The full config path (preprocess, detect, directive, theme) against the
//! oracle's `parse-cases.json` from `dump.mjs`: for each diagram text,
//! mermaid 12.2.0's `getConfig()` after `parse(text)` with the base config
//! (R-102, R-103, R-174).
//!
//! `parse` leaves the diagram config scope in its `finally`, so the dump
//! holds the merged config without `resolveAppearance`; the section-level
//! `theme`, `look`, and `layout` keys are the only ones that differ from the
//! in-render state and are skipped. Everything else, theme variables
//! included, must match byte for byte.

use std::fs;
use std::path::PathBuf;

use serde_json::{Map, Value};

use kitty_markdown_helper::mermaid::config::{BaseConfig, Resolved, resolve};

fn cases() -> Map<String, Value> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/themes/parse-cases.json");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    match serde_json::from_str(&text).expect("golden is JSON") {
        Value::Object(map) => map,
        other => panic!("expected an object, got {other}"),
    }
}

const APPEARANCE_KEYS: [&str; 3] = ["theme", "look", "layout"];

/// Section-level appearance keys reflect the scope reset in the dump, not
/// the render; see the module docs.
fn skipped(path: &str, key: &str) -> bool {
    path.matches('.').count() == 1 && APPEARANCE_KEYS.contains(&key)
}

fn diff(path: &str, ours: &Value, oracle: &Value, out: &mut Vec<String>) {
    match (ours, oracle) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in x {
                if skipped(path, k) {
                    continue;
                }
                match y.get(k) {
                    Some(w) => diff(&format!("{path}.{k}"), v, w, out),
                    None => out.push(format!("{path}.{k}: only in ours ({v})")),
                }
            }
            for k in y.keys() {
                if !x.contains_key(k) && !skipped(path, k) {
                    out.push(format!("{path}.{k}: only in oracle ({})", y[k]));
                }
            }
        }
        _ if ours != oracle => out.push(format!("{path}: ours {ours}, oracle {oracle}")),
        _ => {}
    }
}

fn check(name: &str, case: &Value, resolved: &Resolved) {
    let mut problems = Vec::new();
    assert_eq!(
        resolved.diagram_type,
        case["diagramType"].as_str().expect("diagramType"),
        "{name}: diagram type"
    );
    diff("", &resolved.raw, &case["config"], &mut problems);
    diff(
        ".themeVariables",
        &resolved.theme_variables.to_json(),
        &case["themeVariables"],
        &mut problems,
    );
    assert!(
        problems.is_empty(),
        "{name}: {} differences\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn parse_cases_match_the_oracle() {
    let base = BaseConfig::new("default");
    let mut checked = 0;
    for (name, case) in &cases() {
        let text = case["text"].as_str().expect("text");
        let resolved = resolve(text, &base).unwrap_or_else(|e| panic!("{name}: {e}"));
        check(name, case, &resolved);
        checked += 1;
    }
    assert!(checked >= 15, "only {checked} cases");
}

#[test]
fn front_matter_title_reaches_the_result() {
    let r = resolve(
        "---\ntitle: Hello\n---\ngraph TD\n  A-->B\n",
        &BaseConfig::new("default"),
    )
    .expect("resolve");
    assert_eq!(r.title(), Some("Hello"));
    assert_eq!(r.code(), "graph TD\n  A-->B\n");
}
