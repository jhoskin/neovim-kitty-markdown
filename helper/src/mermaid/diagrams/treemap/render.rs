//! Port of `diagrams/treemap/renderer.ts` (`draw`), mermaid 12.2.0.
//!
//! The layout is `d3.treemap()` over `d3.hierarchy(root)` at `nodeWidth *
//! 10` by `nodeHeight * 10`, sorted by value, with a 35 px top padding and
//! 10 px side padding on every node with children, `padding` between
//! siblings, and `round(true)`. Sections get a 25 px header with the name
//! and the value; leaves get a centered label and value whose font sizes
//! shrink until they fit (`getComputedTextLength`, from the text measurer).
//! The root section is drawn with `display: none`, so the scene leaves it
//! out. `setupViewPortForSVG` frames the `getBBox()` of what is drawn with
//! `diagramPadding`; the bounds here are the rects and the text boxes the
//! renderer places, strokes excluded as in a `getBBox`.
//!
//! The renderer clips the labels to their cells with `clipPath`s, which the
//! scene has no element for. The fitting loops keep a label inside its
//! cell except in a treemap of more than 20 leaves, where only the font
//! size floor applies; there an overlong label runs past its cell.

use super::db::{TreemapConfig, TreemapDb, TreemapNode};
use super::format::Format;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::layout::d3::hierarchy::{Hierarchy, Node, Treemap, js_round};
use crate::mermaid::scene::{
    Baseline, Common, Element, FontSpec, FontStyle, FontWeight, Group, Paint, Rect, Scene, Style,
    Text, TextAnchor, TextRun, TransformOp, ViewBox,
};
use crate::mermaid::shapes::util::apply_declarations;
use crate::mermaid::styles::treemap::{ClassStyles, OrdinalScale, TreemapStyles, theme_paint};
use crate::mermaid::theme::ThemeVariables;
use crate::text::fonts::Weight;
use crate::text::measure::{FontSpec as MeasureSpec, FontStyle as MeasureStyle, Shaped};

/// `DEFAULT_INNER_PADDING`, `SECTION_INNER_PADDING`, `SECTION_HEADER_HEIGHT`.
const DEFAULT_INNER_PADDING: f64 = 10.0;
const SECTION_INNER_PADDING: f64 = 10.0;
const SECTION_HEADER_HEIGHT: f64 = 25.0;
/// The section label's `x` and font size, the value's font size.
const SECTION_LABEL_X: f64 = 6.0;
const SECTION_LABEL_SIZE: f64 = 12.0;
const SECTION_VALUE_SIZE: f64 = 10.0;
/// `valueScaleFactor`.
const VALUE_SCALE_FACTOR: f64 = 0.6;
/// Chrome's `hanging` baseline: 80 % of the ascent above the alphabetic one.
const HANGING_AS_PART_OF_ASCENT: f64 = 0.8;

/// An axis-aligned box in user units, what `getBBox()` reports.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn rect(x: f64, y: f64, width: f64, height: f64) -> Bounds {
        Bounds {
            min_x: x,
            min_y: y,
            max_x: x + width,
            max_y: y + height,
        }
    }

    fn union(self, other: Bounds) -> Bounds {
        Bounds {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }
}

fn extend(bounds: &mut Option<Bounds>, more: Bounds) {
    *bounds = Some(bounds.map_or(more, |b| b.union(more)));
}

/// The sizes `isComplexTreemap` switches between.
struct LeafSizes {
    base_label: f64,
    base_value: f64,
    min_label: f64,
    min_value: f64,
    label_padding: f64,
    min_display: f64,
    spacing: f64,
}

impl LeafSizes {
    fn new(complex: bool) -> LeafSizes {
        if complex {
            LeafSizes {
                base_label: 16.0,
                base_value: 14.0,
                min_label: 4.0,
                min_value: 4.0,
                label_padding: 2.0,
                min_display: 8.0,
                spacing: 1.0,
            }
        } else {
            LeafSizes {
                base_label: 38.0,
                base_value: 28.0,
                min_label: 8.0,
                min_value: 6.0,
                label_padding: 4.0,
                min_display: 10.0,
                spacing: 2.0,
            }
        }
    }

