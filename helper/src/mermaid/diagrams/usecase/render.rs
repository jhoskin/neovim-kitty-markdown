//! `usecaseRenderer.ts` (mermaid 12.2.0): `getData` and
//! `prepareUsecaseLayoutData` into the unified renderer with dagre, the
//! title, and `setupViewPortForSVG` with `diagramPadding`.
//!
//! The use case stylesheet sets fonts per kind of element through the
//! `--mermaid-usecase-*` custom properties: actor labels in the actor
//! font (14 px Open Sans by default), use case labels in the use case
//! font (12 px), edge labels at 10 px, everything else in the theme font.
//! The shared shapes measure every label in the theme font, so a
//! [`KindMeasurer`] wraps the text measurer: the renderer marks each
//! label with a private-use character that names its font, and the
//! measurer strips the mark and measures in that font. After the render,
//! [`restyle`] applies the stylesheet rules the shapes have no hook for
//! (see `styles::usecase`).

use std::collections::HashMap;

use serde_json::Value;

use super::db::{DataConfig, NodeKind, UsecaseDb};
use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::layout::unified::{LayoutData, UnifiedConfig, render_with_dagre};
use crate::mermaid::scene::markers::MarkerType;
use crate::mermaid::scene::{
    Common, Element, FontSpec, FontWeight, Group, LineCap, Marker, Paint, Scene, Style, Text,
    TextRun, ViewBox,
};
use crate::mermaid::shapes::measure::{LabelBox, LabelMeasurer, LabelOpts};
use crate::mermaid::shapes::util::{
    BBox, Declarations, NodeStyles, parse_css_color, styles2string,
};
use crate::mermaid::shapes::{Node, NodeExtra, NodeStyleVars, ShapeCtx};
use crate::mermaid::styles::usecase::{
    DASHED_RELATIONSHIP, EDGE_LABEL_FONT_SIZE, EDGE_LABEL_PADDING_X, SHAPE_STROKE_WIDTH,
    UsecaseStyles,
};

/// mermaid-cli renders into `<svg id="my-svg">`.
pub const DIAGRAM_ID: &str = "my-svg";
/// `conf?.nodeSpacing || 50`-style fallbacks of the unified renderer.
const DEFAULT_SPACING: f64 = 50.0;

/// The marks [`KindMeasurer`] reads: the actor font, the use case font,
/// and the edge label font.
const MARK_ACTOR: char = '\u{E000}';
const MARK_USECASE: char = '\u{E001}';
const MARK_EDGE: char = '\u{E002}';

fn number(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().trim_end_matches("px").parse().ok(),
        _ => None,
    }
}

fn truthy(v: Option<f64>) -> Option<f64> {
    v.filter(|n| *n != 0.0 && !n.is_nan())
}

fn weight(v: Option<&Value>) -> Option<FontWeight> {
    match v? {
        Value::Number(n) => n.as_u64().map(|w| FontWeight::Number(w as u16)),
        Value::String(s) => match s.trim() {
            "normal" => Some(FontWeight::Normal),
            "bold" => Some(FontWeight::Bold),
            other => other.parse::<u16>().ok().map(FontWeight::Number),
        },
        _ => None,
    }
}

/// The `usecase` config `getData` merges (`{...state.config,
/// ...globalConfig.usecase}`).
struct Conf {
    data: DataConfig,
    node_spacing: f64,
    rank_spacing: f64,
    actor_font: FontSpec,
    usecase_font: FontSpec,
    diagram_padding: f64,
}

impl Conf {
    fn from_resolved(resolved: &Resolved) -> Conf {
        let uc = resolved.raw.get("usecase");
        let get = |key: &str| uc.and_then(|u| u.get(key));
        let num = |key: &str, default: f64| number(get(key)).unwrap_or(default);
        let family = |key: &str| {
            get(key)
                .and_then(Value::as_str)
                .unwrap_or("\"Open Sans\", sans-serif")
                .to_string()
        };
        Conf {
            data: DataConfig {
                wrapping_width: num("wrappingWidth", 120.0),
                min_node_width: num("minNodeWidth", 120.0),
            },
            node_spacing: truthy(number(get("nodeSpacing"))).unwrap_or(DEFAULT_SPACING),
            rank_spacing: truthy(number(get("rankSpacing"))).unwrap_or(DEFAULT_SPACING),
            actor_font: FontSpec {
                family: Some(family("actorFontFamily")),
                size: Some(num("actorFontSize", 14.0)),
                weight: weight(get("actorFontWeight")),
                style: None,
            },
            usecase_font: FontSpec {
                family: Some(family("usecaseFontFamily")),
                size: Some(num("usecaseFontSize", 12.0)),
                weight: weight(get("usecaseFontWeight")),
                style: None,
            },
            diagram_padding: num("diagramPadding", 20.0),
        }
    }
}

