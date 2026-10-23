//! The C4 elements as mermaid 12.2.0 draws them: `c4ShapeAdapter.ts`
//! builds a unified-renderer node from each shape, and the unified shapes
//! (`person`, `roundedRect`, `cylinder`, `tiltedCylinder`, `subroutine`)
//! draw it with `c4LabelHelper.ts` stacking the name, the stereotype, and
//! the description.
//!
//! The shared shape modules take their label from the generic
//! `labelHelper`; a node with a `stereotype` takes the stacked label
//! instead, so this module ports the five shape bodies the C4 renderer
//! reaches with that label and reuses the geometry helpers and the
//! intersection functions of [`crate::mermaid::shapes`].

use crate::mermaid::scene::{
    Common, Element, FontSpec, FontWeight, Group, Length, Paint, Point, Style, Text, TextAnchor,
    TextRun, TransformOp,
};
use crate::mermaid::shapes::measure::measure_with_metrics;
use crate::mermaid::shapes::util::{
    circle_element, generate_circle_points, group_element, insert_polygon_shape, path_bbox,
    path_element, rect_element, translate,
};
use crate::mermaid::shapes::{Intersector, LabelBox, LabelOpts, Node};
use crate::mermaid::shapes::{cylinder, tilted_cylinder};
use crate::mermaid::styles::c4::{
    C4Styles, DEFAULT_FONT_COLOR, DESCR_FONT_SCALE, SHAPE_STROKE_WIDTH, TYPE_FONT_SCALE,
};
use crate::mermaid::styles::sequence::css_paint;
use crate::text::TextMetrics;

use super::conf::{C4Font, Conf};
use super::db::C4Shape;

/// `SECTION_GAP` of `c4LabelHelper.ts`.
const SECTION_GAP: f64 = 3.0;
/// `MIN_WRAP_WIDTH` of `c4LabelHelper.ts`.
const MIN_WRAP_WIDTH: f64 = 32.0;
/// The `rx:12px; ry:12px` the adapter puts inline on rounded shapes; CSS
/// wins over the `rx` attribute `roundedRect` writes.
const ROUNDED_RADIUS: f64 = 12.0;
/// `FRAME_WIDTH` of `subroutine.ts`.
const FRAME_WIDTH: f64 = 8.0;

/// The unified shape an element resolves to (`resolveNodeShape`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeShape {
    Person,
    Rounded,
    Cylinder,
    HorizontalCylinder,
    FramedRect,
}

/// `SHAPE_KEYWORDS`: the Structurizr keywords `$shape`, `$sprite`, and
/// `$tags` accept.
fn keyword_shape(value: Option<&str>) -> Option<NodeShape> {
    match value?.to_lowercase().as_str() {
        "person" => Some(NodeShape::Person),
        "box" | "rounded" => Some(NodeShape::Rounded),
        "cylinder" | "database" | "db" => Some(NodeShape::Cylinder),
        "queue" | "pipe" => Some(NodeShape::HorizontalCylinder),
        "component" => Some(NodeShape::FramedRect),
        _ => None,
    }
}

/// `resolveNodeShape(shape)`: explicit `$shape`/`$sprite`, then a `$tags`
/// token, then the element type.
pub fn resolve_node_shape(shape: &C4Shape) -> NodeShape {
    let fields = &shape.fields;
    if let Some(explicit) =
        keyword_shape(fields.str("shape")).or_else(|| keyword_shape(fields.str("sprite")))
    {
        return explicit;
    }
    if let Some(tags) = fields.str("tags").filter(|t| !t.is_empty()) {
        for tag in tags.split(',') {
            if let Some(tagged) = keyword_shape(Some(tag.trim())) {
                return tagged;
            }
        }
    }
    let kind = shape.type_c4_shape.as_str();
    if kind == "person" || kind == "external_person" {
        NodeShape::Person
    } else if kind.ends_with("_db") {
        NodeShape::Cylinder
    } else if kind.ends_with("_queue") {
        NodeShape::HorizontalCylinder
    } else {
        NodeShape::Rounded
    }
}