    /// `Math.max(minValueFontSize, Math.min(baseValueFontSize,
    /// Math.round(labelSize * valueScaleFactor)))`.
    fn value_size(&self, label_size: f64) -> f64 {
        self.min_value.max(
            self.base_value
                .min(js_round(label_size * VALUE_SCALE_FACTOR)),
        )
    }
}

/// The value formatter `draw` builds from `config.valueFormat`, with its
/// `$` special cases; a specifier d3 rejects falls back to `format(',')`.
pub struct ValueFormat {
    prefix: &'static str,
    format: Format,
}

impl ValueFormat {
    pub fn new(spec: &str) -> ValueFormat {
        let build = |prefix: &'static str, s: &str| {
            Format::new(s)
                .ok()
                .map(|format| ValueFormat { prefix, format })
        };
        let made = if spec == "$0,0" {
            build("$", ",")
        } else if let Some(rest) = spec.strip_prefix('$') {
            if spec.contains(',') {
                // `/\.\d+/.exec(formatStr)`: the first precision anywhere.
                let precision = first_precision(spec);
                build("$", &format!(",{precision}"))
            } else {
                build("$", rest)
            }
        } else {
            build("", spec)
        };
        made.unwrap_or_else(|| ValueFormat {
            prefix: "",
            format: Format::comma(),
        })
    }

    pub fn format(&self, value: f64) -> String {
        format!("{}{}", self.prefix, self.format.format(value))
    }
}

/// The first `\.\d+` in `s`, or the empty string.
fn first_precision(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '.' {
            let digits: String = chars[i + 1..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if !digits.is_empty() {
                return format!(".{digits}");
            }
        }
    }
    String::new()
}

/// SVG text collapses white space runs and drops the leading and trailing
/// ones before it lays out or measures a line.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct Painter<'a> {
    ctx: &'a RenderCtx<'a>,
    family: String,
    bounds: Option<Bounds>,
}

impl Painter<'_> {
    fn spec(&self, family: Option<&str>, size: f64, weight: u16, italic: bool) -> MeasureSpec {
        let spec = MeasureSpec::new(family.unwrap_or(&self.family), size as f32).weight(weight);
        if italic {
            spec.style(MeasureStyle::Italic)
        } else {
            spec
        }
    }

    fn shape(&self, text: &str, spec: &MeasureSpec) -> Shaped {
        self.ctx.metrics.shape(&collapse(text), spec)
    }

    /// `getComputedTextLength()`.
    fn length(&self, text: &str, spec: &MeasureSpec) -> f64 {
        f64::from(self.shape(text, spec).width)
    }

    /// The distance from the `y` of a text to its alphabetic baseline for a
    /// `dominant-baseline`: half the x-height down for `middle`, 80 % of
    /// the ascent down for `hanging`.
    fn baseline_shift(&self, spec: &MeasureSpec, baseline: Baseline) -> f64 {
        let fonts = self.ctx.fonts;
        let Some(face) = fonts.resolve(&spec.family, Weight(spec.weight), spec.style.into()) else {
            return 0.0;
        };
        match baseline {
            Baseline::Middle => fonts
                .with_face(face, |f| {
                    let upem = f64::from(f.units_per_em());
                    let units = f.x_height().filter(|v| *v > 0).map(f64::from).or_else(|| {
                        f.glyph_index('x')
                            .and_then(|g| f.glyph_bounding_box(g))
                            .map(|r| f64::from(r.y_max))
                    });
                    units.map_or(0.0, |u| u / upem * f64::from(spec.size)) / 2.0
                })
                .unwrap_or(0.0),
            Baseline::Hanging => {
                let (ascent, _) = self.ctx.metrics.face_float_metrics(face, spec.size);
                f64::from(ascent) * HANGING_AS_PART_OF_ASCENT
            }
            _ => 0.0,
        }
    }

    /// Adds the `getBBox()` of one line of text to the bounds: the advance
    /// box joined with the ink, placed by the anchor and the baseline.
    #[allow(clippy::too_many_arguments)]
    fn add_text_bounds(
        &mut self,
        text: &str,
        spec: &MeasureSpec,
        x: f64,
        y: f64,
        anchor: TextAnchor,
        baseline: Baseline,
        offset: (f64, f64),
    ) {
        if collapse(text).is_empty() {
            return;
        }
        let shaped = self.shape(text, spec);
        let baseline_y = y + self.baseline_shift(spec, baseline) + offset.1;
        let advance = f64::from(shaped.width);
        let start = offset.0
            + match anchor {
                TextAnchor::Start => x,
                TextAnchor::Middle => x - advance / 2.0,
                TextAnchor::End => x - advance,
            };
        let (ascent, descent) = (f64::from(shaped.ascent), f64::from(shaped.descent));
        let b = match shaped.ink {
            Some(ink) => Bounds {
                min_x: start + f64::from(ink.min_x).min(0.0),
                min_y: baseline_y - f64::from(ink.top).max(ascent),
                max_x: start + f64::from(ink.max_x).max(advance),
                max_y: baseline_y + f64::from(ink.bottom).max(descent),
            },
            None => Bounds {
                min_x: start,
                min_y: baseline_y - ascent,
                max_x: start + advance,
                max_y: baseline_y + descent,
            },
        };
        extend(&mut self.bounds, b);
    }
}

