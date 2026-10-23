//! Port of mermaid 12.2.0 `diagrams/state/stateRenderer-v3-unified.ts`
//! (`draw`): the layout data goes through the unified dagre renderer with
//! the `barb` marker, then `insertTitle` and `setupViewPortForSVG` with an
//! 8 px padding turn the content box into the viewBox.

use std::collections::HashMap;

use serde_json::Value;

use super::fetch::Fetched;
use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::layout::unified::{LayoutData, UnifiedConfig, render_with_dagre};
use crate::mermaid::scene::markers::MarkerType;
use crate::mermaid::scene::{
    Baseline, Common, Element, FontSpec, FontWeight, Group, Length, Paint, Scene, Style, Text,
    TextAnchor, TextRun, ViewBox,
};
use crate::mermaid::shapes::util::{BBox, styles2string};
use crate::mermaid::shapes::{Node, ShapeCtx};
use crate::mermaid::styles::state::{NodeInfo, StateStyles, TITLE_FONT_SIZE};
use crate::text::TextMetrics;
use crate::text::measure::{FontSpec as MeasureSpec, FontStyle as MeasureStyle};

/// The `padding` `draw` hands `setupViewPortForSVG`.
const VIEWPORT_PADDING: f64 = 8.0;
/// `data4Layout.nodeSpacing = conf?.nodeSpacing || 50`, same for ranks.
const DEFAULT_SPACING: f64 = 50.0;

/// A positive number from the raw config, as `||` reads it.
fn truthy_number(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(Value::as_f64)
        .filter(|v| *v != 0.0 && !v.is_nan())
}

/// What the style walk needs per node group.
fn node_infos(nodes: &[Node]) -> HashMap<String, NodeInfo> {
    nodes
        .iter()
        .map(|n| {
            let labels = styles2string(n).label_styles;
            let info = NodeInfo {
                shape: n.shape.clone(),
                italic: labels
                    .get("font-style")
                    .is_some_and(|v| v.trim() == "italic"),
                bold: labels
                    .get("font-weight")
                    .is_some_and(|v| matches!(v.trim(), "bold" | "bolder" | "700" | "800" | "900")),
            };
            (n.dom_id().to_string(), info)
        })
        .collect()
}

/// Places HTML label text on the baseline Blink gives it.
///
/// The shared label builder draws an HTML line at `dy = lineHeight / 2`
/// with `dominant-baseline: central`, which resvg resolves from the
/// unrounded font ascent and descent. Blink lays the line box out with the
/// ascent and descent rounded to whole pixels and the half-leading
/// floored, so its baseline sits at `floor((lineHeight - (A + D)) / 2) +
/// A`; for Trebuchet MS at 16 px that is 17 px into a 24 px line, where
/// `central` lands at 17.73 px and the text reads one pixel low against
/// the oracle. This rewrites the first line's `dy` to the Blink baseline
/// and switches to the alphabetic baseline; later lines keep their
/// `lineHeight` steps.
fn blink_html_baselines(group: &mut Group, metrics: &TextMetrics<'_>, family: &str, size: f64) {
    for child in &mut group.children {
        match child {
            Element::Group(g) => blink_html_baselines(g, metrics, family, size),
            Element::Text(t) if t.baseline == Some(Baseline::Central) => {
                blink_text_baseline(t, metrics, family, size);
                merge_runs(t);
            }
            _ => {}
        }
    }
}

fn blink_text_baseline(t: &mut Text, metrics: &TextMetrics<'_>, family: &str, size: f64) {
    let family = t.font.family.clone().unwrap_or_else(|| family.to_string());
    let size = t.font.size.unwrap_or(size);
    let mut spec = MeasureSpec::new(family, size as f32);
    if matches!(t.font.weight, Some(FontWeight::Bold)) {
        spec = spec.weight(700);
    }
    if matches!(t.font.style, Some(crate::mermaid::scene::FontStyle::Italic)) {
        spec = spec.style(MeasureStyle::Italic);
    }
    let shaped = metrics.shape("x", &spec);
    let (ascent, descent) = (f64::from(shaped.ascent), f64::from(shaped.descent));
    let Some(first) = t.runs.first_mut() else {
        return;
    };
    let Some(Length::Px(half_line)) = first.dy else {
        return;
    };
    let line_height = half_line * 2.0;
    first.dy = Some(Length::Px(
        ((line_height - (ascent + descent)) / 2.0).floor() + ascent,
    ));
    t.baseline = Some(Baseline::Alphabetic);
}

