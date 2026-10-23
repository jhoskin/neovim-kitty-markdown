//! Parity of the fcose port against cytoscape-fcose 2.2.0.
//!
//! `helper/tests/goldens/architecture/fcose/dump.mjs` runs every
//! `<name>.spec.json` there through cytoscape and fcose in node, the way
//! mermaid's architecture renderer does (two seeded runs from the origin),
//! and writes `<name>.expected.json` with the centre fcose relocated to and
//! every leaf position after each run. This test replays both runs with
//! the same seed and centres and compares every position to 1e-6.

use std::fs;
use std::path::{Path, PathBuf};

use kitty_markdown_helper::mermaid::layout::fcose::{
    self, AlignmentConstraint, Edge, Node, Options, RelativePlacement,
};
use serde_json::Value;

const TOLERANCE: f64 = 1e-6;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/architecture/fcose")
}

/// `withSeededRandom` of mermaid's `architectureSeed.ts`.
fn seeded(seed: u32) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state = state.wrapping_add(1_831_565_813);
        let mut t = state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

fn strings(v: &Value) -> Vec<Vec<String>> {
    v.as_array()
        .map(|groups| {
            groups
                .iter()
                .map(|g| {
                    g.as_array()
                        .map(|ids| {
                            ids.iter()
                                .filter_map(|id| id.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default()
}

fn build(spec: &Value) -> (Vec<Node>, Vec<Edge>, Options) {
    let specs = spec["nodes"].as_array().cloned().unwrap_or_default();
    let index = |id: &str| specs.iter().position(|n| n["id"].as_str() == Some(id));
    let padding = spec["padding"].as_f64().unwrap_or(0.0);
    let nodes: Vec<Node> = specs
        .iter()
        .map(|n| {
            let group = n["group"].as_bool().unwrap_or(false);
            Node {
                id: n["id"].as_str().unwrap_or_default().to_string(),
                parent: n["parent"].as_str().and_then(index),
                x: 0.0,
                y: 0.0,
                width: n["width"].as_f64().unwrap_or(0.0),
                height: n["height"].as_f64().unwrap_or(0.0),
                padding: if group { padding } else { 0.0 },
            }
        })
        .collect();
    let edges: Vec<Edge> = spec["edges"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|e| {
            Some(Edge {
                source: index(e["source"].as_str()?)?,
                target: index(e["target"].as_str()?)?,
                ideal_length: e["idealLength"].as_f64()?,
                elasticity: e["elasticity"].as_f64()?,
            })
        })
        .collect();
    let alignment = &spec["alignmentConstraint"];
    let relative = spec["relativePlacementConstraint"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|c| {
            let gap = c["gap"].as_f64().unwrap_or(f64::NAN);
            let s = |k: &str| c[k].as_str().map(str::to_string);
            match (s("left"), s("right"), s("top"), s("bottom")) {
                (Some(left), Some(right), _, _) => {
                    Some(RelativePlacement::Horizontal { left, right, gap })
                }
                (_, _, Some(top), Some(bottom)) => {
                    Some(RelativePlacement::Vertical { top, bottom, gap })
                }
                _ => None,
            }
        })
        .collect();
    let options = Options {
        num_iter: spec["numIter"].as_f64().unwrap_or(2500.0),
        alignment: alignment.is_object().then(|| AlignmentConstraint {
            vertical: strings(&alignment["vertical"]),
            horizontal: strings(&alignment["horizontal"]),
        }),
        relative,
        ..Options::default()
    };
    (nodes, edges, options)
}

fn check(name: &str, spec: &Value, expected: &Value) -> Vec<String> {
    let (mut nodes, edges, options) = build(spec);
    let seed = spec["seed"].as_u64().unwrap_or(1) as u32;
    let mut problems = Vec::new();
    let runs = expected["runs"].as_array().cloned().unwrap_or_default();
    if runs.len() != 2 {
        problems.push(format!("{name}: {} runs, want 2", runs.len()));
    }
    for (r, run) in runs.iter().enumerate() {
        let mut compared = 0;
        let center = (
            run["center"][0].as_f64().unwrap_or(0.0),
            run["center"][1].as_f64().unwrap_or(0.0),
        );
        let mut random = seeded(seed);
        let result = fcose::run(&nodes, &edges, &options, Some(center), &mut random)
            .unwrap_or_else(|e| panic!("{name} run {r}: {e}"));
        for (i, node) in nodes.iter_mut().enumerate() {
            let Some(want) = run["positions"].get(&node.id) else {
                continue;
            };
            let (x, y) = result[i];
            let (wx, wy) = (
                want[0].as_f64().unwrap_or(f64::NAN),
                want[1].as_f64().unwrap_or(f64::NAN),
            );
            if (x - wx).abs() > TOLERANCE || (y - wy).abs() > TOLERANCE {
                problems.push(format!(
                    "{name} run {r} {}: got ({x}, {y}), want ({wx}, {wy})",
                    node.id
                ));
            }
            node.x = x;
            node.y = y;
            compared += 1;
        }
        if compared == 0 {
            problems.push(format!("{name} run {r}: no positions to compare"));
        }
    }
    problems
}

#[test]
fn fcose_matches_cytoscape_fcose() {
    let dir = fixtures_dir();
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter_map(|f| f.strip_suffix(".spec.json").map(str::to_string))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no fcose specs in {}", dir.display());
    let mut problems = Vec::new();
    for name in &names {
        let read = |suffix: &str| -> Value {
            let path = dir.join(format!("{name}{suffix}"));
            let text =
                fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        };
        problems.extend(check(name, &read(".spec.json"), &read(".expected.json")));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
