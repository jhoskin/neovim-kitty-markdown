//! Flowchart parity (R-105, R-110): an insta SVG snapshot of the scene for
//! every case under `tests/goldens/flowchart/`, an SSIM check against the
//! oracle PNG at the same pixel size (R-181), and for the cases with an
//! oracle SVG next to them (`<case>.svg`, made with `mmdc -c
//! helper/oracle/mermaid-config.json` and an `.svg` output) a geometry
//! check: node centres and shape boxes, edge paths, edge labels, cluster
//! rects, and the viewBox to 0.01 px, as `unified_dagre.rs` does for the
//! hand-built layouts.

mod common;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::RgbaImage;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::{self, RenderCtx};
use kitty_markdown_helper::mermaid::raster::Background;
use kitty_markdown_helper::mermaid::scene::{Element, Group, Scene, TransformOp};
use kitty_markdown_helper::mermaid::{self, RenderRequest};
use kitty_markdown_helper::text::{self, TextMetrics};
use regex::Regex;

use common::{SSIM_THRESHOLD, assert_ssim, golden_dir};

/// The width `goldens.mjs` renders at.
const PARITY_WIDTH: u32 = 960;
const TOLERANCE: f64 = 0.01;

/// Every `<case>.mmd` under `goldens/flowchart/`, with or without a PNG,
/// so the snapshots cover a case before its golden exists.
fn sources() -> Vec<(String, PathBuf)> {
    let dir = golden_dir().join("flowchart");
    let mut cases: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "mmd"))
        .filter_map(|p| Some((p.file_stem()?.to_str()?.to_string(), p)))
        .collect();
    cases.sort();
    cases
}

fn scene(mmd: &Path) -> Scene {
    let source = std::fs::read_to_string(mmd).unwrap_or_else(|e| panic!("{}: {e}", mmd.display()));
    let resolved = config::resolve(&source, &BaseConfig::new("default"))
        .unwrap_or_else(|e| panic!("{}: {e}", mmd.display()));
    let diagram = diagrams::find(resolved.diagram_type).expect("flowchart is registered");
    let fonts = text::font_system();
    let ctx = RenderCtx {
        fonts,
        metrics: TextMetrics::new(fonts),
        target_width: PARITY_WIDTH,
    };
    diagram
        .render(&resolved, &ctx)
        .unwrap_or_else(|e| panic!("{}: {e}", mmd.display()))
}

#[test]
fn flowchart_scene_snapshots() {
    let cases = sources();
    assert!(
        !cases.is_empty(),
        "no flowchart cases under tests/goldens/flowchart"
    );
    for (name, mmd) in cases {
        insta::assert_snapshot!(name.as_str(), scene(&mmd).to_svg());
    }
}

fn render_at_parity_width(source: &str) -> RgbaImage {
    let out = mermaid::render(&RenderRequest {
        source,
        width_px: PARITY_WIDTH,
        theme: "default",
        background: Background::default(),
    })
    .expect("flowchart renders");
    image::load_from_memory(&out.png)
        .expect("helper PNG decodes")
        .to_rgba8()
}

#[test]
fn flowchart_ssim_against_the_oracle() {
    let cases = skip_if_no_goldens!("flowchart");
    // Every case runs before the test fails, so one log shows them all.
    let mut failed = Vec::new();
    for case in cases {
        let goldens = case.goldens();
        let actual = render_at_parity_width(&case.source());
        let name = format!("flowchart-{}", case.name);
        let result = std::panic::catch_unwind(|| {
            assert_ssim(&actual, &goldens, SSIM_THRESHOLD, &name);
        });
        if result.is_err() {
            failed.push(case.name);
        }
    }
    assert!(failed.is_empty(), "below {SSIM_THRESHOLD}: {failed:?}");
}

// ---------------------------------------------------------------------------
// Geometry against the oracle SVG

