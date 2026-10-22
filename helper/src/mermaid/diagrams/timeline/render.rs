//! Port of `timelineRenderer.ts` (`draw`, `drawTasks`, `drawEvents`) and
//! the `svgDraw.js` functions it calls (`initGraphics`, `drawNode`,
//! `getVirtualNodeHeight`, `wrap`, `defaultBkg`), mermaid 12.2.0, for the
//! `LR` direction.
//!
//! Three browser facts shape the numbers here:
//!
//! - `wrap` measures each tspan with `getComputedTextLength()` in the live
//!   svg, so the measure font is the render font: the root `font-family`
//!   and `font-size` of the stylesheet, which come from the theme. The
//!   `drawNode` text has `dominant-baseline: middle`, which Chrome applies
//!   as a shift of half the x-height down; the shift is written as the
//!   text `y` so the scene and the bounds use the same number.
//! - The first tspan has `dy="1em"`, the rest `dy="1.1em"`, and an empty
//!   tspan positions no character, so its `dy` never applies. `getBBox()`
//!   on the `<text>` is the union of the line boxes (rounded ascent and
//!   descent of the primary face joined with the glyph ink, as the text
//!   measurer models it).
//! - `draw` measures the tasks' heights with `descr: task`, the whole task
//!   object, so every task box is at least as tall as the one-line text
//!   `[object Object]`; and `if (task.events)` is true for an empty array,
//!   so every task gets its dashed line.
//!
//! `svg.node().getBBox()` is replaced by running bounds over the geometry
//! the renderer places; strokes and markers do not count in a `getBBox`,
//! so they are left out here too.

use serde_json::Value;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::diagrams::sequence::conf::{BODY_DEFAULT_FAMILY, css_family_valid};
use crate::mermaid::jison::lexer::js_trim;
use crate::mermaid::scene::path::fmt_num;
use crate::mermaid::scene::{
    Common, Element, FontSpec, FontWeight, Group, Line, Marker, MarkerUnits, Orient, Path,
    PathData, Scene, Style, Text, TextAnchor, TextRun, TransformOp, ViewBox,
};
use crate::mermaid::styles::timeline::{TimelineStyles, brighten};
use crate::mermaid::theme::vars::js_truthy;
use crate::text::fonts::Weight;
use crate::text::measure::Shaped;
use crate::text::{FontSpec as MeasureSpec, FontSystem, TextMetrics};

use super::db::{Task, TimelineDb};

/// The svg id mermaid-cli renders with; marker and node ids hang off it.
pub const DIAGRAM_ID: &str = "my-svg";

/// `width: 150` and `padding: 20` of every `Block` the renderer builds.
const BLOCK_WIDTH: f64 = 150.0;
const BLOCK_PADDING: f64 = 20.0;
/// `maxHeight: 50` of an event block.
const EVENT_MAX_HEIGHT: f64 = 50.0;
/// `.eventWrapper { filter: brightness(120%) }`.
const EVENT_BRIGHTNESS: f64 = 1.2;
/// The `<br>` token `wrap` breaks on (case-sensitive, no slash).
const BR: &str = "<br>";

/// The config `draw` reads.
struct Conf {
    /// `conf.timeline?.leftMargin ?? 50`.
    left_margin: f64,
    /// `conf.timeline?.padding ?? 50`, the `setupGraphViewbox` padding.
    padding: f64,
    /// `conf.timeline?.disableMulticolor`.
    disable_multicolor: bool,
    /// `conf.fontSize` with a `px` suffix removed, the number `drawNode`
    /// adds half a line of to every box.
    font_size: f64,
}

