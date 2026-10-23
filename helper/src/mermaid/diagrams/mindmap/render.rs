//! Port of `mindmapRenderer.ts` `draw` (mermaid 12.2.0).
//!
//! `getData` names `cose-bilkent` only when the user config has no
//! `layout`. The base config (`helper/oracle/mermaid-config.json`) passes
//! `layout: dagre` to `initialize`, which `getUserDefinedConfig` counts as
//! the user's, so the oracle lays mindmaps out with the unified dagre
//! renderer, and so does this port. `draw` then adjusts a few shapes'
//! sizes and paddings, renders, and frames the content with
//! `setupViewPortForSVG` and `mindmap.padding`.

use serde_json::Value;

use super::db::MindmapDb;
use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::layout::unified::{LayoutData, UnifiedConfig, render_with_dagre};
use crate::mermaid::scene::markers::MarkerType;
use crate::mermaid::scene::{Common, Element, FontSpec, Group, Paint, Scene, Style, ViewBox};
use crate::mermaid::shapes::{Dir, Node, ShapeCtx};
use crate::mermaid::styles::mindmap::MindmapStyles;

/// mermaid-cli's SVG id; `render.ts` prefixes every `domId` with it.
pub const DIAGRAM_ID: &str = "my-svg";
/// `nodeSpacing: 50` and `rankSpacing: 50` of `getData`.
const SPACING: f64 = 50.0;
/// `defaultConfig.mindmap.padding`.
const DEFAULT_PADDING: f64 = 10.0;

/// A positive number from the raw config, as `||` reads it.
fn truthy_number(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(Value::as_f64)
        .filter(|v| *v != 0.0 && !v.is_nan())
}

/// The `data4Layout.nodes.forEach` of `draw`: `rounded` gets `padding 15`
/// and no width, `rect` `padding 10` and no width, `hexagon` no width and
/// no height. (`radius`, `taper`, and `stroke` on the rounded node are
/// read by no classic shape.) A width of 0 is falsy in every shape, so it
/// is `None` here. The `circle` branch never runs: the circle shape is
/// `mindmapCircle`.
fn adjust_node(node: &mut Node) {
    match node.shape.as_str() {
        "rounded" => {
            node.width = None;
            node.padding = Some(15.0);
        }
        "rect" => {
            node.width = None;
            node.padding = Some(10.0);
        }
        "hexagon" => {
            node.width = None;
            node.height = None;
        }
        _ => {}
    }
    // `render.ts`: `node.domId = ${diagramId}-${domId || id}`.
    let dom_id = node.dom_id.clone().unwrap_or_else(|| node.id.clone());
    node.dom_id = Some(format!("{DIAGRAM_ID}-{dom_id}"));
}

pub fn draw(
    db: &mut MindmapDb,
    resolved: &Resolved,
    ctx: &RenderCtx<'_>,
) -> Result<Scene, HelperError> {
    let cfg = &resolved.config;
    let raw = &resolved.raw;
    let mindmap = &cfg.mindmap;
    let color_by_depth = mindmap.get("colorBy").and_then(Value::as_str) == Some("depth");
    let padding = mindmap
        .get("padding")
        .and_then(Value::as_f64)
        .unwrap_or(DEFAULT_PADDING);
    let styles = MindmapStyles::from_theme(&resolved.theme_variables);

    let mut data = db.get_data(color_by_depth);
    // `if (!mm) return;`: no root, nothing drawn, no viewBox set up.
    if data.nodes.is_empty() {
        let mut scene = Scene::new(2.0 * padding, 2.0 * padding);
        scene.font_family = styles.font_family.clone();
        return Ok(scene);
    }
    for node in &mut data.nodes {
        adjust_node(node);
    }

    let mut shapes = ShapeCtx::new(&ctx.metrics);
    shapes.vars = styles.node_vars();
    shapes.html_labels = cfg.effective_html_labels();
    shapes.wrapping_width = *cfg.flowchart.wrapping_width;

    let layout = LayoutData {
        nodes: data.nodes,
        edges: data.edges,
        direction: Dir::TB,
        node_spacing: Some(SPACING),
        rank_spacing: Some(SPACING),
        diagram_type: "mindmap".to_string(),
        diagram_id: DIAGRAM_ID.to_string(),
        markers: vec![MarkerType::Point],
    };
    let config = UnifiedConfig {
        node_spacing: truthy_number(raw.get("nodeSpacing")),
        rank_spacing: truthy_number(raw.get("rankSpacing")),
        flowchart_node_spacing: Some(*cfg.flowchart.node_spacing),
        flowchart_rank_spacing: Some(*cfg.flowchart.rank_spacing),
        curve: Some(cfg.flowchart.curve.clone()),
        sub_graph_title_margin_top: *cfg.flowchart.sub_graph_title_margin.top,
        sub_graph_title_margin_bottom: *cfg.flowchart.sub_graph_title_margin.bottom,
    };
    let rendered = render_with_dagre(&layout, &config, &shapes)?;
    let mut root = rendered.root;
    styles.apply(&mut root);

    // `setupViewPortForSVG(svg, mindmap.padding, 'mindmapDiagram', ...)`.
    let b = rendered.bounds;
    let width = b.width + 2.0 * padding;
    let height = b.height + 2.0 * padding;
    let mut scene = Scene::new(width, height);
    scene.view_box = ViewBox::new(b.x - padding, b.y - padding, width, height);
    scene.font_family = styles.font_family.clone();
    scene.defs = rendered.markers.into_iter().map(Element::Marker).collect();
    // The stylesheet's root rule: `fill: textColor; font-size`.
    scene.root = Group {
        common: Common::with_style(Style {
            fill: Some(Paint::Color(styles.text_color)),
            font: FontSpec {
                size: Some(styles.font_size),
                ..FontSpec::default()
            },
            ..Style::default()
        }),
        children: vec![Element::Group(root)],
    };
    Ok(scene)
}
