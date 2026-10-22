//! Port of `pieRenderer.ts` (`draw` and `createPieArcs`) with the parts of
//! d3-shape (`pie`, `arc`, `arc.centroid`) and d3-path (`Path.arc`) it
//! calls, and d3-scale's `scaleOrdinal` reduced to the one lookup it does.
//!
//! The layout is fixed: a 450 by 450 chart with a 40 px margin, the legend
//! to the right of it at `12 * LEGEND_RECT_SIZE`, and the title 25 px below
//! the top. The only measured quantities are the widest legend text, which
//! sets the SVG width, and the title width, which can widen the viewBox on
//! both sides.

use std::f64::consts::{PI, TAU};

use indexmap::IndexMap;
use serde_json::Value;

use super::db::PieDb;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::scene::{
    Circle, Color, Common, Element, FontSpec, Group, Paint, Path, PathData, Rect, Scene, Style,
    Text, TextAnchor, TextRun, TransformOp, ViewBox,
};
use crate::mermaid::styles::pie::{PieStyles, parse_int_prefix};
use crate::mermaid::theme::khroma::js_number;
use crate::text::FontSpec as MeasureFont;
use crate::text::fonts::{self, FontSystem, Style as FaceStyle, Weight};

const MARGIN: f64 = 40.0;
const LEGEND_RECT_SIZE: f64 = 18.0;
const LEGEND_SPACING: f64 = 4.0;
const HEIGHT: f64 = 450.0;
const PIE_WIDTH: f64 = HEIGHT;
/// `.pieCircle.highlighted { scale: 1.05 }`.
const HIGHLIGHT_SCALE: f64 = 1.05;

/// d3-shape's `epsilon`.
const SHAPE_EPSILON: f64 = 1e-12;
/// d3-path's `epsilon`.
const PATH_EPSILON: f64 = 1e-6;

/// A `d3.PieArcDatum<D3Section>` with the fields the renderer reads.
#[derive(Debug, Clone, PartialEq)]
struct ArcDatum<'a> {
    label: &'a str,
    value: f64,
    start_angle: f64,
    end_angle: f64,
}

/// `createPieArcs`: sections under one percent of the total are dropped,
/// then `d3.pie().value(d => d.value).sort(null)` lays the rest out
/// clockwise from twelve o'clock over the full circle.
fn create_pie_arcs(sections: &IndexMap<String, f64>) -> Vec<ArcDatum<'_>> {
    let sum: f64 = sections.values().sum();
    let data: Vec<(&str, f64)> = sections
        .iter()
        .map(|(label, value)| (label.as_str(), *value))
        .filter(|(_, value)| (value / sum) * 100.0 >= 1.0)
        .collect();
    // `pie(data)`: only positive values count toward the sum and get an
    // angle; `sort(null)` keeps the data order.
    let positive: f64 = data.iter().map(|(_, v)| *v).filter(|v| *v > 0.0).sum();
    let k = if positive != 0.0 { TAU / positive } else { 0.0 };
    let mut a0 = 0.0;
    data.into_iter()
        .map(|(label, value)| {
            let a1 = a0 + if value > 0.0 { value * k } else { 0.0 };
            let datum = ArcDatum {
                label,
                value,
                start_angle: a0,
                end_angle: a1,
            };
            a0 = a1;
            datum
        })
        .collect()
}

/// d3-path's `Path` as far as `arc()` from d3-shape drives it: the current
/// point decides whether an `arc` call first lines to its start.
struct D3Path {
    data: PathData,
    current: Option<(f64, f64)>,
}

impl D3Path {
    fn new() -> D3Path {
        D3Path {
            data: PathData::new(),
            current: None,
        }
    }

    fn move_to(&mut self, x: f64, y: f64) {
        self.data.move_to(x, y);
        self.current = Some((x, y));
    }

    fn line_to(&mut self, x: f64, y: f64) {
        self.data.line_to(x, y);
        self.current = Some((x, y));
    }

