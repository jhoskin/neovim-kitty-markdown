//! Port of `ishikawaRenderer.ts` (`draw`), mermaid 12.2.0, classic look.
//!
//! The head sits at the right end of a horizontal spine. Causes alternate
//! above and below it, in pairs from right to left; each pair starts
//! where the leftmost text of the previous pair ends. A cause is a bone
//! at 82 degrees with a boxed label at its far end. Its descendants are
//! flattened and spread evenly along the bone: even depths are
//! horizontal bones that attach to their parent's diagonal, odd depths
//! are diagonals that start on their parent's horizontal bone.
//!
//! The renderer measures with `getBBox()` three times: the head label
//! sizes the head, the cause labels size their boxes, and the texts of a
//! pair move the spine end. `applyPaddedViewBox` frames the `getBBox()`
//! of the whole SVG with `diagramPadding`. The bounds here follow Blink:
//! the geometry of lines, rects, and the head path (its tight curve
//! bounds), the advance box joined with the ink for text, no strokes,
//! and no markers. The text is placed on its alphabetic baseline with the
//! shift Blink applies for the `dominant-baseline` of each class.
//!
//! The rough.js `handDrawn` look is rejected before a renderer runs
//! (R-175), so only the marker path of the classic look is ported.

use std::collections::HashMap;

use serde_json::Value;

use super::db::{IshikawaDb, IshikawaNode};
use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::scene::{
    Baseline, Common, Element, FontSpec, FontWeight, Group, Length, Line, Marker, MarkerUnits,
    Orient, Path, PathData, Rect, Scene, Style, Text, TextAnchor, TextRun, TransformOp, ViewBox,
};
use crate::mermaid::styles::ishikawa::{
    HEAD_LABEL_FONT_SIZE, HEAD_LABEL_FONT_WEIGHT, IshikawaStyles,
};
use crate::mermaid::styles::pie::parse_int_prefix;
use crate::text::fonts::Weight;
use crate::text::measure::FontSpec as MeasureSpec;

/// `FONT_SIZE_DEFAULT`, `SPINE_BASE_LENGTH`, `BONE_STUB`, `BONE_BASE`,
/// `BONE_PER_CHILD`.
const FONT_SIZE_DEFAULT: f64 = 14.0;
const SPINE_BASE_LENGTH: f64 = 250.0;
const BONE_STUB: f64 = 30.0;
const BONE_BASE: f64 = 60.0;
const BONE_PER_CHILD: f64 = 5.0;
/// `ANGLE = (82 * Math.PI) / 180`.
const ANGLE_DEGREES: f64 = 82.0;
/// `drawMultilineText(..., y + 11 * direction, ...)` for a cause label.
const CAUSE_LABEL_OFFSET: f64 = 11.0;
/// `wrapText(child.text, 15)` for every label below a cause.
const SUB_LABEL_MAX_CHARS: usize = 15;
/// `const lh = fontSize * 1.05`.
const LINE_HEIGHT_EM: f64 = 1.05;
/// Blink's `hanging` baseline: 80 % of the ascent above the alphabetic one.
const HANGING_AS_PART_OF_ASCENT: f64 = 0.8;
/// `ishikawa-arrow-${id}` with mermaid-cli's `my-svg` id.
const MARKER_ID: &str = "ishikawa-arrow-my-svg";

/// An axis-aligned box in user units, what `getBBox()` reports.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn points(x1: f64, y1: f64, x2: f64, y2: f64) -> Bounds {
        Bounds {
            min_x: x1.min(x2),
            min_y: y1.min(y2),
            max_x: x1.max(x2),
            max_y: y1.max(y2),
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

    fn translated(self, dx: f64, dy: f64) -> Bounds {
        Bounds {
            min_x: self.min_x + dx,
            min_y: self.min_y + dy,
            max_x: self.max_x + dx,
            max_y: self.max_y + dy,
        }
    }

    fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
}

fn extend(acc: &mut Option<Bounds>, b: Bounds) {
    *acc = Some(match acc {
        Some(a) => a.union(b),
        None => b,
    });
}