/// `stereotypeLabel(typeC4Shape)`: `Software System` for every system.
fn stereotype_label(type_c4_shape: &str) -> String {
    let base = type_c4_shape
        .strip_prefix("external_")
        .unwrap_or(type_c4_shape);
    let base = base
        .strip_suffix("_db")
        .or_else(|| base.strip_suffix("_queue"))
        .unwrap_or(base);
    match base {
        "person" => "Person".to_string(),
        "system" => "Software System".to_string(),
        "container" => "Container".to_string(),
        "component" => "Component".to_string(),
        other => other.replace('_', " "),
    }
}

/// `stereotypeText(shape)`: `[Container: Node.js]` or `[Person]`.
fn stereotype_text(shape: &C4Shape) -> String {
    let stereotype = stereotype_label(&shape.type_c4_shape);
    match shape.fields.text("techn").filter(|t| !t.is_empty()) {
        Some(techn) => format!("[{stereotype}: {techn}]"),
        None => format!("[{stereotype}]"),
    }
}

/// One drawn element, centered on the origin like a unified shape.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawnShape {
    /// The `g.node` group: the geometry first, the label last.
    pub group: Group,
    /// `node.width` and `node.height` after `updateNodeBounds`.
    pub width: f64,
    pub height: f64,
    /// The data `node.intersect` closes over.
    pub intersector: Intersector,
}

impl DrawnShape {
    /// `node.intersect(point)` with the node centered at `(cx, cy)`.
    pub fn intersect(&self, cx: f64, cy: f64, point: Point) -> Point {
        let node = Node {
            x: cx,
            y: cy,
            width: Some(self.width),
            height: Some(self.height),
            intersector: self.intersector.clone(),
            ..Node::default()
        };
        node.intersect(point)
    }
}

/// One measured label section.
struct Section {
    class: &'static str,
    font: C4Font,
    bbox: LabelBox,
}

/// The stacked label of `c4LabelHelper`, before the shape positions it.
struct StackedLabel {
    group: Group,
    width: f64,
    height: f64,
}

fn scene_font(font: &C4Font) -> FontSpec {
    let weight = match font.weight_number() {
        400 => FontWeight::Normal,
        700 => FontWeight::Bold,
        n => FontWeight::Number(n),
    };
    FontSpec {
        family: Some(font.family.clone()),
        size: Some(font.size),
        weight: Some(weight),
        style: None,
    }
}

/// The paint of a C4 text: its fill. The raster step draws Chrome's
/// synthetic bold for a bold font without a bold face.
pub fn text_style(fill: &Paint) -> Style {
    Style {
        fill: Some(fill.clone()),
        ..Style::default()
    }
}

/// `createText(sectionEl, text, { useHtmlLabels: false, markdown: false,
/// width })` as SVG text: one line per `tspan.text-outer-tspan`, centered
/// on `x = 0`, the first baseline 1 em below the top.
fn section_text(bbox: &LabelBox, font: &C4Font, fill: &Paint) -> Text {
    let mut runs = Vec::new();
    for (i, line) in bbox.lines.iter().enumerate() {
        for (j, run) in line.runs.iter().enumerate() {
            let (text, x, dy) = if j == 0 {
                (
                    run.text.clone(),
                    Some(0.0),
                    Some(Length::Em(if i == 0 { 1.0 } else { 1.1 })),
                )
            } else {
                (format!(" {}", run.text), None, None)
            };
            runs.push(TextRun {
                text,
                x,
                dy,
                ..TextRun::default()
            });
        }
    }
    Text {
        common: Common::with_style(text_style(fill)),
        x: 0.0,
        y: 0.0,
        anchor: Some(TextAnchor::Middle),
        baseline: None,
        font: scene_font(font),
        runs,
    }
}