    /// `Path.arc(x, y, r, a0, a1, ccw)`.
    fn arc(&mut self, x: f64, y: f64, r: f64, a0: f64, a1: f64, ccw: bool) {
        let dx = r * a0.cos();
        let dy = r * a0.sin();
        let x0 = x + dx;
        let y0 = y + dy;
        let sweep = !ccw;
        let mut da = if ccw { a0 - a1 } else { a1 - a0 };
        match self.current {
            None => self.move_to(x0, y0),
            Some((cx, cy)) if (cx - x0).abs() > PATH_EPSILON || (cy - y0).abs() > PATH_EPSILON => {
                self.line_to(x0, y0)
            }
            Some(_) => {}
        }
        if r == 0.0 {
            return;
        }
        if da < 0.0 {
            da = da % TAU + TAU;
        }
        if da > TAU - PATH_EPSILON {
            self.data.arc_to(r, r, 0.0, true, sweep, x - dx, y - dy);
            self.data.arc_to(r, r, 0.0, true, sweep, x0, y0);
            self.current = Some((x0, y0));
        } else if da > PATH_EPSILON {
            let x1 = x + r * a1.cos();
            let y1 = y + r * a1.sin();
            self.data.arc_to(r, r, 0.0, da >= PI, sweep, x1, y1);
            self.current = Some((x1, y1));
        }
    }

    fn close_path(&mut self) {
        if self.current.is_some() {
            self.data.close();
        }
    }
}

/// d3-shape `arc()` with `innerRadius(r0)`, `outerRadius(r1)`, no padding
/// and no corner radius, for one `PieArcDatum`.
fn arc_path(r0: f64, r1: f64, start_angle: f64, end_angle: f64) -> PathData {
    let (r0, r1) = if r1 < r0 { (r1, r0) } else { (r0, r1) };
    let a0 = start_angle - PI / 2.0;
    let a1 = end_angle - PI / 2.0;
    let da = (a1 - a0).abs();
    let cw = a1 > a0;
    let mut path = D3Path::new();
    // d3 writes these as `!(x > epsilon)`; the values are finite here, so
    // `<=` is the same test.
    if r1 <= SHAPE_EPSILON {
        path.move_to(0.0, 0.0);
    } else if da > TAU - SHAPE_EPSILON {
        path.move_to(r1 * a0.cos(), r1 * a0.sin());
        path.arc(0.0, 0.0, r1, a0, a1, !cw);
        if r0 > SHAPE_EPSILON {
            path.move_to(r0 * a1.cos(), r0 * a1.sin());
            path.arc(0.0, 0.0, r0, a1, a0, cw);
        }
    } else {
        let x01 = r1 * a0.cos();
        let y01 = r1 * a0.sin();
        let x10 = r0 * a1.cos();
        let y10 = r0 * a1.sin();
        path.move_to(x01, y01);
        if da > SHAPE_EPSILON {
            path.arc(0.0, 0.0, r1, a0, a1, !cw);
        }
        if r0 <= SHAPE_EPSILON || da <= SHAPE_EPSILON {
            path.line_to(x10, y10);
        } else {
            path.arc(0.0, 0.0, r0, a1, a0, cw);
        }
    }
    path.close_path();
    path.data
}

/// `arc.centroid` for an arc whose inner and outer radius are both `r`.
fn centroid(r: f64, start_angle: f64, end_angle: f64) -> (f64, f64) {
    let a = (start_angle + end_angle) / 2.0 - PI / 2.0;
    (a.cos() * r, a.sin() * r)
}

/// `Number.prototype.toFixed(0)`: the nearest integer, ties toward the
/// larger value.
fn to_fixed0(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    if !x.is_finite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    js_number((x + 0.5).floor())
}

/// `parseFontSize(themeVariables.pieOuterStrokeWidth)` with the `?? 2`
/// fallback: a number is taken as is, a string by its leading integer.
fn outer_stroke_width(value: &Value) -> f64 {
    match value {
        Value::Number(n) => n.as_f64().unwrap_or(2.0),
        Value::String(s) => parse_int_prefix(s).unwrap_or(2.0),
        _ => 2.0,
    }
}

