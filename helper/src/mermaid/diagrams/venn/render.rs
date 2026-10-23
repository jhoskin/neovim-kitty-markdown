//! Port of `vennRenderer.ts` `draw` (mermaid 12.2.0) and the drawing half
//! of venn.js 2.0.0's `VennDiagram` chart.
//!
//! `draw` runs `VennDiagram` on a detached `div` at `width` by `height -
//! titleHeight` with the chart's own 15 px padding, restyles what it
//! drew, and moves it under a `translate(0, titleHeight)` group. A second
//! layout, `venn.layout` with the config padding, only places the text
//! nodes. The chart's labels go through `wrapText`, which measures with
//! `getComputedTextLength()` on the detached element; that returns 0, so
//! a label never wraps and is one `tspan` with `dy="0.35em"`. The chart
//! also calls `.attr('d', fn, round)`, which d3 reads as two arguments,
//! so the paths are not rounded.
//!
//! The text nodes are HTML spans in `foreignObject`s: flex boxes that
//! centre their text both ways and wrap it at the box width. The scene
//! draws them as SVG text lines with the same breaks and line height.

use indexmap::IndexMap;
use serde_json::Value;

use super::db::{VennData, VennDb, VennStyleData, VennTextData};
use super::layout::{self, Area, Circle, Solution};
use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::scene::{
    Baseline, Common, Element, FontSpec, Group, Length, Paint, Path, PathData, Scene, Style, Text,
    TextAnchor, TextRun, TransformOp, ViewBox,
};
use crate::mermaid::styles::gantt::css_color;
use crate::mermaid::styles::sequence::css_paint;
use crate::mermaid::styles::venn::{TITLE_FONT_SIZE, VennStyles};
use crate::mermaid::theme::khroma;
use crate::text::FontSpec as MeasureSpec;
use crate::text::fonts::{Style as FontStyle, Weight};

/// `const REFERENCE_WIDTH = 1600`: sizes scale by `width / 1600`.
const REFERENCE_WIDTH: f64 = 1600.0;
/// `VennDiagram`'s `padding = 15`; `draw` never sets it.
const CHART_PADDING: f64 = 15.0;

/// The `venn` config values `draw` reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VennConfig {
    pub width: f64,
    pub height: f64,
    pub padding: f64,
    pub use_debug_layout: bool,
}

