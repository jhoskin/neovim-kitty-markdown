//! Port of the radar `renderer.ts` (`draw`, `drawFrame`, `drawGraticule`,
//! `drawAxes`, `drawCurves`, `relativeRadius`, `closedRoundCurve`,
//! `drawLegend`).
//!
//! The layout is fixed by the config: a `width` by `height` chart inside
//! the four margins, centred by one group. Nothing is measured; labels are
//! placed with `text-anchor` and `dominant-baseline`.

use std::f64::consts::PI;

use serde_json::Value;

use super::db::{Graticule, RadarDb};
use crate::mermaid::scene::{
    Baseline, Circle, Color, Common, Element, FontSpec, Group, Line, Paint, Path, PathData, Point,
    Polygon, Rect, Scene, Style, Text, TextAnchor, TextRun, TransformOp, ViewBox,
};
use crate::mermaid::styles::radar::RadarStyles;

/// `drawLegend`: one row every 20 px.
const LEGEND_LINE_HEIGHT: f64 = 20.0;
/// `drawLegend`: the box is 12 px square whatever `legendBoxSize` says.
const LEGEND_BOX_SIZE: f64 = 12.0;
/// `drawLegend`: the label starts 16 px right of the box origin.
const LEGEND_TEXT_X: f64 = 16.0;
/// `drawAxes`: `labelPad`, pushes a label outward from the axis end.
const LABEL_PAD: f64 = 4.0;

/// `RadarDiagramConfig`: `DEFAULT_CONFIG.radar` with `getConfig().radar`
/// merged over it.
#[derive(Debug, Clone, PartialEq)]
pub struct RadarConfig {
    pub width: f64,
    pub height: f64,
    pub margin_top: f64,
    pub margin_right: f64,
    pub margin_bottom: f64,
    pub margin_left: f64,
    pub axis_scale_factor: f64,
    pub axis_label_factor: f64,
    pub curve_tension: f64,
}

impl RadarConfig {
    /// The `radar` section of the effective config; a value that is not a
    /// number keeps the schema default.
    pub fn from_raw(raw: &Value) -> RadarConfig {
        let section = &raw["radar"];
        let number =
            |key: &str, default: f64| section.get(key).and_then(Value::as_f64).unwrap_or(default);
        RadarConfig {
            width: number("width", 600.0),
            height: number("height", 600.0),
            margin_top: number("marginTop", 50.0),
            margin_right: number("marginRight", 50.0),
            margin_bottom: number("marginBottom", 50.0),
            margin_left: number("marginLeft", 50.0),
            axis_scale_factor: number("axisScaleFactor", 1.0),
            axis_label_factor: number("axisLabelFactor", 1.05),
            curve_tension: number("curveTension", 0.17),
        }
    }
}

fn font(size: f64) -> FontSpec {
    FontSpec {
        size: Some(size),
        ..FontSpec::default()
    }
}

fn text(
    class: &str,
    at: Point,
    anchor: TextAnchor,
    baseline: Baseline,
    fill: Color,
    size: f64,
    content: &str,
) -> Element {
    Element::Text(Text {
        common: Common {
            class: Some(class.to_string()),
            style: Style {
                fill: Some(Paint::Color(fill)),
                ..Style::default()
            },
            ..Common::default()
        },
        x: at.x,
        y: at.y,
        anchor: Some(anchor),
        baseline: Some(baseline),
        font: font(size),
        runs: vec![TextRun::new(content)],
    })
}

/// The fill and stroke of a `.radarCurve-i` or `.radarLegendBox-i` rule.
/// Past `THEME_COLOR_LIMIT` there is no rule: the SVG default black fill
/// and no stroke.
fn index_style(styles: &RadarStyles, index: usize, stroke_width: Option<f64>) -> Style {
    match styles.colors.get(index) {
        Some(color) => Style {
            fill: Some(color.map_or(Paint::Color(Color::BLACK), Paint::Color)),
            fill_opacity: Some(styles.curve_opacity),
            stroke: color.map(Paint::Color),
            stroke_width,
            ..Style::default()
        },
        None => Style {
            fill: Some(Paint::Color(Color::BLACK)),
            ..Style::default()
        },
    }
}