impl Conf {
    fn from_resolved(resolved: &Resolved) -> Conf {
        let timeline = &resolved.config.timeline;
        let num =
            |key: &str, default: f64| timeline.get(key).and_then(Value::as_f64).unwrap_or(default);
        let typed_size = *resolved.config.font_size;
        let font_size = match resolved.raw.get("fontSize") {
            Some(Value::String(s)) => s.replace("px", "").trim().parse().unwrap_or(typed_size),
            Some(Value::Number(n)) => n.as_f64().unwrap_or(typed_size),
            _ => typed_size,
        };
        Conf {
            left_margin: num("leftMargin", 50.0),
            padding: num("padding", 50.0),
            disable_multicolor: timeline.get("disableMulticolor").is_some_and(js_truthy),
            font_size,
        }
    }
}

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

    fn shifted(self, dx: f64, dy: f64) -> Bounds {
        Bounds {
            min_x: self.min_x + dx,
            min_y: self.min_y + dy,
            max_x: self.max_x + dx,
            max_y: self.max_y + dy,
        }
    }

    fn width(self) -> f64 {
        self.max_x - self.min_x
    }

    fn height(self) -> f64 {
        self.max_y - self.min_y
    }
}

fn extend(bounds: &mut Option<Bounds>, more: Bounds) {
    *bounds = Some(bounds.map_or(more, |b| b.union(more)));
}

/// `getBBox()` of one line of text drawn at `(x, baseline)` with `anchor`:
/// the advance box joined with the ink, as `Shaped::bbox` does.
fn text_bounds(shaped: &Shaped, x: f64, baseline: f64, anchor: TextAnchor) -> Bounds {
    let advance = f64::from(shaped.width);
    let start = match anchor {
        TextAnchor::Start => x,
        TextAnchor::Middle => x - advance / 2.0,
        TextAnchor::End => x - advance,
    };
    let (ascent, descent) = (f64::from(shaped.ascent), f64::from(shaped.descent));
    match shaped.ink {
        Some(ink) => Bounds {
            min_x: start + f64::from(ink.min_x).min(0.0),
            min_y: baseline - f64::from(ink.top).max(ascent),
            max_x: start + f64::from(ink.max_x).max(advance),
            max_y: baseline + f64::from(ink.bottom).max(descent),
        },
        None => Bounds {
            min_x: start,
            min_y: baseline - ascent,
            max_x: start + advance,
            max_y: baseline + descent,
        },
    }
}

/// `text.split(/(\s+|<br>)/)`: the words with the separators kept as
/// tokens, and an empty string between two adjacent separators, as
/// `String.prototype.split` with a capturing group returns them.
fn split_words(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c.is_whitespace() {
            let end = rest
                .char_indices()
                .find(|(_, ch)| !ch.is_whitespace())
                .map_or(rest.len(), |(i, _)| i);
            tokens.push(std::mem::take(&mut word));
            tokens.push(rest[..end].to_string());
            rest = &rest[end..];
        } else if let Some(after) = rest.strip_prefix(BR) {
            tokens.push(std::mem::take(&mut word));
            tokens.push(BR.to_string());
            rest = after;
        } else {
            word.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    tokens.push(word);
    tokens
}

/// `line.join(' ').trim()`.
fn join_trim(line: &[String]) -> String {
    js_trim(&line.join(" ")).to_string()
}

/// Character data as the renderer lays it out under the default
/// `xml:space`: runs of white space become one space, ends are trimmed.
fn collapse(text: &str) -> String {
    text.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
}

/// `wrap(text, width)` from `svgDraw.js`: the text of every tspan it leaves
/// behind, in order. The current tspan is set to the joined line after each
/// word; when the line overflows `width` or the word is `<br>`, the word
/// comes off again and starts a new tspan whose text is the word itself.
fn wrap(text: &str, width: f64, measure: &dyn Fn(&str) -> f64) -> Vec<String> {
    let mut tspans: Vec<String> = vec![String::new()];
    let mut line: Vec<String> = Vec::new();
    for word in split_words(text) {
        line.push(word.clone());
        let joined = join_trim(&line);
        let over = measure(&joined) > width;
        if let Some(last) = tspans.last_mut() {
            *last = joined;
        }
        if over || word == BR {
            line.pop();
            if let Some(last) = tspans.last_mut() {
                *last = join_trim(&line);
            }
            line = if word == BR {
                vec![String::new()]
            } else {
                vec![word.clone()]
            };
            tspans.push(word);
        }
    }
    tspans
}

/// A wrapped `<text>` as Chrome lays it out: the non-empty tspans with
/// their `dy` in pixels, and the `getBBox()` relative to the text origin.
struct TextLayout {
    runs: Vec<(String, f64)>,
    bounds: Option<Bounds>,
}

/// Where `drawNode` puts a block and which section colors it.
struct Block<'a> {
    descr: &'a str,
    /// `wrap`'s width; `drawNode` adds the padding on both sides after.
    width: f64,
    padding: f64,
    max_height: f64,
}

