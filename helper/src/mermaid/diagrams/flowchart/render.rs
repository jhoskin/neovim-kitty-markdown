//! Port of `diagrams/flowchart/flowRenderer-v3-unified.ts` (`draw`):
//! `getData()` into the unified renderer with the dagre layout, the
//! diagram title (`utils.insertTitle`), and `setupViewPortForSVG` with
//! `flowchart.diagramPadding` around the content box.

use super::db::FlowDb;
use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::config::defaults::MermaidConfig;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::layout::unified::{UnifiedConfig, render_with_dagre};
use crate::mermaid::scene::markers::MarkerType;
use crate::mermaid::scene::{
    Common, Element, FontSpec, Group, Paint, Scene, Style, Text, TextAnchor, TextRun, ViewBox,
};
use crate::mermaid::shapes::util::BBox;
use crate::mermaid::shapes::{Dir, ShapeCtx};
use crate::mermaid::styles::flowchart::{FlowchartStyles, TITLE_FONT_SIZE};
use crate::text;

/// mermaid-cli renders into `<svg id="my-svg">`; dom ids hang off it.
pub const DIAGRAM_ID: &str = "my-svg";

/// `getEffectiveHtmlLabels(config)`: the top-level `htmlLabels`, then the
/// deprecated `flowchart.htmlLabels`, then `true`.
pub fn effective_html_labels(config: &MermaidConfig) -> bool {
    if let Some(v) = config.html_labels {
        return v;
    }
    match &config.flowchart.html_labels {
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::String(s) => s != "false",
        serde_json::Value::Null => true,
        serde_json::Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        _ => true,
    }
}

/// `conf?.nodeSpacing || 50`.
fn or_default(v: f64, default: f64) -> f64 {
    if v == 0.0 || v.is_nan() { default } else { v }
}

/// A top-level number of the raw config (`nodeSpacing`, `rankSpacing`),
/// which has no schema default and is absent unless a user sets it.
fn raw_number(resolved: &Resolved, key: &str) -> Option<f64> {
    resolved
        .raw
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .filter(|v| *v != 0.0)
}

/// `utils.insertTitle(svg, 'flowchartTitleText', titleTopMargin, title)`:
/// a centred text above the content, and its `getBBox()`.
fn title_element(
    title: &str,
    bounds: BBox,
    top_margin: f64,
    styles: &FlowchartStyles,
    ctx: &RenderCtx<'_>,
) -> (Element, BBox) {
    let x = bounds.x + bounds.width / 2.0;
    let y = -top_margin;
    let spec = text::FontSpec::new(styles.font_family.clone(), TITLE_FONT_SIZE as f32);
    let shaped = ctx.metrics.shape(title, &spec);
    let size = shaped.bbox();
    let bbox = BBox {
        x: x - f64::from(size.width) / 2.0,
        y: y - f64::from(shaped.ascent),
        width: f64::from(size.width),
        height: f64::from(size.height),
    };
    let element = Element::Text(Text {
        common: Common {
            class: Some("flowchartTitleText".to_string()),
            style: Style {
                fill: Some(Paint::Color(styles.text_color)),
                ..Style::default()
            },
            ..Common::default()
        },
        x,
        y,
        anchor: Some(TextAnchor::Middle),
        baseline: None,
        font: FontSpec {
            size: Some(TITLE_FONT_SIZE),
            ..FontSpec::default()
        },
        runs: vec![TextRun::new(title)],
    });
    (element, bbox)
}

/// `draw(text, id, version, diag)`.
pub fn draw(db: &FlowDb, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
    let config = &resolved.config;
    let fc = &config.flowchart;
    let styles = FlowchartStyles::resolve(&resolved.theme_variables, config);

    let mut shapes = ShapeCtx::new(&ctx.metrics);
    shapes.vars = styles.vars.clone();
    shapes.html_labels = effective_html_labels(config);
    shapes.wrapping_width = *fc.wrapping_width;

    let mut data = db.get_data();
    data.diagram_type = "flowchart-v2".to_string();
    data.diagram_id = DIAGRAM_ID.to_string();
    data.direction = db.direction().and_then(Dir::parse).unwrap_or_default();
    data.node_spacing = Some(or_default(*fc.node_spacing, 50.0));
    data.rank_spacing = Some(or_default(*fc.rank_spacing, 50.0));
    data.markers = vec![MarkerType::Point, MarkerType::Circle, MarkerType::Cross];

    let unified = UnifiedConfig {
        node_spacing: raw_number(resolved, "nodeSpacing"),
        rank_spacing: raw_number(resolved, "rankSpacing"),
        flowchart_node_spacing: Some(*fc.node_spacing),
        flowchart_rank_spacing: Some(*fc.rank_spacing),
        curve: Some(fc.curve.clone()),
        sub_graph_title_margin_top: *fc.sub_graph_title_margin.top,
        sub_graph_title_margin_bottom: *fc.sub_graph_title_margin.bottom,
    };
    let rendered = render_with_dagre(&data, &unified, &shapes)?;

    let mut children = vec![Element::Group(rendered.root)];
    let mut bounds = rendered.bounds;
    if let Some(title) = resolved.title().filter(|t| !t.is_empty()) {
        let (element, bbox) = title_element(title, bounds, *fc.title_top_margin, &styles, ctx);
        children.push(element);
        bounds = bounds.union(bbox);
    }

    // `setupViewPortForSVG(svg, padding, 'flowchart', useMaxWidth)`.
    let padding = *fc.diagram_padding;
    let width = bounds.width + 2.0 * padding;
    let height = bounds.height + 2.0 * padding;
    let mut scene = Scene::new(width, height);
    scene.view_box = ViewBox::new(bounds.x - padding, bounds.y - padding, width, height);
    scene.font_family = styles.font_family.clone();
    scene.defs = rendered.markers.into_iter().map(Element::Marker).collect();
    scene.root = Group {
        common: Common {
            class: Some("flowchart".to_string()),
            style: Style {
                fill: Some(Paint::Color(styles.text_color)),
                font: FontSpec {
                    size: Some(styles.font_size),
                    ..FontSpec::default()
                },
                ..Style::default()
            },
            ..Common::default()
        },
        children,
    };
    Ok(scene)
}