/// `lerp(a, b, t)`.
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// The UTF-16 length JavaScript's `String.length` reports.
fn js_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// `wrapText(text, maxChars)`: greedy word wrap by character count.
fn wrap_text(text: &str, max_chars: usize) -> String {
    if js_len(text) <= max_chars {
        return text.to_string();
    }
    let mut lines: Vec<String> = Vec::new();
    // `text.split(/\s+/)`; the db trims the text, so no word is empty
    // unless the text is.
    for word in text.split(char::is_whitespace).filter(|w| !w.is_empty()) {
        match lines.last_mut() {
            Some(last) if js_len(last) + 1 + js_len(word) <= max_chars => {
                last.push(' ');
                last.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines.join("\n")
}

/// `splitLines(text)`: `text.split(/<br\s*\/?>|\n/)`.
fn split_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c == '\n' {
            lines.push(std::mem::take(&mut current));
            rest = &rest[1..];
            continue;
        }
        if let Some(tail) = rest.strip_prefix("<br") {
            let after_space = tail.trim_start_matches(|c: char| c.is_whitespace());
            let after_slash = after_space.strip_prefix('/').unwrap_or(after_space);
            if let Some(after) = after_slash.strip_prefix('>') {
                lines.push(std::mem::take(&mut current));
                rest = after;
                continue;
            }
        }
        current.push(c);
        rest = &rest[c.len_utf8()..];
    }
    lines.push(current);
    lines
}

/// SVG text collapses white space runs and drops the leading and trailing
/// ones before it lays out or measures a line.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `countDescendants(node)`.
fn count_descendants(node: &IshikawaNode) -> usize {
    node.children
        .iter()
        .map(|child| 1 + count_descendants(child))
        .sum()
}

/// `sideStats(nodes)`: the descendant total and the largest count of one
/// cause.
fn side_stats<'a>(nodes: impl Iterator<Item = &'a IshikawaNode>) -> (usize, usize) {
    nodes.fold((0, 0), |(total, max), node| {
        let d = count_descendants(node);
        (total + d, max.max(d))
    })
}

/// `LabelEntry`.
#[derive(Debug, Clone, PartialEq)]
struct LabelEntry {
    text: String,
    depth: usize,
    /// `parentIndex`; -1 is the cause itself.
    parent: isize,
    child_count: usize,
}

/// `flattenTree(children, direction)`: even depths in pre-order (close to
/// the spine), odd depths in post-order (inside the parent's wedge); the
/// upper side walks each level in reverse.
fn flatten_tree(children: &[IshikawaNode], direction: f64) -> (Vec<LabelEntry>, Vec<usize>) {
    fn walk(
        nodes: &[IshikawaNode],
        parent: isize,
        depth: usize,
        direction: f64,
        entries: &mut Vec<LabelEntry>,
        y_order: &mut Vec<usize>,
    ) {
        let ordered: Vec<&IshikawaNode> = if direction < 0.0 {
            nodes.iter().rev().collect()
        } else {
            nodes.iter().collect()
        };
        for child in ordered {
            let idx = entries.len();
            entries.push(LabelEntry {
                text: wrap_text(&child.text, SUB_LABEL_MAX_CHARS),
                depth,
                parent,
                child_count: child.children.len(),
            });
            let has_children = !child.children.is_empty();
            if depth.is_multiple_of(2) {
                y_order.push(idx);
                if has_children {
                    walk(
                        &child.children,
                        idx as isize,
                        depth + 1,
                        direction,
                        entries,
                        y_order,
                    );
                }
            } else {
                if has_children {
                    walk(
                        &child.children,
                        idx as isize,
                        depth + 1,
                        direction,
                        entries,
                        y_order,
                    );
                }
                y_order.push(idx);
            }
        }
    }
    let mut entries = Vec::new();
    let mut y_order = Vec::new();
    walk(children, -1, 2, direction, &mut entries, &mut y_order);
    (entries, y_order)
}

/// `BoneInfo`.
#[derive(Debug, Clone, Copy)]
struct Bone {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    child_count: usize,
    children_drawn: usize,
}

/// How a label's `y` relates to its glyphs, from its class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LabelBaseline {
    Middle,
    Alphabetic,
    Hanging,
}

struct Painter<'a, 'f> {
    ctx: &'a RenderCtx<'f>,
    styles: &'a IshikawaStyles,
    /// `parseFontSize(config.fontSize)[0] ?? 14`: the layout size.
    font_size: f64,
    /// The bounds of everything outside the head group.
    bounds: Option<Bounds>,
}

