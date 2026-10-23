//! Port of the eventmodeling `renderer.ts` `draw` (mermaid 12.2.0): the
//! swimlanes, the boxes, the relations with their arrowhead marker, and
//! `setupGraphViewbox` with the diagram padding.
//!
//! The oracle writes each box text as HTML in a `foreignObject`: a
//! `display: table` div filling the object and a `table-cell` span,
//! centred both ways, that holds `<b>name</b><br/><br/><code>data</code>`.
//! The scene has no HTML, so this module lays that markup out the way
//! Blink does and writes SVG text: the bold name lines centred, an empty
//! line, then the `display: block` code element, left aligned, in the
//! generic `monospace` face and at most `textMaxWidth` wide. The data has
//! every space turned into `&nbsp;`, so the code wraps only where a source
//! newline (a collapsible space) gives the line breaker a chance, or where
//! `wrapLabel` put a `<br/>`.

use serde_json::Value;

use super::db::{BOX_PADDING, EmBox, Relation, SWIMLANE_PADDING, State, Swimlane, TEXT_MAX_WIDTH};
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::scene::{
    Baseline, ClipPath, Color, Common, Element, FontSpec, FontWeight, Group, Marker, MarkerUnits,
    Orient, Paint, Path, PathData, Point, Polygon, Rect, Scene, Style, Text, TextAnchor, TextRun,
    ViewBox,
};
use crate::mermaid::styles::eventmodeling::EventModelingStyles;
use crate::text::fonts::{Style as FaceStyle, Weight};
use crate::text::measure::FontSpec as MeasureSpec;

/// mermaid-cli's SVG id; the marker id is `em-arrowhead-${id}`.
const SVG_ID: &str = "my-svg";
/// The generic family of `<code>` in Chrome's UA stylesheet.
const MONOSPACE: &str = "monospace";
/// The lane label offset: `.attr('x', 30)` and `swimlane.y + 30`.
const LABEL_OFFSET: f64 = 30.0;
/// The `foreignObject` top inset: `y + 10` in `renderD3Box`.
const TEXT_TOP_INSET: f64 = 10.0;

/// `eventmodeling.padding` and the rest of the config the renderer reads.
#[derive(Debug, Clone, PartialEq)]
pub struct EventModelingConfig {
    pub padding: f64,
}

impl EventModelingConfig {
    /// `config?.padding ?? 30` from `getConfig().eventmodeling`.
    pub fn from_raw(raw: &Value) -> EventModelingConfig {
        EventModelingConfig {
            padding: raw["eventmodeling"]
                .get("padding")
                .and_then(Value::as_f64)
                .unwrap_or(30.0),
        }
    }
}

/// The union of the painted boxes, the way `getBBox` sums the children of
/// the root.
#[derive(Debug, Clone, Copy, Default)]
struct Bounds {
    rect: Option<(f64, f64, f64, f64)>,
}

impl Bounds {
    fn add(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let (x0, y0, x1, y1) = (x, y, x + width, y + height);
        self.rect = Some(match self.rect {
            None => (x0, y0, x1, y1),
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
        });
    }
}

/// Line metrics Blink uses for `line-height: normal`: the rounded ascent,
/// the rounded descent, and the rounded line gap of the primary face.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LineMetrics {
    ascent: f64,
    descent: f64,
    line_gap: f64,
}

impl LineMetrics {
    fn of(ctx: &RenderCtx<'_>, spec: &MeasureSpec) -> LineMetrics {
        let fonts = ctx.metrics.fonts();
        let face = fonts.resolve(&spec.family, Weight(spec.weight), FaceStyle::Normal);
        let Some(face) = face else {
            return LineMetrics {
                ascent: 0.0,
                descent: 0.0,
                line_gap: 0.0,
            };
        };
        let (ascent, descent) = ctx.metrics.face_ascent_descent(face, spec.size);
        let line_gap = fonts
            .with_face(face, |f| {
                f32::from(f.line_gap()) / f.units_per_em().max(1) as f32 * spec.size
            })
            .unwrap_or(0.0);
        LineMetrics {
            ascent: f64::from(ascent),
            descent: f64::from(descent),
            line_gap: f64::from(line_gap).round(),
        }
    }

    /// A line box holding this font and `other` on one baseline, with
    /// half the line gap of each as leading above and below.
    fn with(self, other: LineMetrics) -> LineMetrics {
        let above = |m: LineMetrics| m.ascent + (m.line_gap / 2.0).floor();
        let below = |m: LineMetrics| m.descent + (m.line_gap / 2.0).ceil();
        LineMetrics {
            ascent: above(self).max(above(other)),
            descent: below(self).max(below(other)),
            line_gap: 0.0,
        }
    }

