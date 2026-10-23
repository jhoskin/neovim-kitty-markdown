//! Port of `wardleyRenderer.ts` (`draw`).
//!
//! The map has a fixed size: `size [w, h]` in the source, else the
//! `wardley-beta` config `width` by `height` (900 by 600). Coordinates are
//! percentages projected into the chart inside `padding`. Only the
//! annotations box measures text.
//!
//! No stylesheet survives `draw` (see `styles::wardley`), so the root has
//! no `font-family`. While `draw` runs, the SVG sits in the div `render`
//! creates with `style="font-family: <config fontFamily>"`, so the
//! annotations box is measured in that family. mermaid-cli then puts the
//! SVG markup into its own page, where the text takes the page default.

use std::f64::consts::{FRAC_PI_6, PI, SQRT_2};

use serde_json::Value;

use super::db::{Flow, Node, Strategy, WardleyDb};
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::scene::{
    Baseline, Circle, Common, Element, FontSpec, FontWeight, Group, Line, Marker, MarkerUnits,
    Orient, Paint, Path, PathData, Rect, Scene, Style, Text, TextAnchor, TextRun, TransformOp,
    ViewBox,
};
use crate::mermaid::styles::wardley::{WardleyStyles, fixed};
use crate::text::measure::FontSpec as MeasureSpec;

/// The `id` mermaid-cli gives the SVG; the marker ids carry it.
const SVG_ID: &str = "my-svg";
/// The default font of mermaid-cli's headless page, which the text
/// inherits with no stylesheet in the SVG.
const PAGE_FONT: &str = "\"Times New Roman\", serif";
/// `DEFAULT_STAGES`.
const DEFAULT_STAGES: [&str; 4] = ["Genesis", "Custom Built", "Product", "Commodity"];

/// `getConfigValues()`: `getConfig()['wardley-beta']` with the renderer's
/// defaults. mermaid 12.2.0 has no schema defaults for this section.
#[derive(Debug, Clone, PartialEq)]
pub struct WardleyConfig {
    pub width: f64,
    pub height: f64,
    pub padding: f64,
    pub node_radius: f64,
    pub node_label_offset: f64,
    pub axis_font_size: f64,
    pub label_font_size: f64,
    pub show_grid: bool,
}

impl WardleyConfig {
    pub fn from_raw(raw: &Value) -> WardleyConfig {
        let section = &raw["wardley-beta"];
        let number =
            |key: &str, default: f64| section.get(key).and_then(Value::as_f64).unwrap_or(default);
        // `if (configValues.showGrid)`: JS truthiness.
        let show_grid = match section.get("showGrid") {
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_f64().is_some_and(|v| v != 0.0 && !v.is_nan()),
            Some(Value::String(s)) => !s.is_empty(),
            Some(Value::Array(_)) | Some(Value::Object(_)) => true,
            _ => false,
        };
        WardleyConfig {
            width: number("width", 900.0),
            height: number("height", 600.0),
            padding: number("padding", 48.0),
            node_radius: number("nodeRadius", 6.0),
            node_label_offset: number("nodeLabelOffset", 8.0),
            axis_font_size: number("axisFontSize", 12.0),
            label_font_size: number("labelFontSize", 10.0),
            show_grid,
        }
    }
}

fn group(class: &str, children: Vec<Element>) -> Element {
    Element::Group(Group {
        common: Common {
            class: Some(class.to_string()),
            ..Common::default()
        },
        children,
    })
}

fn line(class: Option<&str>, from: (f64, f64), to: (f64, f64), style: Style) -> Element {
    Element::Line(Line {
        common: Common {
            class: class.map(str::to_string),
            style,
            ..Common::default()
        },
        x1: from.0,
        y1: from.1,
        x2: to.0,
        y2: to.1,
    })
}

fn stroke(paint: &Option<Paint>, width: f64) -> Style {
    Style {
        stroke: paint.clone(),
        stroke_width: Some(width),
        ..Style::default()
    }
}

fn circle(
    class: Option<&str>,
    c: (f64, f64),
    r: f64,
    fill: Option<Paint>,
    style: Style,
) -> Element {
    Element::Circle(Circle {
        common: Common {
            class: class.map(str::to_string),
            style: Style { fill, ..style },
            ..Common::default()
        },
        cx: c.0,
        cy: c.1,
        r,
    })
}