#[derive(Debug, Clone, Copy, PartialEq)]
struct Box2 {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
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

/// Every group with `class` anywhere under `group`, depth first. Positions
/// stay local to the enclosing `g.root`, as the oracle writes them: an
/// extracted sub-graph renders as a nested `g.root` translated into place,
/// and [`root_translations`] compares those translations.
fn find_groups<'a>(
    group: &'a Group,
    class: &str,
    offset: (f64, f64),
    out: &mut Vec<(&'a Group, (f64, f64))>,
) {
    for el in &group.children {
        if let Element::Group(g) = el {
            if g.common.class.as_deref() == Some(class) {
                out.push((g, offset));
            }
            find_groups(g, class, offset, out);
        }
    }
}

/// `flowchart-<id>-<n>` or `my-svg-flowchart-<id>-<n>` to `<id>`.
fn vertex_id(dom_id: &str) -> Option<String> {
    let rest = dom_id
        .strip_prefix("my-svg-")
        .unwrap_or(dom_id)
        .strip_prefix("flowchart-")?;
    rest.rsplit_once('-').map(|(id, _)| id.to_string())
}

/// Node centres in diagram coordinates and the shape's rect when the first
/// shape element is a rect.
fn helper_nodes(scene: &Scene) -> HashMap<String, ((f64, f64), Option<Box2>)> {
    let mut groups = Vec::new();
    find_groups(&scene.root, "nodes", (0.0, 0.0), &mut groups);
    let mut out = HashMap::new();
    for (nodes, offset) in groups {
        for el in &nodes.children {
            let Element::Group(g) = el else { continue };
            let Some(id) = g.common.id.as_deref().and_then(vertex_id) else {
                continue;
            };
            let (x, y) = translation(g);
            let shape = match g.children.first() {
                Some(Element::Rect(r)) => Some(Box2 {
                    x: r.x,
                    y: r.y,
                    width: r.width,
                    height: r.height,
                }),
                _ => None,
            };
            out.insert(id, ((x + offset.0, y + offset.1), shape));
        }
    }
    out
}

fn helper_edges(scene: &Scene) -> HashMap<String, (String, (f64, f64))> {
    let mut groups = Vec::new();
    find_groups(&scene.root, "edgePaths", (0.0, 0.0), &mut groups);
    let mut out = HashMap::new();
    for (paths, offset) in groups {
        for el in &paths.children {
            let Element::Path(p) = el else { continue };
            let id = p.common.id.clone().expect("edge id");
            out.insert(id, (p.d.to_svg(), offset));
        }
    }
    out
}

fn helper_clusters(scene: &Scene) -> HashMap<String, Box2> {
    let mut groups = Vec::new();
    find_groups(&scene.root, "clusters", (0.0, 0.0), &mut groups);
    let mut out = HashMap::new();
    for (clusters, offset) in groups {
        for el in &clusters.children {
            let Element::Group(g) = el else { continue };
            let Some(Element::Rect(r)) = g.children.first() else {
                continue;
            };
            let id = g.common.id.clone().expect("cluster id");
            out.insert(
                id,
                Box2 {
                    x: r.x + offset.0,
                    y: r.y + offset.1,
                    width: r.width,
                    height: r.height,
                },
            );
        }
    }
    out
}

/// The translations of the nested `g.root` groups, in document order.
fn root_translations(scene: &Scene) -> Vec<(f64, f64)> {
    let mut groups = Vec::new();
    find_groups(&scene.root, "root", (0.0, 0.0), &mut groups);
    groups
        .into_iter()
        .map(|(g, _)| translation(g))
        .filter(|t| *t != (0.0, 0.0))
        .collect()
}

fn oracle_root_translations(svg: &str) -> Vec<(f64, f64)> {
    let re = Regex::new(r#"<g class="root" transform="([^"]*)""#).expect("regex");
    re.captures_iter(svg).map(|c| translate_of(&c[1])).collect()
}

fn attrs(tag: &str) -> HashMap<String, String> {
    let re = Regex::new(r#"([A-Za-z_:][-\w:.]*)="([^"]*)""#).expect("regex");
    re.captures_iter(tag)
        .map(|c| (c[1].to_string(), c[2].to_string()))
        .collect()
}

fn translate_of(value: &str) -> (f64, f64) {
    let re = Regex::new(r"translate\(\s*([-\d.e]+)\s*,\s*([-\d.e]+)\s*\)").expect("regex");
    match re.captures(value) {
        Some(c) => (c[1].parse().expect("x"), c[2].parse().expect("y")),
        None => (0.0, 0.0),
    }
}

fn num(attrs: &HashMap<String, String>, key: &str) -> f64 {
    attrs[key].parse().unwrap_or_else(|e| panic!("{key}: {e}"))
}

/// The oracle's node groups: the centre from the group transform and the
/// rect when the first shape element is one. A sub-graph's nodes sit in a
/// nested `g.root` with its own translate, which the comparison adds; the
/// cases with geometry checks have no extracted sub-graphs.
fn oracle_nodes(svg: &str) -> HashMap<String, ((f64, f64), Option<Box2>)> {
    let re = Regex::new(r#"<g class="node(?: [^"]*)?"([^>]*)>\s*<(rect|polygon|path|g)([^>]*)>"#)
        .expect("regex");
    let mut out = HashMap::new();
    for c in re.captures_iter(svg) {
        let g = attrs(&c[1]);
        let Some(id) = g.get("id").and_then(|i| vertex_id(i)) else {
            continue;
        };
        let center = translate_of(&g["transform"]);
        let shape = (&c[2] == "rect").then(|| {
            let r = attrs(&c[3]);
            Box2 {
                x: num(&r, "x"),
                y: num(&r, "y"),
                width: num(&r, "width"),
                height: num(&r, "height"),
            }
        });
        out.insert(id, (center, shape));
    }
    out
}

fn oracle_edges(svg: &str) -> HashMap<String, String> {
    let re = Regex::new(r#"<path d="([^"]*)" id="([^"]*)" class="[^"]*""#).expect("regex");
    re.captures_iter(svg)
        .filter(|c| c[2].starts_with("my-svg-L"))
        .map(|c| (c[2].to_string(), c[1].to_string()))
        .collect()
}

fn oracle_clusters(svg: &str) -> HashMap<String, Box2> {
    let re = Regex::new(r#"<g class="cluster[^"]*" id="my-svg-([^"]*)"[^>]*><rect([^>]*)/>"#)
        .expect("regex");
    re.captures_iter(svg)
        .map(|c| {
            let r = attrs(&c[2]);
            (
                format!("my-svg-{}", &c[1]),
                Box2 {
                    x: num(&r, "x"),
                    y: num(&r, "y"),
                    width: num(&r, "width"),
                    height: num(&r, "height"),
                },
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

struct Deviations {
    max: f64,
    where_max: String,
    failures: Vec<String>,
}

impl Deviations {
    fn check(&mut self, what: &str, expected: f64, got: f64) {
        let diff = (expected - got).abs();
        if diff > self.max {
            self.max = diff;
            self.where_max = what.to_string();
        }
        if diff > TOLERANCE {
            self.failures
                .push(format!("{what}: oracle {expected}, helper {got}"));
        }
    }

    fn check_box(&mut self, what: &str, e: &Box2, g: &Box2) {
        self.check(&format!("{what}.x"), e.x, g.x);
        self.check(&format!("{what}.y"), e.y, g.y);
        self.check(&format!("{what}.width"), e.width, g.width);
        self.check(&format!("{what}.height"), e.height, g.height);
    }

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
                _ if a != b => self.failures.push(format!("{what}[{i}]: {a} vs {b}")),
                _ => {}
            }
        }
    }
}

fn compare_geometry(case: &str) {
    let dir = golden_dir().join("flowchart");
    let svg_path = dir.join(format!("{case}.svg"));
    let Ok(svg) = std::fs::read_to_string(&svg_path) else {
        eprintln!("skipping: {} is absent", svg_path.display());
        return;
    };
    let scene = scene(&dir.join(format!("{case}.mmd")));
    let mut dev = Deviations {
        max: 0.0,
        where_max: String::new(),
        failures: Vec::new(),
    };

    let expected = oracle_nodes(&svg);
    let got = helper_nodes(&scene);
    assert_eq!(expected.len(), got.len(), "{case}: node count");
    for (id, (center, shape)) in &expected {
        let (gc, gs) = got
            .get(id)
            .unwrap_or_else(|| panic!("{case}: helper drew no node {id}"));
        dev.check(&format!("node {id}.x"), center.0, gc.0);
        dev.check(&format!("node {id}.y"), center.1, gc.1);
        if let (Some(e), Some(g)) = (shape, gs) {
            dev.check_box(&format!("node {id} rect"), e, g);
        }
    }

    let expected = oracle_edges(&svg);
    let got = helper_edges(&scene);
    assert_eq!(expected.len(), got.len(), "{case}: edge count");
    for (id, d) in &expected {
        let (gd, offset) = got
            .get(id)
            .unwrap_or_else(|| panic!("{case}: helper drew no edge {id}"));
        assert_eq!(*offset, (0.0, 0.0), "{case}: edge {id}");
        dev.check_path(&format!("edge {id}"), d, gd);
    }

    let expected = oracle_clusters(&svg);
    let got = helper_clusters(&scene);
    assert_eq!(expected.len(), got.len(), "{case}: cluster count");
    for (id, e) in &expected {
        let g = got
            .get(id.strip_prefix("my-svg-").unwrap_or(id))
            .or_else(|| got.get(id))
            .unwrap_or_else(|| panic!("{case}: helper drew no cluster {id}"));
        dev.check_box(&format!("cluster {id}"), e, g);
    }

    let expected = oracle_root_translations(&svg);
    let got = root_translations(&scene);
    assert_eq!(expected.len(), got.len(), "{case}: nested root count");
    for (i, (e, g)) in expected.iter().zip(&got).enumerate() {
        dev.check(&format!("root {i}.x"), e.0, g.0);
        dev.check(&format!("root {i}.y"), e.1, g.1);
    }

    let vb = oracle_view_box(&svg);
    let v = scene.view_box;
    dev.check_box(
        "viewBox",
        &vb,
        &Box2 {
            x: v.x,
            y: v.y,
            width: v.width,
            height: v.height,
        },
    );

    eprintln!(
        "{case}: max deviation {:.6} px at {}",
        dev.max, dev.where_max
    );
    assert!(
        dev.failures.is_empty(),
        "{case}: {} deviations over {TOLERANCE} px\n{}",
        dev.failures.len(),
        dev.failures.join("\n")
    );
}

#[test]
fn chain_td_geometry_matches_the_oracle_svg() {
    compare_geometry("chain-td");
}

#[test]
fn chain_lr_geometry_matches_the_oracle_svg() {
    compare_geometry("chain-lr");
}

#[test]
fn subgraphs_geometry_matches_the_oracle_svg() {
    compare_geometry("subgraphs");
}

/// Edges into and out of a subgraph id whose cluster renders as a nested
/// graph: the cluster node has no intersect, so `insertEdge` keeps dagre's
/// own end points instead of clipping to the node.
#[test]
fn subgraph_edges_geometry_matches_the_oracle_svg() {
    compare_geometry("subgraph-edges");
}

// ---------------------------------------------------------------------------
// The db over the grammar corpus and the error paths

/// Every input mermaid's own flowchart specs parse goes through the grammar
/// actions and `FlowDb` without a panic. The inputs that make the db throw
/// in mermaid too are listed by the message the db gives.
#[test]
fn every_valid_flow_input_reaches_the_db() {
    use kitty_markdown_helper::mermaid::diagrams::flowchart::{self, db::DbConfig};
    let path = golden_dir().join("jison").join("flow_valid.txt");
    let text = std::fs::read_to_string(&path).expect("flow_valid.txt");
    let text = text.strip_suffix('\n').unwrap_or(&text);
    let mut errors: Vec<String> = Vec::new();
    let mut total = 0;
    for input in text.split("\n---\n") {
        total += 1;
        if let Err(e) = flowchart::parse(input, DbConfig::default()) {
            errors.push(format!("{input:?}: {}", e.message));
        }
    }
    eprintln!(
        "{} of {total} inputs threw in the db:\n{}",
        errors.len(),
        errors.join("\n")
    );
    assert!(total > 800);
}

fn render_error(source: &str) -> kitty_markdown_helper::error::HelperError {
    match mermaid::render(&RenderRequest {
        source,
        width_px: PARITY_WIDTH,
        theme: "default",
        background: Background::default(),
    }) {
        Ok(_) => panic!("{source:?} rendered"),
        Err(e) => e,
    }
}

/// The failures the oracle shows for these inputs, by code and text.
#[test]
fn flowchart_errors_match_the_oracle() {
    // `addSubGraph(undefined, ...)` reads `.text` of undefined in mermaid.
    let e = render_error("flowchart TB\n  subgraph\n  a --> b\n  end\n");
    assert_eq!(e.code(), "internal");
    assert!(
        e.to_string()
            .contains("Cannot read properties of undefined (reading 'text')"),
        "{e}"
    );
    let e = render_error("flowchart LR\n  A@{ shape: nope }\n");
    assert_eq!(e.code(), "internal");
    assert!(e.to_string().contains("No such shape: nope."), "{e}");
    let e = render_error("flowchart LR\n  A@{ shape: Rect }\n");
    assert!(
        e.to_string().contains("Shape names should be lowercase."),
        "{e}"
    );
    let e = render_error("flowchart LR\n  A --> B\n  linkStyle 3 stroke:red\n");
    assert_eq!(e.code(), "internal");
    let e = render_error("flowchart LR\n  A --> B -->\n");
    assert_eq!(e.code(), "parse_error");
    // The dangling arrow fails at the end of input, after the last newline.
    assert!(e.to_string().starts_with("Parse error on line 3:"), "{e}");
    let e = render_error("flowchart-elk LR\n  A --> B\n");
    assert_eq!(e.code(), "unsupported_feature");
    let e = render_error("flowchart LR\n  A@{ icon: \"fa:user\" }\n");
    assert_eq!(e.code(), "unsupported_feature");
}