    fn top_to_baseline(self) -> f64 {
        self.ascent + (self.line_gap / 2.0).floor()
    }

    fn height(self) -> f64 {
        self.ascent + self.descent + self.line_gap
    }
}

/// The fonts of a box text.
struct BoxFonts {
    name: MeasureSpec,
    code: MeasureSpec,
    /// The line box of a name line: the table cell's strut and the bold
    /// run.
    name_line: LineMetrics,
    /// The line of the second `<br>`: the strut alone.
    strut_line: LineMetrics,
    code_line: LineMetrics,
    code_space: f64,
    /// The face Chrome picks for `monospace`, by name: resvg maps the
    /// generic family to its own default, not Chrome's.
    code_family: String,
}

impl BoxFonts {
    fn new(ctx: &RenderCtx<'_>, styles: &EventModelingStyles) -> BoxFonts {
        let size = styles.font_size as f32;
        let regular = MeasureSpec::new(styles.font_family.clone(), size);
        let name = regular.clone().weight(700);
        let code = MeasureSpec::new(MONOSPACE, size);
        let strut = LineMetrics::of(ctx, &regular);
        let bold = LineMetrics::of(ctx, &name);
        let code_line = LineMetrics::of(ctx, &code);
        let code_space = f64::from(ctx.metrics.shape(" ", &code).width);
        let fonts = ctx.metrics.fonts();
        let code_family = fonts
            .resolve(MONOSPACE, Weight(code.weight), FaceStyle::Normal)
            .and_then(|id| fonts.db().face(id))
            .and_then(|face| face.families.first().map(|(name, _)| name.clone()))
            .unwrap_or_else(|| MONOSPACE.to_string());
        BoxFonts {
            code_family,
            name,
            code,
            name_line: strut.with(bold),
            strut_line: strut,
            code_line,
            code_space,
        }
    }
}

/// Collapsible white space inside the code element; `&nbsp;` is not.
fn is_collapsible(c: char) -> bool {
    matches!(c, ' ' | '\n' | '\t' | '\r')
}

/// The line boxes of the code element: each `<br/>` ends a line, even an
/// empty one; the text after the last `<br/>` opens a line only when it
/// has content. Within a segment the words fill each line greedily up to
/// `available`; a word that does not fit alone overflows.
fn code_lines(
    segments: &[String],
    available: f64,
    fonts: &BoxFonts,
    ctx: &RenderCtx<'_>,
) -> Vec<String> {
    let width = |s: &str| f64::from(ctx.metrics.shape(s, &fonts.code).width);
    let mut lines = Vec::new();
    for (i, segment) in segments.iter().enumerate() {
        let last = i + 1 == segments.len();
        let words: Vec<&str> = segment
            .split(is_collapsible)
            .filter(|w| !w.is_empty())
            .collect();
        if words.is_empty() {
            if !last {
                lines.push(String::new());
            }
            continue;
        }
        let mut line = String::new();
        let mut line_width = 0.0;
        for word in words {
            let word_width = width(word);
            if line.is_empty() {
                line.push_str(word);
                line_width = word_width;
            } else if line_width + fonts.code_space + word_width <= available + 1e-6 {
                line.push(' ');
                line.push_str(word);
                line_width += fonts.code_space + word_width;
            } else {
                lines.push(std::mem::take(&mut line));
                line.push_str(word);
                line_width = word_width;
            }
        }
        lines.push(line);
    }
    lines
}

/// One line of box text, clipped to the `foreignObject`.
fn plain_text(
    x: f64,
    y: f64,
    anchor: TextAnchor,
    font: FontSpec,
    fill: Color,
    clip: &ClipPath,
    s: &str,
) -> Element {
    Element::Text(Text {
        common: Common::with_style(Style {
            clip_path: Some(clip.clone()),
            ..Style::fill(fill)
        }),
        x,
        y,
        anchor: Some(anchor),
        baseline: Some(Baseline::Alphabetic),
        font,
        runs: vec![TextRun::new(s)],
    })
}

