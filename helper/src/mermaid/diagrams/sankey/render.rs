//! Port of `sankeyRenderer.ts` (`draw`), mermaid 12.2.0.
//!
//! The DOM it builds, in order: an empty `<g>`, `g.nodes` with one
//! translated `g.node` and its `rect` per node, `g.node-labels` with
//! `font-size="14"` and one `text` per node (two for `labelStyle:
//! outlined`), and `g.links` with `fill="none"` and `stroke-opacity="0.5"`
//! holding one `g.link` per link with `mix-blend-mode: multiply`, a
//! `linearGradient` for `linkColor: gradient`, and the link path.
//!
//! Browser facts the port has to model:
//!
//! - The label text is `${id}\n${prefix}${value}${suffix}`. The SVG text
//!   has `white-space: normal`, so Chrome draws the newline as a space on
//!   one line. resvg follows SVG 1.1 `xml:space` and would delete it, so
//!   the scene gets the collapsed text.
//! - The label `dy` (`0em` or `0.35em`) is folded into `y`: one em is the
//!   14 px of the label group.
//! - The gradients go into the scene's `defs` instead of the link group;
//!   where a `linearGradient` sits does not change what it paints.
//! - `setupGraphViewbox(undefined, svg, 0, useMaxWidth)` reads
//!   `svg.getBBox()`. Here that is the union of the node rects, the link
//!   curves (their control points share the end points' `y`, so a curve's
//!   box is its end points' box), and the label text boxes. Strokes do not
//!   count in a `getBBox`.

use indexmap::IndexMap;
use serde_json::Value;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::diagrams::sequence::conf::{BODY_DEFAULT_FAMILY, css_family_valid};
use crate::mermaid::layout::d3::sankey::{self, Align, Sankey, SankeyGraph};
use crate::mermaid::scene::{
    BlendMode, Color, Common, Element, FontSpec, GradientStop, GradientUnits, Group, LineJoin,
    LinearGradient, Paint, Path, Rect, Scene, Style, Text, TextAnchor, TextRun, TransformOp,
    ViewBox,
};
use crate::mermaid::styles::sankey::{LABEL_HALO_WIDTH, SankeyStyles};
use crate::mermaid::styles::sequence::css_paint;
use crate::mermaid::theme::khroma::{js_number, js_round};
use crate::mermaid::theme::vars::js_truthy;
use crate::text::FontSpec as MeasureSpec;
use crate::text::measure::Shaped;

use super::db::SankeyDb;

/// `attr('font-size', 14)` on `g.node-labels`.
const LABEL_FONT_SIZE: f64 = 14.0;
/// The gap between a node and its label, `d.x1 + 6` and `d.x0 - 6`.
const LABEL_GAP: f64 = 6.0;
/// `stroke-opacity: 0.5` of `g.links` and `.link`.
const LINK_OPACITY: f64 = 0.5;
/// `d3.schemeTableau10` (d3-scale-chromatic 3).
const TABLEAU10: [&str; 10] = [
    "#4e79a7", "#f28e2c", "#e15759", "#76b7b2", "#59a14f", "#edc949", "#af7aa1", "#ff9da7",
    "#9c755f", "#bab0ab",
];

/// `labelStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LabelStyle {
    Legacy,
    Outlined,
}

/// `linkColor`.
#[derive(Debug, Clone, PartialEq)]
enum LinkColor {
    Gradient,
    Source,
    Target,
    /// Any other value, written as the `stroke` attribute.
    Fixed(String),
}

/// The config `draw` reads, `conf?.x ?? defaultSankeyConfig.x`.
struct Conf {
    width: f64,
    height: f64,
    node_alignment: Align,
    prefix: String,
    suffix: String,
    show_values: bool,
    node_width: f64,
    node_padding: f64,
    label_style: LabelStyle,
    node_colors: IndexMap<String, String>,
    link_color: LinkColor,
}

