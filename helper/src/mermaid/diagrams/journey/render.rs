//! Port of `diagrams/user-journey/journeyRenderer.ts` and `svgDraw.js`
//! (mermaid 12.2.0) to scene elements.
//!
//! `draw` lays the actor legend down the left edge, then one column per
//! task on a fixed grid (`conf.width` wide, `conf.taskMargin` apart) with
//! a section header over each run of tasks in the same section, a dashed
//! line from the task box down to a face whose height is the score, and
//! an arrow along `conf.height * 4` under the boxes. Nothing is measured
//! except the legend names, so the geometry is arithmetic on the config.
//!
//! The box labels are HTML (`textPlacement: fo` puts a `div` in a
//! `foreignObject`), which the browser wraps and centers as a table cell;
//! [`Renderer::html_label`] lays that text out with the measurer and emits
//! one `<text>` per line where the browser puts the baseline.

use std::f64::consts::{PI, TAU};

use crate::mermaid::scene::{
    Baseline, Circle, Common, Element, FontSpec, Group, Length, Line, Marker, MarkerUnits, Orient,
    Path, PathData, Rect, Style, Text, TextAnchor, TextRun, TransformOp,
};
use crate::mermaid::styles::journey::JourneyStyles;
use crate::mermaid::styles::sequence::css_paint;
use crate::text::fonts::{Style as FaceStyle, Weight};
use crate::text::{FontSpec as MeasureSpec, TextMetrics};

use super::conf::{Conf, TextPlacement};
use super::db::{JourneyDb, Task};

/// mermaid-cli renders into `<svg id="my-svg">`; the marker id hangs off it.
pub const DIAGRAM_ID: &str = "my-svg";

/// `drawFace`'s `radius`.
const FACE_RADIUS: f64 = 15.0;
/// `drawTask`'s `maxHeight`: where every task line ends and the lowest
/// face (score 0) sits.
const FACE_BASE_Y: f64 = 300.0 + 5.0 * 30.0;
/// `bounds.insert` for a task reaches down to the same line.
const TASK_STOP_Y: f64 = FACE_BASE_Y;
/// One score step in pixels of face height.
const SCORE_STEP: f64 = 30.0;
/// The legend starts at `yPos = 60` and advances 20 px per line.
const LEGEND_TOP: f64 = 60.0;
const LEGEND_LINE: f64 = 20.0;
/// The viewBox starts 25 px above the drawing and the title adds 70.
const VIEWBOX_TOP: f64 = -25.0;
const TITLE_EXTRA_HEIGHT: f64 = 70.0;

/// Chrome's CSS parser rejects a `font-family` with a stray `;` and leaves
/// the property unset, so the element inherits the root family.
fn css_family_valid(family: &str) -> bool {
    !family.trim().is_empty() && !family.contains(';')
}

/// The `bounds` object of `journeyRenderer.ts`. `sequenceItems` is never
/// pushed to in the journey renderer, so `updateBounds` has nothing to
/// walk and `insert` alone shapes the box.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Bounds {
    startx: Option<f64>,
    stopx: Option<f64>,
    starty: Option<f64>,
    stopy: Option<f64>,
}

fn update_val(slot: &mut Option<f64>, val: f64, pick: fn(f64, f64) -> f64) {
    *slot = Some(match *slot {
        None => val,
        Some(old) => pick(val, old),
    });
}

impl Bounds {
    fn insert(&mut self, startx: f64, starty: f64, stopx: f64, stopy: f64) {
        update_val(&mut self.startx, startx.min(stopx), f64::min);
        update_val(&mut self.starty, starty.min(stopy), f64::min);
        update_val(&mut self.stopx, startx.max(stopx), f64::max);
        update_val(&mut self.stopy, starty.max(stopy), f64::max);
    }
}

/// `actors[name]` in `draw`: the legend color and the sort position.
#[derive(Debug, Clone, PartialEq)]
struct Actor {
    name: String,
    color: String,
    position: usize,
}