struct Renderer<'a> {
    styles: TimelineStyles,
    conf: Conf,
    metrics: &'a TextMetrics<'a>,
    /// The root font: the theme's family at the theme's size.
    body: MeasureSpec,
    /// The theme `font-size` in pixels, what `1em` is in a tspan `dy`.
    css_font_size: f64,
    /// The x-height of the root font in pixels: the `middle` baseline
    /// shift, and a quarter of the title's `4ex`.
    x_height: f64,
    /// `conf.themeVariables.THEME_COLOR_LIMIT ?? 12`.
    max_sections: f64,
    elements: Vec<Element>,
    bounds: Option<Bounds>,
    /// `nodeCount`, the running id of the node background paths.
    node_count: usize,
}

/// The x-height of the face `spec` resolves to, in pixels: the OS/2 value
/// Chrome reads through the platform font, or the height of the `x` glyph
/// when the table has none.
fn x_height(fonts: &FontSystem, spec: &MeasureSpec) -> f64 {
    let Some(face) = fonts.resolve(&spec.family, Weight(spec.weight), spec.style.into()) else {
        return 0.0;
    };
    fonts
        .with_face(face, |f| {
            let upem = f64::from(f.units_per_em());
            let units = f.x_height().filter(|v| *v > 0).map(f64::from).or_else(|| {
                f.glyph_index('x')
                    .and_then(|g| f.glyph_bounding_box(g))
                    .map(|r| f64::from(r.y_max))
            });
            units.map_or(0.0, |u| u / upem * f64::from(spec.size))
        })
        .unwrap_or(0.0)
}