/// A config value as the template literal prints it.
fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.as_f64().map(js_number).unwrap_or_default(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

impl Conf {
    fn from_resolved(resolved: &Resolved) -> Conf {
        let conf = &resolved.config.sankey;
        let get = |key: &str| conf.get(key).filter(|v| !v.is_null());
        let num = |key: &str, default: f64| get(key).and_then(Value::as_f64).unwrap_or(default);
        let width = num("width", 600.0);
        Conf {
            width,
            // `conf?.height ?? defaultSankeyConfig.width!`.
            height: num("height", 600.0),
            // An alignment mermaid's map does not have would make d3 lay
            // every node at NaN; the default stands in for it.
            node_alignment: get("nodeAlignment")
                .and_then(Value::as_str)
                .and_then(Align::from_config)
                .unwrap_or(Align::Justify),
            prefix: get("prefix").map(js_string).unwrap_or_default(),
            suffix: get("suffix").map(js_string).unwrap_or_default(),
            show_values: get("showValues").is_none_or(js_truthy),
            node_width: num("nodeWidth", 10.0),
            node_padding: num("nodePadding", 12.0),
            label_style: match get("labelStyle").and_then(Value::as_str) {
                Some("outlined") => LabelStyle::Outlined,
                _ => LabelStyle::Legacy,
            },
            node_colors: get("nodeColors")
                .and_then(Value::as_object)
                .map(|m| {
                    m.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default(),
            link_color: match get("linkColor").map(js_string).as_deref() {
                None | Some("gradient") => LinkColor::Gradient,
                Some("source") => LinkColor::Source,
                Some("target") => LinkColor::Target,
                Some(other) => LinkColor::Fixed(other.to_string()),
            },
        }
    }
}

/// `d3.scaleOrdinal(d3.schemeTableau10)` behind `getNodeColor`: an id the
/// domain does not hold yet is appended, so colors go out in the order
/// `colorScheme` is first called, and a node with a `nodeColors` entry never
/// takes a slot.
struct NodeColors<'c> {
    custom: &'c IndexMap<String, String>,
    domain: IndexMap<String, usize>,
}

impl NodeColors<'_> {
    fn get(&mut self, id: &str) -> String {
        if let Some(c) = self.custom.get(id) {
            return c.clone();
        }
        let next = self.domain.len();
        let i = *self.domain.entry(id.to_string()).or_insert(next);
        TABLEAU10[i % TABLEAU10.len()].to_string()
    }
}

/// `getText`: the id, and with `showValues` the value rounded to two
/// decimals between the prefix and the suffix.
fn label_text(conf: &Conf, id: &str, value: f64) -> String {
    if !conf.show_values {
        return id.to_string();
    }
    let rounded = js_round(value * 100.0) / 100.0;
    format!("{id}\n{}{}{}", conf.prefix, js_number(rounded), conf.suffix)
}

/// The text as Chrome lays it out under `white-space: normal`: each run of
/// whitespace is one space, and the ends are trimmed.
fn collapse(text: &str) -> String {
    text.split(|c: char| c.is_ascii_whitespace())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `findCentralNodeLayer`: the layer of the first node with the largest
/// value.
fn central_node_layer(graph: &SankeyGraph) -> usize {
    let mut max_value = 0.0;
    let mut layer = 0;
    for node in &graph.nodes {
        if node.value > max_value {
            max_value = node.value;
            layer = node.layer;
        }
    }
    layer
}

/// An axis-aligned box in user units, what `getBBox()` reports.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Bounds {
        Bounds {
            min_x: x0.min(x1),
            min_y: y0.min(y1),
            max_x: x0.max(x1),
            max_y: y0.max(y1),
        }
    }
}

fn extend(bounds: &mut Option<Bounds>, more: Bounds) {
    *bounds = Some(match *bounds {
        None => more,
        Some(b) => Bounds {
            min_x: b.min_x.min(more.min_x),
            min_y: b.min_y.min(more.min_y),
            max_x: b.max_x.max(more.max_x),
            max_y: b.max_y.max(more.max_y),
        },
    });
}

/// `getBBox()` of one line of text at `(x, baseline)`: the advance box of
/// the primary face joined with the ink, as the text measurer models it.
fn text_bounds(shaped: &Shaped, x: f64, baseline: f64, anchor: TextAnchor) -> Bounds {
    let advance = f64::from(shaped.width);
    let start = match anchor {
        TextAnchor::Start => x,
        TextAnchor::Middle => x - advance / 2.0,
        TextAnchor::End => x - advance,
    };
    let (ascent, descent) = (f64::from(shaped.ascent), f64::from(shaped.descent));
    match shaped.ink {
        Some(ink) => Bounds {
            min_x: start + f64::from(ink.min_x).min(0.0),
            min_y: baseline - f64::from(ink.top).max(ascent),
            max_x: start + f64::from(ink.max_x).max(advance),
            max_y: baseline + f64::from(ink.bottom).max(descent),
        },
        None => Bounds::new(
            start,
            baseline - ascent,
            start + advance,
            baseline + descent,
        ),
    }
}