/// `drawGraticule`: one circle or polygon per tick, the rings evenly
/// spaced out to the radius.
fn draw_graticule(
    g: &mut Vec<Element>,
    num_axes: usize,
    radius: f64,
    ticks: f64,
    graticule: Graticule,
    styles: &RadarStyles,
) {
    let style = Style {
        fill: Some(
            styles
                .graticule_color
                .map_or(Paint::Color(Color::BLACK), Paint::Color),
        ),
        fill_opacity: Some(styles.graticule_opacity),
        stroke: styles.graticule_color.map(Paint::Color),
        stroke_width: Some(styles.graticule_stroke_width),
        ..Style::default()
    };
    let common = || Common {
        class: Some("radarGraticule".to_string()),
        style: style.clone(),
        ..Common::default()
    };
    let mut i = 0.0;
    while i < ticks {
        let r = (radius * (i + 1.0)) / ticks;
        match graticule {
            Graticule::Circle => g.push(Element::Circle(Circle {
                common: common(),
                cx: 0.0,
                cy: 0.0,
                r,
            })),
            Graticule::Polygon => {
                let n = num_axes as f64;
                let points = (0..num_axes)
                    .map(|j| {
                        let angle = (2.0 * j as f64 * PI) / n - PI / 2.0;
                        Point::new(r * angle.cos(), r * angle.sin())
                    })
                    .collect();
                g.push(Element::Polygon(Polygon {
                    common: common(),
                    points,
                }));
            }
        }
        i += 1.0;
    }
}

/// `drawAxes`: a line from the centre per axis and its label just past
/// the end, anchored away from the centre.
fn draw_axes(
    g: &mut Vec<Element>,
    db: &RadarDb,
    radius: f64,
    cfg: &RadarConfig,
    styles: &RadarStyles,
) {
    let axes = db.axes();
    let n = axes.len() as f64;
    for (i, axis) in axes.iter().enumerate() {
        let angle = (2.0 * i as f64 * PI) / n - PI / 2.0;
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        g.push(Element::Line(Line {
            common: Common {
                class: Some("radarAxisLine".to_string()),
                style: Style {
                    stroke: styles.axis_color.map(Paint::Color),
                    stroke_width: Some(styles.axis_stroke_width),
                    ..Style::default()
                },
                ..Common::default()
            },
            x1: 0.0,
            y1: 0.0,
            x2: radius * cfg.axis_scale_factor * cos_a,
            y2: radius * cfg.axis_scale_factor * sin_a,
        }));
        let anchor = if cos_a > 0.01 {
            TextAnchor::Start
        } else if cos_a < -0.01 {
            TextAnchor::End
        } else {
            TextAnchor::Middle
        };
        let baseline = if sin_a > 0.01 {
            Baseline::Hanging
        } else if sin_a < -0.01 {
            Baseline::Auto
        } else {
            Baseline::Central
        };
        g.push(text(
            "radarAxisLabel",
            Point::new(
                radius * cfg.axis_label_factor * cos_a + LABEL_PAD * cos_a,
                radius * cfg.axis_label_factor * sin_a + LABEL_PAD * sin_a,
            ),
            anchor,
            baseline,
            styles.text_color,
            styles.axis_label_font_size,
            &axis.label,
        ));
    }
}

/// `relativeRadius`: the value clipped to `[min, max]` and scaled to the
/// radius.
pub fn relative_radius(value: f64, min: f64, max: f64, radius: f64) -> f64 {
    let clipped = value.max(min).min(max);
    (radius * (clipped - min)) / (max - min)
}

