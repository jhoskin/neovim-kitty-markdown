//! Port of `xychartRenderer.ts`: the `main` group with its background rect,
//! the lazily created class groups, and the three element kinds.
//!
//! `configureSvgSize(svg, height, width, true)` gives the SVG
//! `width="100%"` with a `max-width` of the chart width and the viewBox
//! `0 0 width height`, so the natural size is the configured chart size.

use std::collections::HashMap;

use crate::error::HelperError;
use crate::mermaid::scene::{
    Baseline, Color, Common, Element, FontSpec, Group, Paint, Path, Rect, Scene, Style, Text,
    TextAnchor, TextRun, TransformOp,
};
use crate::mermaid::theme::khroma::js_number;
use crate::text::{FontSpec as MeasureFont, TextMetrics};

use super::builder::{
    self, DrawableData, DrawableElem, HorizontalPos, RectElem, TextDimensionCalculator, TextElem,
    VerticalPos,
};
use super::db::{Db, Orientation};

/// `TextDimensionCalculatorWithFont`: `computeDimensionOfText` appends a
/// `tspan` under a group with the font size and reads its
/// `getBoundingClientRect`, which is the advance box of the shaped run: the
/// summed advances wide and the primary face's ascent plus descent tall. An
/// empty string has no fragment and measures nothing.
pub struct FontMeasurer<'a> {
    metrics: &'a TextMetrics<'a>,
    family: String,
}

impl<'a> FontMeasurer<'a> {
    pub fn new(metrics: &'a TextMetrics<'a>, family: &str) -> FontMeasurer<'a> {
        FontMeasurer {
            metrics,
            family: family.to_string(),
        }
    }
}

impl TextDimensionCalculator for FontMeasurer<'_> {
    fn get_max_dimension(&self, texts: &[String], font_size: f64) -> builder::Dimension {
        let spec = MeasureFont::new(self.family.clone(), font_size as f32);
        let mut dim = builder::Dimension::default();
        for text in texts {
            if text.is_empty() {
                continue;
            }
            let shaped = self.metrics.shape(text, &spec);
            dim.width = dim.width.max(f64::from(shaped.width));
            dim.height = dim
                .height
                .max(f64::from(shaped.ascent) + f64::from(shaped.descent));
        }
        dim
    }
}

/// One `g` in the renderer's class tree, filled in creation order.
#[derive(Default)]
struct Node {
    class: String,
    children: Vec<Child>,
}

enum Child {
    Element(Box<Element>),
    Node(usize),
}

/// `getGroup(gList)`: the renderer keys groups by the concatenation of the
/// class names on the way down and appends a missing group to its parent
/// when it is first needed.
struct GroupTree {
    nodes: Vec<Node>,
    by_prefix: HashMap<String, usize>,
}

impl GroupTree {
    fn new() -> GroupTree {
        GroupTree {
            nodes: vec![Node {
                class: "main".to_string(),
                children: Vec::new(),
            }],
            by_prefix: HashMap::new(),
        }
    }

    fn group(&mut self, names: &[String]) -> usize {
        let mut elem = 0;
        let mut prefix = String::new();
        for (i, name) in names.iter().enumerate() {
            let mut parent = 0;
            if i > 0
                && let Some(p) = self.by_prefix.get(&prefix)
            {
                parent = *p;
            }
            prefix.push_str(name);
            elem = match self.by_prefix.get(&prefix) {
                Some(e) => *e,
                None => {
                    let id = self.nodes.len();
                    self.nodes.push(Node {
                        class: name.clone(),
                        children: Vec::new(),
                    });
                    self.nodes[parent].children.push(Child::Node(id));
                    self.by_prefix.insert(prefix.clone(), id);
                    id
                }
            };
        }
        elem
    }

    fn push(&mut self, node: usize, element: Element) {
        self.nodes[node]
            .children
            .push(Child::Element(Box::new(element)));
    }

    fn into_group(mut self) -> Group {
        let root = std::mem::take(&mut self.nodes[0]);
        self.build(root)
    }