/// Temporary: central fix in `text::fontdb_arc` (case-folded alias faces
/// in the shared font database); drop this rewrite at integration.
///
/// The font chain spelled the way the raster step can find it. resvg
/// matches family names case-sensitively while CSS, mermaid's measurer,
/// and `FontSystem` do not, so a lowercase `"trebuchet ms"` from the theme
/// would rasterize in the fallback serif. Each concrete family is replaced
/// by the spelling the font database holds; generic families and unknown
/// ones pass through.
fn raster_font_family(fonts: &FontSystem, chain: &str) -> String {
    let families: Vec<String> = fonts::parse_families(chain)
        .into_iter()
        .map(|family| {
            let lower = family.to_ascii_lowercase();
            let generic = matches!(
                lower.as_str(),
                "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy" | "system-ui"
            );
            if generic {
                return family;
            }
            fonts
                .resolve_family(&family, Weight(400), FaceStyle::Normal)
                .and_then(|id| fonts.family_name(id))
                .map(|name| format!("\"{name}\""))
                .unwrap_or_else(|| format!("\"{family}\""))
        })
        .collect();
    if families.is_empty() {
        chain.to_string()
    } else {
        families.join(", ")
    }
}

fn text_style(fill: Color, size: f64) -> Style {
    Style {
        fill: Some(Paint::Color(fill)),
        font: FontSpec {
            size: Some(size),
            ..FontSpec::default()
        },
        ..Style::default()
    }
}

fn text(
    class: &str,
    x: f64,
    y: f64,
    anchor: Option<TextAnchor>,
    style: Style,
    content: &str,
) -> Text {
    Text {
        common: Common {
            class: Some(class.to_string()),
            style,
            ..Common::default()
        },
        x,
        y,
        anchor,
        baseline: None,
        font: FontSpec::default(),
        runs: vec![TextRun::new(content)],
    }
}

