//! `erRenderer-unified.ts` (mermaid 12.2.0): `getData` into the unified
//! renderer with dagre and the four cardinality markers, the title, and
//! `setupViewPortForSVG` with an 8 px padding.
//!
//! The renderer overrides `flowchart.nodeSpacing` and `rankSpacing` with
//! `er.nodeSpacing || 140` and `er.rankSpacing || 80` before the render,
//! so those are the spacings dagre sees unless the root config sets its
//! own. The ER stylesheet rules (`styles::er`) that the shared renderer
//! has no hook for are applied to its output afterwards: the marker paint,
//! the dashed line pattern, and the 14 px relationship labels. Three
//! measuring rules the shared shapes lack go through [`ErMeasurer`]: the
//! 14 px label size, Blink's 1/64 px rounding of HTML label widths, and
//! `calculateTextWidth` in the root font.

use std::collections::HashSet;

use serde_json::Value;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::layout::unified::{LayoutData, UnifiedConfig, render_with_dagre};
use crate::mermaid::scene::markers::MarkerType;
use crate::mermaid::scene::{
    Color, Common, Element, FontSpec, Group, Marker, Paint, Scene, Style, Text, TextAnchor,
    TextRun, ViewBox,
};
use crate::mermaid::shapes::util::BBox;
use crate::mermaid::shapes::{LabelBox, LabelMeasurer, LabelOpts, ShapeCtx};
use crate::mermaid::styles::er::{DASHED, EDGE_LABEL_FONT_SIZE, ErStyles, MARKER_STROKE_WIDTH};
use crate::text::TextMetrics;
use crate::text::measure::FontSpec as MeasureSpec;

use super::db::ErDb;

/// mermaid-cli renders into `<svg id="my-svg">`; marker and node ids hang
/// off it.
pub const DIAGRAM_ID: &str = "my-svg";
/// `data4Layout.type`: the detector id, which also names the markers
/// (`my-svg_er-onlyOneStart`).
const DIAGRAM_TYPE: &str = "er";
/// `const padding = 8` in `draw`, passed to `setupViewPortForSVG`.
const VIEWPORT_PADDING: f64 = 8.0;
/// `conf?.nodeSpacing || 140`, `conf?.rankSpacing || 80`.
const DEFAULT_NODE_SPACING: f64 = 140.0;
const DEFAULT_RANK_SPACING: f64 = 80.0;
/// `insertEdgeLabel` measures a label with `createText`'s default width.
const EDGE_LABEL_WRAP_WIDTH: f64 = 200.0;