/// `c4LabelHelper(parent, node)`: measures each non-empty section at the
/// wrap width and stacks them `SECTION_GAP` apart, each centered on the
/// widest.
fn stacked_label(
    metrics: &TextMetrics<'_>,
    sections: &[(&'static str, String, C4Font)],
    wrap_width: Option<f64>,
    fill: &Paint,
) -> StackedLabel {
    let measured: Vec<Section> = sections
        .iter()
        .filter(|(_, text, _)| !text.is_empty())
        .map(|(class, text, font)| {
            let opts = LabelOpts {
                font: scene_font(font),
                html: false,
                width: wrap_width,
                min_width: 0.0,
                markdown: false,
            };
            Section {
                class,
                font: font.clone(),
                bbox: measure_with_metrics(metrics, text, &opts, &font.family),
            }
        })
        .collect();
    let total_width = measured.iter().map(|s| s.bbox.width).fold(0.0, f64::max);
    let mut y = 0.0;
    let mut children = Vec::new();
    for section in &measured {
        let b = &section.bbox;
        children.push(Element::Group(Group {
            common: Common {
                class: Some(section.class.to_string()),
                transform: translate(total_width / 2.0 - b.x - b.width / 2.0, y - b.y),
                ..Common::default()
            },
            children: vec![Element::Text(section_text(b, &section.font, fill))],
        }));
        y += b.height + SECTION_GAP;
    }
    let total_height = if measured.is_empty() {
        0.0
    } else {
        y - SECTION_GAP
    };
    StackedLabel {
        group: Group {
            common: Common {
                class: Some("label".to_string()),
                transform: translate(-total_width / 2.0, -total_height / 2.0),
                ..Common::default()
            },
            children,
        },
        width: total_width,
        height: total_height,
    }
}

/// The node's inline style: `fill`, `stroke` (`elementCssStyles`) and the
/// stylesheet's outline width.
fn geometry_style(fill: Option<Paint>, stroke: Option<Paint>, stroke_width: f64) -> Style {
    Style {
        fill,
        stroke,
        stroke_width: Some(stroke_width),
        ..Style::default()
    }
}

/// Draws one element as `buildC4Node` and its shape handler do:
/// `padding` is `c4ShapePadding` and the width hint is `c4.width`.
pub fn draw(
    shape: &C4Shape,
    conf: &Conf,
    styles: &C4Styles,
    metrics: &TextMetrics<'_>,
) -> DrawnShape {
    let kind = resolve_node_shape(shape);
    let type_c4_shape = shape.type_c4_shape.as_str();
    let (fill, stroke) = C4Styles::element_colors(
        conf,
        type_c4_shape,
        shape.fields.str("bgColor"),
        shape.fields.str("borderColor"),
    );
    let font_color = css_paint(shape.fields.str("fontColor").unwrap_or(DEFAULT_FONT_COLOR));
    let label_font = styles.element_font(conf, type_c4_shape);
    let sections = vec![
        (
            "c4-name",
            shape.label().to_string(),
            label_font.clone().bold(),
        ),
        (
            "c4-type",
            stereotype_text(shape),
            label_font.clone().sized(label_font.size * TYPE_FONT_SCALE),
        ),
        (
            "c4-descr",
            shape.fields.text("descr").unwrap_or("").to_string(),
            label_font.clone().sized(label_font.size * DESCR_FONT_SCALE),
        ),
    ];
    let padding = conf.c4_shape_padding;
    // The width each shape hands `c4LabelHelper` after its own adjustment.
    let node_width = match kind {
        NodeShape::Cylinder => (conf.width - padding).max(8.0),
        // `tiltedCylinder` subtracts `rx * 3`, which is 0 without a height.
        NodeShape::HorizontalCylinder => (conf.width - padding / 2.0).max(10.0),
        _ => conf.width,
    };
    let wrap_width = if conf.wrap {
        let w = if node_width != 0.0 {
            (node_width - 2.0 * padding).max(MIN_WRAP_WIDTH)
        } else {
            200.0
        };
        Some(w)
    } else {
        None
    };
    let label = stacked_label(metrics, &sections, wrap_width, &font_color);
    let (bw, bh) = (label.width, label.height);
    let mut label_group = label.group;
    label_group.common.style.fill = Some(font_color.clone());

    let mut classes = format!("node c4-shape c4-{type_c4_shape}");
    if type_c4_shape.starts_with("external_") {
        classes.push_str(" c4-external");
    }
    let style = geometry_style(fill, stroke, SHAPE_STROKE_WIDTH);

    let (geometry, width, height, intersector) = match kind {
        NodeShape::Person => {
            // `person.ts`.
            let w = (bw + padding * 2.0).max(conf.width).max(100.0);
            let head_radius = (w * 0.23).clamp(16.0, 56.0);
            let overlap = head_radius * 0.27;
            let body_height = bh + padding * 2.0;
            let body_radius = (w * 0.177).min(body_height * 0.45);
            let total_height = body_height + 2.0 * head_radius - overlap;
            let top = -total_height / 2.0;
            let body_top = top + 2.0 * head_radius - overlap;
            let group = group_element(
                "basic label-container",
                Vec::new(),
                vec![
                    rect_element(
                        -w / 2.0,
                        body_top,
                        w,
                        body_height,
                        Some(body_radius),
                        Some(body_radius),
                        "",
                        style.clone(),
                    ),
                    circle_element(0.0, top + head_radius, head_radius, "", style),
                ],
            );
            let body_center_y = body_top + body_height / 2.0;
            label_group.common.transform = translate(-bw / 2.0, body_center_y - bh / 2.0);
            (
                vec![group],
                w,
                total_height,
                Intersector::Polygon(person_outline(
                    w,
                    head_radius,
                    top,
                    body_top,
                    body_radius,
                    total_height,
                )),
            )
        }
        NodeShape::Rounded => {
            // `roundedRect.ts` through `drawRect`, label padding = padding.
            let total_width = (bw + padding * 2.0).max(conf.width);
            let total_height = bh + padding * 2.0;
            let rect = rect_element(
                -total_width / 2.0,
                -total_height / 2.0,
                total_width,
                total_height,
                Some(ROUNDED_RADIUS),
                Some(ROUNDED_RADIUS),
                "basic label-container",
                style,
            );
            (vec![rect], total_width, total_height, Intersector::Rect)
        }
        NodeShape::Cylinder => {
            // `cylinder.ts`: the width shrank by the padding before the
            // label, and grows back by it here.
            let w = node_width.max(bw) + padding;
            let rx = w / 2.0;
            let ry = rx / (2.5 + w / 50.0);
            let h = bh + padding + ry;
            let d = cylinder::cylinder_path(0.0, 0.0, w, h, rx, ry);
            let bbox = path_bbox(&d);
            let path = path_element(
                d,
                "basic label-container outer-path",
                style,
                translate(-w / 2.0, -(h / 2.0 + ry)),
            );
            label_group.common.transform = translate(-bw / 2.0, -(bh / 2.0) + padding / 1.5);
            (
                vec![path],
                bbox.width,
                bbox.height,
                Intersector::Cylinder { rx, ry },
            )
        }
        NodeShape::HorizontalCylinder => {
            // `tiltedCylinder.ts` with `labelPadding = padding / 2`.
            let label_padding = padding / 2.0;
            let h = bh + label_padding;
            let ry = h / 2.0;
            let rx = ry / (2.5 + h / 50.0);
            let w = node_width.max(bw) + rx + label_padding;
            let d = tilted_cylinder::tilted_cylinder_path(w, h, rx, ry);
            let bbox = path_bbox(&d);
            let path = path_element(
                d,
                "basic label-container outer-path",
                style,
                translate(-w / 2.0, h / 2.0),
            );
            label_group.common.transform = translate(-bw / 2.0 - rx, -bh / 2.0);
            (
                vec![path],
                bbox.width,
                bbox.height,
                Intersector::TiltedCylinder { rx, ry },
            )
        }
        NodeShape::FramedRect => {
            // `subroutine.ts`: a polygon, outside the 2 px stylesheet rule.
            let total_width = (bw + 2.0 * FRAME_WIDTH + padding).max(conf.width);
            let total_height = bh + padding;
            let w = total_width - 2.0 * FRAME_WIDTH;
            let h = total_height;
            let points = [
                Point::new(0.0, 0.0),
                Point::new(w, 0.0),
                Point::new(w, -h),
                Point::new(0.0, -h),
                Point::new(0.0, 0.0),
                Point::new(-FRAME_WIDTH, 0.0),
                Point::new(w + FRAME_WIDTH, 0.0),
                Point::new(w + FRAME_WIDTH, -h),
                Point::new(-FRAME_WIDTH, -h),
                Point::new(-FRAME_WIDTH, 0.0),
            ];
            let mut polygon_style = style;
            polygon_style.stroke_width = None;
            let polygon = insert_polygon_shape(w, h, &points, polygon_style);
            (
                vec![polygon],
                w + 2.0 * FRAME_WIDTH,
                h,
                Intersector::Polygon(points.to_vec()),
            )
        }
    };

    let mut children = geometry;
    children.push(Element::Group(label_group));
    DrawnShape {
        group: Group {
            common: Common {
                id: Some(format!("{}-{}", super::render::DIAGRAM_ID, shape.alias)),
                class: Some(classes),
                ..Common::default()
            },
            children,
        },
        width,
        height,
        intersector,
    }
}

/// The person's edge outline: the head's exposed arc joined to the body's
/// rounded rectangle (`person.ts`).
fn person_outline(
    w: f64,
    head_radius: f64,
    top: f64,
    body_top: f64,
    body_radius: f64,
    total_height: f64,
) -> Vec<Point> {
    let head_center_y = top + head_radius;
    let phi_right_deg =
        ((body_top - head_center_y) / head_radius).min(1.0).asin() * 180.0 / std::f64::consts::PI;
    let mut outline = generate_circle_points(
        0.0,
        -head_center_y,
        head_radius,
        24,
        180.0 + phi_right_deg,
        -phi_right_deg,
    );
    let corners = [
        (
            -(-w / 2.0 + body_radius),
            -(body_top + body_radius),
            90.0,
            0.0,
        ),
        (
            -(-w / 2.0 + body_radius),
            -(total_height / 2.0 - body_radius),
            360.0,
            270.0,
        ),
        (
            -(w / 2.0 - body_radius),
            -(total_height / 2.0 - body_radius),
            270.0,
            180.0,
        ),
        (
            -(w / 2.0 - body_radius),
            -(body_top + body_radius),
            180.0,
            90.0,
        ),
    ];
    for (cx, cy, start, end) in corners {
        outline.extend(generate_circle_points(cx, cy, body_radius, 12, start, end));
    }
    outline
}

/// The group that places a drawn element: unified shapes are centered on
/// the origin, the C4 grid stores the top-left corner.
pub fn positioned(drawn: &DrawnShape, x: f64, y: f64) -> Element {
    Element::Group(Group {
        common: Common {
            transform: vec![TransformOp::Translate(
                x + drawn.width / 2.0,
                y + drawn.height / 2.0,
            )],
            ..Common::default()
        },
        children: vec![Element::Group(drawn.group.clone())],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::diagrams::c4::db::Fields;

    fn shape(kind: &str) -> C4Shape {
        C4Shape {
            alias: "a".to_string(),
            type_c4_shape: kind.to_string(),
            parent_boundary: "global".to_string(),
            wrap: false,
            fields: Fields::default(),
        }
    }

    #[test]
    fn shapes_follow_the_type_then_the_keywords() {
        assert_eq!(resolve_node_shape(&shape("person")), NodeShape::Person);
        assert_eq!(
            resolve_node_shape(&shape("external_system_db")),
            NodeShape::Cylinder
        );
        assert_eq!(
            resolve_node_shape(&shape("container_queue")),
            NodeShape::HorizontalCylinder
        );
        let mut tagged = shape("system");
        tagged.fields.set(
            "tags",
            crate::mermaid::diagrams::c4::db::Field::Str("x, Database".into()),
        );
        assert_eq!(resolve_node_shape(&tagged), NodeShape::Cylinder);
    }

    #[test]
    fn stereotypes_name_the_base_type() {
        let mut c = shape("external_container_db");
        assert_eq!(stereotype_text(&c), "[Container]");
        c.fields.set_text("techn", "Oracle".to_string());
        assert_eq!(stereotype_text(&c), "[Container: Oracle]");
        assert_eq!(stereotype_label("system_queue"), "Software System");
    }
}