/// The `foreignObject` text of one box as SVG text. `index` is the box's
/// place in the diagram, for a clip id no other box shares.
fn box_text(
    b: &EmBox,
    index: usize,
    fo: (f64, f64, f64, f64),
    fonts: &BoxFonts,
    styles: &EventModelingStyles,
    ctx: &RenderCtx<'_>,
) -> Vec<Element> {
    let (fx, fy, fw, fh) = fo;
    let name_width = |s: &str| f64::from(ctx.metrics.shape(s, &fonts.name).width);
    let widest_name = b
        .text
        .name_lines
        .iter()
        .map(|l| name_width(l))
        .fold(0.0, f64::max);
    // A table grows to its min-content width, so an unbreakable name
    // wider than the object widens the cell.
    let cell_width = fw.max(widest_name);
    let code = b
        .text
        .code
        .as_ref()
        .map(|segments| code_lines(segments, cell_width.min(TEXT_MAX_WIDTH), fonts, ctx));

    let name_height = b.text.name_lines.len() as f64 * fonts.name_line.height();
    let code_height = code.as_ref().map_or(0.0, |lines| {
        fonts.strut_line.height() + lines.len() as f64 * fonts.code_line.height()
    });
    let content_height = name_height + code_height;
    // `vertical-align: middle` in a cell as tall as the object.
    let top = fy + ((fh - content_height) / 2.0).max(0.0);

    let name_font = FontSpec {
        weight: Some(FontWeight::Bold),
        ..FontSpec::default()
    };
    let code_font = FontSpec {
        family: Some(fonts.code_family.clone()),
        ..FontSpec::default()
    };
    // The `foreignObject` clips its content to its own box (`overflow:
    // hidden` in the UA stylesheet): a code line that `&nbsp;` keeps from
    // wrapping, or a name that widens the cell, is cut at the edge, through
    // the middle of a glyph if need be.
    let clip = ClipPath::rect(format!("em-clip-{SVG_ID}-{index}"), fx, fy, fw, fh);
    let color = styles.html_text_color;
    let mut out = Vec::new();
    let mut y = top;
    for line in &b.text.name_lines {
        let baseline = y + fonts.name_line.top_to_baseline();
        out.push(plain_text(
            fx + cell_width / 2.0,
            baseline,
            TextAnchor::Middle,
            name_font.clone(),
            color,
            &clip,
            line,
        ));
        y += fonts.name_line.height();
    }
    if let Some(lines) = code {
        y += fonts.strut_line.height();
        for line in lines {
            let baseline = y + fonts.code_line.top_to_baseline();
            if !line.is_empty() {
                out.push(plain_text(
                    fx,
                    baseline,
                    TextAnchor::Start,
                    code_font.clone(),
                    color,
                    &clip,
                    &line,
                ));
            }
            y += fonts.code_line.height();
        }
    }
    out
}

fn paint(color: Option<Color>) -> Paint {
    color.map_or(Paint::None, Paint::Color)
}

/// `renderD3Swimlane`.
fn swimlane(
    lane: &Swimlane,
    max_r: f64,
    styles: &EventModelingStyles,
    ctx: &RenderCtx<'_>,
    bounds: &mut Bounds,
) -> Element {
    let width = max_r + SWIMLANE_PADDING;
    bounds.add(0.0, lane.y, width, lane.height);
    let label_spec =
        MeasureSpec::new(styles.font_family.clone(), styles.font_size as f32).weight(700);
    let shaped = ctx.metrics.shape(&lane.label, &label_spec);
    let label_y = lane.y + LABEL_OFFSET;
    if !lane.label.is_empty() {
        bounds.add(
            LABEL_OFFSET,
            label_y - f64::from(shaped.ascent),
            f64::from(shaped.width),
            f64::from(shaped.ascent + shaped.descent),
        );
    }
    let rect = Element::Rect(Rect {
        common: Common::with_style(Style {
            // A dropped `fill` attribute inherits the root `fill`.
            fill: Some(paint(styles.swimlane_fill.or(styles.text_color))),
            stroke: Some(paint(styles.swimlane_stroke)),
            ..Style::default()
        }),
        x: 0.0,
        y: lane.y,
        width,
        height: lane.height,
        rx: Some(3.0),
        ry: None,
    });
    let label = Element::Text(Text {
        common: Common::default(),
        x: LABEL_OFFSET,
        y: label_y,
        anchor: None,
        baseline: None,
        font: FontSpec {
            weight: Some(FontWeight::Bold),
            ..FontSpec::default()
        },
        runs: vec![TextRun::new(lane.label.clone())],
    });
    Element::Group(Group {
        common: Common {
            class: Some("em-swimlane".to_string()),
            ..Common::default()
        },
        children: vec![rect, label],
    })
}

/// `renderD3Box`.
fn em_box(
    b: &EmBox,
    index: usize,
    lane: &Swimlane,
    fonts: &BoxFonts,
    styles: &EventModelingStyles,
    ctx: &RenderCtx<'_>,
    bounds: &mut Bounds,
) -> Element {
    let y = lane.y + SWIMLANE_PADDING;
    let colors = styles.box_colors(b.kind);
    bounds.add(b.x, y, b.width, b.height);
    let fo = (
        b.x + BOX_PADDING,
        y + TEXT_TOP_INSET,
        b.width - 2.0 * BOX_PADDING,
        b.height - 2.0 * BOX_PADDING,
    );
    bounds.add(fo.0, fo.1, fo.2, fo.3);
    let mut children = vec![Element::Rect(Rect {
        common: Common::with_style(Style {
            fill: Some(paint(colors.fill.or(styles.text_color))),
            stroke: Some(paint(colors.stroke)),
            ..Style::default()
        }),
        x: b.x,
        y,
        width: b.width,
        height: b.height,
        rx: Some(3.0),
        ry: None,
    })];
    children.extend(box_text(b, index, fo, fonts, styles, ctx));
    Element::Group(Group {
        common: Common {
            class: Some("em-box".to_string()),
            ..Common::default()
        },
        children,
    })
}