fn number(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// A truthy number, as `x || 140` reads it.
fn truthy(v: Option<f64>) -> Option<f64> {
    v.filter(|n| *n != 0.0 && !n.is_nan())
}

/// Measures the relationship labels at the 14 px of `.edgeLabel .label`.
///
/// The shared `insertEdgeLabel` measures every label in the shapes' font,
/// 16 px here, and dagre sizes the label ranks from that. This wrapper
/// stands in until the shape context carries an edge label font size: a
/// call is an edge label when it wraps at `createText`'s default 200 px
/// and its text is a relationship role. An entity label measures with the
/// wrapping width of `labelHelper` (`flowchart.wrappingWidth`) or the
/// text width plus 100 (`addText`), so it only matches when that width is
/// also 200 and the text is a role too.
///
/// It also answers `calculateTextWidth(text, config)`, which `erBox` calls
/// for the minimum entity width and the `addText` wrap width. The shared
/// shapes pass that through `measure` with no wrap width and no markdown,
/// which no other ER label does; mermaid measures it as SVG text in the
/// root `fontFamily` and `fontSize` (`calculateTextDimensions`), not in
/// the label font.
struct ErMeasurer<'a, 'f> {
    metrics: &'a TextMetrics<'f>,
    roles: HashSet<String>,
    /// `config.fontFamily` and `config.fontSize`.
    text_width_font: MeasureSpec,
}

impl LabelMeasurer for ErMeasurer<'_, '_> {
    fn measure(&self, text: &str, opts: &LabelOpts) -> LabelBox {
        let inner: &dyn LabelMeasurer = self.metrics;
        if opts.width.is_none() && !opts.markdown {
            let mut b = inner.measure(text, opts);
            b.width = f64::from(
                self.metrics
                    .calculate_text_width(text, &self.text_width_font),
            );
            return b;
        }
        let mut b = if opts.width == Some(EDGE_LABEL_WRAP_WIDTH) && self.roles.contains(text) {
            let opts = LabelOpts {
                font: FontSpec {
                    size: Some(EDGE_LABEL_FONT_SIZE),
                    ..opts.font.clone()
                },
                ..opts.clone()
            };
            inner.measure(text, &opts)
        } else {
            inner.measure(text, opts)
        };
        if opts.html {
            // Blink lays the shrink-to-fit `div` out in LayoutUnits, 1/64 px,
            // rounding the text width up; dagre's alignment choice is
            // sensitive to that last fraction.
            b.width = layout_unit_ceil(b.width);
            for line in &mut b.lines {
                line.width = layout_unit_ceil(line.width);
            }
        }
        b
    }
}

/// Rounds up to Blink's `LayoutUnit` of 1/64 px.
fn layout_unit_ceil(v: f64) -> f64 {
    (v * 64.0 - 1e-6).ceil() / 64.0
}

/// Visits every group of the tree, parents before children.
fn for_each_group(g: &mut Group, f: &mut dyn FnMut(&mut Group)) {
    f(g);
    for child in &mut g.children {
        if let Element::Group(c) = child {
            for_each_group(c, f);
        }
    }
}

fn has_class(class: Option<&str>, name: &str) -> bool {
    class.is_some_and(|c| c.split_whitespace().any(|x| x == name))
}

/// `.marker { fill: none !important; stroke: lineColor !important;
/// stroke-width: 1 }`. The shared renderer painted every unset fill with
/// `lineColor`; the paths inherit `none` again, the circles keep their own
/// white.
fn restyle_markers(markers: &mut [Marker], line_color: Color) {
    let prefix = format!("{DIAGRAM_ID}_{DIAGRAM_TYPE}-");
    for marker in markers.iter_mut().filter(|m| m.id.starts_with(&prefix)) {
        marker.style.fill = Some(Paint::None);
        marker.style.stroke = Some(Paint::Color(line_color));
        marker.style.stroke_width = Some(MARKER_STROKE_WIDTH);
        for child in &mut marker.children {
            match child {
                Element::Path(p) => {
                    p.common.style.fill = Some(Paint::None);
                    p.common.style.stroke = Some(Paint::Color(line_color));
                }
                Element::Circle(c) => {
                    c.common.style.fill = Some(Paint::Color(Color::WHITE));
                    c.common.style.stroke = Some(Paint::Color(line_color));
                }
                _ => {}
            }
        }
    }
}

/// Sets the font size of every text under `g`.
fn set_font_size(g: &mut Group, size: f64) {
    for child in &mut g.children {
        match child {
            Element::Text(t) => t.font.size = Some(size),
            Element::Group(c) => set_font_size(c, size),
            _ => {}
        }
    }
}

/// `.edge-pattern-dashed { stroke-dasharray: 8,8 }` on the relationship
/// lines, and `.edgeLabel .label { font-size: 14px }` on their labels.
/// The SVG label background (`.edgeLabel .label rect { fill:
/// edgeLabelBackground }`) has no `opacity` rule here, unlike the
/// flowchart one the shared label follows.
fn restyle_edges(root: &mut Group) {
    for_each_group(root, &mut |g| {
        let label = has_class(g.common.class.as_deref(), "label");
        if has_class(g.common.class.as_deref(), "edgeLabel") {
            set_font_size(g, EDGE_LABEL_FONT_SIZE);
        }
        for child in &mut g.children {
            match child {
                Element::Path(p)
                    if has_class(p.common.class.as_deref(), "relationshipLine")
                        && has_class(p.common.class.as_deref(), "edge-pattern-dashed") =>
                {
                    p.common.style.stroke_dasharray = Some(DASHED.to_vec());
                }
                Element::Rect(r) if label && has_class(r.common.class.as_deref(), "background") => {
                    r.common.style.opacity = None;
                }
                _ => {}
            }
        }
    });
}

/// `utils.insertTitle(svg, 'erDiagramTitleText', titleTopMargin, title)`:
/// a middle-anchored text at the content's horizontal centre,
/// `titleTopMargin` above the top, in the root font. Returns the text and
/// its box.
fn title_text(
    title: &str,
    bounds: &BBox,
    top_margin: f64,
    shapes: &ShapeCtx,
    ctx: &RenderCtx<'_>,
) -> (Element, BBox) {
    let x = bounds.x + bounds.width / 2.0;
    let y = -top_margin;
    let spec = MeasureSpec::new(
        shapes.vars.font_family.clone(),
        shapes.vars.font_size as f32,
    );
    let shaped = ctx.metrics.shape(title, &spec);
    let width = f64::from(shaped.width);
    let ascent = f64::from(shaped.ascent);
    let height = ascent + f64::from(shaped.descent);
    let text = Element::Text(Text {
        common: Common {
            class: Some("erDiagramTitleText".to_string()),
            ..Common::default()
        },
        x,
        y,
        anchor: Some(TextAnchor::Middle),
        baseline: None,
        font: FontSpec::default(),
        runs: vec![TextRun::new(title)],
    });
    let bbox = BBox {
        x: x - width / 2.0,
        y: y - ascent,
        width,
        height,
    };
    (text, bbox)
}

/// Lays out and draws the ER diagram.
pub fn draw(db: &ErDb, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
    let cfg = &resolved.config;
    let er = &cfg.er;
    let tv = &resolved.theme_variables;

    let (mut nodes, edges, direction) = db.get_data();
    let roles: HashSet<String> = edges.iter().filter_map(|e| e.label.clone()).collect();
    let measurer = ErMeasurer {
        metrics: &ctx.metrics,
        roles,
        text_width_font: MeasureSpec::new(cfg.font_family.clone(), *cfg.font_size as f32),
    };

    let html_labels = cfg.effective_html_labels();
    let mut shapes = ShapeCtx::new(&measurer);
    shapes.vars = crate::mermaid::shapes::NodeStyleVars::from_theme(tv, cfg);
    let styles = ErStyles::resolve(tv, &shapes.vars);
    styles.apply_to(&mut shapes.vars, html_labels);
    shapes.html_labels = html_labels;
    shapes.wrapping_width = *cfg.flowchart.wrapping_width;
    // `config.er?.diagramPadding ?? 10`, `config.er?.entityPadding ?? 6`,
    // `config.er!.minEntityWidth!`.
    shapes.er_diagram_padding = *er.diagram_padding;
    shapes.er_entity_padding = *er.entity_padding;
    shapes.er_min_entity_width = *er.min_entity_width;

    for node in &mut nodes {
        // `lookUpDomId`: with `setDiagramId(id)` the DOM ids carry the SVG
        // id; the unified renderer prefixes the cluster ids the same way.
        if !node.is_group {
            let dom_id = node.dom_id.as_deref().unwrap_or(&node.id);
            node.dom_id = Some(format!("{DIAGRAM_ID}-{dom_id}"));
            // `erBox` sets `node.useHtmlLabels = config.htmlLabels`, the raw
            // root value, and reads it back for its padding; unset is falsy.
            node.use_html_labels = Some(cfg.html_labels == Some(true));
        }
    }

    let data = LayoutData {
        nodes,
        edges,
        direction,
        node_spacing: None,
        rank_spacing: None,
        diagram_type: DIAGRAM_TYPE.to_string(),
        diagram_id: DIAGRAM_ID.to_string(),
        markers: vec![
            MarkerType::OnlyOne,
            MarkerType::ZeroOrOne,
            MarkerType::OneOrMore,
            MarkerType::ZeroOrMore,
        ],
    };
    let flow = &cfg.flowchart;
    let unified = UnifiedConfig {
        node_spacing: truthy(number(resolved.raw.get("nodeSpacing"))),
        rank_spacing: truthy(number(resolved.raw.get("rankSpacing"))),
        flowchart_node_spacing: Some(
            truthy(Some(*er.node_spacing)).unwrap_or(DEFAULT_NODE_SPACING),
        ),
        flowchart_rank_spacing: Some(
            truthy(Some(*er.rank_spacing)).unwrap_or(DEFAULT_RANK_SPACING),
        ),
        curve: Some(flow.curve.clone()),
        sub_graph_title_margin_top: *flow.sub_graph_title_margin.top,
        sub_graph_title_margin_bottom: *flow.sub_graph_title_margin.bottom,
    };
    let mut rendered = render_with_dagre(&data, &unified, &shapes)?;
    restyle_markers(&mut rendered.markers, styles.line_color);
    restyle_edges(&mut rendered.root);

    let mut bounds = rendered.bounds;
    let mut children = vec![Element::Group(rendered.root)];
    if let Some(title) = resolved.title().filter(|t| !t.is_empty()) {
        let (text, bbox) = title_text(title, &bounds, *er.title_top_margin, &shapes, ctx);
        children.push(text);
        bounds = bounds.union(bbox);
    }

    // setupViewPortForSVG(svg, 8, 'erDiagram', useMaxWidth)
    let width = bounds.width + VIEWPORT_PADDING * 2.0;
    let height = bounds.height + VIEWPORT_PADDING * 2.0;
    let mut scene = Scene::new(width, height);
    scene.view_box = ViewBox::new(
        bounds.x - VIEWPORT_PADDING,
        bounds.y - VIEWPORT_PADDING,
        width,
        height,
    );
    scene.font_family = shapes.vars.font_family.clone();
    scene.defs = rendered.markers.into_iter().map(Element::Marker).collect();
    // The root rule of the stylesheet: `fill: textColor; font-size`.
    scene.root = Group {
        common: Common::with_style(Style {
            fill: Some(Paint::Color(styles.text_color)),
            font: FontSpec {
                size: Some(shapes.vars.font_size),
                ..FontSpec::default()
            },
            ..Style::default()
        }),
        children,
    };
    Ok(scene)
}