/// `closedRoundCurve`: a closed Catmull-Rom style spline, one cubic Bézier
/// per pair of neighbouring points with control points `tension` of the
/// way along the neighbours' chord.
pub fn closed_round_curve(points: &[Point], tension: f64) -> PathData {
    let mut d = PathData::new();
    let n = points.len();
    let Some(first) = points.first() else {
        return d;
    };
    d.move_to(first.x, first.y);
    for i in 0..n {
        let p0 = points[(i + n - 1) % n];
        let p1 = points[i];
        let p2 = points[(i + 1) % n];
        let p3 = points[(i + 2) % n];
        d.cubic_to(
            p1.x + (p2.x - p0.x) * tension,
            p1.y + (p2.y - p0.y) * tension,
            p2.x - (p3.x - p1.x) * tension,
            p2.y - (p3.y - p1.y) * tension,
            p2.x,
            p2.y,
        );
    }
    d.close();
    d
}

/// `drawCurves`: a curve without one entry per axis is skipped; the
/// circle graticule gets round curves, the polygon one straight edges.
fn draw_curves(
    g: &mut Vec<Element>,
    db: &RadarDb,
    min: f64,
    max: f64,
    cfg: &RadarConfig,
    styles: &RadarStyles,
) {
    let num_axes = db.axes().len();
    let n = num_axes as f64;
    let radius = cfg.width.min(cfg.height) / 2.0;
    for (index, curve) in db.curves().iter().enumerate() {
        if curve.entries.len() != num_axes {
            continue;
        }
        let points: Vec<Point> = curve
            .entries
            .iter()
            .enumerate()
            .map(|(i, &entry)| {
                let angle = (2.0 * PI * i as f64) / n - PI / 2.0;
                let r = relative_radius(entry, min, max, radius);
                Point::new(r * angle.cos(), r * angle.sin())
            })
            .collect();
        let common = Common {
            class: Some(format!("radarCurve-{index}")),
            style: index_style(styles, index, Some(styles.curve_stroke_width)),
            ..Common::default()
        };
        match db.options().graticule {
            Graticule::Circle => g.push(Element::Path(Path {
                common,
                d: closed_round_curve(&points, cfg.curve_tension),
            })),
            Graticule::Polygon => g.push(Element::Polygon(Polygon { common, points })),
        }
    }
}

/// `drawLegend`: a box and a label per curve, from three quarters of the
/// way to the top-right corner.
fn draw_legend(g: &mut Vec<Element>, db: &RadarDb, cfg: &RadarConfig, styles: &RadarStyles) {
    if !db.options().show_legend {
        return;
    }
    let legend_x = ((cfg.width / 2.0 + cfg.margin_right) * 3.0) / 4.0;
    let legend_y = (-(cfg.height / 2.0 + cfg.margin_top) * 3.0) / 4.0;
    for (index, curve) in db.curves().iter().enumerate() {
        g.push(Element::Group(Group {
            common: Common {
                transform: vec![TransformOp::Translate(
                    legend_x,
                    legend_y + index as f64 * LEGEND_LINE_HEIGHT,
                )],
                ..Common::default()
            },
            children: vec![
                Element::Rect(Rect {
                    common: Common {
                        class: Some(format!("radarLegendBox-{index}")),
                        style: index_style(styles, index, None),
                        ..Common::default()
                    },
                    x: 0.0,
                    y: 0.0,
                    width: LEGEND_BOX_SIZE,
                    height: LEGEND_BOX_SIZE,
                    rx: None,
                    ry: None,
                }),
                text(
                    "radarLegendText",
                    Point::new(LEGEND_TEXT_X, 0.0),
                    TextAnchor::Start,
                    Baseline::Hanging,
                    styles.text_color,
                    styles.legend_font_size,
                    &curve.label,
                ),
            ],
        }));
    }
}