fn weight_number(weight: Option<FontWeight>, default: u16) -> u16 {
    match weight {
        Some(FontWeight::Bold) => 700,
        Some(FontWeight::Normal) => 400,
        Some(FontWeight::Number(n)) => n,
        None => default,
    }
}

/// A `<text>` with one run.
#[allow(clippy::too_many_arguments)]
fn text_element(
    class: &str,
    x: f64,
    y: f64,
    anchor: TextAnchor,
    baseline: Baseline,
    font: FontSpec,
    fill: Option<Paint>,
    content: &str,
) -> Element {
    Element::Text(Text {
        common: Common {
            class: Some(class.to_string()),
            style: Style {
                fill,
                ..Style::default()
            },
            ..Common::default()
        },
        x,
        y,
        anchor: Some(anchor),
        baseline: Some(baseline),
        font,
        runs: vec![TextRun::new(content)],
    })
}

/// The label font of a text with the class label styles over the
/// renderer's own.
fn class_font(class: &ClassStyles, size: f64, weight: FontWeight, style: FontStyle) -> FontSpec {
    FontSpec {
        family: class.label_font_family(),
        size: Some(size),
        weight: Some(class.label_font_weight().unwrap_or(weight)),
        style: Some(class.label_font_style().unwrap_or(style)),
    }
}

fn is_italic(style: Option<FontStyle>) -> bool {
    matches!(style, Some(FontStyle::Italic) | Some(FontStyle::Oblique))
}