/// Measures a marked label in its kind's font, an unmarked one as the
/// inner measurer would.
struct KindMeasurer<'m> {
    inner: &'m dyn LabelMeasurer,
    actor: FontSpec,
    usecase: FontSpec,
    edge: FontSpec,
}

impl LabelMeasurer for KindMeasurer<'_> {
    fn measure(&self, text: &str, opts: &LabelOpts) -> LabelBox {
        let Some(mark) = text
            .chars()
            .find(|c| matches!(*c, MARK_ACTOR | MARK_USECASE | MARK_EDGE))
        else {
            return self.inner.measure(text, opts);
        };
        let font = match mark {
            MARK_ACTOR => &self.actor,
            MARK_USECASE => &self.usecase,
            _ => &self.edge,
        };
        let stripped: String = text.chars().filter(|c| *c != mark).collect();
        let opts = LabelOpts {
            font: font.merged_over(&opts.font),
            ..opts.clone()
        };
        let mut bbox = self.inner.measure(&stripped, &opts);
        if mark == MARK_EDGE && opts.html && !bbox.is_empty() {
            bbox.width += 2.0 * EDGE_LABEL_PADDING_X;
            for line in &mut bbox.lines {
                line.width += 2.0 * EDGE_LABEL_PADDING_X;
            }
        }
        bbox
    }
    fn text_width(&self, text: &str, family: &str, size: f64) -> f64 {
        self.inner.text_width(text, family, size)
    }
}

fn mark(text: &mut Option<String>, mark: char) {
    if let Some(t) = text.as_mut().filter(|t| !t.is_empty()) {
        t.insert(0, mark);
    }
}

/// What [`restyle`] needs per node group: its kind and its own styles.
struct NodeInfo {
    kind: NodeKind,
    styles: NodeStyles,
}

/// What [`restyle`] needs per edge path: its relationship type, its
/// inline declarations, and its classes.
struct EdgeInfo {
    kind: &'static str,
    inline: Declarations,
    animated: bool,
}

fn decl_color(decls: &Declarations, key: &str) -> Option<Paint> {
    decls.get(key).and_then(|v| parse_css_color(v))
}

/// A `stroke-width` declaration in pixels (`2px`, `2`).
fn decl_width(decls: &Declarations) -> Option<f64> {
    decls
        .get("stroke-width")
        .and_then(|v| v.trim().trim_end_matches("px").trim().parse().ok())
}

fn has_class(common: &Common, class: &str) -> bool {
    common
        .class
        .as_deref()
        .is_some_and(|c| c.split_whitespace().any(|x| x == class))
}

fn class_starts(common: &Common, prefix: &str) -> bool {
    common
        .class
        .as_deref()
        .is_some_and(|c| c.split_whitespace().any(|x| x.starts_with(prefix)))
}

/// Sets the font and fill of every text below `g`.
fn paint_texts(g: &mut Group, font: Option<&FontSpec>, fill: Option<Paint>) {
    for child in &mut g.children {
        match child {
            Element::Text(t) => {
                if let Some(f) = font {
                    t.font = f.merged_over(&t.font);
                }
                if let Some(p) = &fill {
                    t.common.style.fill = Some(p.clone());
                }
            }
            Element::Group(c) => paint_texts(c, font, fill.clone()),
            _ => {}
        }
    }
}