/// A drawn text and its `getBBox()`.
struct DrawnText {
    element: Element,
    bbox: Bounds,
}

impl Painter<'_, '_> {
    fn label_spec(&self) -> MeasureSpec {
        MeasureSpec::new(
            self.styles.font_family.clone(),
            self.styles.font_size as f32,
        )
    }

    fn head_spec(&self) -> MeasureSpec {
        MeasureSpec::new(self.styles.font_family.clone(), HEAD_LABEL_FONT_SIZE as f32)
            .weight(HEAD_LABEL_FONT_WEIGHT)
    }

    /// The distance from a label's `y` down to its alphabetic baseline:
    /// half the x-height for `middle`, 80 % of the ascent for `hanging`.
    fn baseline_shift(&self, spec: &MeasureSpec, baseline: LabelBaseline) -> f64 {
        let fonts = self.ctx.fonts;
        let Some(face) = fonts.resolve(&spec.family, Weight(spec.weight), spec.style.into()) else {
            return 0.0;
        };
        match baseline {
            LabelBaseline::Middle => fonts
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
            LabelBaseline::Hanging => {
                let (ascent, _) = self.ctx.metrics.face_float_metrics(face, spec.size);
                f64::from(ascent) * HANGING_AS_PART_OF_ASCENT
            }
            LabelBaseline::Alphabetic => 0.0,
        }
    }

    /// `drawMultilineText(g, text, x, y, cls, anchor, fontSize)`: one
    /// `tspan` per line, `lh` apart, the block centred on `y`. An empty
    /// line has no characters, so its `dy` moves nothing and the scene
    /// leaves it out.
    #[allow(clippy::too_many_arguments)]
    fn multiline_text(
        &self,
        text: &str,
        x: f64,
        y: f64,
        class: &str,
        anchor: TextAnchor,
        baseline: LabelBaseline,
        spec: &MeasureSpec,
    ) -> DrawnText {
        let lines = split_lines(text);
        let lh = self.font_size * LINE_HEIGHT_EM;
        let top = y - (lines.len() as f64 - 1.0) * lh / 2.0;
        let shift = self.baseline_shift(spec, baseline);
        let mut runs = Vec::new();
        let mut bbox: Option<Bounds> = None;
        let mut pos = top;
        let mut first_y = None;
        for (i, line) in lines.iter().enumerate() {
            let collapsed = collapse(line);
            if collapsed.is_empty() {
                continue;
            }
            let dy = if i == 0 { 0.0 } else { lh };
            pos += dy;
            let run_dy = if first_y.is_none() { 0.0 } else { dy };
            first_y.get_or_insert(pos);
            let baseline_y = pos + shift;
            let shaped = self.ctx.metrics.shape(&collapsed, spec);
            let advance = f64::from(shaped.width);
            let start = match anchor {
                TextAnchor::Start => x,
                TextAnchor::Middle => x - advance / 2.0,
                TextAnchor::End => x - advance,
            };
            let (ascent, descent) = (f64::from(shaped.ascent), f64::from(shaped.descent));
            let line_box = match shaped.ink {
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
            extend(&mut bbox, line_box);
            runs.push(TextRun {
                x: Some(x),
                dy: Some(Length::Px(run_dy)),
                ..TextRun::new(collapsed)
            });
        }
        let weight = (spec.weight != 400).then_some(FontWeight::Number(spec.weight));
        let element = Element::Text(Text {
            common: Common {
                class: Some(class.to_string()),
                style: self.styles.text(),
                ..Common::default()
            },
            x,
            y: first_y.unwrap_or(top) + shift,
            anchor: Some(anchor),
            baseline: Some(Baseline::Alphabetic),
            font: FontSpec {
                family: Some(self.styles.font_family.clone()),
                size: Some(f64::from(spec.size)),
                weight,
                ..FontSpec::default()
            },
            runs,
        });
        DrawnText {
            element,
            // `getBBox()` of a text without glyphs is all zeros.
            bbox: bbox.unwrap_or(Bounds::points(0.0, 0.0, 0.0, 0.0)),
        }
    }

    /// `drawLine(g, x1, y1, x2, y2, cls)`; branches carry the arrow marker.
    fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, class: &str) -> Element {
        extend(&mut self.bounds, Bounds::points(x1, y1, x2, y2));
        let mut style = if class == "ishikawa-sub-branch" {
            self.styles.sub_branch()
        } else {
            self.styles.branch()
        };
        if class != "ishikawa-spine" {
            style.marker_start = Some(MARKER_ID.to_string());
        }
        Element::Line(Line {
            common: Common {
                class: Some(class.to_string()),
                style,
                ..Common::default()
            },
            x1,
            y1,
            x2,
            y2,
        })
    }