/// `draw`: the scene for the db.
pub fn draw(
    db: &TreemapDb,
    cfg: &TreemapConfig,
    tv: &ThemeVariables,
    ctx: &RenderCtx<'_>,
    fallback_font: &str,
) -> Scene {
    let styles = TreemapStyles::resolve(tv, fallback_font);
    let inner_padding = if cfg.padding.is_nan() {
        DEFAULT_INNER_PADDING
    } else {
        cfg.padding
    };
    let title = db.title.as_str();
    let title_height = if title.is_empty() { 0.0 } else { 30.0 };
    let width = if cfg.node_width != 0.0 {
        cfg.node_width * SECTION_INNER_PADDING
    } else {
        960.0
    };
    let height = if cfg.node_height != 0.0 {
        cfg.node_height * SECTION_INNER_PADDING
    } else {
        500.0
    };
    let svg_width = width;
    let value_format = ValueFormat::new(&cfg.value_format);

    let mut color_scale = OrdinalScale::sections(tv);
    let mut color_scale_peer = OrdinalScale::peers(tv);
    let mut color_scale_label = OrdinalScale::labels(tv);

    let mut painter = Painter {
        ctx,
        family: styles.font_family.clone(),
        bounds: None,
    };
    let mut scene_children = Vec::new();

    if !title.is_empty() {
        let size = styles.title_font_size;
        let spec = painter.spec(None, size, 400, false);
        painter.add_text_bounds(
            title,
            &spec,
            svg_width / 2.0,
            title_height / 2.0,
            TextAnchor::Middle,
            Baseline::Middle,
            (0.0, 0.0),
        );
        scene_children.push(text_element(
            "treemapTitle",
            svg_width / 2.0,
            title_height / 2.0,
            TextAnchor::Middle,
            Baseline::Middle,
            FontSpec {
                size: Some(size),
                ..FontSpec::default()
            },
            styles.title_color.map(Paint::Color),
            title,
        ));
    }

    // `hierarchy(root).sum(d => d.value ?? 0).sort((a, b) => b.value - a.value)`.
    let root = db.root();
    let mut h: Hierarchy<&TreemapNode> =
        Hierarchy::new(&root, |d| d.children.iter().flatten().collect());
    h.sum(|d| d.value.unwrap_or(0.0));
    h.sort(|a, b| b.value - a.value);
    let branch = |d: &Node<&TreemapNode>| {
        if d.children.is_some() {
            SECTION_INNER_PADDING
        } else {
            0.0
        }
    };
    Treemap::new()
        .size([width, height])
        .padding_top(|d| {
            if d.children.is_some() {
                SECTION_HEADER_HEIGHT + SECTION_INNER_PADDING
            } else {
                0.0
            }
        })
        .padding_inner(move |_| inner_padding)
        .padding_left(branch)
        .padding_right(branch)
        .padding_bottom(branch)
        .round(true)
        .layout(&mut h);

    let mut container = Group {
        common: Common {
            class: Some("treemapContainer".to_string()),
            transform: vec![TransformOp::Translate(0.0, title_height)],
            ..Common::default()
        },
        children: Vec::new(),
    };
    let offset = (0.0, title_height);

    // Sections: the nodes with children, breadth first. Each color scale
    // sees the names in the order the d3 attribute calls ask for them.
    let branches: Vec<usize> = h
        .descendants()
        .into_iter()
        .filter(|&i| h.nodes[i].children.is_some())
        .collect();
    let fills: Vec<String> = branches
        .iter()
        .map(|&i| color_scale.get(&h.nodes[i].data.name))
        .collect();
    let strokes: Vec<String> = branches
        .iter()
        .map(|&i| color_scale_peer.get(&h.nodes[i].data.name))
        .collect();
    let section_labels: Vec<String> = branches
        .iter()
        .map(|&i| {
            let n = &h.nodes[i];
            if n.depth == 0 {
                String::new()
            } else {
                color_scale_label.get(&n.data.name)
            }
        })
        .collect();
    for (k, &i) in branches.iter().enumerate() {
        let n = &h.nodes[i];
        // The root section is `display: none`.
        if n.depth == 0 {
            continue;
        }
        let w = n.x1 - n.x0;
        let class = ClassStyles::from_compiled(n.data.css_compiled_styles.as_deref());
        let mut group = Group {
            common: Common {
                class: Some("treemapSection".to_string()),
                transform: vec![TransformOp::Translate(n.x0, n.y0)],
                ..Common::default()
            },
            children: Vec::new(),
        };
        let at = (offset.0 + n.x0, offset.1 + n.y0);
        // The header rect: no fill, no stroke, but it counts in `getBBox`.
        extend(
            &mut painter.bounds,
            Bounds::rect(at.0, at.1, w, SECTION_HEADER_HEIGHT),
        );
        let mut rect_style = Style {
            fill: theme_paint(&fills[k]),
            fill_opacity: Some(0.6),
            stroke: theme_paint(&strokes[k]),
            stroke_width: Some(2.0),
            stroke_opacity: Some(0.4),
            ..Style::default()
        };
        apply_declarations(&mut rect_style, &class.node);
        apply_declarations(&mut rect_style, &class.border);
        group.children.push(Element::Rect(Rect {
            common: Common {
                class: Some(format!("treemapSection section{k}")),
                style: rect_style,
                ..Common::default()
            },
            x: 0.0,
            y: 0.0,
            width: w,
            height: n.y1 - n.y0,
            rx: None,
            ry: None,
        }));
        extend(
            &mut painter.bounds,
            Bounds::rect(at.0, at.1, w, n.y1 - n.y0),
        );

        let label_fill = class
            .label_fill()
            .or_else(|| theme_paint(&section_labels[k]));
        let label_font = class_font(
            &class,
            class.label_font_size().unwrap_or(SECTION_LABEL_SIZE),
            FontWeight::Bold,
            FontStyle::Normal,
        );
        let label_spec = painter.spec(
            label_font.family.as_deref(),
            label_font.size.unwrap_or(SECTION_LABEL_SIZE),
            weight_number(label_font.weight, 700),
            is_italic(label_font.style),
        );
        let label = section_label(&painter, &n.data.name, w, n.value, cfg, &label_spec);
        painter.add_text_bounds(
            &label,
            &label_spec,
            SECTION_LABEL_X,
            SECTION_HEADER_HEIGHT / 2.0,
            TextAnchor::Start,
            Baseline::Middle,
            at,
        );
        group.children.push(text_element(
            "treemapSectionLabel",
            SECTION_LABEL_X,
            SECTION_HEADER_HEIGHT / 2.0,
            TextAnchor::Start,
            Baseline::Middle,
            label_font,
            label_fill.clone(),
            &label,
        ));

        if cfg.show_values {
            let value = if n.value != 0.0 && !n.value.is_nan() {
                value_format.format(n.value)
            } else {
                String::new()
            };
            let value_font = class_font(
                &class,
                class.label_font_size().unwrap_or(SECTION_VALUE_SIZE),
                FontWeight::Normal,
                FontStyle::Italic,
            );
            let value_spec = painter.spec(
                value_font.family.as_deref(),
                value_font.size.unwrap_or(SECTION_VALUE_SIZE),
                weight_number(value_font.weight, 400),
                is_italic(value_font.style),
            );
            painter.add_text_bounds(
                &value,
                &value_spec,
                w - 10.0,
                SECTION_HEADER_HEIGHT / 2.0,
                TextAnchor::End,
                Baseline::Middle,
                at,
            );
            group.children.push(text_element(
                "treemapSectionValue",
                w - 10.0,
                SECTION_HEADER_HEIGHT / 2.0,
                TextAnchor::End,
                Baseline::Middle,
                value_font,
                label_fill,
                &value,
            ));
        }
        container.children.push(Element::Group(group));
    }

    // Leaves, in pre-order.
    let leaves = h.leaves();
    let sizes = LeafSizes::new(leaves.len() > 20);
    let leaf_fills: Vec<String> = leaves
        .iter()
        .map(|&i| {
            let n = &h.nodes[i];
            let name = match n.parent {
                Some(p) => &h.nodes[p].data.name,
                None => &n.data.name,
            };
            color_scale.get(name)
        })
        .collect();
    let leaf_labels: Vec<String> = leaves
        .iter()
        .map(|&i| color_scale_label.get(&h.nodes[i].data.name))
        .collect();
    for (k, &i) in leaves.iter().enumerate() {
        let n = &h.nodes[i];
        let (w, ht) = (n.x1 - n.x0, n.y1 - n.y0);
        let at = (offset.0 + n.x0, offset.1 + n.y0);
        let class = ClassStyles::from_compiled(n.data.css_compiled_styles.as_deref());
        let selector = n
            .data
            .class_selector
            .as_deref()
            .map(|c| format!(" {c}"))
            .unwrap_or_default();
        let mut group = Group {
            common: Common {
                class: Some(format!("treemapNode treemapLeafGroup leaf{k}{selector}x")),
                transform: vec![TransformOp::Translate(n.x0, n.y0)],
                ..Common::default()
            },
            children: Vec::new(),
        };
        let mut rect_style = Style {
            fill: theme_paint(&leaf_fills[k]),
            fill_opacity: Some(0.3),
            stroke: theme_paint(&leaf_fills[k]),
            stroke_width: Some(3.0),
            ..Style::default()
        };
        apply_declarations(&mut rect_style, &class.node);
        group.children.push(Element::Rect(Rect {
            common: Common {
                class: Some("treemapLeaf".to_string()),
                style: rect_style,
                ..Common::default()
            },
            x: 0.0,
            y: 0.0,
            width: w,
            height: ht,
            rx: None,
            ry: None,
        }));
        extend(&mut painter.bounds, Bounds::rect(at.0, at.1, w, ht));

        let fill = class.label_fill().or_else(|| theme_paint(&leaf_labels[k]));
        let family = class.label_font_family();
        let weight = weight_number(class.label_font_weight(), 400);
        let italic = is_italic(class.label_font_style());
        let base_family = painter.family.clone();
        let spec_at = |size: f64| {
            let spec = MeasureSpec::new(family.as_deref().unwrap_or(&base_family), size as f32)
                .weight(weight);
            if italic {
                spec.style(MeasureStyle::Italic)
            } else {
                spec
            }
        };
        let name = n.data.name.as_str();
        let label_size = fit_label(&painter, name, w, ht, &sizes, &class, &spec_at);
        let Some(label_size) = label_size else {
            container.children.push(Element::Group(group));
            continue;
        };
        let label_spec = spec_at(label_size);
        painter.add_text_bounds(
            name,
            &label_spec,
            w / 2.0,
            ht / 2.0,
            TextAnchor::Middle,
            Baseline::Middle,
            at,
        );
        group.children.push(text_element(
            "treemapLabel",
            w / 2.0,
            ht / 2.0,
            TextAnchor::Middle,
            Baseline::Middle,
            class_font(&class, label_size, FontWeight::Normal, FontStyle::Normal),
            fill.clone(),
            name,
        ));

        if cfg.show_values {
            let value_size = sizes.value_size(label_size);
            let value_spec = spec_at(value_size);
            let value = if n.value != 0.0 && !n.value.is_nan() {
                value_format.format(n.value)
            } else {
                String::new()
            };
            let top = ht / 2.0 + label_size / 2.0 + sizes.spacing;
            let fits = painter.length(&value, &value_spec) <= w - 2.0 * sizes.label_padding
                && top + value_size <= ht - 4.0
                && value_size >= sizes.min_value;
            if fits {
                painter.add_text_bounds(
                    &value,
                    &value_spec,
                    w / 2.0,
                    top,
                    TextAnchor::Middle,
                    Baseline::Hanging,
                    at,
                );
                group.children.push(text_element(
                    "treemapValue",
                    w / 2.0,
                    top,
                    TextAnchor::Middle,
                    Baseline::Hanging,
                    class_font(&class, value_size, FontWeight::Normal, FontStyle::Normal),
                    fill,
                    &value,
                ));
            }
        }
        container.children.push(Element::Group(group));
    }
    scene_children.push(Element::Group(container));

    // `setupViewPortForSVG(svg, diagramPadding, ...)`.
    let padding = if cfg.diagram_padding.is_nan() {
        8.0
    } else {
        cfg.diagram_padding
    };
    let b = painter.bounds.unwrap_or(Bounds::rect(0.0, 0.0, 0.0, 0.0));
    let view_w = (b.max_x - b.min_x) + 2.0 * padding;
    let view_h = (b.max_y - b.min_y) + 2.0 * padding;
    let mut scene = Scene::new(view_w, view_h);
    scene.view_box = ViewBox::new(b.min_x - padding, b.min_y - padding, view_w, view_h);
    scene.font_family = styles.font_family.clone();
    // `#id { font-size: 16px; fill: textColor }` from the global stylesheet.
    scene.root.common.style = Style {
        fill: styles.text_color.map(Paint::Color),
        font: FontSpec {
            size: Some(styles.root_font_size),
            ..FontSpec::default()
        },
        ..Style::default()
    };
    scene.root.children = scene_children;
    scene
}