/// The actor glyph: the group takes the fill, stroke, and width; the
/// paths and circles take the actor border directly (the descendant rule
/// beats what they inherit) and keep a hollow `fill="none"`.
fn restyle_actor(g: &mut Group, info: &NodeInfo, s: &UsecaseStyles) {
    let decls = &info.styles.node_styles;
    let fill = decl_color(decls, "fill").unwrap_or(Paint::Color(s.actor_bkg));
    let stroke = decl_color(decls, "stroke").unwrap_or(Paint::Color(s.actor_border));
    let width = decl_width(decls).unwrap_or(SHAPE_STROKE_WIDTH);
    for child in &mut g.children {
        let Element::Group(glyph) = child else {
            continue;
        };
        if !class_starts(&glyph.common, "usecase-actor-glyph") {
            continue;
        }
        glyph.common.style.fill = Some(fill.clone());
        glyph.common.style.stroke = Some(stroke.clone());
        glyph.common.style.stroke_width = Some(width);
        for part in &mut glyph.children {
            let common = match part {
                Element::Path(p) => &mut p.common,
                Element::Circle(c) => &mut c.common,
                _ => continue,
            };
            let hollow = common.style.fill == Some(Paint::None);
            if has_class(common, "usecase-business-marker") {
                // `stroke: inherit !important; stroke-width: inherit
                // !important`.
                common.style.stroke = Some(stroke.clone());
                common.style.fill = Some(Paint::None);
            } else {
                common.style.stroke = Some(Paint::Color(s.actor_border));
                common.style.fill = Some(if hollow { Paint::None } else { fill.clone() });
            }
            common.style.stroke_width = Some(width);
        }
    }
}

/// The use case body: `fill`, `stroke`, `2px` unless the inline style
/// sets them; the business marker takes the border colour.
fn restyle_usecase(g: &mut Group, info: &NodeInfo, s: &UsecaseStyles) {
    let decls = &info.styles.node_styles;
    let fill = decl_color(decls, "fill").unwrap_or(Paint::Color(s.bkg));
    let stroke = decl_color(decls, "stroke").unwrap_or(Paint::Color(s.border));
    for child in &mut g.children {
        match child {
            Element::Ellipse(e) => {
                e.common.style.fill = Some(fill.clone());
                e.common.style.stroke = Some(stroke.clone());
                e.common.style.stroke_width = Some(decl_width(decls).unwrap_or(SHAPE_STROKE_WIDTH));
            }
            Element::Rect(r)
                if class_starts(&r.common, "label-container") || has_class(&r.common, "basic") =>
            {
                r.common.style.fill = Some(fill.clone());
                r.common.style.stroke = Some(stroke.clone());
                r.common.style.stroke_width = Some(decl_width(decls).unwrap_or(SHAPE_STROKE_WIDTH));
            }
            Element::Path(p) if has_class(&p.common, "usecase-business-marker") => {
                p.common.style.stroke = Some(stroke.clone());
                p.common.style.fill = Some(Paint::None);
                p.common.style.stroke_width = Some(decl_width(decls).unwrap_or(1.0));
            }
            _ => {}
        }
    }
}

/// The JSON table cells: `fill: mainBkg; stroke: nodeBorder`, under the
/// inline style.
fn restyle_json(g: &mut Group, info: &NodeInfo, s: &UsecaseStyles) {
    fn walk(g: &mut Group, fill: &Paint, stroke: &Paint, width: Option<f64>) {
        for child in &mut g.children {
            match child {
                Element::Rect(r) => {
                    r.common.style.fill = Some(fill.clone());
                    r.common.style.stroke = Some(stroke.clone());
                    r.common.style.stroke_width = Some(width.unwrap_or(1.0));
                }
                Element::Group(c) => walk(c, fill, stroke, width),
                _ => {}
            }
        }
    }
    let decls = &info.styles.node_styles;
    let fill = decl_color(decls, "fill").unwrap_or(Paint::Color(s.main_bkg));
    let stroke = decl_color(decls, "stroke").unwrap_or(Paint::Color(s.node_border));
    walk(g, &fill, &stroke, decl_width(decls));
}

struct Restyle<'a> {
    nodes: &'a HashMap<String, NodeInfo>,
    edges: &'a HashMap<String, EdgeInfo>,
    styles: &'a UsecaseStyles,
    conf: &'a Conf,
    edge_font: &'a FontSpec,
}

fn label_color(info: &NodeInfo) -> Option<Paint> {
    decl_color(&info.styles.label_styles, "color")
}

