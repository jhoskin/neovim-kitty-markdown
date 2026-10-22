//! Theme variables byte for byte against the oracle dumps in
//! `tests/goldens/themes/<theme>.json` and `<theme>-override.json`
//! (R-099, R-100, R-104). The dumps come from `dump.mjs`: mermaid 12.2.0's
//! `getConfig().themeVariables` after `initialize({ theme })`, which runs
//! `getThemeVariables` twice (once in `initialize`, once in `setSiteConfig`
//! with the first result as overrides). The Rust side does the same.

use std::fs;
use std::path::PathBuf;

use serde_json::{Map, Value};

use kitty_markdown_helper::mermaid::theme::{self, BUILTIN_THEMES, ThemeVariables};

fn goldens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/themes")
}

fn load(name: &str) -> Map<String, Value> {
    let path = goldens().join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    match serde_json::from_str::<Value>(&text).expect("golden is JSON") {
        Value::Object(map) => map,
        other => panic!("{name}: expected an object, got {other}"),
    }
}

/// `initialize` computes the theme from the user's `themeVariables`, then
/// `setSiteConfig` recomputes it with that full result as the overrides.
fn site_theme(name: &str, overrides: Option<&Map<String, Value>>) -> ThemeVariables {
    let first = theme::resolve(name, overrides).expect("first pass");
    let as_overrides = match first.to_json() {
        Value::Object(map) => map,
        _ => unreachable!(),
    };
    theme::resolve(name, Some(&as_overrides)).expect("second pass")
}

fn assert_same(name: &str, expected: &Map<String, Value>, actual: &ThemeVariables) {
    let actual = match actual.to_json() {
        Value::Object(map) => map,
        _ => unreachable!(),
    };
    let mut problems = Vec::new();
    for (k, want) in expected {
        match actual.get(k) {
            None => problems.push(format!("{k}: missing (oracle {want})")),
            Some(got) if got != want => problems.push(format!("{k}: got {got}, oracle {want}")),
            _ => {}
        }
    }
    for k in actual.keys() {
        if !expected.contains_key(k) {
            problems.push(format!("{k}: extra (oracle has no such variable)"));
        }
    }
    assert!(
        problems.is_empty(),
        "{name}: {} differences\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn builtin_themes_match_the_oracle() {
    for name in BUILTIN_THEMES {
        let expected = load(&format!("{name}.json"));
        assert_same(name, &expected, &site_theme(name, None));
    }
}

#[test]
fn builtin_themes_with_overrides_match_the_oracle() {
    let mut overrides = Map::new();
    overrides.insert("primaryColor".into(), Value::from("#ff0000"));
    overrides.insert("fontFamily".into(), Value::from("Arial"));
    for name in BUILTIN_THEMES {
        let expected = load(&format!("{name}-override.json"));
        assert_same(
            &format!("{name}-override"),
            &expected,
            &site_theme(name, Some(&overrides)),
        );
    }
}

#[test]
fn a_single_pass_equals_the_double_pass() {
    // `updateCurrentConfig` runs `getThemeVariables` once when a directive
    // switches the theme; the result must not depend on the pass count.
    for name in BUILTIN_THEMES {
        let once = theme::resolve(name, None).expect("single pass").to_json();
        let twice = site_theme(name, None).to_json();
        assert_eq!(once, twice, "{name}");
    }
}