fn common(class: &str, style: Style) -> Common {
    Common {
        id: None,
        class: Some(class.to_string()),
        transform: Vec::new(),
        style,
    }
}

fn circle(cx: f64, cy: f64, r: f64, class: Option<&str>, style: Style) -> Element {
    Element::Circle(Circle {
        common: Common {
            class: class.map(str::to_string),
            style,
            ..Common::default()
        },
        cx,
        cy,
        r,
    })
}

fn line(x1: f64, y1: f64, x2: f64, y2: f64, class: Option<&str>, style: Style) -> Element {
    Element::Line(Line {
        common: Common {
            class: class.map(str::to_string),
            style,
            ..Common::default()
        },
        x1,
        y1,
        x2,
        y2,
    })
}

/// d3-shape `arc()` with `innerRadius(r0)`, `outerRadius(r1)`, no padding
/// and no corner radius, for the two mouth arcs: an outer arc, a line to
/// the inner radius, the inner arc back, and a close.
fn annular_arc(r0: f64, r1: f64, start_angle: f64, end_angle: f64) -> PathData {
    let (r0, r1) = if r1 < r0 { (r1, r0) } else { (r0, r1) };
    let a0 = start_angle - PI / 2.0;
    let a1 = end_angle - PI / 2.0;
    let da = (a1 - a0).abs();
    let cw = a1 > a0;
    let mut path = PathData::new();
    path.move_to(r1 * a0.cos(), r1 * a0.sin());
    // `path.arc(0, 0, r1, a0, a1, !cw)`: the current point is already on
    // the arc, so d3 writes the arc command alone.
    path.arc_to(r1, r1, 0.0, da >= PI, cw, r1 * a1.cos(), r1 * a1.sin());
    // `path.arc(0, 0, r0, a1, a0, cw)`: a line to the inner start, then
    // the arc back.
    path.line_to(r0 * a1.cos(), r0 * a1.sin());
    let da_inner = if cw { a1 - a0 } else { a0 - a1 }.rem_euclid(TAU);
    path.arc_to(
        r0,
        r0,
        0.0,
        da_inner >= PI,
        !cw,
        r0 * a0.cos(),
        r0 * a0.sin(),
    );
    path.close();
    path
}

pub struct Renderer<'a> {
    conf: &'a Conf,
    styles: &'a JourneyStyles,
    metrics: &'a TextMetrics<'a>,
    db: &'a JourneyDb,
    /// What a `<text>` or an HTML `div` without its own font renders in:
    /// the stylesheet's root family at the root size.
    root_font: MeasureSpec,
    actors: Vec<Actor>,
}

/// What `draw` leaves behind: the elements in document order, the marker
/// definition, and the `viewBox` as `[x, y, width, height]`.
pub struct Drawn {
    pub children: Vec<Element>,
    pub defs: Vec<Element>,
    pub view_box: [f64; 4],
}

