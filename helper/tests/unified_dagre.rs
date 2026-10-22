//! The unified renderer end to end: real text metrics, the real dagre
//! port, and the theme, against the oracle's SVG for three flowcharts
//! (`helper/tests/goldens/unified/<case>.mmd` next to `<case>.svg`, made
//! with `mmdc -e svg -c helper/oracle/mermaid-config.json`). Node centres,
//! node sizes, edge paths, edge label positions, cluster rects, and the
//! content box are compared to 0.01 px.
//!
//! The `LayoutData` is built by hand the way `flowDb.getData()` would
//! build it for each source (mermaid 12.2.0 `diagrams/flowchart/flowDb.ts`),
//! so the flowchart package can reuse the recipe:
//!
//! - Nodes come in `getData` order: the subgraphs first (iterated from the
//!   last declared to the first), then the vertices in first-mention order.
//! - A vertex (`addNodeFromVertex`): `id`, `label` (the text, or the id
//!   when the vertex has no text), `labelType` (`text` for plain brackets),
//!   `labelStyle: ''`, `parentId` from the enclosing subgraph, `padding:
//!   flowchart.padding` (15), `cssClasses: 'default '` plus the vertex
//!   classes, `shape` from `getTypeFromVertex` (`squareRect` for `[...]`
//!   and for a bare id, `diamond` for `{...}`, `stadium` for `([...])`,
//!   ...), `minWidth: flowchart.minNodeWidth` (120), `domId: flowchart-
//!   <id>-<vertexCounter>`, `look: classic`. [`flow_node`] is that record.
//! - A subgraph: `id`, `label: title`, `labelType`, `labelStyle: ''`,
//!   `parentId`, `padding: 8`, `cssClasses` from the subgraph classes,
//!   `shape: 'rect'`, `dir` when the subgraph sets one, `isGroup: true`.
//!   [`group_node`] is that record.
//! - Edges come in declaration order with `id: L_<start>_<end>_<n>` where
//!   `n` counts the edges between that pair so far, `label: text`,
//!   `labelType`, `labelpos: 'c'`, `minlen: length` (the dash count, 1 for
//!   `-->`), `classes: 'edge-thickness-normal edge-pattern-solid
//!   flowchart-link'`, `arrowTypeStart: 'none'`, `arrowTypeEnd:
//!   'arrow_point'`, `thickness: normal`, `pattern: solid`, `curve:
//!   flowchart.curve` (`basis`). [`flow_edge`] is that record.
//! - `direction` is the graph's `TB`/`TD`/`LR`/...; `type` is
//!   `flowchart-v2`; `markers` are `[point, circle, cross]` (only `point`
//!   is needed for the geometry here).

use std::collections::HashMap;
use std::path::PathBuf;

use kitty_markdown_helper::mermaid::config::defaults::MermaidConfig;
use kitty_markdown_helper::mermaid::edges::Edge;
use kitty_markdown_helper::mermaid::layout::unified::{
    LayoutData, Rendered, UnifiedConfig, element_bbox, render_with_dagre,
};
use kitty_markdown_helper::mermaid::scene::markers::MarkerType;
use kitty_markdown_helper::mermaid::scene::{Element, Group, TransformOp};
use kitty_markdown_helper::mermaid::shapes::measure::LabelOpts;
use kitty_markdown_helper::mermaid::shapes::{Dir, LabelMeasurer, Node, NodeStyleVars, ShapeCtx};
use kitty_markdown_helper::mermaid::theme;
use kitty_markdown_helper::text::{TextMetrics, font_system};
use regex::Regex;
use serde::Deserialize;

const TOLERANCE: f64 = 0.01;
/// The measurer runs in `f32` and lands within two font units of Chrome at
/// 12 and 14 px (0.012 to 0.015 px); the geometry tests measure 16 px
/// labels, where the advances are exact.
const METRICS_TOLERANCE: f64 = 0.02;
/// `flowchart.diagramPadding`, which `setupViewPortForSVG` adds around the
/// content box to make the viewBox.
const DIAGRAM_PADDING: f64 = 8.0;

// ---------------------------------------------------------------------------
// flowDb.getData() records

fn flow_node(id: &str, label: &str, shape: &str, counter: usize) -> Node {
    Node {
        dom_id: Some(format!("flowchart-{id}-{counter}")),
        ..Node::flowchart(id, label, shape)
    }
}