    fn build(&mut self, node: Node) -> Group {
        let mut group = Group {
            common: Common {
                class: Some(node.class),
                ..Common::default()
            },
            children: Vec::new(),
        };
        for child in node.children {
            match child {
                Child::Element(e) => group.children.push(*e),
                Child::Node(id) => {
                    let inner = std::mem::take(&mut self.nodes[id]);
                    group.children.push(Element::Group(self.build(inner)));
                }
            }
        }
        group
    }
}

fn rect_element(r: &RectElem) -> Element {
    Element::Rect(Rect {
        common: Common::with_style(Style {
            fill: Some(Paint::Color(r.fill)),
            stroke: Some(Paint::Color(r.stroke_fill)),
            stroke_width: Some(r.stroke_width),
            ..Style::default()
        }),
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
        rx: None,
        ry: None,
    })
}

/// `getDominantBaseLine`.
fn baseline(pos: VerticalPos) -> Baseline {
    match pos {
        VerticalPos::Top => Baseline::TextBeforeEdge,
        VerticalPos::Middle => Baseline::Middle,
    }
}

/// `getTextAnchor`.
fn anchor(pos: HorizontalPos) -> TextAnchor {
    match pos {
        HorizontalPos::Left => TextAnchor::Start,
        HorizontalPos::Right => TextAnchor::End,
        HorizontalPos::Center => TextAnchor::Middle,
    }
}

fn text_element(t: &TextElem) -> Element {
    Element::Text(Text {
        common: Common {
            transform: vec![
                TransformOp::Translate(t.x, t.y),
                TransformOp::Rotate(t.rotation, 0.0, 0.0),
            ],
            style: Style::fill(t.fill),
            ..Common::default()
        },
        x: 0.0,
        y: 0.0,
        anchor: Some(anchor(t.horizontal_pos)),
        baseline: Some(baseline(t.vertical_pos)),
        font: FontSpec {
            size: Some(t.font_size),
            ..FontSpec::default()
        },
        runs: vec![TextRun::new(t.text.clone())],
    })
}

/// A data label as the renderer appends it: plain `x`, `y`, anchor, and
/// baseline attributes, no transform.
fn data_label(
    x: f64,
    y: f64,
    anchor: TextAnchor,
    baseline: Baseline,
    fill: Color,
    size: f64,
    label: &str,
) -> Element {
    Element::Text(Text {
        common: Common::with_style(Style::fill(fill)),
        x,
        y,
        anchor: Some(anchor),
        baseline: Some(baseline),
        font: FontSpec {
            size: Some(size),
            ..FontSpec::default()
        },
        runs: vec![TextRun::new(label.to_string())],
    })
}