/// Joins the word and space runs of one HTML line into one `tspan`.
///
/// The shared label builder keeps every word and every space of an HTML
/// line as its own run. The serializer indents each `tspan` on its own
/// line, and usvg reads that indentation as one more space between runs,
/// so `The state` draws with a double gap. A run with no position and the
/// same font and fill as the one before it continues the same text, so
/// joining them changes nothing but the whitespace.
fn merge_runs(t: &mut Text) {
    let mut merged: Vec<TextRun> = Vec::with_capacity(t.runs.len());
    for run in t.runs.drain(..) {
        match merged.last_mut() {
            Some(prev)
                if run.x.is_none()
                    && run.dy.is_none()
                    && run.font == prev.font
                    && run.fill == prev.fill =>
            {
                prev.text.push_str(&run.text);
            }
            _ => merged.push(run),
        }
    }
    t.runs = merged;
}

/// `utils.insertTitle(svg, 'statediagramTitleText', titleTopMargin, title)`:
/// a centred text above the content, and its box.
fn title_text(
    title: &str,
    bounds: &BBox,
    top_margin: f64,
    styles: &StateStyles,
    ctx: &RenderCtx<'_>,
) -> (Element, BBox) {
    let x = bounds.x + bounds.width / 2.0;
    let y = -top_margin;
    let spec = MeasureSpec::new(styles.vars.font_family.clone(), TITLE_FONT_SIZE as f32);
    let shaped = ctx.metrics.shape(title, &spec);
    let width = f64::from(shaped.width);
    let ascent = f64::from(shaped.ascent);
    let height = ascent + f64::from(shaped.descent);
    let text = Element::Text(Text {
        common: Common {
            class: Some("statediagramTitleText".to_string()),
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
        runs: vec![TextRun {
            text: title.to_string(),
            x: None,
            dy: None,
            font: FontSpec::default(),
            fill: None,
        }],
    });
    let bbox = BBox {
        x: x - width / 2.0,
        y: y - ascent,
        width,
        height,
    };
    (text, bbox)
}

pub fn render(
    resolved: &Resolved,
    ctx: &RenderCtx<'_>,
    fetched: Fetched,
    diagram_id: &str,
) -> Result<Scene, HelperError> {
    let cfg = &resolved.config;
    let raw = &resolved.raw;
    let styles = StateStyles::resolve(&resolved.theme_variables, cfg);

    let mut shapes = ShapeCtx::new(&ctx.metrics);
    shapes.vars = styles.node_vars();
    shapes.html_labels = cfg.effective_html_labels();
    shapes.state_padding = *cfg.state.padding;
    shapes.wrapping_width = *cfg.flowchart.wrapping_width;

    let infos = node_infos(&fetched.nodes);
    let data = LayoutData {
        nodes: fetched.nodes,
        edges: fetched.edges,
        direction: fetched.direction,
        node_spacing: Some(
            truthy_number(raw.pointer("/state/nodeSpacing")).unwrap_or(DEFAULT_SPACING),
        ),
        rank_spacing: Some(
            truthy_number(raw.pointer("/state/rankSpacing")).unwrap_or(DEFAULT_SPACING),
        ),
        diagram_type: "stateDiagram".to_string(),
        diagram_id: diagram_id.to_string(),
        markers: vec![MarkerType::Barb],
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
    let rendered = render_with_dagre(&data, &config, &shapes)?;
    let mut root = rendered.root;
    styles.apply(&mut root, &infos);
    blink_html_baselines(
        &mut root,
        &ctx.metrics,
        &styles.vars.font_family,
        styles.vars.font_size,
    );

    let mut bounds = rendered.bounds;
    let mut children = vec![Element::Group(root)];
    if let Some(title) = resolved.title().filter(|t| !t.is_empty()) {
        let (text, bbox) = title_text(title, &bounds, *cfg.state.title_top_margin, &styles, ctx);
        children.push(text);
        bounds = bounds.union(bbox);
    }

    // `setupViewPortForSVG(svg, 8, 'statediagram', useMaxWidth)`.
    let width = bounds.width + 2.0 * VIEWPORT_PADDING;
    let height = bounds.height + 2.0 * VIEWPORT_PADDING;
    let mut scene = Scene::new(width, height);
    scene.view_box = ViewBox::new(
        bounds.x - VIEWPORT_PADDING,
        bounds.y - VIEWPORT_PADDING,
        width,
        height,
    );
    scene.font_family = styles.vars.font_family.clone();
    scene.defs = rendered.markers.into_iter().map(Element::Marker).collect();
    // The stylesheet's root rule: `fill: textColor; font-size`.
    scene.root = Group {
        common: Common::with_style(Style {
            fill: Some(Paint::Color(styles.text_color)),
            font: FontSpec {
                size: Some(styles.vars.font_size),
                ..FontSpec::default()
            },
            ..Style::default()
        }),
        children,
    };
    Ok(scene)
}