fn group_node(id: &str, title: &str) -> Node {
    Node {
        label_style: Some(String::new()),
        label_type: Some("text".to_string()),
        padding: Some(8.0),
        is_group: true,
        ..Node::new(id, title, "rect")
    }
}

fn flow_edge(start: &str, end: &str, counter: usize, label: Option<&str>) -> Edge {
    Edge {
        label: label.map(str::to_string),
        minlen: Some(1.0),
        curve: Some("basis".to_string()),
        ..Edge::flowchart(&format!("L_{start}_{end}_{counter}"), start, end)
    }
}

fn layout_data(direction: Dir, nodes: Vec<Node>, edges: Vec<Edge>) -> LayoutData {
    LayoutData {
        nodes,
        edges,
        direction,
        diagram_type: "flowchart-v2".to_string(),
        diagram_id: "my-svg".to_string(),
        markers: vec![MarkerType::Point],
        ..LayoutData::default()
    }
}

/// `flowchart LR; A --> B --> C`.
fn chain() -> LayoutData {
    layout_data(
        Dir::LR,
        vec![
            flow_node("A", "A", "squareRect", 0),
            flow_node("B", "B", "squareRect", 1),
            flow_node("C", "C", "squareRect", 2),
        ],
        vec![flow_edge("A", "B", 0, None), flow_edge("B", "C", 0, None)],
    )
}

/// `flowchart TD; A[Start] --> B{Decide}; A --> C[Other]; B -->|yes|
/// D[End]; C --> D`.
fn diamond() -> LayoutData {
    layout_data(
        Dir::TB,
        vec![
            flow_node("A", "Start", "squareRect", 0),
            flow_node("B", "Decide", "diamond", 1),
            flow_node("C", "Other", "squareRect", 3),
            flow_node("D", "End", "squareRect", 5),
        ],
        vec![
            flow_edge("A", "B", 0, None),
            flow_edge("A", "C", 0, None),
            flow_edge("B", "D", 0, Some("yes")),
            flow_edge("C", "D", 0, None),
        ],
    )
}

/// `flowchart LR; subgraph S [Group]; B[Two] --> C[Three]; end; A[One]
/// --> B`.
fn subgraph() -> LayoutData {
    let mut b = flow_node("B", "Two", "squareRect", 0);
    b.parent_id = Some("S".to_string());
    let mut c = flow_node("C", "Three", "squareRect", 1);
    c.parent_id = Some("S".to_string());
    layout_data(
        Dir::LR,
        vec![
            group_node("S", "Group"),
            b,
            c,
            flow_node("A", "One", "squareRect", 2),
        ],
        vec![flow_edge("B", "C", 0, None), flow_edge("A", "B", 0, None)],
    )
}

// ---------------------------------------------------------------------------
// The helper side

fn render(data: &LayoutData) -> Rendered {
    let metrics = TextMetrics::new(font_system());
    let tv = theme::resolve("default", None).expect("default theme");
    let mut ctx = ShapeCtx::new(&metrics);
    ctx.vars = NodeStyleVars::from_theme(&tv, &MermaidConfig::default());
    let config = UnifiedConfig {
        flowchart_node_spacing: Some(50.0),
        flowchart_rank_spacing: Some(50.0),
        curve: Some("basis".to_string()),
        ..UnifiedConfig::default()
    };
    render_with_dagre(data, &config, &ctx).expect("renders")
}

fn translation(group: &Group) -> (f64, f64) {
    group
        .common
        .transform
        .iter()
        .fold((0.0, 0.0), |(x, y), op| match op {
            TransformOp::Translate(dx, dy) => (x + dx, y + dy),
            _ => (x, y),
        })
}