    /// `drawHead(svg, x, y, label, fontSize)` at the origin: the group and
    /// its bounds in the group's own space.
    fn head(&self, label: &str) -> (Vec<Element>, Bounds) {
        let max_chars = ((110.0 / (self.font_size * 0.6)).floor()).max(6.0) as usize;
        let spec = self.head_spec();
        let text = self.multiline_text(
            &wrap_text(label, max_chars),
            0.0,
            0.0,
            "ishikawa-head-label",
            TextAnchor::Middle,
            LabelBaseline::Middle,
            &spec,
        );
        let tb = text.bbox;
        let w = (tb.width() + 6.0).max(60.0);
        let h = (tb.height() * 2.0 + 40.0).max(40.0);
        let mut d = PathData::new();
        d.move_to(0.0, -h / 2.0)
            .line_to(0.0, h / 2.0)
            .quad_to(w * 2.4, 0.0, 0.0, -h / 2.0)
            .close();
        let path = Element::Path(Path {
            common: Common {
                class: Some("ishikawa-head".to_string()),
                style: self.styles.boxed(),
                ..Common::default()
            },
            d,
        });
        // The quadratic from (0, h/2) over (2.4w, 0) to (0, -h/2) peaks at
        // t = 0.5, so its tight box reaches 1.2w.
        let path_box = Bounds::points(0.0, -h / 2.0, w * 1.2, h / 2.0);
        let tx = (w - tb.width()) / 2.0 - tb.min_x + 3.0;
        let ty = -tb.min_y - tb.height() / 2.0;
        let mut element = text.element;
        if let Element::Text(t) = &mut element {
            t.common.transform = vec![TransformOp::Translate(tx, ty)];
        }
        (vec![path, element], path_box.union(tb.translated(tx, ty)))
    }

    /// `drawCauseLabel`: the label centred 11 px past the bone's end, in a
    /// box 20 px wider on each side and 2 px taller.
    fn cause_label(&mut self, text: &str, x: f64, y: f64, direction: f64) -> (Element, f64) {
        let spec = self.label_spec();
        let drawn = self.multiline_text(
            text,
            x,
            y + CAUSE_LABEL_OFFSET * direction,
            "ishikawa-label cause",
            TextAnchor::Middle,
            LabelBaseline::Middle,
            &spec,
        );
        let tb = drawn.bbox;
        let rect_box = Bounds {
            min_x: tb.min_x - 20.0,
            min_y: tb.min_y - 2.0,
            max_x: tb.max_x + 20.0,
            max_y: tb.max_y + 2.0,
        };
        extend(&mut self.bounds, rect_box.union(tb));
        let rect = Element::Rect(Rect {
            common: Common {
                class: Some("ishikawa-label-box".to_string()),
                style: self.styles.boxed(),
                ..Common::default()
            },
            x: rect_box.min_x,
            y: rect_box.min_y,
            width: rect_box.width(),
            height: rect_box.height(),
            rx: None,
            ry: None,
        });
        (
            group("ishikawa-label-group", vec![rect, drawn.element]),
            tb.min_x,
        )
    }