impl<'a> Renderer<'a> {
    pub fn new(
        conf: &'a Conf,
        styles: &'a JourneyStyles,
        metrics: &'a TextMetrics<'a>,
        db: &'a JourneyDb,
    ) -> Renderer<'a> {
        let root_font = MeasureSpec::new(styles.font_family.clone(), styles.font_size as f32);
        // `actors[actorName] = { color, position }` in `getActors()` order.
        let actors = db
            .actors()
            .into_iter()
            .enumerate()
            .map(|(position, name)| Actor {
                color: conf.actor_colours[position % conf.actor_colours.len()].clone(),
                name,
                position,
            })
            .collect();
        Renderer {
            conf,
            styles,
            metrics,
            db,
            root_font,
            actors,
        }
    }

    fn actor(&self, name: &str) -> Option<&Actor> {
        self.actors.iter().find(|a| a.name == name)
    }

    /// `getBoundingClientRect().width` of a `<text>` in the root font.
    fn text_width(&self, text: &str) -> f64 {
        f64::from(self.metrics.svg_bbox(text, &self.root_font).width)
    }

    /// `journeyRenderer.draw`.
    pub fn draw(&self) -> Drawn {
        let conf = self.conf;
        let mut children = Vec::new();
        let mut bounds = Bounds::default();

        let (legend, max_width) = self.draw_actor_legend();
        children.extend(legend);
        let left_margin = conf.left_margin + max_width;
        bounds.insert(0.0, 0.0, left_margin, self.actors.len() as f64 * 50.0);
        children.extend(self.draw_tasks(left_margin, &mut bounds));

        let title = self.db.title();
        if let Some(title) = title {
            children.push(self.draw_title(title, left_margin));
        }

        let startx = bounds.startx.unwrap_or(0.0);
        let starty = bounds.starty.unwrap_or(0.0);
        let stopx = bounds.stopx.unwrap_or(0.0);
        let stopy = bounds.stopy.unwrap_or(0.0);
        let height = stopy - starty + 2.0 * conf.diagram_margin_y;
        let width = left_margin + stopx + 2.0 * conf.diagram_margin_x;

        // The activity line: `conf.height * 4` is one section head, one
        // task and the margins; 4 px off the end keeps the arrow point.
        let mut activity = self.styles.activity_line();
        activity.marker_end = Some(format!("{DIAGRAM_ID}-arrowhead"));
        children.push(line(
            left_margin,
            conf.height * 4.0,
            width - left_margin - 4.0,
            conf.height * 4.0,
            None,
            activity,
        ));

        let extra = if title.is_some() {
            TITLE_EXTRA_HEIGHT
        } else {
            0.0
        };
        Drawn {
            children,
            defs: vec![self.arrowhead()],
            view_box: [startx, VIEWBOX_TOP, width, height + extra],
        }
    }

    /// `svgDraw.initGraphics`: the arrowhead marker on the activity line.
    fn arrowhead(&self) -> Element {
        let mut d = PathData::new();
        d.move_to(0.0, 0.0)
            .line_to(0.0, 4.0)
            .line_to(6.0, 2.0)
            .close();
        Element::Marker(Marker {
            id: format!("{DIAGRAM_ID}-arrowhead"),
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
                common: Common::with_style(self.styles.arrowhead()),
                d,
            })],
        })
    }

    /// `drawActorLegend`: a colored circle and the name per actor, the
    /// name wrapped at `maxLabelWidth`. Returns the elements and
    /// `maxWidth`, the widest name line that is wider than the slack in
    /// `leftMargin`.
    fn draw_actor_legend(&self) -> (Vec<Element>, f64) {
        let conf = self.conf;
        let mut elements = Vec::new();
        let mut max_width = 0.0;
        let mut y_pos = LEGEND_TOP;
        for actor in &self.actors {
            elements.push(circle(
                20.0,
                y_pos,
                7.0,
                Some(&format!("actor-{}", actor.position)),
                self.styles.actor_circle(actor.position, &actor.color),
            ));
            let lines = self.wrap_legend_name(&actor.name);
            for (index, text) in lines.iter().enumerate() {
                let y = y_pos + 7.0 + index as f64 * LEGEND_LINE;
                elements.push(self.legend_text(text, y));
                let line_width = self.text_width(text);
                if line_width > max_width && line_width > conf.left_margin - line_width {
                    max_width = line_width;
                }
            }
            y_pos += LEGEND_LINE.max(lines.len() as f64 * LEGEND_LINE);
        }
        (elements, max_width)
    }

    /// The word wrap in `drawActorLegend`: whole lines while they fit in
    /// `maxLabelWidth`, and a word wider than that broken with hyphens.
    fn wrap_legend_name(&self, person: &str) -> Vec<String> {
        let max = self.conf.max_label_width;
        if self.text_width(person) <= max {
            return vec![person.to_string()];
        }
        let mut lines: Vec<String> = Vec::new();
        let mut current = String::new();
        for word in person.split(' ') {
            let test_line = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            if self.text_width(&test_line) > max {
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                }
                current = word.to_string();
                if self.text_width(word) > max {
                    let mut broken = String::new();
                    for ch in word.chars() {
                        broken.push(ch);
                        if self.text_width(&format!("{broken}-")) > max {
                            let mut kept: String = broken.chars().collect();
                            kept.pop();
                            kept.push('-');
                            lines.push(kept);
                            broken = ch.to_string();
                        }
                    }
                    current = broken;
                }
            } else {
                current = test_line;
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
        lines
    }

    /// `svgDrawCommon.drawText` for a legend line: `x` 40 on the element,
    /// the tspan at `x + textMargin * 2`, class `legend`.
    fn legend_text(&self, text: &str, y: f64) -> Element {
        let x = 40.0;
        Element::Text(Text {
            common: common("legend", self.styles.legend_text()),
            x,
            y,
            anchor: None,
            baseline: None,
            font: Default::default(),
            runs: vec![TextRun {
                text: text.to_string(),
                x: Some(x + self.conf.box_text_margin * 2.0),
                ..TextRun::default()
            }],
        })
    }

    /// `drawTasks(diagram, tasks, 0, id)`.
    fn draw_tasks(&self, left_margin: f64, bounds: &mut Bounds) -> Vec<Element> {
        let conf = self.conf;
        let tasks = &self.db.tasks;
        let mut elements = Vec::new();
        let section_v_height = conf.height * 2.0 + conf.diagram_margin_y;
        let task_pos = section_v_height;

        let mut last_section = "";
        let mut section_number = 0usize;
        let mut fill = "#CCC";
        let mut colour = "black";
        let mut num = 0usize;

        for (i, task) in tasks.iter().enumerate() {
            let x = i as f64 * conf.task_margin + i as f64 * conf.width + left_margin;
            if last_section != task.section {
                fill = &conf.section_fills[section_number % conf.section_fills.len()];
                num = section_number % conf.section_fills.len();
                colour = &conf.section_colours[section_number % conf.section_colours.len()];
                let task_count = tasks[i..]
                    .iter()
                    .take_while(|t| t.section == task.section)
                    .count();
                elements.push(self.draw_section(&task.section, x, fill, num, colour, task_count));
                last_section = &task.section;
                section_number += 1;
            }
            elements.push(self.draw_task(task, x, task_pos, fill, num, colour, i));
            bounds.insert(
                x,
                task_pos,
                x + conf.diagram_margin_x + conf.task_margin,
                TASK_STOP_Y,
            );
        }
        elements
    }

    /// `svgDraw.drawSection`: the header rect over the section's tasks and
    /// its label.
    fn draw_section(
        &self,
        text: &str,
        x: f64,
        fill: &str,
        num: usize,
        colour: &str,
        task_count: usize,
    ) -> Element {
        let conf = self.conf;
        let y = 50.0;
        let width =
            conf.width * task_count as f64 + conf.diagram_margin_x * (task_count as f64 - 1.0);
        let height = conf.height;
        let class = format!("journey-section section-type-{num}");
        let mut children = vec![Element::Rect(Rect {
            common: common(&class, self.styles.note_rect(num, fill)),
            x,
            y,
            width,
            height,
            rx: Some(3.0),
            ry: Some(3.0),
        })];
        children.extend(self.draw_box_text(text, x, y, width, height, &class, colour));
        Element::Group(Group {
            common: Common::default(),
            children,
        })
    }

    /// `svgDraw.drawTask`: the dashed line, the face, the box, the actor
    /// dots on its top edge, and the label.
    #[allow(clippy::too_many_arguments)]
    fn draw_task(
        &self,
        task: &Task,
        x: f64,
        y: f64,
        fill: &str,
        num: usize,
        colour: &str,
        index: usize,
    ) -> Element {
        let conf = self.conf;
        let center = x + conf.width / 2.0;
        let mut children = vec![Element::Line(Line {
            common: Common {
                id: Some(format!("{DIAGRAM_ID}-task{index}")),
                class: Some("task-line".to_string()),
                transform: Vec::new(),
                style: self.styles.task_line(),
            },
            x1: center,
            y1: y,
            x2: center,
            y2: FACE_BASE_Y,
        })];
        children.extend(self.draw_face(center, task.score));
        let class = format!("task task-type-{num}");
        children.push(Element::Rect(Rect {
            common: common(&class, self.styles.note_rect(num, fill)),
            x,
            y,
            width: conf.width,
            height: conf.height,
            rx: Some(3.0),
            ry: Some(3.0),
        }));
        let mut x_pos = x + 14.0;
        for person in &task.people {
            // `taskActors` keeps only the people the legend knows; every
            // name a task lists is in `getActors()`, so this always hits.
            if let Some(actor) = self.actor(person) {
                children.push(circle(
                    x_pos,
                    y,
                    7.0,
                    Some(&format!("actor-{}", actor.position)),
                    self.styles.actor_circle(actor.position, &actor.color),
                ));
            }
            x_pos += 10.0;
        }
        children.extend(self.draw_box_text(
            &task.task,
            x,
            y,
            conf.width,
            conf.height,
            "task",
            colour,
        ));
        Element::Group(Group {
            common: Common::default(),
            children,
        })
    }

    /// `svgDraw.drawFace`: the face circle and a group with the eyes and
    /// the mouth. The face sits at `300 + (5 - score) * 30`; a score that
    /// is not a number makes an invalid `cy`, which the browser reads as 0,
    /// and compares false on both sides, so the mouth is flat.
    fn draw_face(&self, cx: f64, score: f64) -> Vec<Element> {
        let cy = if score.is_nan() {
            0.0
        } else {
            300.0 + (5.0 - score) * SCORE_STEP
        };
        let eye_offset = FACE_RADIUS / 3.0;
        let mut features = vec![
            circle(
                cx - eye_offset,
                cy - eye_offset,
                1.5,
                None,
                self.styles.eye(),
            ),
            circle(
                cx + eye_offset,
                cy - eye_offset,
                1.5,
                None,
                self.styles.eye(),
            ),
        ];
        let inner = FACE_RADIUS / 2.0;
        let outer = FACE_RADIUS / 2.2;
        if score > 3.0 {
            features.push(self.mouth_arc(
                annular_arc(inner, outer, PI / 2.0, 3.0 * PI / 2.0),
                cx,
                cy + 2.0,
            ));
        } else if score < 3.0 {
            features.push(self.mouth_arc(
                annular_arc(inner, outer, 3.0 * PI / 2.0, 5.0 * PI / 2.0),
                cx,
                cy + 7.0,
            ));
        } else {
            features.push(line(
                cx - 5.0,
                cy + 7.0,
                cx + 5.0,
                cy + 7.0,
                Some("mouth"),
                self.styles.mouth_line(),
            ));
        }
        vec![
            circle(cx, cy, FACE_RADIUS, Some("face"), self.styles.face()),
            Element::Group(Group {
                common: Common::default(),
                children: features,
            }),
        ]
    }

    fn mouth_arc(&self, d: PathData, tx: f64, ty: f64) -> Element {
        Element::Path(Path {
            common: Common {
                id: None,
                class: Some("mouth".to_string()),
                transform: vec![TransformOp::Translate(tx, ty)],
                style: self.styles.mouth_path(),
            },
            d,
        })
    }

    /// `_drawTextCandidateFunc(conf)`: the label of a section or task box.
    #[allow(clippy::too_many_arguments)]
    fn draw_box_text(
        &self,
        content: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        class: &str,
        colour: &str,
    ) -> Vec<Element> {
        match self.conf.text_placement {
            TextPlacement::ForeignObject => self.html_label(content, x, y, width, height, class),
            TextPlacement::Old => vec![self.text_by_text(content, x, y, width, height, class)],
            TextPlacement::Tspan => self.text_by_tspan(content, x, y, width, height, class, colour),
        }
    }

    /// `byFo`: a `div.label` in a `display: table` cell that fills the
    /// box. The browser wraps the text at the box width (a longer word
    /// widens the table), centers each line, and places the block in the
    /// middle of the cell; a block taller than the box starts at its top.
    fn html_label(
        &self,
        content: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        class: &str,
    ) -> Vec<Element> {
        let words: Vec<&str> = content.split_whitespace().collect();
        if words.is_empty() {
            return Vec::new();
        }
        let width_of = |text: &str| self.text_width(text);
        let min_content = words.iter().map(|w| width_of(w)).fold(0.0, f64::max);
        let avail = width.max(min_content);
        let mut lines: Vec<String> = Vec::new();
        let mut current = String::new();
        for word in words {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            if current.is_empty() || width_of(&candidate) <= avail {
                current = candidate;
            } else {
                lines.push(std::mem::replace(&mut current, word.to_string()));
            }
        }
        lines.push(current);

        let (ascent, descent, line_gap) = self.root_line_metrics();
        let line_height = ascent + descent + line_gap;
        let block_height = line_height * lines.len() as f64;
        let top = if block_height <= height {
            y + (height - block_height) / 2.0
        } else {
            y
        };
        let center_x = x + avail / 2.0;
        lines
            .iter()
            .enumerate()
            .filter(|(i, _)| top + *i as f64 * line_height < y + height)
            .map(|(i, text)| {
                let baseline = top + i as f64 * line_height + line_gap / 2.0 + ascent;
                Element::Text(Text {
                    common: common(class, self.styles.label_text()),
                    x: center_x,
                    y: baseline,
                    anchor: Some(TextAnchor::Middle),
                    baseline: None,
                    font: Default::default(),
                    runs: vec![TextRun::new(text.clone())],
                })
            })
            .collect()
    }

    /// Chrome's `line-height: normal` for the root font: the rounded
    /// ascent, descent, and line gap of the primary face.
    fn root_line_metrics(&self) -> (f64, f64, f64) {
        let fonts = self.metrics.fonts();
        let size = self.root_font.size;
        let Some(face) = fonts.resolve(&self.root_font.family, Weight::NORMAL, FaceStyle::Normal)
        else {
            return (f64::from(size) * 0.8, f64::from(size) * 0.2, 0.0);
        };
        let (ascent, descent) = self.metrics.face_ascent_descent(face, size);
        let line_gap = fonts
            .with_face(face, |f| {
                let upem = f.units_per_em() as f32;
                (f32::from(f.line_gap()) / upem * size).round()
            })
            .unwrap_or(0.0);
        (f64::from(ascent), f64::from(descent), f64::from(line_gap))
    }

    /// `byText`: one `<text>` 5 px under the box center; `font-color` is
    /// not a CSS property, so the root fill applies.
    fn text_by_text(
        &self,
        content: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        class: &str,
    ) -> Element {
        Element::Text(Text {
            common: common(class, Style::default()),
            x: x + width / 2.0,
            y: y + height / 2.0 + 5.0,
            anchor: Some(TextAnchor::Middle),
            baseline: None,
            font: Default::default(),
            runs: vec![TextRun::new(content)],
        })
    }

    /// `byTspan`: one `<text>` per `<br>` line in `taskFont*`, spread
    /// around the box center with `dominant-baseline: central`.
    #[allow(clippy::too_many_arguments)]
    fn text_by_tspan(
        &self,
        content: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        class: &str,
        colour: &str,
    ) -> Vec<Element> {
        let conf = self.conf;
        let lines = crate::text::markdown::split_breaks(content);
        let n = lines.len() as f64;
        let size = conf.task_font_size;
        let family =
            css_family_valid(&conf.task_font_family).then(|| conf.task_font_family.clone());
        lines
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let dy = i as f64 * size - (size * (n - 1.0)) / 2.0;
                let style = Style {
                    fill: Some(css_paint(colour)),
                    font: FontSpec {
                        family: family.clone(),
                        size: Some(size),
                        ..FontSpec::default()
                    },
                    ..Style::default()
                };
                Element::Text(Text {
                    common: common(class, style),
                    x: x + width / 2.0,
                    y: y + height / 2.0,
                    anchor: Some(TextAnchor::Middle),
                    baseline: Some(Baseline::Central),
                    font: Default::default(),
                    runs: vec![TextRun {
                        text: text.to_string(),
                        x: Some(x + width / 2.0),
                        dy: Some(Length::Px(dy)),
                        ..TextRun::default()
                    }],
                })
            })
            .collect()
    }

    /// The title: `font-size` from `titleFontSize` (`4ex` by default,
    /// resolved against the root font's x-height), bold, at `y` 25.
    fn draw_title(&self, title: &str, left_margin: f64) -> Element {
        let conf = self.conf;
        let size = self.title_font_px(&conf.title_font_size);
        let family =
            css_family_valid(&conf.title_font_family).then(|| conf.title_font_family.clone());
        Element::Text(Text {
            common: Common::with_style(self.styles.title_text(&conf.title_color, size, family)),
            x: left_margin,
            y: 25.0,
            anchor: None,
            baseline: None,
            font: Default::default(),
            runs: vec![TextRun::new(title)],
        })
    }

    /// A CSS `font-size` on the title: `ex` and `em` are relative to the
    /// parent, the svg root in its font at the root size.
    fn title_font_px(&self, value: &str) -> f64 {
        let value = value.trim();
        let root_size = self.styles.font_size;
        let number = |s: &str| s.trim().parse::<f64>().ok();
        if let Some(n) = value.strip_suffix("ex").and_then(number) {
            return n * self.root_x_height();
        }
        if let Some(n) = value.strip_suffix("em").and_then(number) {
            return n * root_size;
        }
        if let Some(n) = value.strip_suffix('%').and_then(number) {
            return n / 100.0 * root_size;
        }
        if let Some(n) = value
            .strip_suffix("px")
            .and_then(number)
            .or_else(|| number(value))
        {
            return n;
        }
        4.0 * self.root_x_height()
    }

    /// The x-height of the root font in pixels: the OS/2 `sxHeight`, else
    /// the height of the `x` glyph, else half the size, as Blink resolves
    /// `ex`.
    fn root_x_height(&self) -> f64 {
        let fonts = self.metrics.fonts();
        let size = f64::from(self.root_font.size);
        let x_height = fonts
            .resolve(&self.root_font.family, Weight::NORMAL, FaceStyle::Normal)
            .and_then(|face| {
                fonts.with_face(face, |f| {
                    let upem = f64::from(f.units_per_em());
                    let units = f.x_height().filter(|h| *h > 0).map(f64::from).or_else(|| {
                        let glyph = f.glyph_index('x')?;
                        f.glyph_bounding_box(glyph).map(|b| f64::from(b.y_max))
                    })?;
                    Some(units / upem * size)
                })
            })
            .flatten();
        x_height.unwrap_or(size / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_smile_is_the_bottom_half_ring() {
        let d = annular_arc(7.5, 15.0 / 2.2, PI / 2.0, 3.0 * PI / 2.0).to_svg();
        // Outer radius 7.5 from the right, clockwise through the bottom,
        // then the inner radius back.
        assert!(d.starts_with("M7.5,0A7.5,7.5,0,1,1,-7.5,"), "{d}");
        assert!(d.contains("A6.818"), "{d}");
        assert!(d.ends_with('Z'), "{d}");
    }

    #[test]
    fn bounds_grow_in_every_direction() {
        let mut b = Bounds::default();
        b.insert(0.0, 0.0, 150.0, 100.0);
        b.insert(150.0, 110.0, 250.0, 450.0);
        assert_eq!(b.startx, Some(0.0));
        assert_eq!(b.stopx, Some(250.0));
        assert_eq!(b.stopy, Some(450.0));
    }
}