fn child_group<'a>(group: &'a Group, class: &str) -> &'a Group {
    group
        .children
        .iter()
        .find_map(|el| match el {
            Element::Group(g) if g.common.class.as_deref() == Some(class) => Some(g),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no g.{class}"))
}

#[derive(Debug, Clone, PartialEq)]
struct Box2 {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// A node as drawn: centre, shape box (the first element of the group,
/// which `updateNodeBounds` sizes the node from).
#[derive(Debug, Clone)]
struct DrawnNode {
    center: (f64, f64),
    shape: Box2,
}

fn helper_nodes(rendered: &Rendered) -> HashMap<String, DrawnNode> {
    let mut out = HashMap::new();
    for el in &child_group(&rendered.root, "nodes").children {
        let Element::Group(g) = el else { continue };
        let id = vertex_id(g.common.id.as_deref().expect("node id"));
        let shape = g.children.first().expect("shape element");
        let b = element_bbox(shape);
        out.insert(
            id,
            DrawnNode {
                center: translation(g),
                shape: Box2 {
                    x: b.x,
                    y: b.y,
                    width: b.width,
                    height: b.height,
                },
            },
        );
    }
    out
}

fn helper_edges(rendered: &Rendered) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for el in &child_group(&rendered.root, "edgePaths").children {
        let Element::Path(p) = el else { continue };
        let id = p.common.id.clone().expect("edge id");
        out.insert(id, p.d.to_svg());
    }
    out
}

/// Non-empty edge labels: the `g.edgeLabel` translation and the inner
/// `g.label` translation (which encodes the measured size).
fn helper_edge_labels(rendered: &Rendered) -> Vec<((f64, f64), (f64, f64))> {
    let mut out = Vec::new();
    for el in &child_group(&rendered.root, "edgeLabels").children {
        let Element::Group(outer) = el else { continue };
        if outer.common.class.as_deref() != Some("edgeLabel") {
            continue;
        }
        let Some(Element::Group(inner)) = outer.children.first() else {
            continue;
        };
        if inner.children.is_empty() {
            continue;
        }
        out.push((translation(outer), translation(inner)));
    }
    out
}

fn helper_clusters(rendered: &Rendered) -> HashMap<String, (Box2, (f64, f64))> {
    let mut out = HashMap::new();
    for el in &child_group(&rendered.root, "clusters").children {
        let Element::Group(g) = el else { continue };
        let id = g.common.id.clone().expect("cluster id");
        let Some(Element::Rect(r)) = g.children.first() else {
            panic!("cluster {id} has no rect");
        };
        let label = child_group(g, "cluster-label");
        out.insert(
            id,
            (
                Box2 {
                    x: r.x,
                    y: r.y,
                    width: r.width,
                    height: r.height,
                },
                translation(label),
            ),
        );
    }
    out
}

// ---------------------------------------------------------------------------
// The oracle side

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/unified")
}