impl VennConfig {
    /// `config?.width ?? 800`, `config?.height ?? 450`, `config?.padding
    /// ?? 15`, `config?.useDebugLayout ?? false` over the merged section.
    pub fn from_section(section: &Value) -> VennConfig {
        let num =
            |key: &str, fallback: f64| section.get(key).and_then(Value::as_f64).unwrap_or(fallback);
        VennConfig {
            width: num("width", 800.0),
            height: num("height", 450.0),
            padding: num("padding", 15.0),
            use_debug_layout: section
                .get("useDebugLayout")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }
}

/// `buildStyleByKey(styleData)`: later statements merge into earlier ones.
fn build_style_by_key(style_data: &[VennStyleData]) -> IndexMap<String, IndexMap<String, String>> {
    let mut map: IndexMap<String, IndexMap<String, String>> = IndexMap::new();
    for entry in style_data {
        let key = entry.targets.join("|");
        let existing = map.entry(key).or_default();
        for (k, v) in &entry.styles {
            existing.insert(k.clone(), v.clone());
        }
    }
    map
}

/// `ensurePairwiseSubsets(subsets)`: every pair inside a union of three or
/// more sets, sized a quarter of the smaller set (2.5 without sizes).
pub fn ensure_pairwise_subsets(subsets: &[VennData]) -> Vec<VennData> {
    let mut existing: Vec<String> = subsets
        .iter()
        .map(|s| {
            let mut sets = s.sets.clone();
            sets.sort_by(|a, b| layout::js_str_cmp(a, b));
            sets.join("|")
        })
        .collect();
    let mut individual: IndexMap<String, f64> = IndexMap::new();
    for s in subsets.iter().filter(|s| s.sets.len() == 1) {
        individual.insert(s.sets[0].clone(), s.size);
    }
    let mut synthetic = Vec::new();
    for subset in subsets {
        if subset.sets.len() < 3 {
            continue;
        }
        let mut members = subset.sets.clone();
        members.sort_by(|a, b| layout::js_str_cmp(a, b));
        for i in 0..members.len() - 1 {
            for j in i + 1..members.len() {
                let pair = vec![members[i].clone(), members[j].clone()];
                let key = pair.join("|");
                if !existing.contains(&key) {
                    existing.push(key);
                    let size = match (individual.get(&pair[0]), individual.get(&pair[1])) {
                        (Some(a), Some(b)) => a.min(*b) / 4.0,
                        _ => 2.5,
                    };
                    synthetic.push(VennData {
                        sets: pair,
                        size,
                        label: Some(String::new()),
                    });
                }
            }
        }
    }
    let mut out = subsets.to_vec();
    out.extend(synthetic);
    out
}

/// The `chart` input filter: a single set of size 0 removes every area
/// that names it.
fn drop_empty_sets(data: &[VennData]) -> Vec<VennData> {
    let remove: Vec<&String> = data
        .iter()
        .filter(|d| d.size == 0.0 && d.sets.len() == 1)
        .map(|d| &d.sets[0])
        .collect();
    data.iter()
        .filter(|d| !d.sets.iter().any(|s| remove.contains(&s)))
        .cloned()
        .collect()
}

fn areas_of(data: &[VennData]) -> Vec<Area> {
    data.iter()
        .map(|d| Area {
            sets: d.sets.clone(),
            size: d.size,
            weight: None,
        })
        .collect()
}

fn layout_error(e: layout::LayoutError) -> HelperError {
    HelperError::internal(e.to_string())
}

fn khroma_error(e: khroma::KhromaError) -> HelperError {
    HelperError::internal(e.to_string())
}

/// `intersectionAreaPath(circles)` without rounding, in absolute commands:
/// venn.js writes a lone circle as `M x y m -r 0 a r r 0 1 0 2r 0 a r r 0
/// 1 0 -2r 0`, which is the same outline.
fn area_path(circles: &[Circle]) -> PathData {
    let arcs = layout::intersection_area_arcs(circles);
    let mut d = PathData::new();
    match arcs.as_slice() {
        [] => {
            d.move_to(0.0, 0.0);
        }
        [arc] => {
            let c = arc.circle;
            d.move_to(c.x - c.radius, c.y);
            d.arc_to(c.radius, c.radius, 0.0, true, false, c.x + c.radius, c.y);
            d.arc_to(c.radius, c.radius, 0.0, true, false, c.x - c.radius, c.y);
        }
        [first, ..] => {
            d.move_to(first.p2.x, first.p2.y);
            for arc in &arcs {
                let r = arc.circle.radius;
                d.arc_to(r, r, 0.0, arc.large, arc.sweep, arc.p1.x, arc.p1.y);
            }
        }
    }
    d
}

/// A CSS number such as `0.5` or `4px`; anything else is invalid, and the
/// browser drops the declaration.
fn css_number(value: &str) -> Option<f64> {
    let v = value.trim();
    v.strip_suffix("px").unwrap_or(v).parse().ok()
}

/// The chart label: the area's label, else the set id of a single set.
fn chart_label(datum: &VennData) -> String {
    match &datum.label {
        Some(label) if !label.is_empty() => label.clone(),
        _ if datum.sets.len() == 1 => datum.sets[0].clone(),
        _ => String::new(),
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

struct Ctx<'a, 'f> {
    styles: &'a VennStyles,
    style_by_key: &'a IndexMap<String, IndexMap<String, String>>,
    scale: f64,
    theme_dark: bool,
    render: &'a RenderCtx<'f>,
}

impl Ctx<'_, '_> {
    fn custom(&self, sets: &[String]) -> Option<&IndexMap<String, String>> {
        let mut sorted = sets.to_vec();
        sorted.sort_by(|a, b| layout::js_str_cmp(a, b));
        self.style_by_key.get(&sorted.join("|"))
    }

    /// A chart label: `<text class="label" text-anchor="middle" x y>` with
    /// one `<tspan dy="0.35em">`, at the floored text centre.
    fn label(&self, datum: &VennData, solution: &Solution, fill: Paint) -> Element {
        let centre = solution.text_centre(&datum.sets);
        let (x, y) = centre.map_or((0.0, 0.0), |c| (c.x.floor(), c.y.floor()));
        Element::Text(Text {
            common: Common {
                class: Some("label".to_string()),
                style: Style {
                    fill: Some(fill),
                    ..Style::default()
                },
                ..Common::default()
            },
            x,
            y,
            anchor: Some(TextAnchor::Middle),
            baseline: None,
            font: FontSpec {
                family: Some(self.styles.font_family.clone()),
                size: Some(48.0 * self.scale),
                ..FontSpec::default()
            },
            runs: vec![TextRun {
                x: Some(x),
                dy: Some(Length::Em(0.35)),
                ..TextRun::new(chart_label(datum))
            }],
        })
    }

    /// The `.venn-circle` restyle for the `i`-th circle.
    fn circle(
        &self,
        i: usize,
        datum: &VennData,
        solution: &Solution,
    ) -> Result<Element, HelperError> {
        let custom = self.custom(&datum.sets);
        let get = |k: &str| custom.and_then(|c| c.get(k)).map(String::as_str);
        let truthy = |k: &str| get(k).filter(|v| !v.is_empty());
        let palette = &self.styles.palette;
        let palette_color = (!palette.is_empty()).then(|| palette[i % palette.len()].as_str());
        let base_color = truthy("fill")
            .or(palette_color)
            .unwrap_or(&self.styles.primary_color)
            .to_string();
        let fill_opacity = get("fill-opacity").map_or(Some(0.1), css_number);
        let stroke_color = truthy("stroke").unwrap_or(&base_color).to_string();
        let stroke_width = truthy("stroke-width").map_or(Some(5.0 * self.scale), css_number);
        let text_color = match truthy("color") {
            Some(c) => c.to_string(),
            None if self.theme_dark => khroma::lighten(&base_color, 30.0).map_err(khroma_error)?,
            None => khroma::darken(&base_color, 30.0).map_err(khroma_error)?,
        };
        let circles = solution.circles_of(&datum.sets).unwrap_or_default();
        let path = Element::Path(Path {
            common: Common::with_style(Style {
                fill: Some(css_paint(&base_color)),
                fill_opacity,
                stroke: Some(css_paint(&stroke_color)),
                stroke_width,
                stroke_opacity: Some(0.95),
                ..Style::default()
            }),
            d: area_path(&circles),
        });
        let text = self.label(datum, solution, css_paint(&text_color));
        Ok(group(
            &format!("venn-area venn-circle venn-set-{}", i % 8),
            vec![path, text],
        ))
    }

    /// The `.venn-intersection` restyle: transparent unless a style sets
    /// its fill, which then paints opaque.
    fn intersection(&self, datum: &VennData, solution: &Solution) -> Element {
        let custom = self.custom(&datum.sets);
        let fill = custom.and_then(|c| c.get("fill"));
        let color = custom
            .and_then(|c| c.get("color"))
            .unwrap_or(&self.styles.region_text_fill);
        let circles = solution.circles_of(&datum.sets).unwrap_or_default();
        let path = Element::Path(Path {
            common: Common::with_style(Style {
                fill: Some(fill.map_or(Paint::None, |f| css_paint(f))),
                fill_opacity: Some(if fill.is_some_and(|f| !f.is_empty()) {
                    1.0
                } else {
                    0.0
                }),
                ..Style::default()
            }),
            d: area_path(&circles),
        });
        let text = self.label(datum, solution, css_paint(color));
        group("venn-area venn-intersection", vec![path, text])
    }

    /// Greedy CSS line breaking at spaces (`white-space: normal`,
    /// `overflow-wrap: normal`): a word wider than the box overflows.
    fn wrap(&self, text: &str, spec: &MeasureSpec, max_width: f64) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            if line.is_empty() {
                line = word.to_string();
                continue;
            }
            let candidate = format!("{line} {word}");
            let width = f64::from(self.render.metrics.shape(&candidate, spec).width);
            if width > max_width {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }

    /// `line-height: normal` of the primary face and the baseline offset
    /// inside one line box: Blink rounds the ascent, descent, and line gap
    /// and splits the gap evenly.
    fn line_metrics(&self, spec: &MeasureSpec) -> (f64, f64) {
        let fonts = self.render.metrics.fonts();
        let face = fonts.resolve(&spec.family, Weight(spec.weight), FontStyle::Normal);
        let Some(face) = face else {
            let size = f64::from(spec.size);
            return (size * 1.15, size * 0.9);
        };
        let (ascent, descent) = self.render.metrics.face_ascent_descent(face, spec.size);
        let gap = fonts
            .with_face(face, |f| {
                f32::from(f.line_gap()) / f.units_per_em() as f32 * spec.size
            })
            .unwrap_or(0.0)
            .round();
        let line_height = f64::from(ascent + descent + gap);
        (line_height, f64::from(gap / 2.0 + ascent))
    }

    /// `renderTextNodes`: the text nodes of each area in a grid inside the
    /// area's largest inscribed circle.
    fn text_nodes(
        &self,
        text_nodes: &[VennTextData],
        data: &[VennData],
        solution: &Solution,
        use_debug_layout: bool,
    ) -> Element {
        let scale = self.scale;
        // `layoutByKey`: the last area wins a key.
        let mut by_key: IndexMap<String, &VennData> = IndexMap::new();
        for d in data {
            let mut sorted = d.sets.clone();
            sorted.sort_by(|a, b| layout::js_str_cmp(a, b));
            by_key.insert(sorted.join("|"), d);
        }
        let mut nodes_by_area: IndexMap<String, Vec<&VennTextData>> = IndexMap::new();
        for node in text_nodes {
            nodes_by_area
                .entry(node.sets.join("|"))
                .or_default()
                .push(node);
        }

        let font_size = 40.0 * scale;
        let spec = MeasureSpec::new(self.styles.font_family.clone(), font_size as f32);
        let (line_height, baseline) = self.line_metrics(&spec);

        let mut areas = Vec::new();
        for (key, nodes) in &nodes_by_area {
            let Some(datum) = by_key.get(key) else {
                continue;
            };
            let Some(text) = solution.text_centre(&datum.sets) else {
                continue;
            };
            let circles = solution.circles_of(&datum.sets).unwrap_or_default();
            let (cx, cy) = (text.x, text.y);
            let min_circle_radius = circles
                .iter()
                .map(|c| c.radius)
                .fold(f64::INFINITY, f64::min);
            let inner_raw = circles
                .iter()
                .map(|c| c.radius - (cx - c.x).hypot(cy - c.y))
                .fold(f64::INFINITY, f64::min);
            let mut inner_radius = if inner_raw.is_finite() {
                inner_raw.max(0.0)
            } else {
                0.0
            };
            if inner_radius == 0.0 && min_circle_radius.is_finite() {
                inner_radius = min_circle_radius * 0.6;
            }

            let mut children = Vec::new();
            if use_debug_layout {
                children.push(Element::Circle(crate::mermaid::scene::Circle {
                    common: Common {
                        class: Some("venn-text-debug-circle".to_string()),
                        style: Style {
                            fill: Some(Paint::None),
                            stroke: Some(css_paint("purple")),
                            stroke_width: Some(1.5 * scale),
                            stroke_dasharray: Some(vec![6.0 * scale, 4.0 * scale]),
                            ..Style::default()
                        },
                        ..Common::default()
                    },
                    cx,
                    cy,
                    r: inner_radius,
                }));
            }

            let inner_width = (80.0 * scale).max(inner_radius * 2.0 * 0.95);
            let inner_height = (60.0 * scale).max(inner_radius * 2.0 * 0.95);
            let has_label = datum.label.as_ref().is_some_and(|l| !l.is_empty());
            let label_offset_base = if has_label {
                (32.0 * scale).min(inner_radius * 0.25)
            } else {
                0.0
            };
            let label_offset =
                label_offset_base + if nodes.len() <= 2 { 30.0 * scale } else { 0.0 };
            let start_x = cx - inner_width / 2.0;
            let start_y = cy - inner_height / 2.0 + label_offset;
            let n = nodes.len() as f64;
            let cols = n.sqrt().ceil().max(1.0);
            let rows = (n / cols).ceil().max(1.0);
            let cell_width = inner_width / cols;
            let cell_height = inner_height / rows;

            for (i, node) in nodes.iter().enumerate() {
                let col = (i as f64) % cols;
                let row = (i as f64 / cols).floor();
                let x = start_x + cell_width * (col + 0.5);
                let y = start_y + cell_height * (row + 0.5);
                if use_debug_layout {
                    children.push(Element::Rect(crate::mermaid::scene::Rect {
                        common: Common {
                            class: Some("venn-text-debug-cell".to_string()),
                            style: Style {
                                fill: Some(Paint::None),
                                stroke: Some(css_paint("teal")),
                                stroke_width: Some(scale),
                                stroke_dasharray: Some(vec![4.0 * scale, 3.0 * scale]),
                                ..Style::default()
                            },
                            ..Common::default()
                        },
                        x: start_x + cell_width * col,
                        y: start_y + cell_height * row,
                        width: cell_width,
                        height: cell_height,
                        rx: None,
                        ry: None,
                    }));
                }
                let box_width = cell_width * 0.9;
                let label = node.label.clone().unwrap_or_else(|| node.id.clone());
                let lines = self.wrap(&label, &spec, box_width);
                if lines.is_empty() {
                    continue;
                }
                let color = self
                    .style_by_key
                    .get(&node.id)
                    .and_then(|s| s.get("color"))
                    .map_or(self.styles.text_node_color, |c| css_color(c));
                // The flex item centres in the box, so the first line box
                // starts half the text height above the box centre.
                let top = y - line_height * lines.len() as f64 / 2.0;
                let runs = lines
                    .into_iter()
                    .enumerate()
                    .map(|(k, line)| TextRun {
                        x: Some(x),
                        dy: (k > 0).then_some(Length::Px(line_height)),
                        ..TextRun::new(line)
                    })
                    .collect();
                children.push(Element::Text(Text {
                    common: Common {
                        class: Some("venn-text-node".to_string()),
                        style: Style::fill(color),
                        ..Common::default()
                    },
                    x,
                    y: top + baseline,
                    anchor: Some(TextAnchor::Middle),
                    baseline: Some(Baseline::Alphabetic),
                    font: FontSpec {
                        family: Some(self.styles.font_family.clone()),
                        size: Some(font_size),
                        ..FontSpec::default()
                    },
                    runs,
                }));
            }
            areas.push(group("venn-text-area", children));
        }
        group("venn-text-nodes", areas)
    }
}

pub fn draw(db: &VennDb, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
    let cfg = VennConfig::from_section(&resolved.raw["venn"]);
    let styles = VennStyles::resolve(&resolved.theme_variables);
    let title = db.diagram_title();
    let sets = db.subset_data();
    let text_nodes = db.text_data();
    let style_by_key = build_style_by_key(db.style_data());
    let render_sets = ensure_pairwise_subsets(sets);

    let scale = cfg.width / REFERENCE_WIDTH;
    let title_height = if title.is_some() { 48.0 * scale } else { 0.0 };
    let theme_dark = khroma::is_dark(&styles.background).map_err(khroma_error)?;

    let c = Ctx {
        styles: &styles,
        style_by_key: &style_by_key,
        scale,
        theme_dark,
        render: ctx,
    };

    let mut root = Vec::new();
    if let Some(title) = title {
        root.push(Element::Text(Text {
            common: Common {
                class: Some("venn-title".to_string()),
                style: Style {
                    fill: Some(styles.title_fill.clone()),
                    ..Style::default()
                },
                ..Common::default()
            },
            x: cfg.width * 0.5,
            y: 32.0 * scale,
            anchor: Some(TextAnchor::Middle),
            baseline: Some(Baseline::Middle),
            font: FontSpec {
                family: Some(styles.font_family.clone()),
                size: Some(TITLE_FONT_SIZE),
                ..FontSpec::default()
            },
            runs: vec![TextRun::new(title)],
        }));
    }

    // `VennDiagram().width(svgWidth).height(svgHeight - titleHeight)`.
    let chart_data = drop_empty_sets(&render_sets);
    let chart = layout::solve(
        &areas_of(&chart_data),
        cfg.width,
        cfg.height - title_height,
        CHART_PADDING,
    )
    .map_err(layout_error)?;

    let mut venn_children = Vec::new();
    let mut circle_index = 0;
    for datum in &chart_data {
        if datum.sets.len() == 1 {
            venn_children.push(c.circle(circle_index, datum, &chart)?);
            circle_index += 1;
        } else {
            venn_children.push(c.intersection(datum, &chart));
        }
    }

    if !text_nodes.is_empty() {
        // `venn.layout(renderSets, { width, height: svgHeight -
        // titleHeight, padding: config?.padding ?? 15 })`.
        let placed = layout::solve(
            &areas_of(&render_sets),
            cfg.width,
            cfg.height - title_height,
            cfg.padding,
        )
        .map_err(layout_error)?;
        venn_children.push(c.text_nodes(text_nodes, &render_sets, &placed, cfg.use_debug_layout));
    }

    root.push(Element::Group(Group {
        common: Common {
            transform: vec![TransformOp::Translate(0.0, title_height)],
            ..Common::default()
        },
        children: venn_children,
    }));

    // `configureSvgSize(svg, svgHeight, svgWidth, useMaxWidth)` over the
    // `0 0 svgWidth svgHeight` viewBox.
    Ok(Scene {
        width: cfg.width,
        height: cfg.height,
        view_box: ViewBox::new(0.0, 0.0, cfg.width, cfg.height),
        background: None,
        font_family: styles.font_family.clone(),
        defs: Vec::new(),
        linear_gradients: Vec::new(),
        root: Group {
            common: Common::default(),
            children: root,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(sets: &[&str], size: f64) -> VennData {
        VennData {
            sets: sets.iter().map(|s| s.to_string()).collect(),
            size,
            label: None,
        }
    }

    #[test]
    fn a_three_set_union_adds_its_missing_pairs() {
        let subsets = [
            data(&["A"], 8.0),
            data(&["B"], 4.0),
            data(&["C"], 10.0),
            data(&["A", "B"], 1.0),
            data(&["A", "B", "C"], 0.5),
        ];
        let out = ensure_pairwise_subsets(&subsets);
        assert_eq!(out.len(), 7);
        assert_eq!(out[5].sets, ["A", "C"]);
        assert_eq!(out[5].size, 2.0);
        assert_eq!(out[6].sets, ["B", "C"]);
        assert_eq!(out[6].size, 1.0);
        assert_eq!(out[6].label.as_deref(), Some(""));
    }

    #[test]
    fn an_empty_set_drops_every_area_that_names_it() {
        let out = drop_empty_sets(&[data(&["A"], 0.0), data(&["B"], 3.0), data(&["A", "B"], 1.0)]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].sets, ["B"]);
    }
}