/// Applies the stylesheet rules to the rendered tree.
fn restyle(g: &mut Group, r: &Restyle<'_>) {
    if let Some(info) = g.common.id.as_deref().and_then(|id| r.nodes.get(id)) {
        let user_color = label_color(info);
        match info.kind {
            NodeKind::Actor => {
                restyle_actor(g, info, r.styles);
                let fill = user_color.unwrap_or(Paint::Color(r.styles.actor_text));
                paint_texts(g, Some(&r.conf.actor_font), Some(fill));
            }
            NodeKind::UseCase => {
                restyle_usecase(g, info, r.styles);
                let fill = user_color.unwrap_or(Paint::Color(r.styles.primary_text));
                paint_texts(g, Some(&r.conf.usecase_font), Some(fill));
            }
            NodeKind::Note => {
                paint_texts(g, None, Some(Paint::Color(r.styles.note_text)));
            }
            NodeKind::Json => {
                restyle_json(g, info, r.styles);
                let fill = user_color.unwrap_or(Paint::Color(r.styles.primary_text));
                paint_texts(g, None, Some(fill));
            }
            NodeKind::Boundary => {}
        }
        return;
    }
    if has_class(&g.common, "edgeLabel") {
        paint_texts(
            g,
            Some(r.edge_font),
            Some(Paint::Color(r.styles.primary_text)),
        );
        return;
    }
    for child in &mut g.children {
        match child {
            Element::Group(c) => restyle(c, r),
            Element::Path(p) => {
                let Some(info) = p.common.id.as_deref().and_then(|id| r.edges.get(id)) else {
                    continue;
                };
                let style = &mut p.common.style;
                if !info.inline.contains_key("stroke") {
                    style.stroke = Some(Paint::Color(match info.kind {
                        "include" => r.styles.include_line,
                        "extend" => r.styles.extend_line,
                        _ => r.styles.line_color,
                    }));
                }
                if matches!(info.kind, "include" | "extend" | "note")
                    && !info.inline.contains_key("stroke-dasharray")
                {
                    style.stroke_dasharray = Some(vec![DASHED_RELATIONSHIP]);
                }
                if info.animated {
                    // `.edge-animation-fast { stroke-dasharray: 9,5
                    // !important; stroke-linecap: round }`.
                    style.stroke_dasharray = Some(vec![9.0, 5.0]);
                    style.stroke_linecap = Some(LineCap::Round);
                }
            }
            _ => {}
        }
    }
}

/// `.marker.extension { fill: mainBkg; stroke: lineColor }`.
fn restyle_markers(markers: &mut [Marker], s: &UsecaseStyles) {
    let prefix = format!("{DIAGRAM_ID}_usecase-extension");
    for marker in markers {
        if !marker.id.starts_with(&prefix) {
            continue;
        }
        marker.style.fill = Some(Paint::Color(s.main_bkg));
        for child in &mut marker.children {
            let common = match child {
                Element::Path(p) => &mut p.common,
                Element::Polygon(p) => &mut p.common,
                _ => continue,
            };
            common.style.fill = Some(Paint::Color(s.main_bkg));
            common.style.stroke = Some(Paint::Color(s.line_color));
        }
    }
}

/// `utils.insertTitle(svg, 'usecaseDiagramTitleText', 0, title)`: a
/// start-anchored text at the content's horizontal centre on `y = 0`.
fn title_element(title: &str, bounds: BBox, shapes: &ShapeCtx) -> (Element, BBox) {
    let x = bounds.x + bounds.width / 2.0;
    let opts = LabelOpts {
        font: shapes.font(),
        html: false,
        width: None,
        min_width: 0.0,
        markdown: false,
    };
    let m = shapes.measurer.measure(title, &opts);
    let text = Text {
        common: Common {
            class: Some("usecaseDiagramTitleText".to_string()),
            ..Common::default()
        },
        x,
        y: 0.0,
        anchor: None,
        baseline: None,
        font: FontSpec::default(),
        runs: vec![TextRun::new(title)],
    };
    let size = shapes.vars.font_size;
    let bbox = BBox {
        x,
        y: -size * 0.9,
        width: m.width,
        height: m.height.max(size * 1.1),
    };
    (Element::Text(text), bbox)
}