/// `draw`.
pub fn draw(db: &RadarDb, cfg: &RadarConfig, styles: &RadarStyles) -> Scene {
    let options = db.options();
    // `Math.max(...curves.map(c => Math.max(...c.entries)))`, `-Infinity`
    // without curves, which then draws nothing.
    let max = options.max.unwrap_or_else(|| {
        db.curves()
            .iter()
            .map(|c| c.entries.iter().copied().fold(f64::NEG_INFINITY, f64::max))
            .fold(f64::NEG_INFINITY, f64::max)
    });
    let min = options.min;
    let radius = cfg.width.min(cfg.height) / 2.0;

    let mut children = Vec::new();
    draw_graticule(
        &mut children,
        db.axes().len(),
        radius,
        options.ticks,
        options.graticule,
        styles,
    );
    draw_axes(&mut children, db, radius, cfg, styles);
    draw_curves(&mut children, db, min, max, cfg, styles);
    draw_legend(&mut children, db, cfg, styles);
    let title = db.diagram_title();
    if !title.is_empty() {
        children.push(text(
            "radarTitle",
            Point::new(0.0, -cfg.height / 2.0 - cfg.margin_top),
            TextAnchor::Middle,
            Baseline::Hanging,
            styles.text_color,
            styles.title_font_size,
            title,
        ));
    }

    // `drawFrame`.
    let total_width = cfg.width + cfg.margin_left + cfg.margin_right;
    let total_height = cfg.height + cfg.margin_top + cfg.margin_bottom;
    let mut scene = Scene::new(total_width, total_height);
    scene.view_box = ViewBox::new(0.0, 0.0, total_width, total_height);
    scene.font_family = styles.font_family.clone();
    scene.root.children.push(Element::Group(Group {
        common: Common {
            transform: vec![TransformOp::Translate(
                cfg.margin_left + cfg.width / 2.0,
                cfg.margin_top + cfg.height / 2.0,
            )],
            ..Common::default()
        },
        children,
    }));
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_curve_matches_the_oracle_path() {
        // The first curve of the docs example (`a["Alice"]{85, 90, 80, 70,
        // 75, 90}`, max 100) starts like the oracle SVG.
        let values = [85.0, 90.0, 80.0, 70.0, 75.0, 90.0];
        let points: Vec<Point> = values
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let angle = (2.0 * PI * i as f64) / 6.0 - PI / 2.0;
                let r = relative_radius(v, 0.0, 100.0, 300.0);
                Point::new(r * angle.cos(), r * angle.sin())
            })
            .collect();
        let d = closed_round_curve(&points, 0.17).to_svg();
        assert!(
            d.starts_with("M0,-255C79.501,-255,198.493,-198.75,233.827,-135"),
            "{d}"
        );
        assert!(d.ends_with('Z'));
    }

    #[test]
    fn an_empty_chart_is_the_bare_frame() {
        use crate::mermaid::theme;
        let tv = theme::resolve("default", None).unwrap();
        let styles = RadarStyles::resolve(&tv, "serif");
        let cfg = RadarConfig::from_raw(&json!({}));
        let scene = draw(&RadarDb::new(), &cfg, &styles);
        assert_eq!((scene.width, scene.height), (700.0, 700.0));
        let Element::Group(g) = &scene.root.children[0] else {
            panic!("no frame group");
        };
        // Five graticule circles and nothing else.
        assert_eq!(g.children.len(), 5);
    }

    #[test]
    fn values_clip_to_the_range() {
        assert_eq!(relative_radius(10.0, 20.0, 80.0, 300.0), 0.0);
        assert_eq!(relative_radius(95.0, 20.0, 80.0, 300.0), 300.0);
        assert_eq!(relative_radius(50.0, 20.0, 80.0, 300.0), 150.0);
    }

    #[test]
    fn config_reads_the_section_with_defaults() {
        let cfg = RadarConfig::from_raw(&json!({ "radar": { "curveTension": 0.4, "width": 400 } }));
        assert_eq!(cfg.curve_tension, 0.4);
        assert_eq!(cfg.width, 400.0);
        assert_eq!(cfg.height, 600.0);
        assert_eq!(cfg.axis_label_factor, 1.05);
        assert_eq!(RadarConfig::from_raw(&json!({})).margin_top, 50.0);
    }
}