fn oracle_svg(case: &str) -> String {
    let path = golden_dir().join(format!("{case}.svg"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn attrs(tag: &str) -> HashMap<String, String> {
    let re = Regex::new(r#"([A-Za-z_:][-\w:.]*)="([^"]*)""#).expect("regex");
    re.captures_iter(tag)
        .map(|c| (c[1].to_string(), c[2].to_string()))
        .collect()
}

fn num(attrs: &HashMap<String, String>, key: &str) -> f64 {
    attrs
        .get(key)
        .unwrap_or_else(|| panic!("no {key} in {attrs:?}"))
        .parse()
        .unwrap_or_else(|e| panic!("{key}: {e}"))
}

fn translate_of(value: &str) -> (f64, f64) {
    let re = Regex::new(r"translate\(\s*([-\d.e]+)\s*,\s*([-\d.e]+)\s*\)").expect("regex");
    let c = re
        .captures(value)
        .unwrap_or_else(|| panic!("not a translate: {value}"));
    (c[1].parse().expect("x"), c[2].parse().expect("y"))
}

/// `flowchart-<id>-<n>` (the helper's `domId`) or `my-svg-flowchart-<id>-<n>`
/// (the oracle prefixes it with the diagram id) to `<id>`.
fn vertex_id(dom_id: &str) -> String {
    let rest = dom_id
        .strip_prefix("my-svg-")
        .unwrap_or(dom_id)
        .strip_prefix("flowchart-")
        .unwrap_or_else(|| panic!("unexpected node id {dom_id}"));
    rest.rsplit_once('-')
        .map(|(id, _)| id.to_string())
        .expect("counter suffix")
}

fn oracle_nodes(svg: &str) -> HashMap<String, DrawnNode> {
    // The node group, then its first shape element.
    let re =
        Regex::new(r#"<g class="node[^"]*"([^>]*)>\s*<(rect|polygon)([^>]*)/>"#).expect("regex");
    let mut out = HashMap::new();
    for c in re.captures_iter(svg) {
        let g = attrs(&c[1]);
        let center = translate_of(&g["transform"]);
        let shape = attrs(&c[3]);
        let b = match &c[2] {
            "rect" => Box2 {
                x: num(&shape, "x"),
                y: num(&shape, "y"),
                width: num(&shape, "width"),
                height: num(&shape, "height"),
            },
            _ => {
                let (dx, dy) = translate_of(&shape["transform"]);
                let pts: Vec<(f64, f64)> = shape["points"]
                    .split_whitespace()
                    .map(|p| {
                        let (x, y) = p.split_once(',').expect("point");
                        (x.parse().expect("x"), y.parse().expect("y"))
                    })
                    .collect();
                let min_x = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
                let max_x = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
                let min_y = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
                let max_y = pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
                Box2 {
                    x: min_x + dx,
                    y: min_y + dy,
                    width: max_x - min_x,
                    height: max_y - min_y,
                }
            }
        };
        out.insert(vertex_id(&g["id"]), DrawnNode { center, shape: b });
    }
    out
}

fn oracle_edges(svg: &str) -> HashMap<String, String> {
    let re =
        Regex::new(r#"<path d="([^"]*)" id="([^"]*)" class="[^"]*flowchart-link""#).expect("regex");
    re.captures_iter(svg)
        .map(|c| (c[2].to_string(), c[1].to_string()))
        .collect()
}

fn oracle_edge_labels(svg: &str) -> Vec<((f64, f64), (f64, f64))> {
    let re = Regex::new(
        r#"<g class="edgeLabel" transform="([^"]*)"><g class="label"[^>]*transform="([^"]*)"><foreignObject width="([^"]*)""#,
    )
    .expect("regex");
    re.captures_iter(svg)
        .filter(|c| c[3].parse::<f64>().expect("width") > 0.0)
        .map(|c| (translate_of(&c[1]), translate_of(&c[2])))
        .collect()
}

fn oracle_clusters(svg: &str) -> HashMap<String, (Box2, (f64, f64))> {
    let re = Regex::new(
        r#"<g class="cluster[^"]*" id="my-svg-([^"]*)"[^>]*><rect([^>]*)/><g class="cluster-label" transform="([^"]*)""#,
    )
    .expect("regex");
    re.captures_iter(svg)
        .map(|c| {
            let r = attrs(&c[2]);
            (
                c[1].to_string(),
                (
                    Box2 {
                        x: num(&r, "x"),
                        y: num(&r, "y"),
                        width: num(&r, "width"),
                        height: num(&r, "height"),
                    },
                    translate_of(&c[3]),
                ),
            )
        })
        .collect()
}

fn oracle_view_box(svg: &str) -> Box2 {
    let re = Regex::new(r#"viewBox="([^"]*)""#).expect("regex");
    let v: Vec<f64> = re.captures(svg).expect("viewBox")[1]
        .split_whitespace()
        .map(|s| s.parse().expect("number"))
        .collect();
    Box2 {
        x: v[0],
        y: v[1],
        width: v[2],
        height: v[3],
    }
}

// ---------------------------------------------------------------------------
// Comparison

/// Tracks the largest deviation so the test log reports it.
struct Deviations {
    max: f64,
    where_max: String,
    /// Every numeric comparison with its deviation, in order.
    values: Vec<(String, f64)>,
    /// Structural mismatches, which no tolerance forgives.
    failures: Vec<String>,
}

impl Deviations {
    fn new() -> Deviations {
        Deviations {
            max: 0.0,
            where_max: String::new(),
            values: Vec::new(),
            failures: Vec::new(),
        }
    }

    fn check(&mut self, what: &str, expected: f64, got: f64) {
        let diff = (expected - got).abs();
        if diff > self.max {
            self.max = diff;
            self.where_max = what.to_string();
        }
        self.values.push((
            format!("{what}: oracle {expected}, helper {got} (diff {diff:.4})"),
            diff,
        ));
    }

    fn check_box(&mut self, what: &str, expected: &Box2, got: &Box2) {
        self.check(&format!("{what}.x"), expected.x, got.x);
        self.check(&format!("{what}.y"), expected.y, got.y);
        self.check(&format!("{what}.width"), expected.width, got.width);
        self.check(&format!("{what}.height"), expected.height, got.height);
    }

    fn check_point(&mut self, what: &str, expected: (f64, f64), got: (f64, f64)) {
        self.check(&format!("{what}.x"), expected.0, got.0);
        self.check(&format!("{what}.y"), expected.1, got.1);
    }

    /// Path data as d3 wrote it: the same command letters in the same
    /// order, every number within tolerance.
    fn check_path(&mut self, what: &str, expected: &str, got: &str) {
        let tokens = Regex::new(r"[A-Za-z]|-?\d+(?:\.\d+)?(?:e-?\d+)?").expect("regex");
        let e: Vec<&str> = tokens.find_iter(expected).map(|m| m.as_str()).collect();
        let g: Vec<&str> = tokens.find_iter(got).map(|m| m.as_str()).collect();
        if e.len() != g.len() {
            self.failures.push(format!(
                "{what}: {} tokens in the oracle, {} in the helper\n  oracle {expected}\n  helper {got}",
                e.len(),
                g.len()
            ));
            return;
        }
        for (i, (a, b)) in e.iter().zip(&g).enumerate() {
            match (a.parse::<f64>(), b.parse::<f64>()) {
                (Ok(x), Ok(y)) => self.check(&format!("{what}[{i}]"), x, y),
                _ if a != b => self.failures.push(format!(
                    "{what}[{i}]: command {a} in the oracle, {b} in the helper"
                )),
                _ => {}
            }
        }
    }

    fn finish(self, case: &str) {
        self.finish_within(case, TOLERANCE);
    }

    fn finish_within(mut self, case: &str, tolerance: f64) {
        eprintln!(
            "{case}: {} values compared, max deviation {:.6} px at {}",
            self.values.len(),
            self.max,
            self.where_max
        );
        let over = self
            .values
            .iter()
            .filter(|(_, diff)| *diff > tolerance)
            .map(|(line, _)| line.clone());
        self.failures.extend(over);
        assert!(
            self.failures.is_empty(),
            "{case}: {} deviations over {tolerance} px\n{}",
            self.failures.len(),
            self.failures.join("\n")
        );
    }
}

fn compare(case: &str, data: &LayoutData) {
    let svg = oracle_svg(case);
    let rendered = render(data);
    let mut dev = Deviations::new();

    let expected_nodes = oracle_nodes(&svg);
    let got_nodes = helper_nodes(&rendered);
    assert_eq!(expected_nodes.len(), got_nodes.len(), "{case}: node count");
    for (id, expected) in &expected_nodes {
        let got = got_nodes
            .get(id)
            .unwrap_or_else(|| panic!("{case}: helper drew no node {id}"));
        dev.check_point(&format!("node {id} centre"), expected.center, got.center);
        dev.check_box(&format!("node {id} shape"), &expected.shape, &got.shape);
    }

    let expected_edges = oracle_edges(&svg);
    let got_edges = helper_edges(&rendered);
    assert_eq!(expected_edges.len(), got_edges.len(), "{case}: edge count");
    for (id, expected) in &expected_edges {
        let got = got_edges
            .get(id)
            .unwrap_or_else(|| panic!("{case}: helper drew no edge {id}"));
        dev.check_path(&format!("edge {id}"), expected, got);
    }

    let expected_labels = oracle_edge_labels(&svg);
    let got_labels = helper_edge_labels(&rendered);
    assert_eq!(
        expected_labels.len(),
        got_labels.len(),
        "{case}: edge label count"
    );
    for (i, (expected, got)) in expected_labels.iter().zip(&got_labels).enumerate() {
        dev.check_point(&format!("edge label {i}"), expected.0, got.0);
        dev.check_point(&format!("edge label {i} inner"), expected.1, got.1);
    }

    let expected_clusters = oracle_clusters(&svg);
    let got_clusters = helper_clusters(&rendered);
    assert_eq!(
        expected_clusters.len(),
        got_clusters.len(),
        "{case}: cluster count"
    );
    for (id, (expected_rect, expected_label)) in &expected_clusters {
        let (got_rect, got_label) = got_clusters
            .get(id)
            .unwrap_or_else(|| panic!("{case}: helper drew no cluster {id}"));
        dev.check_box(&format!("cluster {id} rect"), expected_rect, got_rect);
        dev.check_point(&format!("cluster {id} label"), *expected_label, *got_label);
    }

    // `setupViewPortForSVG`: the content box plus `diagramPadding`.
    let view_box = oracle_view_box(&svg);
    let b = rendered.bounds;
    dev.check_box(
        "viewBox",
        &view_box,
        &Box2 {
            x: b.x - DIAGRAM_PADDING,
            y: b.y - DIAGRAM_PADDING,
            width: b.width + 2.0 * DIAGRAM_PADDING,
            height: b.height + 2.0 * DIAGRAM_PADDING,
        },
    );

    dev.finish(case);
}

#[test]
fn chain_matches_the_oracle() {
    compare("chain", &chain());
}

#[test]
fn diamond_with_edge_label_matches_the_oracle() {
    compare("diamond", &diamond());
}

#[test]
fn subgraph_with_outside_edge_matches_the_oracle() {
    compare("subgraph", &subgraph());
}

// ---------------------------------------------------------------------------
// The label measurer against the oracle's getBBox / getBoundingClientRect

#[derive(Deserialize)]
struct Metrics {
    cases: Vec<Case>,
    wrapped: Vec<Wrapped>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    text: String,
    family: String,
    size: f64,
    weight: u16,
    style: String,
    svg: Size,
    html: Size,
}

#[derive(Deserialize)]
struct Size {
    width: f64,
    height: f64,
}

#[derive(Deserialize)]
struct Wrapped {
    id: String,
    text: String,
    family: String,
    size: f64,
    max_width: f64,
    width: f64,
    height: f64,
    lines: Vec<String>,
}

fn metrics_json() -> Option<Metrics> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/metrics.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipping: {} is absent", path.display());
        return None;
    };
    Some(serde_json::from_str(&text).expect("metrics.json parses"))
}

fn label_opts(family: &str, size: f64, html: bool, width: Option<f64>) -> LabelOpts {
    use kitty_markdown_helper::mermaid::scene::FontSpec;
    LabelOpts {
        font: FontSpec {
            family: Some(family.to_string()),
            size: Some(size),
            ..FontSpec::default()
        },
        html,
        width,
        min_width: 0.0,
        markdown: true,
    }
}

/// Upright ASCII strings at weight 400: the cases where Chrome's `getBBox`
/// is the advance box, which is what `createFormattedText` sizes a label
/// from. Italic ink overhang and fallback fonts are the text package's own
/// calibration (`text_metrics.rs`).
#[test]
fn measurer_matches_the_oracle_boxes() {
    let Some(metrics) = metrics_json() else {
        return;
    };
    let text = TextMetrics::new(font_system());
    let mut dev = Deviations::new();
    let mut compared = 0;
    for case in metrics
        .cases
        .iter()
        .filter(|c| c.style == "normal" && c.weight == 400 && c.text.is_ascii())
    {
        let html = text.measure(&case.text, &label_opts(&case.family, case.size, true, None));
        dev.check(
            &format!("{} html.width", case.id),
            case.html.width,
            html.width,
        );
        dev.check(
            &format!("{} html.height", case.id),
            case.html.height,
            html.height,
        );
        // The golden's `svg` box is one `<text>` node; a label puts each
        // word in its own tspan, so only a single word compares.
        if !case.text.is_empty() && case.text.chars().all(char::is_alphanumeric) {
            let svg = text.measure(
                &case.text,
                &label_opts(&case.family, case.size, false, None),
            );
            dev.check(&format!("{} svg.width", case.id), case.svg.width, svg.width);
            dev.check(
                &format!("{} svg.height", case.id),
                case.svg.height,
                svg.height,
            );
            dev.check(&format!("{} svg.x", case.id), -case.svg.width / 2.0, svg.x);
        }
        compared += 1;
    }
    assert!(compared >= 20, "only {compared} cases compared");
    dev.finish_within("metrics cases", METRICS_TOLERANCE);
}

/// The HTML wrapping path: `max-width` reached, `display: table` with
/// `break-spaces`, the browser's line breaks.
#[test]
fn measurer_wraps_like_chrome() {
    let Some(metrics) = metrics_json() else {
        return;
    };
    let text = TextMetrics::new(font_system());
    let mut dev = Deviations::new();
    for case in metrics.wrapped.iter().filter(|w| w.text.is_ascii()) {
        let got = text.measure(
            &case.text,
            &label_opts(&case.family, case.size, true, Some(case.max_width)),
        );
        let what = format!("{} {:?}", case.id, &case.text[..20]);
        dev.check(&format!("{what} width"), case.width, got.width);
        dev.check(&format!("{what} height"), case.height, got.height);
        let lines: Vec<String> = got
            .lines
            .iter()
            .map(|l| {
                l.runs
                    .iter()
                    .map(|r| r.text.as_str())
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .collect();
        assert_eq!(lines, case.lines, "{what}: line breaks");
    }
    dev.finish_within("metrics wrapped", METRICS_TOLERANCE);
}