fn layout_error(err: sankey::SankeyError) -> HelperError {
    HelperError::internal(err.to_string())
}

/// `sankeyRenderer.draw` for a parsed db.
pub fn draw(db: &SankeyDb, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
    let conf = Conf::from_resolved(resolved);
    let styles = SankeyStyles::resolve(&resolved.theme_variables);
    // `.node-labels` and the root rule carry the same family; when Chrome
    // rejects it both drop and the page default applies.
    let family = if css_family_valid(&styles.font_family) {
        styles.font_family.clone()
    } else {
        BODY_DEFAULT_FAMILY.to_string()
    };

    let (ids, links) = db.graph();
    let graph = Sankey::new()
        .node_width(conf.node_width)
        .node_padding(conf.node_padding + if conf.show_values { 15.0 } else { 0.0 })
        .node_align(conf.node_alignment)
        .extent(0.0, 0.0, conf.width, conf.height)
        .layout(&ids, &links)
        .map_err(layout_error)?;
    let central_layer = central_node_layer(&graph);
    let mut colors = NodeColors {
        custom: &conf.node_colors,
        domain: IndexMap::new(),
    };
    let mut bounds: Option<Bounds> = None;
    // `Uid.next` numbers every uid of the render from 1: the nodes first,
    // then the gradients.
    let mut uid = 0usize;

    let mut node_elements = Vec::with_capacity(graph.nodes.len());
    for node in &graph.nodes {
        uid += 1;
        let (w, h) = (node.x1 - node.x0, node.y1 - node.y0);
        extend(
            &mut bounds,
            Bounds::new(node.x0, node.y0, node.x0 + w, node.y0 + h),
        );
        node_elements.push(Element::Group(Group {
            common: Common {
                id: Some(format!("node-{uid}")),
                class: Some("node".to_string()),
                transform: vec![TransformOp::Translate(node.x0, node.y0)],
                style: Style::default(),
            },
            children: vec![Element::Rect(Rect {
                common: Common::with_style(Style {
                    // An invalid `fill` attribute inherits the root fill.
                    fill: Some(match css_paint(&colors.get(&node.id)) {
                        Paint::None => styles.text_color.clone(),
                        paint => paint,
                    }),
                    ..Style::default()
                }),
                x: 0.0,
                y: 0.0,
                width: w,
                height: h,
                rx: None,
                ry: None,
            })],
        }));
    }

    let spec = MeasureSpec::new(family.clone(), LABEL_FONT_SIZE as f32);
    let dy = if conf.show_values {
        0.0
    } else {
        0.35 * LABEL_FONT_SIZE
    };
    let label_position = |node: &sankey::SankeyNode| -> (f64, TextAnchor) {
        if conf.label_style == LabelStyle::Outlined {
            if node.layer < central_layer {
                return (node.x0 - LABEL_GAP, TextAnchor::End);
            }
            return (node.x1 + LABEL_GAP, TextAnchor::Start);
        }
        if node.x0 < conf.width / 2.0 {
            (node.x1 + LABEL_GAP, TextAnchor::Start)
        } else {
            (node.x0 - LABEL_GAP, TextAnchor::End)
        }
    };
    let passes: &[Option<&str>] = match conf.label_style {
        LabelStyle::Legacy => &[None],
        LabelStyle::Outlined => &[Some("sankey-label-bg"), Some("sankey-label-fg")],
    };
    let mut label_elements = Vec::new();
    for &class in passes {
        for node in &graph.nodes {
            let text = collapse(&label_text(&conf, &node.id, node.value));
            let (x, anchor) = label_position(node);
            let y = (node.y1 + node.y0) / 2.0 + dy;
            let shaped = ctx.metrics.shape(&text, &spec);
            extend(&mut bounds, text_bounds(&shaped, x, y, anchor));
            let style = match class {
                Some("sankey-label-bg") => Style {
                    stroke: Some(styles.label_halo.clone()),
                    stroke_width: Some(LABEL_HALO_WIDTH),
                    stroke_linejoin: Some(LineJoin::Round),
                    ..Style::default()
                },
                Some(_) => Style {
                    fill: Some(styles.label_fill.clone()),
                    ..Style::default()
                },
                None => Style::default(),
            };
            label_elements.push(Element::Text(Text {
                common: Common {
                    class: class.map(str::to_string),
                    ..Common::with_style(style)
                },
                x,
                y,
                anchor: Some(anchor),
                baseline: None,
                font: FontSpec::default(),
                runs: vec![TextRun::new(text)],
            }));
        }
    }

    let mut gradients = Vec::new();
    let mut link_elements = Vec::with_capacity(graph.links.len());
    for (i, link) in graph.links.iter().enumerate() {
        let source = &graph.nodes[link.source];
        let target = &graph.nodes[link.target];
        let stroke = match &conf.link_color {
            LinkColor::Gradient => {
                uid += 1;
                let id = format!("linearGradient-{uid}");
                let stop = |offset: f64, color: String| GradientStop {
                    offset,
                    color: match css_paint(&color) {
                        Paint::Color(c) => c,
                        // An invalid `stop-color` keeps the initial black.
                        _ => Color::BLACK,
                    },
                };
                gradients.push(LinearGradient {
                    id: id.clone(),
                    units: GradientUnits::UserSpaceOnUse,
                    x1: Some(source.x1),
                    y1: None,
                    x2: Some(target.x0),
                    y2: None,
                    stops: vec![
                        stop(0.0, colors.get(&source.id)),
                        stop(100.0, colors.get(&target.id)),
                    ],
                });
                Paint::Reference(id)
            }
            LinkColor::Source => css_paint(&colors.get(&source.id)),
            LinkColor::Target => css_paint(&colors.get(&target.id)),
            LinkColor::Fixed(color) => css_paint(color),
        };
        extend(
            &mut bounds,
            Bounds::new(source.x1, link.y0, target.x0, link.y1),
        );
        link_elements.push(Element::Group(Group {
            common: Common {
                class: Some("link".to_string()),
                ..Common::with_style(Style {
                    mix_blend_mode: Some(BlendMode::Multiply),
                    ..Style::default()
                })
            },
            children: vec![Element::Path(Path {
                common: Common::with_style(Style {
                    stroke: Some(stroke),
                    stroke_width: Some(link.width.max(1.0)),
                    ..Style::default()
                }),
                d: sankey::link_horizontal(&graph, i),
            })],
        }));
    }

    // `getBBox()` returns an `SVGRect` of single-precision floats, so a
    // node bottom at 400.00000000000006 reports 400.
    let b = bounds.unwrap_or(Bounds::new(0.0, 0.0, 0.0, 0.0));
    let single = |v: f64| f64::from(v as f32);
    let (x, y) = (single(b.min_x), single(b.min_y));
    let (width, height) = (single(b.max_x - b.min_x), single(b.max_y - b.min_y));
    let mut scene = Scene::new(width, height);
    scene.view_box = ViewBox::new(x, y, width, height);
    scene.font_family = family.clone();
    scene.linear_gradients = gradients;
    scene.root = Group {
        common: Common::with_style(Style {
            fill: Some(styles.text_color.clone()),
            ..Style::default()
        }),
        children: vec![
            Element::Group(Group {
                common: Common {
                    class: Some("nodes".to_string()),
                    ..Common::default()
                },
                children: node_elements,
            }),
            Element::Group(Group {
                common: Common {
                    class: Some("node-labels".to_string()),
                    ..Common::with_style(Style {
                        font: FontSpec {
                            family: Some(family),
                            size: Some(LABEL_FONT_SIZE),
                            ..FontSpec::default()
                        },
                        ..Style::default()
                    })
                },
                children: label_elements,
            }),
            Element::Group(Group {
                common: Common {
                    class: Some("links".to_string()),
                    ..Common::with_style(Style {
                        fill: Some(Paint::None),
                        stroke_opacity: Some(LINK_OPACITY),
                        ..Style::default()
                    })
                },
                children: link_elements,
            }),
        ],
    };
    Ok(scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_follow_first_use_and_skip_custom_nodes() {
        let custom: IndexMap<String, String> = [("b".to_string(), "#123456".to_string())].into();
        let mut colors = NodeColors {
            custom: &custom,
            domain: IndexMap::new(),
        };
        assert_eq!(colors.get("a"), "#4e79a7");
        assert_eq!(colors.get("b"), "#123456");
        assert_eq!(colors.get("c"), "#f28e2c");
        assert_eq!(colors.get("a"), "#4e79a7");
    }

    #[test]
    fn whitespace_collapses_like_chrome() {
        assert_eq!(collapse("A\n$ 15 "), "A $ 15");
    }
}