/// One `<text>` with the attributes the renderer sets.
struct Label<'a> {
    class: Option<&'a str>,
    at: (f64, f64),
    fill: Option<Paint>,
    size: f64,
    bold: bool,
    anchor: TextAnchor,
    baseline: Option<Baseline>,
    rotate: Option<f64>,
}

impl Label<'_> {
    fn draw(self, content: &str) -> Element {
        Element::Text(Text {
            common: Common {
                class: self.class.map(str::to_string),
                transform: self
                    .rotate
                    .map(|angle| vec![TransformOp::Rotate(angle, self.at.0, self.at.1)])
                    .unwrap_or_default(),
                style: Style {
                    fill: self.fill,
                    ..Style::default()
                },
                ..Common::default()
            },
            x: self.at.0,
            y: self.at.1,
            anchor: Some(self.anchor),
            baseline: self.baseline,
            font: FontSpec {
                size: Some(self.size),
                weight: Some(if self.bold {
                    FontWeight::Bold
                } else {
                    FontWeight::Normal
                }),
                ..FontSpec::default()
            },
            runs: vec![TextRun::new(content)],
        })
    }
}

/// The `M 0 0 L 10 5 L 0 10 z` triangle, or its mirror for a start
/// marker.
fn marker(
    id: &str,
    ref_x: f64,
    size: f64,
    orient: Orient,
    mirrored: bool,
    fill: &Option<Paint>,
) -> Element {
    let mut d = PathData::new();
    if mirrored {
        d.move_to(10.0, 0.0).line_to(0.0, 5.0).line_to(10.0, 10.0);
    } else {
        d.move_to(0.0, 0.0).line_to(10.0, 5.0).line_to(0.0, 10.0);
    }
    d.close();
    Element::Marker(Marker {
        id: format!("{id}-{SVG_ID}"),
        class: None,
        view_box: Some(ViewBox::new(0.0, 0.0, 10.0, 10.0)),
        ref_x,
        ref_y: 5.0,
        marker_width: size,
        marker_height: size,
        orient,
        marker_units: MarkerUnits::StrokeWidth,
        style: Style::default(),
        children: vec![Element::Path(Path {
            common: Common::with_style(Style {
                fill: fill.clone(),
                stroke: Some(Paint::None),
                ..Style::default()
            }),
            d,
        })],
    })
}

/// SVG text collapses white space before it measures or draws.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `${n}` for an annotation number.
fn number_text(n: f64) -> String {
    crate::mermaid::theme::khroma::js_number(n)
}

struct Painter<'a> {
    db: &'a WardleyDb,
    cfg: &'a WardleyConfig,
    theme: &'a WardleyStyles,
    ctx: &'a RenderCtx<'a>,
    /// The family of the div around the SVG while `draw` measures.
    measure_family: &'a str,
    width: f64,
    height: f64,
    chart_width: f64,
    chart_height: f64,
    /// `positions`: the projected center of every node, in node order. A
    /// pipeline moves its parent.
    positions: Vec<(f64, f64)>,
}

