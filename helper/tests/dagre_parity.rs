//! Parity of the dagre port against dagre-d3-es 7.0.14.
//!
//! `helper/oracle/dagre-dump.mjs` lays out each graph in a fresh node
//! process and writes `helper/tests/goldens/dagre/<name>.json`. This test
//! rebuilds the graph, resets the dummy id counter the way a fresh process
//! does, runs the port, and compares every node position, edge point, label
//! position, and the graph size to 1e-6. Point counts must match exactly.

use std::fs;
use std::path::{Path, PathBuf};

use kitty_markdown_helper::mermaid::layout::dagre::graphlib::EdgeKey;
use kitty_markdown_helper::mermaid::layout::dagre::util::new_layout_graph;
use kitty_markdown_helper::mermaid::layout::dagre::{
    EdgeLabel, GraphLabel, LayoutGraph, NodeLabel, layout, reset_unique_ids,
};
use serde_json::Value;

const TOLERANCE: f64 = 1e-6;

fn goldens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/dagre")
}

fn number(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn string(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn build_graph(spec: &Value) -> LayoutGraph {
    let mut g = new_layout_graph();
    let options = &spec["options"];
    let defaults = GraphLabel::default();
    g.set_graph(GraphLabel {
        rankdir: string(options, "rankdir").unwrap_or(defaults.rankdir),
        align: string(options, "align"),
        acyclicer: string(options, "acyclicer"),
        ranker: string(options, "ranker"),
        nodesep: number(options, "nodesep").unwrap_or(defaults.nodesep),
        edgesep: number(options, "edgesep").unwrap_or(defaults.edgesep),
        ranksep: number(options, "ranksep").unwrap_or(defaults.ranksep),
        marginx: number(options, "marginx").unwrap_or(defaults.marginx),
        marginy: number(options, "marginy").unwrap_or(defaults.marginy),
        ..GraphLabel::default()
    });

    let nodes = spec["nodes"].as_array().cloned().unwrap_or_default();
    for n in &nodes {
        let id = string(n, "id").unwrap_or_default();
        g.set_node(
            &id,
            NodeLabel {
                width: number(n, "width").unwrap_or(0.0),
                height: number(n, "height").unwrap_or(0.0),
                ..NodeLabel::default()
            },
        );
    }
    for n in &nodes {
        if let Some(parent) = string(n, "parent") {
            let id = string(n, "id").unwrap_or_default();
            g.set_parent(&id, Some(&parent));
        }
    }

    let defaults = EdgeLabel::default();
    for e in spec["edges"].as_array().cloned().unwrap_or_default() {
        let key = EdgeKey::named(
            &string(&e, "v").unwrap_or_default(),
            &string(&e, "w").unwrap_or_default(),
            string(&e, "name").as_deref(),
        );
        g.set_edge(
            &key,
            EdgeLabel {
                minlen: number(&e, "minlen").unwrap_or(defaults.minlen),
                weight: number(&e, "weight").unwrap_or(defaults.weight),
                width: number(&e, "width").unwrap_or(defaults.width),
                height: number(&e, "height").unwrap_or(defaults.height),
                labeloffset: number(&e, "labeloffset").unwrap_or(defaults.labeloffset),
                labelpos: string(&e, "labelpos").unwrap_or(defaults.labelpos.clone()),
                ..EdgeLabel::default()
            },
        );
    }
    g
}

/// Compares one value and records the largest deviation seen.
fn check(
    name: &str,
    what: &str,
    expected: f64,
    actual: f64,
    max_dev: &mut f64,
    failures: &mut Vec<String>,
) {
    let dev = (expected - actual).abs();
    if dev.is_nan() || dev > TOLERANCE {
        failures.push(format!("{name}: {what}: expected {expected}, got {actual}"));
    } else if dev > *max_dev {
        *max_dev = dev;
    }
}

fn compare(
    name: &str,
    spec: &Value,
    g: &LayoutGraph,
    max_dev: &mut f64,
    failures: &mut Vec<String>,
) {
    let out = &spec["out"];

    let nodes = out["nodes"].as_object().cloned().unwrap_or_default();
    assert_eq!(nodes.len(), g.node_count(), "{name}: node count");
    for (id, expected) in &nodes {
        let node = g.node(id);
        check(
            name,
            &format!("node {id} x"),
            number(expected, "x").unwrap_or(f64::NAN),
            node.x,
            max_dev,
            failures,
        );
        check(
            name,
            &format!("node {id} y"),
            number(expected, "y").unwrap_or(f64::NAN),
            node.y,
            max_dev,
            failures,
        );
        if let Some(width) = number(expected, "width") {
            check(
                name,
                &format!("node {id} width"),
                width,
                node.width,
                max_dev,
                failures,
            );
        }
        if let Some(height) = number(expected, "height") {
            check(
                name,
                &format!("node {id} height"),
                height,
                node.height,
                max_dev,
                failures,
            );
        }
    }

    let edges = out["edges"].as_object().cloned().unwrap_or_default();
    assert_eq!(edges.len(), g.edge_count(), "{name}: edge count");
    for (key, expected) in &edges {
        let mut parts = key.split('\u{1}');
        let v = parts.next().unwrap_or_default();
        let w = parts.next().unwrap_or_default();
        let edge_name = parts.next().filter(|n| !n.is_empty());
        let edge_key = EdgeKey::named(v, w, edge_name);
        let Some(edge) = g.try_edge(&edge_key) else {
            failures.push(format!("{name}: edge {v} -> {w} ({edge_name:?}) missing"));
            continue;
        };
        let points = expected["points"].as_array().cloned().unwrap_or_default();
        if points.len() != edge.points.len() {
            failures.push(format!(
                "{name}: edge {v} -> {w} ({edge_name:?}): expected {} points, got {}",
                points.len(),
                edge.points.len()
            ));
            continue;
        }
        for (i, (p, q)) in points.iter().zip(&edge.points).enumerate() {
            let what = format!("edge {v} -> {w} ({edge_name:?}) point {i}");
            check(
                name,
                &format!("{what} x"),
                number(p, "x").unwrap_or(f64::NAN),
                q.x,
                max_dev,
                failures,
            );
            check(
                name,
                &format!("{what} y"),
                number(p, "y").unwrap_or(f64::NAN),
                q.y,
                max_dev,
                failures,
            );
        }
        match (number(expected, "x"), edge.x) {
            (Some(x), Some(ax)) => {
                check(
                    name,
                    &format!("edge {v} -> {w} label x"),
                    x,
                    ax,
                    max_dev,
                    failures,
                );
                check(
                    name,
                    &format!("edge {v} -> {w} label y"),
                    number(expected, "y").unwrap_or(f64::NAN),
                    edge.y.unwrap_or(f64::NAN),
                    max_dev,
                    failures,
                );
            }
            (None, None) => {}
            (expected_x, actual_x) => failures.push(format!(
                "{name}: edge {v} -> {w} label x: expected {expected_x:?}, got {actual_x:?}"
            )),
        }
    }

    let graph = &out["graph"];
    check(
        name,
        "graph width",
        number(graph, "width").unwrap_or(f64::NAN),
        g.graph().width,
        max_dev,
        failures,
    );
    check(
        name,
        "graph height",
        number(graph, "height").unwrap_or(f64::NAN),
        g.graph().height,
        max_dev,
        failures,
    );
}

#[test]
fn dagre_matches_the_dagre_d3_es_goldens() {
    let dir = goldens_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        eprintln!(
            "dagre goldens absent at {}; run node helper/oracle/dagre-dump.mjs",
            dir.display()
        );
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no goldens in {}", dir.display());

    let mut failures = Vec::new();
    let mut max_dev: f64 = 0.0;
    let mut count = 0;
    for path in &paths {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();
        let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let spec: Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut g = build_graph(&spec);
        reset_unique_ids();
        if let Err(e) = layout(&mut g) {
            failures.push(format!("{name}: layout failed: {e}"));
            continue;
        }
        compare(&name, &spec, &g, &mut max_dev, &mut failures);
        count += 1;
    }

    eprintln!("dagre parity: {count} graphs, max deviation {max_dev:e}");
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures
            .iter()
            .take(60)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
