//! `requirementRenderer.ts` (mermaid 12.2.0): `getData` into the unified
//! renderer with dagre and the `requirement_contains` and
//! `requirement_arrow` markers, the title, and `setupViewPortForSVG`.
//!
//! The rules of `styles::requirement` that the shared renderer has no hook
//! for are applied to its output afterwards: the marker paint
//! (`marker { fill; stroke: relationColor }`), the relationship line
//! stroke (`.relationshipLine`), and the SVG edge label text color.

use serde_json::Value;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::layout::unified::{LayoutData, UnifiedConfig, render_with_dagre};
use crate::mermaid::scene::markers::{MarkerType, insert_markers};
use crate::mermaid::scene::{
    Common, Element, FontSpec, Group, Paint, Scene, Style, Text, TextAnchor, TextRun, ViewBox,
};
use crate::mermaid::shapes::measure::LabelOpts;
use crate::mermaid::shapes::util::BBox;
use crate::mermaid::shapes::{NodeStyleVars, ShapeCtx};
use crate::mermaid::styles::requirement::{RELATIONSHIP_STROKE_WIDTH, RequirementStyles};

use super::db::RequirementDb;

/// mermaid-cli renders into `<svg id="my-svg">`; marker and node ids hang
/// off it.
pub const DIAGRAM_ID: &str = "my-svg";
/// `data4Layout.type = diag.type`.
pub const DIAGRAM_TYPE: &str = "requirement";
/// `const padding = 8` in `draw`, passed to `setupViewPortForSVG`.
const VIEWPORT_PADDING: f64 = 8.0;
/// `conf?.nodeSpacing ?? 50`, `conf?.rankSpacing ?? 50`.
const DEFAULT_SPACING: f64 = 50.0;
/// `conf?.titleTopMargin ?? 25`.
const DEFAULT_TITLE_TOP_MARGIN: f64 = 25.0;