/// The section label after the truncation loop: the name, or the longest
/// prefix with `...` that fits, or `...`, or nothing.
fn section_label(
    painter: &Painter<'_>,
    name: &str,
    header_width: f64,
    value: f64,
    cfg: &TreemapConfig,
    spec: &MeasureSpec,
) -> String {
    let space = if cfg.show_values && value != 0.0 && !value.is_nan() {
        // The value ends 10 px from the right, is estimated at 30 px, and
        // keeps a 10 px gap to the label.
        header_width - 10.0 - 30.0 - 10.0 - SECTION_LABEL_X
    } else {
        header_width - SECTION_LABEL_X - 6.0
    };
    let available = space.max(15.0);
    if painter.length(name, spec) <= available {
        return name.to_string();
    }
    let chars: Vec<char> = name.chars().collect();
    let mut len = chars.len();
    while len > 0 {
        len -= 1;
        if len == 0 {
            return if painter.length("...", spec) > available {
                String::new()
            } else {
                "...".to_string()
            };
        }
        let candidate: String = chars[..len].iter().collect::<String>() + "...";
        if painter.length(&candidate, spec) <= available {
            return candidate;
        }
    }
    name.to_string()
}

/// The three steps of `leafLabels.each`: shrink to the width, shrink label
/// and value together to the height, then hide what still does not fit.
/// Returns the final font size, or `None` for a hidden label.
fn fit_label(
    painter: &Painter<'_>,
    name: &str,
    width: f64,
    height: f64,
    sizes: &LeafSizes,
    class: &ClassStyles,
    spec_at: &dyn Fn(f64) -> MeasureSpec,
) -> Option<f64> {
    let available_width = width - 2.0 * sizes.label_padding;
    let available_height = height - 2.0 * sizes.label_padding;
    if available_width < sizes.min_display || available_height < sizes.min_display {
        return None;
    }
    // `parseInt(self.style('font-size'), 10)`: a class `font-size` comes
    // after the renderer's in the style attribute and wins.
    let mut size = class
        .label_font_size()
        .map(f64::trunc)
        .unwrap_or(sizes.base_label);
    let length = |size: f64| painter.length(name, &spec_at(size));
    while length(size) > available_width && size > sizes.min_label {
        size -= 1.0;
    }
    let mut combined = size + sizes.spacing + sizes.value_size(size);
    while combined > available_height && size > sizes.min_label {
        size -= 1.0;
        combined = size + sizes.spacing + sizes.value_size(size);
    }
    let hidden = if sizes.min_label == 4.0 {
        size < sizes.min_label || available_height < sizes.min_label
    } else {
        length(size) > available_width || size < sizes.min_label || available_height < size
    };
    (!hidden).then_some(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_formats_follow_the_renderer_cases() {
        assert_eq!(ValueFormat::new(",").format(1000.0), "1,000");
        assert_eq!(ValueFormat::new("$0,0").format(700000.0), "$700,000");
        // `$,.2f` keeps only the `.2`: `format(',.2')` is two significant
        // digits.
        assert_eq!(ValueFormat::new("$,.2f").format(1234.5), "$1.2e+3");
        assert_eq!(ValueFormat::new("$.1%").format(0.35), "$35.0%");
        assert_eq!(ValueFormat::new(".1f").format(2.25), "2.3");
        assert_eq!(ValueFormat::new("$,.2f").format(700000.0), "$7e+5");
        // An invalid specifier falls back to `,`.
        assert_eq!(ValueFormat::new("bogus").format(1234.0), "1,234");
        assert_eq!(ValueFormat::new("$bogus").format(1234.0), "1,234");
    }

    #[test]
    fn first_precision_finds_the_first_decimal_run() {
        assert_eq!(first_precision("$,.2f"), ".2");
        assert_eq!(first_precision("$,"), "");
    }
}
