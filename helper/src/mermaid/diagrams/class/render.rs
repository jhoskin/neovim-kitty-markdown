//! `classRenderer-v3-unified.ts` (mermaid 12.2.0): `getData` into the
//! unified renderer with dagre, the title, and `setupViewPortForSVG`.
//!
//! The shared renderer resolves the flowchart-like stylesheet rules. The
//! class rules that differ (`styles::class`) go in before the render: the
//! label color, the edge label background, and the note text color
//! (`.noteLabel .nodeLabel { color: noteTextColor }`) as the shapes' theme
//! variables, the cardinality font (`.edgeTerminals { font-size: 11px }`)
//! as the shapes' edge terminal font. The shared marker code resolves the
//! marker fills (`.extension { fill: transparent !important }` and its
//! siblings, see `markers::stylesheet_rule`).

use std::collections::HashMap;

use serde_json::Value;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::layout::unified::{LayoutData, UnifiedConfig, render_with_dagre};
use crate::mermaid::scene::markers::MarkerType;
use crate::mermaid::scene::{
    Common, Element, FontSpec, Group, Length, Paint, Rect, Scene, Style, Text, TextAnchor, ViewBox,
};
use crate::mermaid::shapes::measure::LabelOpts;
use crate::mermaid::shapes::util::BBox;
use crate::mermaid::shapes::{Classifier, NodeStyleVars, ShapeCtx};
use crate::mermaid::styles::class::{ClassStyles, EDGE_TERMINALS_FONT_SIZE};
use crate::text::TextMetrics;
use crate::text::fonts::{Style as FontStyleDb, Weight};

use super::db::{ClassDb, ClassMember, DataConfig};

/// mermaid-cli renders into `<svg id="my-svg">`; marker and node ids hang
/// off it.
pub const DIAGRAM_ID: &str = "my-svg";
/// `const padding = 8` in `draw`, passed to `setupViewPortForSVG`.
const VIEWPORT_PADDING: f64 = 8.0;
/// `conf?.nodeSpacing || 50`, `conf?.rankSpacing || 50`.
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

/// The `getConfig()` values the class renderer reads.
struct Conf {
    /// `class.padding`, undefined by default.
    class_padding: Option<f64>,
    hierarchical: bool,
    hide_empty_members_box: bool,
    /// `state.nodeSpacing` / `state.rankSpacing`: the renderer reads
    /// `getConfig().state` (`const { state: conf } = getConfig()`).
    node_spacing: f64,
    rank_spacing: f64,
    title_top_margin: f64,
}

impl Conf {
    fn from_resolved(resolved: &Resolved) -> Conf {
        let cfg = &resolved.config;
        let state = resolved.raw.get("state");
        let state_num = |key: &str| number(state.and_then(|s| s.get(key)));
        Conf {
            class_padding: number(cfg.class.extra.get("padding")),
            hierarchical: cfg.class.hierarchical_namespaces,
            hide_empty_members_box: cfg.class.hide_empty_members_box,
            node_spacing: truthy(state_num("nodeSpacing")).unwrap_or(DEFAULT_SPACING),
            rank_spacing: truthy(state_num("rankSpacing")).unwrap_or(DEFAULT_SPACING),
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

fn has_class(g: &Group, class: &str) -> bool {
    g.common
        .class
        .as_deref()
        .is_some_and(|c| c.split_whitespace().any(|x| x == class))
}

/// The underline of a static member (`parseClassifier()` returns
/// `text-decoration:underline;` for `$`). The scene has no text
/// decoration, so the underline is a rect in the text color, placed and
/// sized from the font's `post` table as Blink does, under the full
/// advance width. Blink's skip-ink gaps around descenders are left out.
fn underline_statics(
    root: &mut Group,
    statics: &HashMap<String, (Vec<bool>, Vec<bool>)>,
    metrics: &TextMetrics<'_>,
    family: &str,
) {
    let fonts = metrics.fonts();
    let face = fonts.resolve(family, Weight::NORMAL, FontStyleDb::Normal);
    let post = face.and_then(|id| {
        fonts.with_face(id, |f| {
            let upem = f64::from(f.units_per_em());
            f.underline_metrics()
                .map(|m| (-f64::from(m.position) / upem, f64::from(m.thickness) / upem))
        })
    });
    // Without a `post` table Blink falls back to a 1/9 em offset and a
    // 1/18 em stroke.
    let (offset_em, thickness_em) = post.flatten().unwrap_or((1.0 / 9.0, 1.0 / 18.0));
    for_each_group(root, &mut |node| {
        let Some((members, methods)) = node.common.id.as_ref().and_then(|id| statics.get(id))
        else {
            return;
        };
        for child in &mut node.children {
            let Element::Group(compartment) = child else {
                continue;
            };
            let flags = if has_class(compartment, "members-group") {
                members
            } else if has_class(compartment, "methods-group") {
                methods
            } else {
                continue;
            };
            let labels = compartment.children.iter_mut().filter_map(|e| match e {
                Element::Group(g) => Some(g),
                _ => None,
            });
            for (label, _) in labels.zip(flags.iter()).filter(|(_, s)| **s) {
                let Some(text) = label.children.iter().find_map(|e| match e {
                    Element::Text(t) => Some(t.clone()),
                    _ => None,
                }) else {
                    continue;
                };
                let size = text.font.size.unwrap_or(16.0);
                let baseline = match text.runs.first().and_then(|r| r.dy) {
                    Some(Length::Px(dy)) => text.y + dy,
                    _ => text.y,
                };
                let thickness = (thickness_em * size).round().max(1.0);
                label.children.push(Element::Rect(Rect {
                    common: Common::with_style(Style {
                        fill: text.common.style.fill.clone(),
                        ..Style::default()
                    }),
                    x: text.x - text.x.abs(),
                    y: baseline + (offset_em * size).round() - thickness / 2.0,
                    width: 2.0 * text.x.abs(),
                    height: thickness,
                    rx: None,
                    ry: None,
                }));
            }
        }
    });
}

/// `utils.insertTitle(svg, 'classDiagramTitleText', titleTopMargin, title)`:
/// a `text` at the content's horizontal centre, `titleTopMargin` above the
/// top, with `.attr('text-anchor', 'middle')`. The class
/// `classDiagramTitleText` matches no rule, so the text is in the root
/// font.
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
            class: Some("classDiagramTitleText".to_string()),
            ..Common::default()
        },
        x,
        y,
        anchor: Some(TextAnchor::Middle),
        baseline: None,
        font: FontSpec::default(),
        runs: vec![crate::mermaid::scene::TextRun::new(title)],
    };
    // The SVG text box: the em box above and below the baseline, centred
    // on `x`.
    let size = ctx.vars.font_size;
    let bbox = BBox {
        x: x - m.width / 2.0,
        y: y - size * 0.9,
        width: m.width,
        height: m.height.max(size * 1.1),
    };
    (Element::Text(text), bbox)
}