impl Painter<'_> {
    fn project_x(&self, value: f64) -> f64 {
        self.cfg.padding + (value / 100.0) * self.chart_width
    }

    fn project_y(&self, value: f64) -> f64 {
        self.height - self.cfg.padding - (value / 100.0) * self.chart_height
    }

    fn square_size(&self) -> f64 {
        self.cfg.node_radius * 1.6
    }

    fn index(&self, id: &str) -> Option<usize> {
        self.db.nodes.get_index_of(id)
    }

    fn node(&self, index: usize) -> &Node {
        &self.db.nodes[index]
    }

    /// The title, the axes with their labels, the stages, and the grid.
    fn frame(&self, root: &mut Vec<Element>) {
        let cfg = self.cfg;
        let theme = self.theme;
        let (w, h, pad) = (self.width, self.height, cfg.padding);
        let title = self.db.diagram_title();
        if !title.is_empty() {
            root.push(
                Label {
                    class: Some("wardley-title"),
                    at: (w / 2.0, pad / 2.0),
                    fill: theme.axis_text_color.clone(),
                    size: cfg.axis_font_size * 1.05,
                    bold: true,
                    anchor: TextAnchor::Middle,
                    baseline: Some(Baseline::Middle),
                    rotate: None,
                }
                .draw(title),
            );
        }
        let axis = stroke(&theme.axis_color, 1.0);
        let y_center = pad + self.chart_height / 2.0;
        let axis_label = |class, at, rotate| Label {
            class: Some(class),
            at,
            fill: theme.axis_text_color.clone(),
            size: cfg.axis_font_size,
            bold: true,
            anchor: TextAnchor::Middle,
            baseline: None,
            rotate,
        };
        root.push(group(
            "wardley-axes",
            vec![
                line(None, (pad, h - pad), (w - pad, h - pad), axis.clone()),
                line(None, (pad, pad), (pad, h - pad), axis),
                axis_label(
                    "wardley-axis-label wardley-axis-label-x",
                    (pad + self.chart_width / 2.0, h - pad / 4.0),
                    None,
                )
                .draw("Evolution"),
                axis_label(
                    "wardley-axis-label wardley-axis-label-y",
                    (pad / 3.0, y_center),
                    Some(-90.0),
                )
                .draw("Visibility"),
            ],
        ));

        let stages: Vec<&str> = if self.db.axes.stages.is_empty() {
            DEFAULT_STAGES.to_vec()
        } else {
            self.db.axes.stages.iter().map(String::as_str).collect()
        };
        let boundaries = &self.db.axes.stage_boundaries;
        let positions: Vec<(f64, f64)> = if boundaries.len() == stages.len() {
            let mut previous = 0.0;
            boundaries
                .iter()
                .map(|&b| {
                    let pos = (previous, b);
                    previous = b;
                    pos
                })
                .collect()
        } else {
            let stage_width = 1.0 / stages.len() as f64;
            (0..stages.len())
                .map(|i| (i as f64 * stage_width, (i + 1) as f64 * stage_width))
                .collect()
        };
        let mut stage_children = Vec::new();
        for (index, (stage, (start, end))) in stages.iter().zip(positions).enumerate() {
            let start_x = pad + start * self.chart_width;
            let end_x = pad + end * self.chart_width;
            if index > 0 {
                stage_children.push(line(
                    None,
                    (start_x, pad),
                    (start_x, h - pad),
                    Style {
                        stroke_dasharray: Some(vec![5.0, 5.0]),
                        opacity: Some(0.8),
                        ..stroke(&Some(fixed("#000")), 1.0)
                    },
                ));
            }
            stage_children.push(
                Label {
                    class: Some("wardley-stage-label"),
                    at: ((start_x + end_x) / 2.0, h - pad / 1.5),
                    fill: theme.axis_text_color.clone(),
                    size: cfg.axis_font_size - 2.0,
                    bold: false,
                    anchor: TextAnchor::Middle,
                    baseline: None,
                    rotate: None,
                }
                .draw_regular(stage),
            );
        }
        root.push(group("wardley-stages", stage_children));

        if cfg.show_grid {
            let grid = Style {
                stroke: theme.grid_color.clone(),
                stroke_dasharray: Some(vec![2.0, 6.0]),
                ..Style::default()
            };
            let mut children = Vec::new();
            for i in 1..4 {
                let ratio = f64::from(i) / 4.0;
                let x = pad + self.chart_width * ratio;
                let y = h - pad - self.chart_height * ratio;
                children.push(line(None, (x, pad), (x, h - pad), grid.clone()));
                children.push(line(None, (pad, y), (w - pad, y), grid.clone()));
            }
            root.push(group("wardley-grid", children));
        }
    }

    /// The pipeline boxes and the dotted links between their components.
    /// Each box moves its parent onto its top edge.
    fn pipelines(&mut self, root: &mut Vec<Element>) {
        if self.db.pipelines.is_empty() {
            return;
        }
        let mut boxes = Vec::new();
        let mut links = Vec::new();
        let link_style = Style {
            stroke_dasharray: Some(vec![4.0, 4.0]),
            ..stroke(&self.theme.link_stroke, 1.0)
        };
        for pipeline in self.db.pipelines.values() {
            if pipeline.component_ids.is_empty() {
                continue;
            }
            let mut sorted: Vec<usize> = pipeline
                .component_ids
                .iter()
                .filter_map(|id| self.index(id))
                .collect();
            sorted.sort_by(|&a, &b| self.node(a).x.total_cmp(&self.node(b).x));
            for pair in sorted.windows(2) {
                links.push(line(
                    Some("wardley-pipeline-evolution-link"),
                    self.positions[pair[0]],
                    self.positions[pair[1]],
                    link_style.clone(),
                ));
            }
            let mut min_x = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut y = 0.0;
            for id in &pipeline.component_ids {
                if let Some(i) = self.index(id) {
                    let pos = self.positions[i];
                    min_x = min_x.min(pos.0);
                    max_x = max_x.max(pos.0);
                    y = pos.1;
                }
            }
            if min_x == f64::INFINITY || max_x == f64::NEG_INFINITY {
                continue;
            }
            let padding = 15.0;
            let box_height = self.cfg.node_radius * 4.0;
            let box_top = y - box_height / 2.0;
            if let Some(parent) = self.index(&pipeline.node_id) {
                self.positions[parent] =
                    ((min_x + max_x) / 2.0, box_top - self.square_size() / 6.0);
            }
            boxes.push(Element::Rect(Rect {
                common: Common {
                    class: Some("wardley-pipeline-box".to_string()),
                    style: Style {
                        fill: Some(Paint::None),
                        ..stroke(&self.theme.axis_color, 1.5)
                    },
                    ..Common::default()
                },
                x: min_x - padding,
                y: box_top,
                width: max_x - min_x + padding * 2.0,
                height: box_height,
                rx: Some(4.0),
                ry: Some(4.0),
            }));
        }
        root.push(group("wardley-pipelines", boxes));
        root.push(group("wardley-pipeline-links", links));
    }

    /// The links: each line trimmed to the node outlines, then the labels
    /// above the lines, turned to read left to right.
    fn links(&self, root: &mut Vec<Element>) {
        let db = self.db;
        let valid: Vec<(usize, usize, &super::db::Link)> = db
            .links
            .iter()
            .filter_map(|link| {
                let s = self.index(&link.source)?;
                let t = self.index(&link.target)?;
                // A link from a pipeline component to its own parent is
                // implied by the box.
                let in_own_pipeline = db
                    .pipelines
                    .get(&link.target)
                    .is_some_and(|p| p.component_ids.contains(&link.source));
                (!in_own_pipeline).then_some((s, t, link))
            })
            .collect();
        let radius = |i: usize| {
            if self.node(i).is_pipeline_parent {
                self.square_size() / SQRT_2
            } else {
                self.cfg.node_radius
            }
        };
        let mut children = Vec::new();
        for &(s, t, link) in &valid {
            let (sp, tp) = (self.positions[s], self.positions[t]);
            let (dx, dy) = (tp.0 - sp.0, tp.1 - sp.1);
            let distance = (dx * dx + dy * dy).sqrt();
            let start = (
                sp.0 + dx / distance * radius(s),
                sp.1 + dy / distance * radius(s),
            );
            let end = (
                tp.0 - dx / distance * radius(t),
                tp.1 - dy / distance * radius(t),
            );
            // A link between two nodes at one place gets `NaN` ends, which
            // the browser does not draw.
            if !(start.0.is_finite()
                && start.1.is_finite()
                && end.0.is_finite()
                && end.1.is_finite())
            {
                continue;
            }
            let class = if link.dashed {
                "wardley-link wardley-link--dashed"
            } else {
                "wardley-link"
            };
            let marker_ref = |name: &str| format!("{name}-{SVG_ID}");
            let style = Style {
                stroke_dasharray: link.dashed.then(|| vec![6.0, 6.0]),
                marker_end: matches!(link.flow, Some(Flow::Forward | Flow::Bidirectional))
                    .then(|| marker_ref("link-arrow-end")),
                marker_start: matches!(link.flow, Some(Flow::Backward | Flow::Bidirectional))
                    .then(|| marker_ref("link-arrow-start")),
                ..stroke(&self.theme.link_stroke, 1.0)
            };
            children.push(line(Some(class), start, end, style));
        }
        for &(s, t, link) in &valid {
            let Some(label) = link.label.as_deref().filter(|l| !l.is_empty()) else {
                continue;
            };
            let (sp, tp) = (self.positions[s], self.positions[t]);
            let (dx, dy) = (tp.0 - sp.0, tp.1 - sp.1);
            let distance = (dx * dx + dy * dy).sqrt();
            let offset = 8.0;
            let at = (
                (sp.0 + tp.0) / 2.0 + dy / distance * offset,
                (sp.1 + tp.1) / 2.0 - dx / distance * offset,
            );
            let mut angle = dy.atan2(dx) * 180.0 / PI;
            if !(-90.0..=90.0).contains(&angle) {
                angle += 180.0;
            }
            children.push(
                Label {
                    class: Some("wardley-link-label"),
                    at,
                    fill: self.theme.axis_text_color.clone(),
                    size: self.cfg.label_font_size,
                    bold: false,
                    anchor: TextAnchor::Middle,
                    baseline: Some(Baseline::Middle),
                    rotate: Some(angle),
                }
                .draw_regular(label),
            );
        }
        root.push(group("wardley-links", children));
    }

    /// The `evolve` arrows, stopped short of the target circle.
    fn trends(&self, root: &mut Vec<Element>) {
        let mut children = Vec::new();
        for trend in self.db.trends.values() {
            let Some(origin) = self.index(&trend.node_id).map(|i| self.positions[i]) else {
                continue;
            };
            let target = (
                self.project_x(trend.target_x),
                self.project_y(trend.target_y),
            );
            let (dx, dy) = (target.0 - origin.0, target.1 - origin.1);
            let distance = (dx * dx + dy * dy).sqrt();
            let shorten_by = self.cfg.node_radius + 2.0;
            let end = if distance > shorten_by {
                (
                    target.0 - dx / distance * shorten_by,
                    target.1 - dy / distance * shorten_by,
                )
            } else {
                target
            };
            children.push(line(
                Some("wardley-trend"),
                origin,
                end,
                Style {
                    stroke_dasharray: Some(vec![4.0, 4.0]),
                    marker_end: Some(format!("arrow-{SVG_ID}")),
                    ..stroke(&self.theme.evolution_stroke, 1.0)
                },
            ));
        }
        root.push(group("wardley-trends", children));
    }

    /// One node group: the sourcing overlay, the circle, the market dots,
    /// the pipeline parent square, the inertia bar, and the label, in the
    /// order the renderer's selection passes append them.
    fn node_group(&self, index: usize) -> Element {
        let node = self.node(index);
        let theme = self.theme;
        let cfg = self.cfg;
        let (x, y) = self.positions[index];
        let r = cfg.node_radius;
        let edge = stroke(&theme.component_stroke, 1.0);
        let mut children = Vec::new();
        let overlay = |class, fill: &str, stroke_paint: Option<Paint>| {
            circle(
                Some(class),
                (x, y),
                r * 2.0,
                Some(fixed(fill)),
                Style {
                    stroke: stroke_paint,
                    stroke_width: Some(1.0),
                    ..Style::default()
                },
            )
        };
        match node.source_strategy {
            Some(Strategy::Outsource) => children.push(overlay(
                "wardley-outsource-overlay",
                "#666",
                theme.component_stroke.clone(),
            )),
            Some(Strategy::Buy) => children.push(overlay(
                "wardley-buy-overlay",
                "#ccc",
                theme.component_stroke.clone(),
            )),
            Some(Strategy::Build) => children.push(overlay(
                "wardley-build-overlay",
                "#eee",
                Some(fixed("#000")),
            )),
            Some(Strategy::Market) => children.push(overlay(
                "wardley-market-overlay",
                "white",
                theme.component_stroke.clone(),
            )),
            None => {}
        }
        let is_anchor = node.class_name.as_deref() == Some("anchor");
        let is_market = node.source_strategy == Some(Strategy::Market);
        if !node.is_pipeline_parent && !is_market && !is_anchor {
            children.push(circle(
                None,
                (x, y),
                r,
                theme.component_fill.clone(),
                edge.clone(),
            ));
        }
        if is_market {
            let small = r * 0.7;
            let tri = r * 1.2;
            let top = (x, y - tri);
            let left = (x - tri * FRAC_PI_6.cos(), y + tri * FRAC_PI_6.sin());
            let right = (x + tri * FRAC_PI_6.cos(), y + tri * FRAC_PI_6.sin());
            for (a, b) in [(top, left), (left, right), (right, top)] {
                children.push(line(Some("wardley-market-line"), a, b, edge.clone()));
            }
            for c in [top, left, right] {
                children.push(circle(
                    Some("wardley-market-dot"),
                    c,
                    small,
                    Some(fixed("white")),
                    stroke(&theme.component_stroke, 2.0),
                ));
            }
        }
        let square = self.square_size();
        if node.is_pipeline_parent {
            children.push(Element::Rect(Rect {
                common: Common::with_style(Style {
                    fill: theme.component_fill.clone(),
                    ..edge.clone()
                }),
                x: x - square / 2.0,
                y: y - square / 2.0,
                width: square,
                height: square,
                rx: None,
                ry: None,
            }));
        }
        if node.inertia {
            let mut offset = if node.is_pipeline_parent {
                square / 2.0 + 15.0
            } else {
                r + 15.0
            };
            if node.source_strategy.is_some() {
                offset += r + 10.0;
            }
            let bar = if node.is_pipeline_parent {
                square
            } else {
                r * 2.0
            };
            children.push(line(
                Some("wardley-inertia"),
                (x + offset, y - bar / 2.0),
                (x + offset, y + bar / 2.0),
                stroke(&theme.component_stroke, 6.0),
            ));
        }
        let strategy = node.source_strategy.is_some();
        let (at, fill, anchor, baseline) = if is_anchor {
            (
                (
                    x + node.label_offset_x.unwrap_or(0.0),
                    node.label_offset_y.map_or(y - 3.0, |o| y + o),
                ),
                Some(fixed("#000")),
                TextAnchor::Middle,
                Baseline::Middle,
            )
        } else {
            let default_x = cfg.node_label_offset
                + if strategy && node.label_offset_x.is_none() {
                    10.0
                } else {
                    0.0
                };
            let default_y = -cfg.node_label_offset
                - if strategy && node.label_offset_y.is_none() {
                    10.0
                } else {
                    0.0
                };
            let fill = if node.class_name.as_deref() == Some("evolved") {
                theme.evolution_stroke.clone()
            } else {
                theme.component_label_color.clone()
            };
            (
                (
                    x + node.label_offset_x.unwrap_or(default_x),
                    y + node.label_offset_y.unwrap_or(default_y),
                ),
                fill,
                TextAnchor::Start,
                Baseline::Auto,
            )
        };
        children.push(
            Label {
                class: Some("wardley-node-label"),
                at,
                fill,
                size: cfg.label_font_size,
                bold: is_anchor,
                anchor,
                baseline: Some(baseline),
                rotate: None,
            }
            .draw(&node.label),
        );
        let class = match &node.class_name {
            Some(name) => format!("wardley-node wardley-node--{name}"),
            None => "wardley-node".to_string(),
        };
        Element::Group(Group {
            common: Common {
                class: Some(class),
                ..Common::default()
            },
            children,
        })
    }

    /// The numbered circles and the box that lists their texts.
    fn annotations(&self, root: &mut Vec<Element>) {
        let db = self.db;
        if db.annotations.is_empty() {
            return;
        }
        let theme = self.theme;
        let mut children = Vec::new();
        for annotation in &db.annotations {
            let projected: Vec<(f64, f64)> = annotation
                .coordinates
                .iter()
                .map(|&(x, y)| (self.project_x(x), self.project_y(y)))
                .collect();
            for pair in projected.windows(2) {
                children.push(line(
                    Some("wardley-annotation-line"),
                    pair[0],
                    pair[1],
                    Style {
                        stroke_dasharray: Some(vec![4.0, 4.0]),
                        ..stroke(&theme.axis_color, 1.5)
                    },
                ));
            }
            for &c in &projected {
                children.push(group(
                    "wardley-annotation",
                    vec![
                        circle(
                            None,
                            c,
                            10.0,
                            Some(fixed("white")),
                            stroke(&theme.axis_color, 1.5),
                        ),
                        Label {
                            class: None,
                            at: c,
                            fill: theme.axis_text_color.clone(),
                            size: 10.0,
                            bold: true,
                            anchor: TextAnchor::Middle,
                            baseline: Some(Baseline::Central),
                            rotate: None,
                        }
                        .draw(&number_text(annotation.number)),
                    ],
                ));
            }
        }
        if let Some((bx, by)) = db.annotations_box {
            children.push(self.annotations_box(self.project_x(bx), self.project_y(by)));
        }
        root.push(group("wardley-annotations", children));
    }

    /// The annotations box: one line per annotation with text, sorted by
    /// number, in a box sized from the widest line plus a 105 px buffer and
    /// clamped into the chart.
    fn annotations_box(&self, x: f64, y: f64) -> Element {
        let padding = 10.0;
        let line_height = 16.0;
        let font_size = 11.0;
        let mut sorted: Vec<_> = self
            .db
            .annotations
            .iter()
            .filter(|a| !a.text.is_empty())
            .collect();
        sorted.sort_by(|a, b| a.number.total_cmp(&b.number));
        let lines: Vec<String> = sorted
            .iter()
            .map(|a| format!("{}. {}", number_text(a.number), a.text))
            .collect();
        let mut children = Vec::new();
        let (mut box_x, mut box_y) = (x, y);
        if !lines.is_empty() {
            // `getComputedTextLength()` and `getBBox().height`.
            let spec = MeasureSpec::new(self.measure_family, font_size as f32);
            let mut max_width: f64 = 0.0;
            let mut max_height: f64 = 0.0;
            for text in &lines {
                let shaped = self.ctx.metrics.shape(&collapse(text), &spec);
                max_width = max_width.max(f64::from(shaped.width));
                max_height = max_height.max(f64::from(shaped.bbox().height));
            }
            let box_width = max_width + padding * 2.0 + 105.0;
            let box_height = lines.len() as f64 * line_height + padding * 2.0 + max_height / 2.0;
            let pad = self.cfg.padding;
            box_x = pad.max(box_x.min(self.width - pad - box_width));
            box_y = pad.max(box_y.min(self.height - pad - box_height));
            children.push(Element::Rect(Rect {
                common: Common::with_style(Style {
                    fill: Some(fixed("white")),
                    ..stroke(&self.theme.axis_color, 1.5)
                }),
                x: box_x,
                y: box_y,
                width: box_width,
                height: box_height,
                rx: Some(4.0),
                ry: Some(4.0),
            }));
        }
        for (idx, text) in lines.iter().enumerate() {
            children.push(
                Label {
                    class: None,
                    at: (
                        box_x + padding,
                        box_y + padding + (idx + 1) as f64 * line_height,
                    ),
                    fill: self.theme.axis_text_color.clone(),
                    size: font_size,
                    bold: false,
                    anchor: TextAnchor::Start,
                    baseline: Some(Baseline::Middle),
                    rotate: None,
                }
                .draw_regular(text),
            );
        }
        group("wardley-annotations-box", children)
    }

    /// Notes, accelerators, and deaccelerators.
    fn markings(&self, root: &mut Vec<Element>) {
        let db = self.db;
        let theme = self.theme;
        if !db.notes.is_empty() {
            let children = db
                .notes
                .iter()
                .map(|note| {
                    Label {
                        class: None,
                        at: (self.project_x(note.x), self.project_y(note.y)),
                        fill: theme.axis_text_color.clone(),
                        size: 11.0,
                        bold: true,
                        anchor: TextAnchor::Start,
                        baseline: None,
                        rotate: None,
                    }
                    .draw(&note.text)
                })
                .collect();
            root.push(group("wardley-notes", children));
        }
        for (list, class, pointing_right) in [
            (&db.accelerators, "wardley-accelerators", true),
            (&db.deaccelerators, "wardley-deaccelerators", false),
        ] {
            if list.is_empty() {
                continue;
            }
            let mut children = Vec::new();
            for item in list {
                let (ax, ay) = (self.project_x(item.x), self.project_y(item.y));
                let (arrow_width, arrow_height, head) = (60.0, 30.0, 20.0);
                let top = ay - arrow_height / 2.0;
                let bottom = ay + arrow_height / 2.0;
                let mut d = PathData::new();
                if pointing_right {
                    d.move_to(ax, top)
                        .line_to(ax + arrow_width - head, top)
                        .line_to(ax + arrow_width - head, top - 8.0)
                        .line_to(ax + arrow_width, ay)
                        .line_to(ax + arrow_width - head, bottom + 8.0)
                        .line_to(ax + arrow_width - head, bottom)
                        .line_to(ax, bottom);
                } else {
                    d.move_to(ax + arrow_width, top)
                        .line_to(ax + head, top)
                        .line_to(ax + head, top - 8.0)
                        .line_to(ax, ay)
                        .line_to(ax + head, bottom + 8.0)
                        .line_to(ax + head, bottom)
                        .line_to(ax + arrow_width, bottom);
                }
                d.close();
                children.push(Element::Path(Path {
                    common: Common::with_style(Style {
                        fill: Some(fixed("white")),
                        ..stroke(&theme.component_stroke, 1.0)
                    }),
                    d,
                }));
                children.push(
                    Label {
                        class: None,
                        at: (ax + arrow_width / 2.0, bottom + 15.0),
                        fill: theme.axis_text_color.clone(),
                        size: 10.0,
                        bold: true,
                        anchor: TextAnchor::Middle,
                        baseline: None,
                        rotate: None,
                    }
                    .draw(&item.text),
                );
            }
            root.push(group(class, children));
        }
    }
}