/// `showDataLabel`: the value text inside or beside each bar, in one font
/// size that fits every bar of the plot. The renderer takes the values
/// from the first plot's data for every bar plot, which is ported as is.
fn data_labels(db: &Db, rects: &[RectElem], label_data: &[f64]) -> Vec<Element> {
    let outside = db.config.show_data_label_outside_bar;
    let color = db.theme.data_label_color;
    let char_width_factor = 0.7;
    let valid: Vec<(&RectElem, String)> = rects
        .iter()
        .enumerate()
        .filter_map(|(i, r)| label_data.get(i).map(|v| (r, js_number(*v))))
        .filter(|(r, _)| r.width > 0.0 && r.height > 0.0)
        .collect();
    if valid.is_empty() {
        return Vec::new();
    }
    match db.orientation() {
        Orientation::Horizontal => {
            let right_margin = 10.0;
            let fits = |r: &RectElem, label: &str, font_size: f64| {
                font_size * label.chars().count() as f64 * char_width_factor
                    <= r.width - right_margin
            };
            let uniform = valid
                .iter()
                .map(|(r, label)| {
                    let mut font_size = r.height * 0.7;
                    while !fits(r, label, font_size) && font_size > 0.0 {
                        font_size -= 1.0;
                    }
                    font_size
                })
                .fold(f64::INFINITY, f64::min)
                .floor();
            valid
                .iter()
                .map(|(r, label)| {
                    let x = if outside {
                        r.x + r.width + right_margin
                    } else {
                        r.x + r.width - right_margin
                    };
                    let anchor = if outside {
                        TextAnchor::Start
                    } else {
                        TextAnchor::End
                    };
                    data_label(
                        x,
                        r.y + r.height / 2.0,
                        anchor,
                        Baseline::Middle,
                        color,
                        uniform,
                        label,
                    )
                })
                .collect()
        }
        Orientation::Vertical => {
            let y_offset = 10.0;
            let fits = |r: &RectElem, label: &str, font_size: f64| {
                let text_width = font_size * label.chars().count() as f64 * char_width_factor;
                let center_x = r.x + r.width / 2.0;
                let left = center_x - text_width / 2.0;
                let right = center_x + text_width / 2.0;
                let horizontal = left >= r.x && right <= r.x + r.width;
                let vertical = r.y + y_offset + font_size <= r.y + r.height;
                horizontal && vertical
            };
            let uniform = valid
                .iter()
                .map(|(r, label)| {
                    let mut font_size = r.width / (label.chars().count() as f64 * 0.7);
                    while !fits(r, label, font_size) && font_size > 0.0 {
                        font_size -= 1.0;
                    }
                    font_size
                })
                .fold(f64::INFINITY, f64::min)
                .floor();
            valid
                .iter()
                .map(|(r, label)| {
                    let y = if outside {
                        r.y - y_offset
                    } else {
                        r.y + y_offset
                    };
                    let baseline = if outside {
                        Baseline::Auto
                    } else {
                        Baseline::Hanging
                    };
                    data_label(
                        r.x + r.width / 2.0,
                        y,
                        TextAnchor::Middle,
                        baseline,
                        color,
                        uniform,
                        label,
                    )
                })
                .collect()
        }
    }
}

/// `draw`: the scene for a filled db. Text is measured with `font_family`
/// (the CSS chain) and the scene carries `scene_family`, the face name the
/// rasterizer resolves.
pub fn draw(
    db: &Db,
    metrics: &TextMetrics<'_>,
    font_family: &str,
    scene_family: &str,
) -> Result<Scene, HelperError> {
    if db.plots.is_empty() {
        return Err(HelperError::internal(
            "No Plot to render, please provide a plot with some data",
        ));
    }
    let width = *db.config.width;
    let height = *db.config.height;
    let label_data: Vec<f64> = db.plots[0].data.iter().map(|(_, y)| *y).collect();

    let measurer = FontMeasurer::new(metrics, font_family);
    let shapes: Vec<DrawableElem> = builder::build(db, &measurer);

    let mut tree = GroupTree::new();
    tree.push(
        0,
        Element::Rect(Rect {
            common: Common {
                class: Some("background".to_string()),
                style: Style::fill(db.theme.background_color),
                ..Common::default()
            },
            x: 0.0,
            y: 0.0,
            width,
            height,
            rx: None,
            ry: None,
        }),
    );
    for shape in &shapes {
        if shape.is_empty() {
            continue;
        }
        let group = tree.group(&shape.group_texts);
        match &shape.data {
            DrawableData::Rect(rects) => {
                for r in rects {
                    tree.push(group, rect_element(r));
                }
                if db.config.show_data_label {
                    for label in data_labels(db, rects, &label_data) {
                        tree.push(group, label);
                    }
                }
            }
            DrawableData::Text(texts) => {
                for t in texts {
                    tree.push(group, text_element(t));
                }
            }
            DrawableData::Path(paths) => {
                for p in paths {
                    tree.push(
                        group,
                        Element::Path(Path {
                            common: Common::with_style(Style {
                                fill: Some(match p.fill {
                                    Some(c) => Paint::Color(c),
                                    None => Paint::None,
                                }),
                                stroke: Some(Paint::Color(p.stroke_fill)),
                                stroke_width: Some(p.stroke_width),
                                ..Style::default()
                            }),
                            d: p.path.clone(),
                        }),
                    );
                }
            }
        }
    }

    let mut scene = Scene::new(width, height);
    scene.font_family = scene_family.to_string();
    scene.root = tree.into_group();
    Ok(scene)
}