/// Lays out and draws the class diagram.
pub fn draw(db: &ClassDb, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
    let cfg = &resolved.config;
    let tv = &resolved.theme_variables;
    let conf = Conf::from_resolved(resolved);

    let mut vars = NodeStyleVars::from_theme(tv, cfg);
    let styles = ClassStyles::resolve(tv, &vars);
    styles.apply_to(&mut vars);
    let mut shapes = ShapeCtx::new(&ctx.metrics);
    shapes.vars = vars;
    shapes.vars.note_label_color = Some(styles.note_text_color);
    shapes.html_labels = cfg.effective_html_labels();
    // `labelHelper` wraps at `getConfig().flowchart.wrappingWidth`.
    shapes.wrapping_width = *cfg.flowchart.wrapping_width;
    shapes.class_padding = conf.class_padding.unwrap_or(12.0);
    shapes.hide_empty_members_box = conf.hide_empty_members_box;
    shapes.edge_terminal_font = FontSpec {
        size: Some(EDGE_TERMINALS_FONT_SIZE),
        ..FontSpec::default()
    };

    let data_conf = DataConfig {
        padding: conf.class_padding,
        hierarchical: conf.hierarchical,
        note_bkg_color: tv.color("noteBkgColor", "#fff5ad"),
        note_border_color: tv.color("noteBorderColor", "#aaaa33"),
    };
    let (mut nodes, edges, direction) = db.get_data(&data_conf);
    for node in &mut nodes {
        // `lookUpDomId`: with `setDiagramId(id)` the DOM ids carry the SVG
        // id; the unified renderer prefixes the other node ids the same way.
        if !node.is_group {
            let dom_id = node.dom_id.as_deref().unwrap_or(&node.id);
            node.dom_id = Some(format!("{DIAGRAM_ID}-{dom_id}"));
        }
    }
    let data = LayoutData {
        nodes,
        edges,
        direction,
        node_spacing: Some(conf.node_spacing),
        rank_spacing: Some(conf.rank_spacing),
        diagram_type: "classDiagram".to_string(),
        diagram_id: DIAGRAM_ID.to_string(),
        markers: vec![
            MarkerType::Aggregation,
            MarkerType::Extension,
            MarkerType::Composition,
            MarkerType::Dependency,
            MarkerType::Lollipop,
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
    let mut rendered = render_with_dagre(&data, &unified, &shapes)?;

    if shapes.html_labels {
        let statics: HashMap<String, (Vec<bool>, Vec<bool>)> = db
            .classes
            .values()
            .map(|c| {
                let is_static = |m: &ClassMember| m.shape_classifier() == Classifier::Static;
                (
                    format!("{DIAGRAM_ID}-{}", c.dom_id),
                    (
                        c.members.iter().map(is_static).collect(),
                        c.methods.iter().map(is_static).collect(),
                    ),
                )
            })
            .collect();
        underline_statics(
            &mut rendered.root,
            &statics,
            &ctx.metrics,
            &shapes.vars.font_family,
        );
    }
    let mut bounds = rendered.bounds;

    let mut children = vec![Element::Group(rendered.root)];
    if let Some(title) = resolved.title().filter(|t| !t.is_empty()) {
        let (el, bbox) = title_element(title, bounds, &conf, &shapes);
        children.push(el);
        bounds = bounds.union(bbox);
    }

    // setupViewPortForSVG(svg, 8, 'classDiagram', useMaxWidth)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::shapes::FixedMeasurer;

    #[test]
    fn the_title_is_centred_on_the_content() {
        let m = FixedMeasurer::SNAPSHOT;
        let ctx = ShapeCtx::new(&m);
        let conf = Conf {
            class_padding: None,
            hierarchical: false,
            hide_empty_members_box: false,
            node_spacing: DEFAULT_SPACING,
            rank_spacing: DEFAULT_SPACING,
            title_top_margin: DEFAULT_TITLE_TOP_MARGIN,
        };
        let bounds = BBox {
            x: 10.0,
            y: 0.0,
            width: 200.0,
            height: 100.0,
        };
        let (el, bbox) = title_element("Title", bounds, &conf, &ctx);
        let Element::Text(text) = el else {
            panic!("the title is a text");
        };
        assert_eq!(text.anchor, Some(TextAnchor::Middle));
        assert_eq!(text.x, 110.0);
        assert_eq!(bbox.x + bbox.width / 2.0, 110.0);
    }
}