impl Renderer<'_> {
    /// `getComputedTextLength()` of a tspan holding `text` in the root font.
    fn text_length(&self, text: &str) -> f64 {
        f64::from(self.metrics.shape(&collapse(text), &self.body).width)
    }

    /// `wrap` followed by the browser's layout of the tspans.
    fn layout_text(&self, descr: &str, width: f64) -> TextLayout {
        let measure = |t: &str| self.text_length(t);
        let tspans = wrap(descr, width, &measure);
        let mut runs = Vec::new();
        let mut bounds = None;
        // `dominant-baseline: middle` lowers every glyph by half the
        // x-height; the text has no `y`, so the origin is the group's.
        let mut baseline = self.x_height / 2.0;
        for (i, tspan) in tspans.iter().enumerate() {
            let text = collapse(tspan);
            if text.is_empty() {
                continue;
            }
            let dy = if i == 0 {
                self.css_font_size
            } else {
                1.1 * self.css_font_size
            };
            baseline += dy;
            let shaped = self.metrics.shape(&text, &self.body);
            extend(
                &mut bounds,
                text_bounds(&shaped, 0.0, baseline, TextAnchor::Middle),
            );
            runs.push((text, dy));
        }
        TextLayout { runs, bounds }
    }

    /// `getVirtualNodeHeight`: the text box plus half a line plus the
    /// padding, for a block of `width` before its padding is added.
    fn virtual_node_height(&self, descr: &str, width: f64) -> f64 {
        let layout = self.layout_text(descr, width);
        let bbox_height = layout.bounds.map_or(0.0, Bounds::height);
        bbox_height + self.conf.font_size * 1.1 * 0.5 + BLOCK_PADDING
    }

    /// `drawNode` at `translate(x, y)` inside a wrapper group of class
    /// `wrapper`. Returns the node height after the `maxHeight` floor.
    fn draw_node(
        &mut self,
        block: &Block<'_>,
        full_section: f64,
        wrapper: &str,
        is_event: bool,
        (x, y): (f64, f64),
    ) -> f64 {
        let section = full_section % self.max_sections - 1.0;
        let layout = self.layout_text(block.descr, block.width);
        let bbox_height = layout.bounds.map_or(0.0, Bounds::height);
        let height =
            (bbox_height + self.conf.font_size * 1.1 * 0.5 + block.padding).max(block.max_height);
        let width = block.width + 2.0 * block.padding;

        let mut fill = self.styles.node_fill(section);
        let mut text_fill = self.styles.node_text_fill(section);
        let mut line_style = self.styles.node_line(section);
        if is_event {
            fill = brighten(&fill, EVENT_BRIGHTNESS);
            text_fill = brighten(&text_fill, EVENT_BRIGHTNESS);
            if let Some(stroke) = &line_style.stroke {
                line_style.stroke = Some(brighten(stroke, EVENT_BRIGHTNESS));
            }
        }

        // `defaultBkg`: `M0 h-5 v-(h-10) q0,-5,5,-5 h(w-10) q5,0,5,5 v(h-5)
        // H0 Z`, a path with rounded top corners and a square bottom, then
        // the line along the bottom edge.
        let r = 5.0;
        let mut d = PathData::new();
        d.move_to(0.0, height - r)
            .line_to(0.0, r)
            .quad_to(0.0, 0.0, r, 0.0)
            .line_to(width - r, 0.0)
            .quad_to(width, 0.0, width, r)
            .line_to(width, height)
            .line_to(0.0, height)
            .close();
        let bkg = Element::Group(Group {
            common: Common::default(),
            children: vec![
                Element::Path(Path {
                    common: Common {
                        id: Some(format!("{DIAGRAM_ID}-node-{}", self.node_count)),
                        class: Some("node-bkg".to_string()),
                        transform: Vec::new(),
                        style: Style {
                            fill: Some(fill),
                            ..Style::default()
                        },
                    },
                    d,
                }),
                Element::Line(Line {
                    common: Common {
                        class: Some(format!("node-line-{}", fmt_num(section))),
                        style: line_style,
                        ..Common::default()
                    },
                    x1: 0.0,
                    y1: height,
                    x2: width,
                    y2: height,
                }),
            ],
        });
        self.node_count += 1;

        // mermaid writes one `<text>` with a `tspan` per line. The scene
        // serializer indents the tspans, and that white space becomes a
        // trailing space on every line but the last, which moves a
        // `text-anchor: middle` line left by half a space. One `<text>` per
        // line at the line's baseline renders the same glyphs without it.
        let text_origin = (width / 2.0, block.padding / 2.0);
        let mut baseline = self.x_height / 2.0;
        let lines = layout
            .runs
            .iter()
            .map(|(t, dy)| {
                baseline += dy;
                Element::Text(Text {
                    common: Common::with_style(Style {
                        fill: Some(text_fill.clone()),
                        ..Style::default()
                    }),
                    x: 0.0,
                    y: baseline,
                    anchor: Some(TextAnchor::Middle),
                    baseline: None,
                    font: FontSpec::default(),
                    runs: vec![TextRun::new(t.clone())],
                })
            })
            .collect();
        let text = Element::Group(Group {
            common: Common {
                transform: vec![TransformOp::Translate(text_origin.0, text_origin.1)],
                ..Common::default()
            },
            children: lines,
        });

        let node = Element::Group(Group {
            common: Common {
                class: Some(format!("timeline-node section-{}", fmt_num(section))),
                ..Common::default()
            },
            children: vec![bkg, text],
        });
        self.elements.push(Element::Group(Group {
            common: Common {
                class: Some(wrapper.to_string()),
                transform: vec![TransformOp::Translate(x, y)],
                ..Common::default()
            },
            children: vec![node],
        }));

        extend(&mut self.bounds, Bounds::rect(x, y, width, height));
        if let Some(b) = layout.bounds {
            extend(
                &mut self.bounds,
                b.shifted(x + text_origin.0, y + text_origin.1),
            );
        }
        height
    }

    /// `drawEvents`: the events stacked under a task from `master_y + 100`,
    /// 10 px apart, each at least 50 px tall.
    fn draw_events(&mut self, events: &[String], section_color: f64, master_x: f64, master_y: f64) {
        let mut y = master_y + 100.0;
        for event in events {
            let block = Block {
                descr: event,
                width: BLOCK_WIDTH,
                padding: BLOCK_PADDING,
                max_height: EVENT_MAX_HEIGHT,
            };
            let height = self.draw_node(&block, section_color, "eventWrapper", true, (master_x, y));
            y += 10.0 + height;
        }
    }

    /// `drawTasks`.
    #[allow(clippy::too_many_arguments)]
    fn draw_tasks(
        &mut self,
        tasks: &[&Task],
        mut section_color: f64,
        mut master_x: f64,
        master_y: f64,
        mut max_task_height: f64,
        max_event_line_length: f64,
        is_without_sections: bool,
    ) {
        for task in tasks {
            let block = Block {
                descr: &task.task,
                width: BLOCK_WIDTH,
                padding: BLOCK_PADDING,
                max_height: max_task_height,
            };
            let task_height = self.draw_node(
                &block,
                section_color,
                "taskWrapper",
                false,
                (master_x, master_y),
            );
            max_task_height = max_task_height.max(task_height);

            // The line wrapper is appended before the events, so the dashed
            // line runs under them; its length ignores the events it has.
            let x = master_x + 190.0 / 2.0;
            let y1 = master_y + max_task_height;
            let y2 = master_y + max_task_height + 100.0 + max_event_line_length + 100.0;
            self.elements.push(line_wrapper(Line {
                common: Common::with_style(Style {
                    stroke: Some(self.styles.line_wrapper_stroke()),
                    stroke_width: Some(2.0),
                    stroke_dasharray: Some(vec![5.0, 5.0]),
                    marker_end: Some(marker_ref("arrowhead")),
                    ..Style::default()
                }),
                x1: x,
                y1,
                x2: x,
                y2,
            }));
            extend(&mut self.bounds, Bounds::rect(x, y1, 0.0, y2 - y1));
            self.draw_events(&task.events, section_color, master_x, master_y + 100.0);

            master_x += 200.0;
            if is_without_sections && !self.conf.disable_multicolor {
                section_color += 1.0;
            }
        }
    }
}