fn number(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// A truthy number, as `x || 50` reads it.
fn truthy(v: Option<f64>) -> Option<f64> {
    v.filter(|n| *n != 0.0 && !n.is_nan())
}

/// The `getConfig().state` values `draw` reads: the renderer destructures
/// `state: conf`, not the `requirement` section.
struct Conf {
    node_spacing: f64,
    rank_spacing: f64,
    title_top_margin: f64,
}

impl Conf {
    fn from_resolved(resolved: &Resolved) -> Conf {
        let state = resolved.raw.get("state");
        let state_num = |key: &str| number(state.and_then(|s| s.get(key)));
        Conf {
            node_spacing: state_num("nodeSpacing").unwrap_or(DEFAULT_SPACING),
            rank_spacing: state_num("rankSpacing").unwrap_or(DEFAULT_SPACING),
            title_top_margin: state_num("titleTopMargin").unwrap_or(DEFAULT_TITLE_TOP_MARGIN),
        }
    }
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

/// `.relationshipLine { stroke: relationColor; stroke-width: 1px }`. The
/// edge's own `stroke-dasharray` stays, as its inline style wins.
fn restyle_relationship_lines(root: &mut Group, styles: &RequirementStyles) {
    for_each_group(root, &mut |g| {
        for child in &mut g.children {
            if let Element::Path(p) = child
                && has_class(p.common.class.as_deref(), "relationshipLine")
            {
                p.common.style.stroke = Some(Paint::Color(styles.relation_color));
                p.common.style.stroke_width = Some(RELATIONSHIP_STROKE_WIDTH);
            }
        }
    });
}

/// SVG edge labels: `.edgeLabel .label text { fill: relationLabelColor }`
/// (HTML labels are spans, which `.label text, span` colors instead).
///
/// The label goes in as plain text, whose word split (`nonMarkdownToLines`:
/// `<[^>]+>|[^\s<>]+`) drops the closing `>` of `<<satisfies>>`; mermaid
/// lays the markdown out and decodes the entities per word instead, so the
/// run gets its last `>` back here. The background stays as measured, one
/// `>` narrower than the oracle's.
fn restyle_svg_edge_labels(root: &mut Group, styles: &RequirementStyles) {
    fn paint(g: &mut Group, color: &Paint) {
        for child in &mut g.children {
            match child {
                Element::Text(t) => {
                    t.common.style.fill = Some(color.clone());
                    for run in &mut t.runs {
                        if run.text.starts_with("<<") && !run.text.ends_with(">>") {
                            run.text.push('>');
                        }
                    }
                }
                Element::Group(c) => paint(c, color),
                _ => {}
            }
        }
    }
    let color = Paint::Color(styles.relation_label_color);
    for_each_group(root, &mut |g| {
        if has_class(g.common.class.as_deref(), "edgeLabel") {
            paint(g, &color);
        }
    });
}

/// `utils.insertTitle(svg, 'requirementDiagramTitleText', titleTopMargin,
/// title)`: a middle-anchored `text` at the content's horizontal centre,
/// `titleTopMargin` above the top, in the root font (no rule matches the
/// class).
fn title_element(title: &str, bounds: BBox, conf: &Conf, ctx: &ShapeCtx) -> (Element, BBox) {
    let x = bounds.x + bounds.width / 2.0;
    let y = -conf.title_top_margin;
    let opts = LabelOpts {
        font: ctx.font(),
        html: false,
        width: None,
        min_width: 0.0,
        markdown: false,
    };
    let m = ctx.measurer.measure(title, &opts);
    let text = Text {
        common: Common {
            class: Some("requirementDiagramTitleText".to_string()),
            ..Common::default()
        },
        x,
        y,
        anchor: Some(TextAnchor::Middle),
        baseline: None,
        font: FontSpec::default(),
        runs: vec![TextRun::new(title)],
    };
    let size = ctx.vars.font_size;
    let bbox = BBox {
        x: x - m.width / 2.0,
        y: y - size * 0.9,
        width: m.width,
        height: m.height.max(size * 1.1),
    };
    (Element::Text(text), bbox)
}

/// Lays out and draws the requirement diagram.
pub fn draw(
    db: &RequirementDb,
    resolved: &Resolved,
    ctx: &RenderCtx<'_>,
) -> Result<Scene, HelperError> {
    let cfg = &resolved.config;
    let tv = &resolved.theme_variables;
    let conf = Conf::from_resolved(resolved);

    let mut vars = NodeStyleVars::from_theme(tv, cfg);
    let styles = RequirementStyles::resolve(tv, &vars);
    styles.apply_to(&mut vars);
    let mut shapes = ShapeCtx::new(&ctx.metrics);
    shapes.vars = vars;
    shapes.html_labels = cfg.effective_html_labels();

    let (mut nodes, mut edges, direction) = db.get_data();
    if !shapes.html_labels {
        // `createText` decodes `&lt;`/`&gt;` in SVG text (`decodeHTMLEntities`);
        // the label is plain text, so it goes in decoded and unparsed.
        for edge in &mut edges {
            if let Some(label) = edge.label.as_mut() {
                *label = label.replace("&lt;", "<").replace("&gt;", ">");
            }
            edge.label_type = Some("text".to_string());
        }
    }
    for node in &mut nodes {
        // The unified renderer gives every node the SVG id as a prefix.
        node.dom_id = Some(format!("{DIAGRAM_ID}-{}", node.id));
    }
    let marker_types = vec![
        MarkerType::RequirementContains,
        MarkerType::RequirementArrow,
    ];
    let data = LayoutData {
        nodes,
        edges,
        direction,
        node_spacing: Some(conf.node_spacing),
        rank_spacing: Some(conf.rank_spacing),
        diagram_type: DIAGRAM_TYPE.to_string(),
        diagram_id: DIAGRAM_ID.to_string(),
        markers: marker_types.clone(),
    };
    let flow = &cfg.flowchart;
    let unified = UnifiedConfig {
        node_spacing: truthy(number(resolved.raw.get("nodeSpacing"))),
        rank_spacing: truthy(number(resolved.raw.get("rankSpacing"))),
        flowchart_node_spacing: Some(*flow.node_spacing),
        flowchart_rank_spacing: Some(*flow.rank_spacing),
        curve: Some(flow.curve.clone()),
        sub_graph_title_margin_top: *flow.sub_graph_title_margin.top,
        sub_graph_title_margin_bottom: *flow.sub_graph_title_margin.bottom,
    };
    let mut rendered = render_with_dagre(&data, &unified, &shapes)?;

    restyle_relationship_lines(&mut rendered.root, &styles);
    if !shapes.html_labels {
        restyle_svg_edge_labels(&mut rendered.root, &styles);
    }
    // `marker { fill: relationColor; stroke: relationColor }`. No edge sets
    // an inline stroke, so there are no colored marker clones to keep.
    let mut markers = insert_markers(&marker_types, DIAGRAM_TYPE, DIAGRAM_ID);
    let marker_paint = Style {
        fill: Some(Paint::Color(styles.relation_color)),
        stroke: Some(Paint::Color(styles.relation_color)),
        ..Style::default()
    };
    for m in &mut markers {
        m.apply_paint(&marker_paint);
    }

    let mut bounds = rendered.bounds;
    let mut children = vec![Element::Group(rendered.root)];
    if let Some(title) = resolved.title().filter(|t| !t.is_empty()) {
        let (el, bbox) = title_element(title, bounds, &conf, &shapes);
        children.push(el);
        bounds = bounds.union(bbox);
    }

    // setupViewPortForSVG(svg, 8, 'requirementDiagram', useMaxWidth)
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
    scene.defs = markers.into_iter().map(Element::Marker).collect();
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