/// `renderD3Relation`: from two thirds along the source box to one third
/// along the target box, leaving the source's bottom and entering the
/// target's top, or the other way round when the target lane is higher.
fn relation(
    state: &State,
    rel: &Relation,
    styles: &EventModelingStyles,
    bounds: &mut Bounds,
) -> Option<Element> {
    let source = state.boxes.get(rel.source)?;
    let target = state.boxes.get(rel.target)?;
    let source_y = state.swimlane_of(source)?.y + SWIMLANE_PADDING;
    let target_y = state.swimlane_of(target)?.y + SWIMLANE_PADDING;
    let upwards = source_y > target_y;
    let sx = source.x + (source.width * 2.0) / 3.0;
    let tx = target.x + target.width / 3.0;
    let (sy, ty) = if upwards {
        (source_y, target_y + target.height)
    } else {
        (source_y + source.height, target_y)
    };
    bounds.add(sx.min(tx), sy.min(ty), (tx - sx).abs(), (ty - sy).abs());
    let mut d = PathData::new();
    d.move_to(sx, sy).line_to(tx, ty);
    Some(Element::Path(Path {
        common: Common {
            class: Some("em-relation".to_string()),
            style: Style {
                fill: Some(Paint::None),
                stroke: Some(paint(styles.relation_stroke)),
                stroke_width: Some(1.0),
                marker_end: Some(marker_id()),
                ..Style::default()
            },
            ..Common::default()
        },
        d,
    }))
}

fn marker_id() -> String {
    format!("em-arrowhead-{SVG_ID}")
}

/// The arrowhead: `markerWidth 10`, `markerHeight 7`, `refX 10`, `refY
/// 3.5`, `orient auto`, and the polygon `0 0, 10 3.5, 0 7`.
fn arrowhead(styles: &EventModelingStyles) -> Element {
    Element::Marker(Marker {
        id: marker_id(),
        class: None,
        view_box: None,
        ref_x: 10.0,
        ref_y: 3.5,
        marker_width: 10.0,
        marker_height: 7.0,
        orient: Orient::Auto,
        marker_units: MarkerUnits::StrokeWidth,
        style: Style::default(),
        children: vec![Element::Polygon(Polygon {
            common: Common::with_style(Style {
                fill: Some(paint(styles.arrowhead.or(styles.text_color))),
                ..Style::default()
            }),
            points: vec![
                Point { x: 0.0, y: 0.0 },
                Point { x: 10.0, y: 3.5 },
                Point { x: 0.0, y: 7.0 },
            ],
        })],
    })
}

/// `draw`.
pub fn draw(
    state: &State,
    cfg: &EventModelingConfig,
    styles: &EventModelingStyles,
    ctx: &RenderCtx<'_>,
) -> Scene {
    let fonts = BoxFonts::new(ctx, styles);
    let mut bounds = Bounds::default();
    let mut children = Vec::new();
    for lane in state.sorted_swimlanes() {
        children.push(swimlane(lane, state.max_r, styles, ctx, &mut bounds));
    }
    for (i, b) in state.boxes.iter().enumerate() {
        if let Some(lane) = state.swimlane_of(b) {
            children.push(em_box(b, i, lane, &fonts, styles, ctx, &mut bounds));
        }
    }
    for rel in &state.relations {
        children.extend(relation(state, rel, styles, &mut bounds));
    }

    // `setupGraphViewbox(undefined, svg, padding, useMaxWidth)`.
    let (x0, y0, x1, y1) = bounds.rect.unwrap_or((0.0, 0.0, 0.0, 0.0));
    let width = (x1 - x0) + 2.0 * cfg.padding;
    let height = (y1 - y0) + 2.0 * cfg.padding;
    let mut scene = Scene::new(width, height);
    scene.view_box = ViewBox::new(x0 - cfg.padding, y0 - cfg.padding, width, height);
    scene.font_family = styles.font_family.clone();
    scene.defs = vec![arrowhead(styles)];
    scene.root = Group {
        common: Common::with_style(Style {
            fill: styles.text_color.map(Paint::Color),
            font: FontSpec {
                size: Some(styles.font_size),
                ..FontSpec::default()
            },
            ..Style::default()
        }),
        children,
    };
    scene
}