impl Label<'_> {
    /// A label without a `font-weight` attribute.
    fn draw_regular(self, content: &str) -> Element {
        let mut element = self.draw(content);
        if let Element::Text(text) = &mut element {
            text.font.weight = None;
        }
        element
    }
}

/// `draw`. `font_family` is the config `fontFamily`, which the div
/// around the SVG carries in a `style` attribute, where a trailing `;`
/// just ends the declaration.
pub fn draw(
    db: &WardleyDb,
    cfg: &WardleyConfig,
    theme: &WardleyStyles,
    ctx: &RenderCtx<'_>,
    font_family: &str,
) -> Scene {
    let (width, height) = db.size.unwrap_or((cfg.width, cfg.height));
    let measure_family = font_family.trim().trim_end_matches(';').trim_end();
    let mut painter = Painter {
        db,
        cfg,
        theme,
        ctx,
        measure_family,
        width,
        height,
        chart_width: width - cfg.padding * 2.0,
        chart_height: height - cfg.padding * 2.0,
        positions: Vec::new(),
    };
    painter.positions = db
        .nodes
        .values()
        .map(|n| (painter.project_x(n.x), painter.project_y(n.y)))
        .collect();

    let mut root = vec![Element::Rect(Rect {
        common: Common {
            class: Some("wardley-background".to_string()),
            style: Style {
                fill: theme.background_color.clone(),
                ..Style::default()
            },
            ..Common::default()
        },
        x: 0.0,
        y: 0.0,
        width,
        height,
        rx: None,
        ry: None,
    })];
    painter.frame(&mut root);
    painter.pipelines(&mut root);
    painter.links(&mut root);
    painter.trends(&mut root);
    let nodes = (0..db.nodes.len()).map(|i| painter.node_group(i)).collect();
    root.push(group("wardley-nodes", nodes));
    painter.annotations(&mut root);
    painter.markings(&mut root);

    let mut scene = Scene::new(width, height);
    scene.view_box = ViewBox::new(0.0, 0.0, width, height);
    scene.font_family = PAGE_FONT.to_string();
    scene.root.children.push(Element::Group(Group {
        common: Common {
            class: Some("wardley-map".to_string()),
            ..Common::default()
        },
        children: root,
    }));
    scene.defs = vec![
        marker(
            "arrow",
            9.0,
            6.0,
            Orient::AutoStartReverse,
            false,
            &theme.evolution_stroke,
        ),
        marker(
            "link-arrow-end",
            9.0,
            5.0,
            Orient::Auto,
            false,
            &theme.link_stroke,
        ),
        marker(
            "link-arrow-start",
            1.0,
            5.0,
            Orient::Auto,
            true,
            &theme.link_stroke,
        ),
    ];
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::scene::Color;
    use serde_json::json;

    #[test]
    fn config_reads_the_section_with_defaults() {
        let cfg = WardleyConfig::from_raw(&json!({}));
        assert_eq!((cfg.width, cfg.height, cfg.padding), (900.0, 600.0, 48.0));
        assert!(!cfg.show_grid);
        let cfg =
            WardleyConfig::from_raw(&json!({ "wardley-beta": { "width": 700, "showGrid": 1 } }));
        assert_eq!(cfg.width, 700.0);
        assert!(cfg.show_grid);
    }

    #[test]
    fn black_is_the_fallback_for_a_fixed_color() {
        assert_eq!(fixed("#000"), Paint::Color(Color::BLACK));
    }
}
