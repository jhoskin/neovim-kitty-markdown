//! Port of `draw` in `quadrantRenderer.ts` (mermaid 12.2.0): the builder
//! output as SVG elements in the order the renderer appends them.

use super::builder::{Build, HorizontalPos, LineType, PointType, TextType, VerticalPos};
use crate::mermaid::config::QuadrantChartConfig;
use crate::mermaid::scene::{
    Baseline, Circle, Common, Element, Group, Line, Rect, Scene, Text, TextAnchor, TextRun,
    TransformOp,
};
use crate::mermaid::styles::quadrant as styles;

/// `getDominantBaseLine`.
fn dominant_baseline(pos: HorizontalPos) -> Baseline {
    match pos {
        HorizontalPos::Top => Baseline::Hanging,
        HorizontalPos::Middle => Baseline::Middle,
    }
}

/// `getTextAnchor`.
fn text_anchor(pos: VerticalPos) -> TextAnchor {
    match pos {
        VerticalPos::Left => TextAnchor::Start,
        VerticalPos::Center => TextAnchor::Middle,
    }
}

/// A `<text x="0" y="0">` with `getTransformation`'s `translate(x, y)
/// rotate(r)`.
fn text(t: &TextType) -> Element {
    Element::Text(Text {
        common: Common {
            transform: vec![
                TransformOp::Translate(t.x, t.y),
                TransformOp::Rotate(t.rotation, 0.0, 0.0),
            ],
            style: styles::text_style(t.fill.as_deref()),
            ..Common::default()
        },
        x: 0.0,
        y: 0.0,
        anchor: Some(text_anchor(t.vertical_pos)),
        baseline: Some(dominant_baseline(t.horizontal_pos)),
        font: styles::text_font(t.font_size),
        runs: vec![TextRun::new(t.text.clone())],
    })
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

fn line(l: &LineType) -> Element {
    Element::Line(Line {
        common: Common::with_style(styles::line_style(l.stroke_fill.as_deref(), l.stroke_width)),
        x1: l.x1,
        y1: l.y1,
        x2: l.x2,
        y2: l.y2,
    })
}

fn data_point(p: &PointType) -> Element {
    let circle = Element::Circle(Circle {
        common: Common::with_style(styles::point_style(
            p.fill.as_deref(),
            p.stroke_color.as_deref(),
            &p.stroke_width,
        )),
        cx: p.x,
        cy: p.y,
        r: p.radius,
    });
    group("data-point", vec![circle, text(&p.text)])
}

/// `draw`: a `chartWidth` by `chartHeight` viewBox, `configureSvgSize` with
/// `useMaxWidth`, the root `fill` from the global stylesheet, and the five
/// groups `quadrants`, `border`, `data-points`, `labels`, `title` in that
/// order.
pub fn draw(
    build: &Build,
    config: &QuadrantChartConfig,
    font_family: &str,
    text_color: Option<&str>,
) -> Scene {
    let width = *config.chart_width;
    let height = *config.chart_height;
    let mut scene = Scene::new(width, height);
    scene.font_family = font_family.to_string();
    scene.root.common.style = styles::root_style(text_color);

    let quadrants = build
        .quadrants
        .iter()
        .map(|q| {
            let rect = Element::Rect(Rect {
                common: Common::with_style(styles::text_style(q.fill.as_deref())),
                x: q.x,
                y: q.y,
                width: q.width,
                height: q.height,
                rx: None,
                ry: None,
            });
            group("quadrant", vec![rect, text(&q.text)])
        })
        .collect();
    let borders = build.border_lines.iter().map(line).collect();
    let points = build.points.iter().map(data_point).collect();
    let labels = build
        .axis_labels
        .iter()
        .map(|l| group("label", vec![text(l)]))
        .collect();
    let title = build.title.iter().map(text).collect();

    scene.root.children.push(group(
        "main",
        vec![
            group("quadrants", quadrants),
            group("border", borders),
            group("data-points", points),
            group("labels", labels),
            group("title", title),
        ],
    ));
    scene
}