/// Lays out and draws the use case diagram.
pub fn draw(
    db: &UsecaseDb,
    resolved: &Resolved,
    ctx: &RenderCtx<'_>,
) -> Result<Scene, HelperError> {
    let cfg = &resolved.config;
    let tv = &resolved.theme_variables;
    let conf = Conf::from_resolved(resolved);

    let mut vars = NodeStyleVars::from_theme(tv, cfg);
    let styles = UsecaseStyles::resolve(tv, &vars);
    styles.apply_to(&mut vars);
    let edge_font = FontSpec {
        family: Some(styles.font_family.clone()),
        size: Some(EDGE_LABEL_FONT_SIZE),
        weight: Some(FontWeight::Normal),
        style: None,
    };
    let measurer = KindMeasurer {
        inner: &ctx.metrics,
        actor: conf.actor_font.clone(),
        usecase: conf.usecase_font.clone(),
        edge: edge_font.clone(),
    };
    let mut shapes = ShapeCtx::new(&measurer);
    shapes.vars = vars;
    shapes.html_labels = cfg.effective_html_labels();
    shapes.wrapping_width = conf.data.wrapping_width;

    let data = db.get_data(&conf.data);
    let mut nodes: Vec<Node> = data.nodes;
    let mut node_info = HashMap::new();
    for (node, kind) in nodes.iter_mut().zip(&data.kinds) {
        let dom_id = format!("{DIAGRAM_ID}-{}", node.dom_id());
        node.dom_id = Some(dom_id.clone());
        let label_mark = match kind {
            NodeKind::Actor => Some(MARK_ACTOR),
            NodeKind::UseCase => Some(MARK_USECASE),
            _ => None,
        };
        if let Some(m) = label_mark {
            mark(&mut node.label, m);
            if let NodeExtra::Usecase { stereotype, .. } = &mut node.extra {
                mark(stereotype, m);
            }
        }
        node_info.insert(
            dom_id,
            NodeInfo {
                kind: *kind,
                styles: styles2string(node),
            },
        );
    }
    let mut edges = data.edges;
    let mut edge_info = HashMap::new();
    for (edge, kind) in edges.iter_mut().zip(&data.edge_kinds) {
        mark(&mut edge.label, MARK_EDGE);
        let mut inline = Declarations::new();
        for decl in edge.css_compiled_styles.iter().chain(&edge.style) {
            if let Some((k, v)) = decl.split_once(':') {
                inline.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
        let animated = edge
            .classes
            .as_deref()
            .is_some_and(|c| c.contains("edge-animation-"));
        edge_info.insert(
            format!("{DIAGRAM_ID}-{}", edge.id),
            EdgeInfo {
                kind,
                inline,
                animated,
            },
        );
    }

    let layout = LayoutData {
        nodes,
        edges,
        direction: data.direction,
        node_spacing: Some(conf.node_spacing),
        rank_spacing: Some(conf.rank_spacing),
        diagram_type: "usecase".to_string(),
        diagram_id: DIAGRAM_ID.to_string(),
        markers: vec![
            MarkerType::Point,
            MarkerType::Circle,
            MarkerType::Cross,
            MarkerType::Extension,
        ],
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
    let mut rendered = render_with_dagre(&layout, &unified, &shapes)?;

    restyle(
        &mut rendered.root,
        &Restyle {
            nodes: &node_info,
            edges: &edge_info,
            styles: &styles,
            conf: &conf,
            edge_font: &edge_font,
        },
    );
    restyle_markers(&mut rendered.markers, &styles);

    let mut bounds = rendered.bounds;
    let mut children = vec![Element::Group(rendered.root)];
    if let Some(title) = resolved.title().filter(|t| !t.is_empty()) {
        let (el, bbox) = title_element(title, bounds, &shapes);
        children.push(el);
        bounds = bounds.union(bbox);
    }

    // setupViewPortForSVG(svg, diagramPadding, 'usecaseDiagram', useMaxWidth)
    let padding = conf.diagram_padding;
    let width = bounds.width + padding * 2.0;
    let height = bounds.height + padding * 2.0;
    let mut scene = Scene::new(width, height);
    scene.view_box = ViewBox::new(bounds.x - padding, bounds.y - padding, width, height);
    scene.font_family = shapes.vars.font_family.clone();
    scene.defs = rendered.markers.into_iter().map(Element::Marker).collect();
    scene.root = Group {
        common: Common::with_style(Style {
            fill: Some(Paint::Color(shapes.vars.text_color)),
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