    /// `drawBranch(svg, node, startX, startY, direction, length)`: the
    /// elements for the pair group and the leftmost text `x`.
    fn branch(
        &mut self,
        node: &IshikawaNode,
        start_x: f64,
        start_y: f64,
        direction: f64,
        length: f64,
    ) -> (Vec<Element>, f64) {
        let (cos_a, sin_a) = {
            let angle = ANGLE_DEGREES * std::f64::consts::PI / 180.0;
            (angle.cos(), angle.sin())
        };
        let children = &node.children;
        let line_len = length * if children.is_empty() { 0.2 } else { 1.0 };
        let dx = -cos_a * line_len;
        let dy = sin_a * line_len * direction;
        let end_x = start_x + dx;
        let end_y = start_y + dy;

        let mut out = vec![self.line(start_x, start_y, end_x, end_y, "ishikawa-branch")];
        let (label, mut left) = self.cause_label(&node.text, end_x, end_y, direction);
        out.push(label);
        if children.is_empty() {
            return (out, left);
        }

        let (entries, y_order) = flatten_tree(children, direction);
        let count = entries.len() as f64;
        let mut ys = vec![0.0; entries.len()];
        for (slot, &idx) in y_order.iter().enumerate() {
            ys[idx] = start_y + dy * ((slot as f64 + 1.0) / (count + 1.0));
        }

        let mut bones: HashMap<isize, Bone> = HashMap::new();
        bones.insert(
            -1,
            Bone {
                x0: start_x,
                y0: start_y,
                x1: end_x,
                y1: end_y,
                child_count: children.len(),
                children_drawn: 0,
            },
        );
        let diagonal_x = -cos_a;
        let diagonal_y = sin_a * direction;
        let (odd_class, odd_baseline) = if direction < 0.0 {
            ("ishikawa-label up", LabelBaseline::Alphabetic)
        } else {
            ("ishikawa-label down", LabelBaseline::Hanging)
        };
        let spec = self.label_spec();

        for (i, e) in entries.iter().enumerate() {
            let y = ys[i];
            let Some(par) = bones.get_mut(&e.parent) else {
                continue;
            };
            let (bx0, by0, bx1, line, text) = if e.depth.is_multiple_of(2) {
                // Horizontal bone: attach to the parent's diagonal at the
                // target y, extend left.
                let dy_p = par.y1 - par.y0;
                let t = if dy_p != 0.0 {
                    (y - par.y0) / dy_p
                } else {
                    0.5
                };
                let bx0 = lerp(par.x0, par.x1, t);
                let reach = if e.child_count > 0 {
                    BONE_BASE + e.child_count as f64 * BONE_PER_CHILD
                } else {
                    BONE_STUB
                };
                let bx1 = bx0 - reach;
                let line = self.line(bx0, y, bx1, y, "ishikawa-sub-branch");
                let text = self.multiline_text(
                    &e.text,
                    bx1,
                    y,
                    "ishikawa-label align",
                    TextAnchor::End,
                    LabelBaseline::Middle,
                    &spec,
                );
                (bx0, y, bx1, line, text)
            } else {
                // Diagonal bone: start from an evenly spaced point on the
                // parent's horizontal, angled toward the target y.
                let k = par.children_drawn;
                par.children_drawn += 1;
                let bx0 = lerp(
                    par.x0,
                    par.x1,
                    (par.child_count as f64 - k as f64) / (par.child_count as f64 + 1.0),
                );
                let by0 = par.y0;
                let bx1 = bx0 + diagonal_x * ((y - by0) / diagonal_y);
                let line = self.line(bx0, by0, bx1, y, "ishikawa-sub-branch");
                let text = self.multiline_text(
                    &e.text,
                    bx1,
                    y,
                    odd_class,
                    TextAnchor::End,
                    odd_baseline,
                    &spec,
                );
                (bx0, by0, bx1, line, text)
            };
            extend(&mut self.bounds, text.bbox);
            left = left.min(text.bbox.min_x);
            out.push(group("ishikawa-sub-group", vec![line, text.element]));
            if e.child_count > 0 {
                bones.insert(
                    i as isize,
                    Bone {
                        x0: bx0,
                        y0: by0,
                        x1: bx1,
                        y1: y,
                        child_count: e.child_count,
                        children_drawn: 0,
                    },
                );
            }
        }
        (out, left)
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

/// The `<marker>` the branches start with: a triangle pointing at the
/// spine, scaled by the stroke width.
fn arrow_marker(styles: &IshikawaStyles) -> Element {
    let mut d = PathData::new();
    d.move_to(10.0, 0.0)
        .line_to(0.0, 5.0)
        .line_to(10.0, 10.0)
        .close();
    Element::Marker(Marker {
        id: MARKER_ID.to_string(),
        class: None,
        view_box: Some(ViewBox::new(0.0, 0.0, 10.0, 10.0)),
        ref_x: 0.0,
        ref_y: 5.0,
        marker_width: 6.0,
        marker_height: 6.0,
        orient: Orient::Auto,
        marker_units: MarkerUnits::StrokeWidth,
        style: Style::default(),
        children: vec![Element::Path(Path {
            common: Common {
                class: Some("ishikawa-arrow".to_string()),
                style: styles.arrow(),
                ..Common::default()
            },
            d,
        })],
    })
}

/// `parseFontSize(drawConfig.fontSize)[0] ?? FONT_SIZE_DEFAULT`.
fn layout_font_size(raw: &Value) -> f64 {
    match raw.get("fontSize") {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(FONT_SIZE_DEFAULT),
        Some(Value::String(s)) => parse_int_prefix(s).unwrap_or(FONT_SIZE_DEFAULT),
        _ => FONT_SIZE_DEFAULT,
    }
}

pub fn draw(
    db: &IshikawaDb,
    resolved: &Resolved,
    ctx: &RenderCtx<'_>,
) -> Result<Scene, HelperError> {
    let styles = IshikawaStyles::resolve(&resolved.theme_variables);
    let frame = Frame {
        padding: resolved.raw["ishikawa"]["diagramPadding"]
            .as_f64()
            .unwrap_or(20.0),
        // `drawConfig.ishikawa?.useMaxWidth ?? false`; the config defaults
        // carry `true`.
        use_max_width: resolved.raw["ishikawa"]["useMaxWidth"]
            .as_bool()
            .unwrap_or(false),
    };
    let mut scene = Scene::new(0.0, 0.0);
    scene.font_family = styles.font_family.clone();
    // `if (!root) return;`: mermaid leaves the SVG without a viewBox; the
    // scene keeps an empty padded frame.
    let Some(root) = db.root() else {
        return Ok(frame.apply(scene, Bounds::points(0.0, 0.0, 0.0, 0.0)));
    };
    scene.defs.push(arrow_marker(&styles));

    let mut painter = Painter {
        ctx,
        styles: &styles,
        font_size: layout_font_size(&resolved.raw),
        bounds: None,
    };
    let causes = &root.children;
    let (head_children, head_box) = painter.head(&root.text);

    let mut spine_x = 0.0;
    let mut spine_y = SPINE_BASE_LENGTH;
    let mut pairs = Vec::new();
    if !causes.is_empty() {
        spine_x -= 20.0;
        let (upper_total, upper_max) = side_stats(causes.iter().step_by(2));
        let (lower_total, lower_max) = side_stats(causes.iter().skip(1).step_by(2));
        let descendant_total = upper_total + lower_total;
        let mut upper_len = SPINE_BASE_LENGTH;
        let mut lower_len = SPINE_BASE_LENGTH;
        if descendant_total > 0 {
            let pool = SPINE_BASE_LENGTH * 2.0;
            let min_len = SPINE_BASE_LENGTH * 0.3;
            let total = descendant_total as f64;
            upper_len = min_len.max(pool * (upper_total as f64 / total));
            lower_len = min_len.max(pool * (lower_total as f64 / total));
        }
        let min_spacing = painter.font_size * 2.0;
        upper_len = upper_len.max(upper_max as f64 * min_spacing);
        lower_len = lower_len.max(lower_max as f64 * min_spacing);
        spine_y = upper_len.max(SPINE_BASE_LENGTH);

        for pair in causes.chunks(2) {
            let mut children = Vec::new();
            let mut left = f64::INFINITY;
            for (cause, direction, len) in [
                (pair.first(), -1.0, upper_len),
                (pair.get(1), 1.0, lower_len),
            ] {
                if let Some(cause) = cause {
                    let (elements, l) = painter.branch(cause, spine_x, spine_y, direction, len);
                    children.extend(elements);
                    left = left.min(l);
                }
            }
            pairs.push(group("ishikawa-pair", children));
            spine_x = left;
        }
    }

    let spine = painter.line(spine_x, spine_y, 0.0, spine_y, "ishikawa-spine");
    let head = Element::Group(Group {
        common: Common {
            class: Some("ishikawa-head-group".to_string()),
            transform: vec![TransformOp::Translate(0.0, spine_y)],
            ..Common::default()
        },
        children: head_children,
    });
    let mut children = vec![spine, head];
    children.extend(pairs);
    scene.root.children.push(group("ishikawa", children));

    let mut bounds = head_box.translated(0.0, spine_y);
    if let Some(b) = painter.bounds {
        bounds = bounds.union(b);
    }
    Ok(frame.apply(scene, bounds))
}

/// The arguments of `applyPaddedViewBox(svg, pad, useMaxWidth)`.
struct Frame {
    padding: f64,
    use_max_width: bool,
}

impl Frame {
    /// `applyPaddedViewBox`: the viewBox is the bounds grown by `pad` on
    /// every side, and `configureSvgSize` gives the SVG that size, as
    /// `max-width` with `useMaxWidth` and as fixed attributes without it.
    fn apply(&self, mut scene: Scene, bounds: Bounds) -> Scene {
        let pad = self.padding;
        let (w, h) = fixed_size_frame(
            bounds.width() + pad * 2.0,
            bounds.height() + pad * 2.0,
            self.use_max_width,
        );
        scene.width = w;
        scene.height = h;
        scene.view_box = ViewBox::new(bounds.min_x - pad, bounds.min_y - pad, w, h);
        scene
    }
}

/// The scene size for a frame of `w` by `h`. A fixed-size SVG
/// (`useMaxWidth: false`) is drawn by the browser at scale 1, and the
/// screenshot clip rounds its size up to whole pixels, so the fraction is
/// page background on the right and bottom. Growing the frame to whole
/// pixels from the same origin gives the raster that picture; a
/// `useMaxWidth` SVG is scaled to its clip width instead and keeps its
/// size.
fn fixed_size_frame(w: f64, h: f64, use_max_width: bool) -> (f64, f64) {
    if use_max_width {
        (w, h)
    } else {
        (w.ceil(), h.ceil())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(text: &str, children: Vec<IshikawaNode>) -> IshikawaNode {
        IshikawaNode {
            text: text.to_string(),
            children,
        }
    }

    #[test]
    fn wrap_text_breaks_greedily_at_the_limit() {
        assert_eq!(wrap_text("Out of focus", 15), "Out of focus");
        assert_eq!(
            wrap_text("Beautification filter applied", 15),
            "Beautification\nfilter applied"
        );
        assert_eq!(
            wrap_text("Protective film not removed", 15),
            "Protective film\nnot removed"
        );
        assert_eq!(wrap_text("Blurry Photo", 11), "Blurry\nPhoto");
    }

    #[test]
    fn split_lines_splits_on_br_tags_and_newlines() {
        assert_eq!(
            split_lines("a<br>b<br/>c<br />d\ne"),
            ["a", "b", "c", "d", "e"]
        );
        assert_eq!(split_lines("a<BR>b"), ["a<BR>b"]);
    }

    #[test]
    fn flatten_orders_even_depths_first_and_odd_depths_after_their_children() {
        let children = vec![
            node(
                "LENS",
                vec![node("a", vec![node("a1", vec![])]), node("b", vec![])],
            ),
            node("SENSOR", vec![node("c", vec![])]),
        ];
        let (entries, order) = flatten_tree(&children, 1.0);
        let texts: Vec<&str> = entries.iter().map(|e| e.text.as_str()).collect();
        assert_eq!(texts, ["LENS", "a", "a1", "b", "SENSOR", "c"]);
        assert_eq!(order, [0, 2, 1, 3, 4, 5]);
        assert_eq!(entries[2].parent, 1);
        assert_eq!(entries[2].depth, 4);
        // The upper side walks every level in reverse.
        let (entries, order) = flatten_tree(&children, -1.0);
        let texts: Vec<&str> = entries.iter().map(|e| e.text.as_str()).collect();
        assert_eq!(texts, ["SENSOR", "c", "LENS", "b", "a", "a1"]);
        assert_eq!(order, [0, 1, 2, 3, 5, 4]);
    }

    #[test]
    fn side_stats_count_every_descendant() {
        let causes = [
            node("A", vec![node("a", vec![node("a1", vec![])])]),
            node("B", vec![]),
            node("C", vec![node("c", vec![])]),
        ];
        assert_eq!(side_stats(causes.iter().step_by(2)), (3, 2));
        assert_eq!(side_stats(causes.iter().skip(1).step_by(2)), (0, 0));
    }
}