/// `pieRenderer.draw`.
pub fn draw(db: &PieDb, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Scene {
    let cfg = &resolved.config.pie;
    let tv = &resolved.theme_variables;
    let styles = PieStyles::resolve(tv, &resolved.config.font_family);
    let outer_stroke_width = outer_stroke_width(tv.get("pieOuterStrokeWidth"));

    let text_position = *cfg.text_position;
    let donut = *cfg.donut_hole;
    let inner_hole = if donut > 0.0 && donut <= 0.9 {
        donut
    } else {
        0.0
    };
    let radius = PIE_WIDTH.min(HEIGHT) / 2.0 - MARGIN;

    let sections = db.sections();
    let sum: f64 = sections.values().sum();
    let arcs = create_pie_arcs(sections);
    // Arcs that would label as 0% are dropped a second time; after the one
    // percent filter this only matters for a zero total.
    let filtered: Vec<&ArcDatum<'_>> = arcs
        .iter()
        .filter(|d| to_fixed0((d.value / sum) * 100.0) != "0")
        .collect();
    // `scaleOrdinal(pie1..pie12).domain(sections.keys())`.
    let color = |label: &str| -> Color {
        let i = sections.get_index_of(label).unwrap_or(0);
        styles.colors[i % styles.colors.len()]
    };

    let mut pie = Group::default();
    pie.children.push(Element::Circle(Circle {
        common: Common {
            class: Some("pieOuterCircle".to_string()),
            style: Style {
                fill: Some(Paint::None),
                stroke: styles.outer_stroke_color.map(Paint::Color),
                stroke_width: Some(styles.outer_stroke_width),
                ..Style::default()
            },
            ..Common::default()
        },
        cx: 0.0,
        cy: 0.0,
        r: radius + outer_stroke_width / 2.0,
    }));
    for d in &filtered {
        let highlighted = cfg.highlight_slice == d.label;
        let mut class = "pieCircle".to_string();
        if cfg.highlight_slice == "hover" {
            class.push_str(" highlightedOnHover");
        } else if highlighted {
            class.push_str(" highlighted");
        }
        pie.children.push(Element::Path(Path {
            common: Common {
                class: Some(class),
                transform: if highlighted {
                    vec![TransformOp::Scale(HIGHLIGHT_SCALE, HIGHLIGHT_SCALE)]
                } else {
                    Vec::new()
                },
                style: Style {
                    fill: Some(Paint::Color(color(d.label))),
                    stroke: styles.stroke_color.map(Paint::Color),
                    stroke_width: Some(styles.stroke_width),
                    opacity: Some(if highlighted { 1.0 } else { styles.opacity }),
                    ..Style::default()
                },
                ..Common::default()
            },
            d: arc_path(inner_hole * radius, radius, d.start_angle, d.end_angle),
        }));
    }
    for d in &filtered {
        let (cx, cy) = centroid(radius * text_position, d.start_angle, d.end_angle);
        let mut label = text(
            "slice",
            0.0,
            0.0,
            Some(TextAnchor::Middle),
            text_style(styles.section_color, styles.section_size),
            &format!("{}%", to_fixed0((d.value / sum) * 100.0)),
        );
        label.common.transform = vec![TransformOp::Translate(cx, cy)];
        pie.children.push(Element::Text(label));
    }

    let title = db.diagram_title();
    let title_font = MeasureFont::new(styles.font_family.clone(), styles.title_size as f32);
    let title_width = if title.is_empty() {
        0.0
    } else {
        f64::from(ctx.metrics.svg_bbox(title, &title_font).width)
    };

    let legend_font = MeasureFont::new(styles.font_family.clone(), styles.legend_size as f32);
    let legend_texts: Vec<String> = sections
        .iter()
        .map(|(label, value)| {
            if db.show_data() {
                format!("{label} [{}]", js_number(*value))
            } else {
                label.clone()
            }
        })
        .collect();
    // `Math.max(...[])` is `-Infinity`, which then flows into the width.
    let longest_text_width = legend_texts
        .iter()
        .map(|t| f64::from(ctx.metrics.svg_bbox(t, &legend_font).width))
        .fold(f64::NEG_INFINITY, f64::max);

    let mut chart_and_legend_height = HEIGHT;
    let mut chart_and_legend_width = PIE_WIDTH + MARGIN;
    let legend_height = LEGEND_RECT_SIZE + LEGEND_SPACING;
    let count = sections.len() as f64;
    let total_legend_height = count * legend_height;
    let offset = (legend_height * count) / 2.0;
    let legend_block = LEGEND_RECT_SIZE + LEGEND_SPACING + longest_text_width;

    let legend_transform: Box<dyn Fn(f64) -> (f64, f64)> = match cfg.legend_position.as_str() {
        "center" => Box::new(move |i| {
            (
                -longest_text_width / 2.0 - (LEGEND_RECT_SIZE + LEGEND_SPACING),
                i * legend_height - offset,
            )
        }),
        "top" => {
            chart_and_legend_height += total_legend_height;
            pie.common.transform = vec![TransformOp::Translate(
                0.0,
                total_legend_height + legend_height,
            )];
            Box::new(move |i| {
                (
                    -longest_text_width / 2.0 - (LEGEND_RECT_SIZE + LEGEND_SPACING),
                    i * legend_height - radius,
                )
            })
        }
        "bottom" => {
            chart_and_legend_height += total_legend_height;
            Box::new(move |i| {
                (
                    -longest_text_width / 2.0 - (LEGEND_RECT_SIZE + LEGEND_SPACING),
                    i * legend_height + radius + legend_height,
                )
            })
        }
        "left" => {
            chart_and_legend_width += legend_block;
            pie.common.transform = vec![TransformOp::Translate(
                longest_text_width + LEGEND_RECT_SIZE + LEGEND_SPACING,
                0.0,
            )];
            Box::new(move |i| {
                (
                    -radius - (LEGEND_RECT_SIZE + LEGEND_SPACING),
                    i * legend_height - offset,
                )
            })
        }
        _ => {
            chart_and_legend_width += legend_block;
            Box::new(move |i| (12.0 * LEGEND_RECT_SIZE, i * legend_height - offset))
        }
    };

    let mut outer = Group {
        common: Common {
            transform: vec![TransformOp::Translate(PIE_WIDTH / 2.0, HEIGHT / 2.0)],
            ..Common::default()
        },
        children: vec![Element::Group(pie)],
    };
    if !title.is_empty() {
        outer.children.push(Element::Text(text(
            "pieTitleText",
            0.0,
            -(HEIGHT - 50.0) / 2.0,
            Some(TextAnchor::Middle),
            text_style(styles.title_color, styles.title_size),
            title,
        )));
    }
    for (i, ((label, _), content)) in sections.iter().zip(&legend_texts).enumerate() {
        let (tx, ty) = legend_transform(i as f64);
        let fill = color(label);
        outer.children.push(Element::Group(Group {
            common: Common {
                class: Some("legend".to_string()),
                transform: vec![TransformOp::Translate(tx, ty)],
                ..Common::default()
            },
            children: vec![
                Element::Rect(Rect {
                    common: Common::with_style(Style {
                        fill: Some(Paint::Color(fill)),
                        stroke: Some(Paint::Color(fill)),
                        ..Style::default()
                    }),
                    x: 0.0,
                    y: 0.0,
                    width: LEGEND_RECT_SIZE,
                    height: LEGEND_RECT_SIZE,
                    rx: None,
                    ry: None,
                }),
                Element::Text(text(
                    "legendText",
                    LEGEND_RECT_SIZE + LEGEND_SPACING,
                    LEGEND_RECT_SIZE - LEGEND_SPACING,
                    None,
                    text_style(styles.legend_color, styles.legend_size),
                    content,
                )),
            ],
        }));
    }

    // The title sits at `pieWidth / 2` in SVG coordinates; a title wider
    // than the chart widens the viewBox on both sides.
    let title_left = PIE_WIDTH / 2.0 - title_width / 2.0;
    let title_right = PIE_WIDTH / 2.0 + title_width / 2.0;
    let view_box_x = title_left.min(0.0);
    let view_box_right = chart_and_legend_width.max(title_right);
    let total_width = view_box_right - view_box_x;

    let mut scene = Scene::new(total_width, chart_and_legend_height);
    scene.view_box = ViewBox::new(view_box_x, 0.0, total_width, chart_and_legend_height);
    scene.font_family = raster_font_family(ctx.fonts, &styles.font_family);
    scene.root.children.push(Element::Group(outer));
    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_paths_match_the_oracle_svg() {
        // `pie title Pets / "Dogs" : 386 / "Cats" : 85`, from the oracle.
        let mut sections = IndexMap::new();
        sections.insert("Dogs".to_string(), 386.0);
        sections.insert("Cats".to_string(), 85.0);
        let arcs = create_pie_arcs(&sections);
        assert_eq!(
            arc_path(0.0, 185.0, arcs[0].start_angle, arcs[0].end_angle).to_svg(),
            "M0,-185A185,185,0,1,1,-167.623,-78.278L0,0Z"
        );
        assert_eq!(
            arc_path(0.0, 185.0, arcs[1].start_angle, arcs[1].end_angle).to_svg(),
            "M-167.623,-78.278A185,185,0,0,1,0,-185L0,0Z"
        );
        let (cx, cy) = centroid(185.0 * 0.75, arcs[0].start_angle, arcs[0].end_angle);
        assert!((cx - 74.5177955801092).abs() < 1e-9);
        assert!((cy - 117.04127751302553).abs() < 1e-9);
    }

    #[test]
    fn a_single_slice_is_two_half_circle_arcs() {
        let mut sections = IndexMap::new();
        sections.insert("All".to_string(), 7.0);
        let arcs = create_pie_arcs(&sections);
        assert_eq!(
            arc_path(0.0, 185.0, arcs[0].start_angle, arcs[0].end_angle).to_svg(),
            "M0,-185A185,185,0,1,1,0,185A185,185,0,1,1,0,-185Z"
        );
        assert_eq!(
            arc_path(37.0, 185.0, arcs[0].start_angle, arcs[0].end_angle).to_svg(),
            "M0,-185A185,185,0,1,1,0,185A185,185,0,1,1,0,-185M0,-37A37,37,0,1,0,0,37A37,37,0,1,0,0,-37Z"
        );
    }

    #[test]
    fn small_slices_drop_out_and_the_rest_fill_the_circle() {
        let mut sections = IndexMap::new();
        sections.insert("Big".to_string(), 1000.0);
        sections.insert("Tiny".to_string(), 3.0);
        sections.insert("Small".to_string(), 12.0);
        let arcs = create_pie_arcs(&sections);
        let labels: Vec<&str> = arcs.iter().map(|a| a.label).collect();
        assert_eq!(labels, vec!["Big", "Small"]);
        assert!((arcs[1].end_angle - TAU).abs() < 1e-12);
        assert_eq!(to_fixed0(0.4), "0");
        assert_eq!(to_fixed0(0.5), "1");
        assert_eq!(to_fixed0(81.95), "82");
    }
}