fn line_wrapper(line: Line) -> Element {
    Element::Group(Group {
        common: Common {
            class: Some("lineWrapper".to_string()),
            ..Common::default()
        },
        children: vec![Element::Line(line)],
    })
}

pub fn marker_ref(name: &str) -> String {
    format!("{DIAGRAM_ID}-{name}")
}

/// `initGraphics`: the one arrowhead marker. Its path sets no fill, so it
/// takes the root rule's `fill: textColor`.
fn arrowhead(styles: &TimelineStyles) -> Element {
    let mut d = PathData::new();
    d.move_to(0.0, 0.0)
        .line_to(0.0, 4.0)
        .line_to(6.0, 2.0)
        .close();
    Element::Marker(Marker {
        id: marker_ref("arrowhead"),
        class: None,
        view_box: None,
        ref_x: 5.0,
        ref_y: 2.0,
        marker_width: 6.0,
        marker_height: 4.0,
        orient: Orient::Auto,
        marker_units: MarkerUnits::StrokeWidth,
        style: Style::default(),
        children: vec![Element::Path(Path {
            common: Common::with_style(Style {
                fill: Some(styles.text_color.clone()),
                ..Style::default()
            }),
            d,
        })],
    })
}

/// `timelineRenderer.draw` for a parsed db.
pub fn draw(
    db: &TimelineDb,
    resolved: &Resolved,
    ctx: &RenderCtx<'_>,
) -> Result<Scene, HelperError> {
    let styles = TimelineStyles::resolve(&resolved.theme_variables);
    let root_family = if css_family_valid(&styles.font_family) {
        styles.font_family.clone()
    } else {
        BODY_DEFAULT_FAMILY.to_string()
    };
    let conf = Conf::from_resolved(resolved);
    let body = MeasureSpec::new(root_family.clone(), styles.font_size as f32);
    let max_sections = resolved
        .theme_variables
        .get("THEME_COLOR_LIMIT")
        .as_f64()
        .unwrap_or(12.0);
    let left_margin = conf.left_margin;
    let padding = conf.padding;
    let mut r = Renderer {
        css_font_size: styles.font_size,
        x_height: x_height(ctx.fonts, &body),
        styles,
        conf,
        metrics: &ctx.metrics,
        body,
        max_sections,
        elements: Vec::new(),
        bounds: None,
        node_count: 0,
    };

    let tasks = db.tasks();
    let sections = db.sections();

    // The sizing passes: `getVirtualNodeHeight` for every section, every
    // task (as `[object Object]`), and every event.
    let mut max_section_height = 0.0f64;
    for section in sections {
        let h = r.virtual_node_height(section, BLOCK_WIDTH);
        max_section_height = max_section_height.max(h + 20.0);
    }
    let mut max_task_height = 0.0f64;
    let mut max_event_line_length = 0.0f64;
    for task in tasks {
        let h = r.virtual_node_height("[object Object]", BLOCK_WIDTH);
        max_task_height = max_task_height.max(h + 20.0);
        let mut events_height: f64 = task
            .events
            .iter()
            .map(|event| r.virtual_node_height(event, BLOCK_WIDTH))
            .sum();
        if !task.events.is_empty() {
            events_height += (task.events.len() - 1) as f64 * 10.0;
        }
        max_event_line_length = max_event_line_length.max(events_height);
    }

    let mut master_x = 50.0 + left_margin;
    let section_begin_y = 50.0;
    let mut master_y = 50.0;
    let has_sections = !sections.is_empty();
    if has_sections {
        for (section_number, section) in sections.iter().enumerate() {
            let section_number = section_number as f64;
            let tasks_for_section: Vec<&Task> =
                tasks.iter().filter(|t| t.section == *section).collect();
            let count = tasks_for_section.len().max(1) as f64;
            let block = Block {
                descr: section,
                width: 200.0 * count - 50.0,
                padding: BLOCK_PADDING,
                max_height: max_section_height,
            };
            r.draw_node(
                &block,
                section_number,
                "sectionWrapper",
                false,
                (master_x, section_begin_y),
            );
            master_y += max_section_height + 50.0;
            if !tasks_for_section.is_empty() {
                r.draw_tasks(
                    &tasks_for_section,
                    section_number,
                    master_x,
                    master_y,
                    max_task_height,
                    max_event_line_length,
                    false,
                );
            }
            master_x += 200.0 * count;
            master_y = section_begin_y;
        }
    } else {
        let all: Vec<&Task> = tasks.iter().collect();
        r.draw_tasks(
            &all,
            0.0,
            master_x,
            master_y,
            max_task_height,
            max_event_line_length,
            true,
        );
    }

    // `svg.node().getBBox()` before the title and the activity line; an
    // svg with nothing drawn reports an all-zero box.
    let content = r.bounds.unwrap_or(Bounds::rect(0.0, 0.0, 0.0, 0.0));

    let title = collapse(db.diagram_title());
    if !title.is_empty() {
        // `font-size: 4ex` resolves against the parent's font, the root.
        let size = 4.0 * r.x_height;
        let spec = MeasureSpec::new(root_family.clone(), size as f32).weight(700);
        let x = content.width() / 2.0 - left_margin;
        let y = 20.0;
        let shaped = r.metrics.shape(&title, &spec);
        extend(&mut r.bounds, text_bounds(&shaped, x, y, TextAnchor::Start));
        r.elements.push(Element::Text(Text {
            common: Common::default(),
            x,
            y,
            anchor: None,
            baseline: None,
            font: FontSpec {
                size: Some(size),
                weight: Some(FontWeight::Bold),
                ..FontSpec::default()
            },
            runs: vec![TextRun::new(title)],
        }));
    }

    let depth_y = if has_sections {
        max_section_height + max_task_height + 150.0
    } else {
        max_task_height + 100.0
    };
    let x2 = content.width() + 3.0 * left_margin;
    r.elements.push(line_wrapper(Line {
        common: Common::with_style(Style {
            stroke: Some(r.styles.line_wrapper_stroke()),
            stroke_width: Some(4.0),
            marker_end: Some(marker_ref("arrowhead")),
            ..Style::default()
        }),
        x1: left_margin,
        y1: depth_y,
        x2,
        y2: depth_y,
    }));
    extend(
        &mut r.bounds,
        Bounds::rect(left_margin, depth_y, x2 - left_margin, 0.0),
    );

    // `setupGraphViewbox(undefined, svg, padding, useMaxWidth)`.
    let bounds = r.bounds.unwrap_or(Bounds::rect(0.0, 0.0, 0.0, 0.0));
    let width = bounds.width() + 2.0 * padding;
    let height = bounds.height() + 2.0 * padding;
    let mut scene = Scene::new(width, height);
    scene.view_box = ViewBox::new(
        bounds.min_x - padding,
        bounds.min_y - padding,
        width,
        height,
    );
    scene.font_family = root_family;
    scene.defs = vec![arrowhead(&r.styles)];
    scene.root = Group {
        common: Common::with_style(r.styles.root()),
        children: r.elements,
    };
    Ok(scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_keeps_separators_like_js() {
        assert_eq!(split_words("a b"), ["a", " ", "b"]);
        assert_eq!(split_words(" a"), ["", " ", "a"]);
        assert_eq!(split_words("a"), ["a"]);
        assert_eq!(split_words(""), [""]);
        assert_eq!(
            split_words("Steam <br>power"),
            ["Steam", " ", "", "<br>", "power"]
        );
        assert_eq!(split_words("a  b "), ["a", "  ", "b", " ", ""]);
    }

    #[test]
    fn wrap_breaks_on_width_and_br() {
        // Ten pixels per rendered character; the raw tspan text keeps the
        // separator tokens that `join` put between the words.
        let measure = |t: &str| collapse(t).chars().count() as f64 * 10.0;
        assert_eq!(wrap("Hello world", 200.0, &measure), ["Hello   world"]);
        assert_eq!(wrap("Hello world", 80.0, &measure), ["Hello", "world"]);
        assert_eq!(wrap("Steam <br>power", 500.0, &measure), ["Steam", "power"]);
        // A word wider than the box leaves an empty first tspan behind.
        assert_eq!(
            wrap("Internationalization ok", 100.0, &measure),
            ["", "Internationalization", "ok"]
        );
        // A trailing `<br>` leaves an empty tspan, since `split` yields an
        // empty string after a trailing separator.
        assert_eq!(wrap("end<br>", 500.0, &measure), ["end", ""]);
    }

    #[test]
    fn collapse_folds_runs_of_spaces() {
        assert_eq!(collapse("  a   b "), "a b");
        assert_eq!(collapse(""), "");
    }
}
