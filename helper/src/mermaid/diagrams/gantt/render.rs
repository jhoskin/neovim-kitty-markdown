//! Port of `ganttRenderer.js` (mermaid 12.2.0) `draw`: the chart height,
//! the time scale, the section rows, the task bars and labels, the grid
//! axis, the section titles, the today line, and the title.
//!
//! The browser gives mermaid one input the helper has to supply itself: the
//! container width `w`. mermaid-cli renders in a viewport of `size + 16`
//! pixels with an 8 px body margin, so `w` is the parity `size`; here it is
//! the target width unless `gantt.useWidth` names one.

use indexmap::IndexMap;
use regex::Regex;
use std::sync::LazyLock;

use super::dates::{self, Unit};
use super::db::{GanttDb, Task};
use crate::error::HelperError;
use crate::mermaid::config::{GanttConfig, Resolved};
use crate::mermaid::diagrams::RenderCtx;
use crate::mermaid::layout::d3::axis::{Axis, AxisLayout, Orient};
use crate::mermaid::layout::d3::scale_time::ScaleTime;
use crate::mermaid::layout::d3::time::{self, Interval, Ms};
use crate::mermaid::layout::d3::time_format;
use crate::mermaid::scene::{
    Common, Element, FontSpec, Group, Line, Path, Rect, Scene, Text, TextRun,
};
use crate::mermaid::styles::gantt::{GanttStyles, LabelFlags, TaskState, TextPlacement};
use crate::text::{FontSpec as MeasureSpec, FontStyle as MeasureStyle};

/// mermaid-cli's SVG id; element ids are `<id>-<task id>`.
const SVG_ID: &str = "my-svg";
const MAX_TICK_COUNT: f64 = 10_000.0;

static TICK_INTERVAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([1-9]\d*)(millisecond|second|minute|hour|day|week|month)$").expect("valid regex")
});
static LINE_BREAK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").expect("valid regex"));

/// XML's default white space handling for `<text>` content: the browser
/// collapses runs of white space and drops it at both ends before layout,
/// so `getBBox().width` and the painted glyphs both see the trimmed string.
pub fn xml_collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// JavaScript `Object.keys` order for string keys: canonical array indices
/// first in numeric order, then the others in insertion order.
fn js_object_keys<'a>(keys: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let keys: Vec<&str> = keys.collect();
    let is_index = |k: &str| {
        (k == "0" || (!k.starts_with('0') && k.chars().all(|c| c.is_ascii_digit())))
            && k.parse::<u64>().is_ok_and(|n| n < u64::from(u32::MAX))
    };
    let mut indices: Vec<&str> = keys.iter().copied().filter(|k| is_index(k)).collect();
    indices.sort_by_key(|k| k.parse::<u64>().unwrap_or(0));
    let mut out = indices;
    out.extend(keys.iter().copied().filter(|k| !is_index(k)));
    out
}

/// `getMaxIntersections(tasks, orderOffset)`: packs a section's tasks into
/// rows for `displayMode: compact` and rewrites each task's `order`.
fn max_intersections(tasks: &mut [Task], order_offset: i64) -> i64 {
    let mut timeline: Vec<Ms> = vec![Ms::MIN; tasks.len()];
    let mut sorted: Vec<usize> = (0..tasks.len()).collect();
    sorted.sort_by(|&a, &b| {
        let (ta, tb) = (&tasks[a], &tasks[b]);
        ta.start_time
            .cmp(&tb.start_time)
            .then(ta.order.cmp(&tb.order))
    });
    let mut max = 0i64;
    for i in sorted {
        for (j, slot) in timeline.iter_mut().enumerate() {
            if tasks[i].start_time >= *slot {
                *slot = tasks[i].end_time;
                tasks[i].order = j as i64 + order_offset;
                if j as i64 > max {
                    max = j as i64;
                }
                break;
            }
        }
    }
    max
}

struct Chart<'a> {
    conf: &'a GanttConfig,
    styles: GanttStyles,
    w: f64,
    h: f64,
    scale: ScaleTime,
    /// `categories`: unique section names of the non-vert tasks.
    categories: Vec<String>,
    /// `categoryHeights` in JavaScript key order.
    category_heights: Vec<(String, i64)>,
    tasks_without_vert: usize,
}

impl Chart<'_> {
    fn sec_num(&self, section: &str) -> usize {
        let styles = (*self.conf.number_section_styles).max(1.0) as usize;
        self.categories
            .iter()
            .position(|c| c == section)
            .map_or(0, |i| i % styles)
    }

    fn time(&self, ms: Ms) -> f64 {
        self.scale.apply(ms)
    }
}

pub fn draw(
    db: &mut GanttDb,
    resolved: &Resolved,
    ctx: &RenderCtx<'_>,
) -> Result<Scene, HelperError> {
    let conf = &resolved.config.gantt;
    let styles = GanttStyles::from_theme(&resolved.theme_variables);
    let mut w = f64::from(ctx.target_width);
    if let Some(use_width) = &conf.use_width {
        w = **use_width;
    }

    let mut task_array = db
        .get_tasks()
        .map_err(|e| HelperError::internal(e.to_string()))?;
    let without_vert: Vec<usize> = (0..task_array.len())
        .filter(|&i| !task_array[i].vert)
        .collect();

    let mut categories: Vec<String> = Vec::new();
    for &i in &without_vert {
        if !categories.contains(&task_array[i].section) {
            categories.push(task_array[i].section.clone());
        }
    }

    let bar_height = *conf.bar_height;
    let bar_gap = *conf.bar_gap;
    let top_padding = *conf.top_padding;
    let mut h = 2.0 * top_padding;
    let compact = db.display_mode() == "compact" || conf.display_mode == "compact";
    let mut category_heights: Vec<(String, i64)> = Vec::new();
    if compact {
        let mut by_section: IndexMap<String, Vec<usize>> = IndexMap::new();
        for &i in &without_vert {
            by_section
                .entry(task_array[i].section.clone())
                .or_default()
                .push(i);
        }
        let order: Vec<String> = js_object_keys(by_section.keys().map(String::as_str))
            .into_iter()
            .map(str::to_string)
            .collect();
        let mut intersections = 0i64;
        for category in order {
            let indices = by_section.get(&category).cloned().unwrap_or_default();
            let mut group: Vec<Task> = indices.iter().map(|&i| task_array[i].clone()).collect();
            let height = max_intersections(&mut group, intersections) + 1;
            for (k, &i) in indices.iter().enumerate() {
                task_array[i].order = group[k].order;
            }
            intersections += height;
            h += height as f64 * (bar_height + bar_gap);
            category_heights.push((category, height));
        }
    } else {
        h += without_vert.len() as f64 * (bar_height + bar_gap);
        for category in &categories {
            let count = without_vert
                .iter()
                .filter(|&&i| task_array[i].section == *category)
                .count() as i64;
            category_heights.push((category.clone(), count));
        }
    }

    let min_start = task_array.iter().map(|t| t.start_time).min();
    let max_end = task_array.iter().map(|t| t.end_time).max();
    let (Some(min_start), Some(max_end)) = (min_start, max_end) else {
        // No tasks: d3 gets `undefined` for both ends and mermaid draws an
        // empty chart with the axis in an invalid state.
        return Ok(empty_chart(w, h, &styles, conf, db, ctx));
    };
    let scale = ScaleTime::new([min_start, max_end], [0.0, 1.0])
        .range_round([0.0, w - *conf.left_padding - *conf.right_padding]);

    // `taskArray.sort(taskCompare)`: by start, stable.
    task_array.sort_by_key(|a| a.start_time);

    let chart = Chart {
        conf,
        styles,
        w,
        h,
        scale,
        categories,
        category_heights,
        tasks_without_vert: without_vert.len(),
    };

    let mut scene = Scene::new(w, h);
    scene.font_family = chart.styles.font_family.clone();
    let root = &mut scene.root.children;

    // `makeGantt`
    let gap = bar_height + bar_gap;
    root.push(draw_exclude_days(&chart, db, &task_array));
    root.extend(make_grid(&chart, db));
    let (sections, tasks) = draw_rects(&chart, db, &mut task_array, ctx);
    root.push(sections);
    root.push(tasks);
    root.push(vert_labels(&chart, gap, ctx));
    if let Some(today) = draw_today(&chart, db) {
        root.push(today);
    }
    root.push(title_text(&chart, db, resolved));
    Ok(scene)
}

/// What `draw` produces when `getTasks()` returns nothing: the title alone
/// is the only thing the browser paints for the `NaN` geometry.
fn empty_chart(
    w: f64,
    h: f64,
    styles: &GanttStyles,
    conf: &GanttConfig,
    db: &GanttDb,
    _ctx: &RenderCtx<'_>,
) -> Scene {
    let mut scene = Scene::new(w, h);
    scene.font_family = styles.font_family.clone();
    let title = xml_collapse(db.diagram_title());
    if !title.is_empty() {
        scene.root.children.push(Element::Text(Text {
            common: Common {
                class: Some("titleText".to_string()),
                style: styles.title_text(),
                ..Common::default()
            },
            x: w / 2.0,
            y: *conf.title_top_margin,
            anchor: None,
            baseline: None,
            font: FontSpec::default(),
            runs: vec![TextRun::new(title)],
        }));
    }
    scene
}

fn group(class: Option<&str>, children: Vec<Element>) -> Element {
    Element::Group(Group {
        common: Common {
            class: class.map(str::to_string),
            ..Common::default()
        },
        children,
    })
}

fn rect(common: Common, x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        common,
        x,
        y,
        width,
        height,
        rx: None,
        ry: None,
    }
}

/// `drawExcludeDays`.
fn draw_exclude_days(chart: &Chart<'_>, db: &GanttDb, tasks: &[Task]) -> Element {
    let excludes = db.excludes();
    let includes = db.includes();
    if excludes.is_empty() && includes.is_empty() {
        return group(None, Vec::new());
    }
    let (Some(min_time), Some(max_time)) = (
        tasks.iter().map(|t| t.start_time).min(),
        tasks.iter().map(|t| t.end_time).max(),
    ) else {
        return group(None, Vec::new());
    };
    if dates::diff_years(max_time, min_time) > 5 {
        // mermaid logs a warning and skips the exclude rects.
        return group(None, Vec::new());
    }
    let date_format = db.date_format();
    let mut ranges: Vec<(Ms, Ms)> = Vec::new();
    let mut range: Option<(Ms, Ms)> = None;
    let mut d = min_time;
    while d <= max_time {
        if db.is_invalid_date(d, date_format, excludes, includes) {
            range = Some(match range {
                None => (d, d),
                Some((start, _)) => (start, d),
            });
        } else if let Some(r) = range.take() {
            ranges.push(r);
        }
        d = dates::add(d, 1.0, Unit::Day);
    }
    let conf = chart.conf;
    let side_pad = *conf.left_padding;
    let children = ranges
        .into_iter()
        .map(|(start, end)| {
            let x = chart.time(dates::start_of_day(start)) + side_pad;
            let width = chart.time(dates::end_of_day(end)) - chart.time(dates::start_of_day(start));
            Element::Rect(rect(
                Common {
                    id: Some(format!(
                        "{SVG_ID}-exclude-{}",
                        dates::format(start, "YYYY-MM-DD")
                    )),
                    class: Some("exclude-range".to_string()),
                    style: chart.styles.exclude_range(),
                    ..Common::default()
                },
                x,
                *conf.grid_line_start_padding,
                width,
                chart.h - *conf.top_padding - *conf.grid_line_start_padding,
            ))
        })
        .collect();
    group(None, children)
}

/// `getEstimatedTickCount`.
fn estimated_tick_count(min_time: Ms, max_time: Ms, every: f64, interval: &str) -> f64 {
    if every <= 0.0 || min_time > max_time {
        return f64::INFINITY;
    }
    let diff = (max_time - min_time) as f64;
    let Some(interval_ms) = dates::duration_ms(interval, every) else {
        return f64::INFINITY;
    };
    if interval_ms <= 0.0 {
        return f64::INFINITY;
    }
    (diff / interval_ms).ceil()
}

/// The `tickInterval` statement or config turned into a d3 interval, or
/// `None` when d3's automatic ticks apply.
fn custom_tick_interval(chart: &Chart<'_>, db: &GanttDb) -> Option<Interval> {
    let text = db
        .tick_interval()
        .filter(|t| !t.is_empty())
        .or(chart.conf.tick_interval.as_deref())
        .unwrap_or("undefined");
    let caps = TICK_INTERVAL_RE.captures(text)?;
    let every: f64 = caps[1].parse().ok()?;
    if every <= 0.0 {
        return None;
    }
    let interval = &caps[2];
    let [min_time, max_time] = chart.scale.domain();
    if estimated_tick_count(min_time, max_time, every, interval) > MAX_TICK_COUNT {
        return None;
    }
    let weekday = db.weekday();
    match interval {
        "millisecond" => time::millisecond().every(every),
        "second" => time::second().every(every),
        "minute" => time::minute().every(every),
        "hour" => time::hour().every(every),
        "day" => time::day().every(every),
        "week" => {
            let i = match weekday {
                "monday" => 1,
                "tuesday" => 2,
                "wednesday" => 3,
                "thursday" => 4,
                "friday" => 5,
                "saturday" => 6,
                _ => 0,
            };
            time::weekday(i).every(every)
        }
        "month" => time::month().every(every),
        _ => None,
    }
}

/// `makeGrid`: the bottom axis, and the top axis when enabled.
fn make_grid(chart: &Chart<'_>, db: &GanttDb) -> Vec<Element> {
    let conf = chart.conf;
    let date_format = db.date_format();
    let user_axis_format = db.axis_format();
    let axis_format: String = if !user_axis_format.is_empty() {
        user_axis_format.to_string()
    } else if date_format == "D" {
        "%d".to_string()
    } else {
        conf.axis_format.clone()
    };
    let tick_size = -chart.h + *conf.top_padding + *conf.grid_line_start_padding;
    let interval = custom_tick_interval(chart, db);
    let values = match &interval {
        Some(interval) => chart.scale.ticks_with(interval),
        None => chart.scale.ticks(None),
    };
    let format = |d: Ms| time_format::format(&axis_format, d);

    let mut out = Vec::new();
    let bottom =
        Axis::new(Orient::Bottom)
            .tick_size(tick_size)
            .layout(&chart.scale, &values, format);
    out.push(axis_element(
        chart,
        &bottom,
        *conf.left_padding,
        chart.h - 50.0,
        // mermaid overrides d3's `dy="0.71em"` with `1em` on the bottom axis.
        Some(1.0),
    ));
    if db.top_axis_enabled() || conf.top_axis {
        let top = Axis::new(Orient::Top)
            .tick_size(tick_size)
            .layout(&chart.scale, &values, format);
        out.push(axis_element(
            chart,
            &top,
            *conf.left_padding,
            *conf.top_padding,
            None,
        ));
    }
    out
}

/// The `g.grid` d3 writes, restyled as mermaid's stylesheet computes it.
fn axis_element(
    chart: &Chart<'_>,
    axis: &AxisLayout,
    tx: f64,
    ty: f64,
    dy_override_em: Option<f64>,
) -> Element {
    let mut children = Vec::new();
    children.push(Element::Path(Path {
        common: Common {
            class: Some("domain".to_string()),
            style: chart.styles.domain_path(),
            ..Common::default()
        },
        d: axis.domain_path.clone(),
    }));
    let dy_em = dy_override_em.unwrap_or(axis.text_dy_em);
    let text_y = axis.text_y + dy_em * axis.font_size;
    for tick in &axis.ticks {
        let line = Element::Line(Line {
            common: Common::with_style(chart.styles.tick_line()),
            x1: 0.0,
            y1: 0.0,
            x2: 0.0,
            y2: axis.line_y2,
        });
        let text = Element::Text(Text {
            common: Common::with_style(chart.styles.tick_text()),
            x: 0.0,
            y: text_y,
            anchor: None,
            baseline: None,
            font: FontSpec::default(),
            runs: vec![TextRun::new(xml_collapse(&tick.label))],
        });
        children.push(Element::Group(Group {
            common: Common {
                class: Some("tick".to_string()),
                transform: vec![crate::mermaid::scene::TransformOp::Translate(
                    tick.position,
                    0.0,
                )],
                style: chart.styles.tick_group(),
                ..Common::default()
            },
            children: vec![line, text],
        }));
    }
    Element::Group(Group {
        common: Common {
            class: Some("grid".to_string()),
            transform: vec![crate::mermaid::scene::TransformOp::Translate(tx, ty)],
            ..Common::default()
        },
        children,
    })
}

/// `drawRects`: the section rows, the task rects, and the task labels.
fn draw_rects(
    chart: &Chart<'_>,
    db: &GanttDb,
    tasks: &mut [Task],
    ctx: &RenderCtx<'_>,
) -> (Element, Element) {
    let conf = chart.conf;
    let gap = *conf.bar_height + *conf.bar_gap;
    let top_pad = *conf.top_padding;
    let side_pad = *conf.left_padding;
    let bar_height = *conf.bar_height;
    let w = chart.w;

    // Vert markers sort last, stable.
    tasks.sort_by_key(|t| t.vert);
    let without_vert: Vec<&Task> = tasks.iter().filter(|t| !t.vert).collect();
    let mut seen_orders: Vec<i64> = Vec::new();
    let mut unique_tasks: Vec<&Task> = Vec::new();
    for task in &without_vert {
        if !seen_orders.contains(&task.order) {
            seen_orders.push(task.order);
            unique_tasks.push(task);
        }
    }
    let section_rects: Vec<Element> = unique_tasks
        .iter()
        .map(|task| {
            let n = chart.sec_num(&task.section);
            Element::Rect(rect(
                Common {
                    class: Some(format!("section section{n}")),
                    style: chart.styles.section_row(n),
                    ..Common::default()
                },
                0.0,
                task.order as f64 * gap + top_pad - 2.0,
                w - *conf.right_padding / 2.0,
                gap,
            ))
        })
        .collect();

    let links = db.links();
    let measure_spec = MeasureSpec::new(chart.styles.font_family.clone(), *conf.font_size as f32);
    let mut rects = Vec::new();
    let mut texts = Vec::new();
    for task in tasks.iter() {
        let n = chart.sec_num(&task.section);
        let start_x = chart.time(task.start_time);
        let end_x = chart.time(task.end_time);
        let render_end_x = chart.time(task.render_end_time.unwrap_or(task.end_time));
        let clickable = task.classes.iter().any(|c| c == "clickable");
        let state = TaskState::of(task.active, task.done, task.crit);

        let x = if task.milestone {
            start_x + side_pad + 0.5 * (end_x - start_x) - 0.5 * bar_height
        } else {
            start_x + side_pad
        };
        let y = if task.vert {
            *conf.grid_line_start_padding
        } else {
            task.order as f64 * gap + top_pad
        };
        let width = if task.milestone {
            bar_height
        } else if task.vert {
            0.08 * bar_height
        } else {
            render_end_x - start_x
        };
        let height = if task.vert {
            chart.tasks_without_vert as f64 * gap + bar_height * 2.0
        } else {
            bar_height
        };
        let origin_x = start_x + side_pad + 0.5 * (end_x - start_x);
        let origin_y = task.order as f64 * gap + top_pad + 0.5 * bar_height;
        let class_state = match state {
            TaskState::Plain => "task",
            TaskState::Active => "active",
            TaskState::Done => "done",
            TaskState::Crit => "crit",
            TaskState::ActiveCrit => "activeCrit",
            TaskState::DoneCrit => "doneCrit",
        };
        let mut class = format!("task {class_state}{n}");
        if task.milestone {
            class = format!("task milestone {class_state}{n}");
        }
        if task.vert {
            class = format!("task vert {class_state}{n}");
        }
        if clickable {
            class.push_str(" clickable");
        }
        let mut common = Common {
            id: Some(format!("{SVG_ID}-{}", task.id)),
            class: Some(class),
            style: chart.styles.task_rect(state, n, task.vert),
            ..Common::default()
        };
        if task.milestone {
            common.transform = GanttStyles::milestone_transform(origin_x, origin_y);
        }
        rects.push(Element::Rect(Rect {
            common,
            x,
            y,
            width,
            height,
            rx: Some(3.0),
            ry: Some(3.0),
        }));

        // The label. `getBBox().width` is measured before any class is set,
        // so the font is the root family at `conf.fontSize`.
        let label = xml_collapse(&task.task);
        let text_width = f64::from(ctx.metrics.svg_bbox(&label, &measure_spec).width);
        let (mut label_start_x, mut label_end_x) = (start_x, render_end_x);
        if task.milestone {
            label_start_x += 0.5 * (end_x - start_x) - 0.5 * bar_height;
            label_end_x = label_start_x + bar_height;
        }
        let text_x = if task.vert {
            start_x + side_pad
        } else if text_width > label_end_x - label_start_x {
            if label_end_x + text_width + 1.5 * *conf.left_padding > w {
                label_start_x + side_pad - 5.0
            } else {
                label_end_x + side_pad + 5.0
            }
        } else {
            (label_end_x - label_start_x) / 2.0 + label_start_x + side_pad
        };
        let text_y = if task.vert {
            *conf.grid_line_start_padding + chart.tasks_without_vert as f64 * gap + 60.0
        } else {
            task.order as f64 * gap + bar_height / 2.0 + (*conf.font_size / 2.0 - 2.0) + top_pad
        };
        // The class branch measures against `endTime`, not `renderEndTime`.
        let class_end_x = if task.milestone {
            start_x + bar_height
        } else {
            end_x
        };
        let placement = if text_width > class_end_x - start_x {
            if class_end_x + text_width + 1.5 * *conf.left_padding > w {
                TextPlacement::OutsideLeft
            } else {
                TextPlacement::OutsideRight
            }
        } else {
            TextPlacement::Inside
        };
        let flags = LabelFlags {
            active: task.active,
            done: task.done,
            crit: task.crit,
            milestone: task.milestone,
            vert: task.vert,
            clickable,
        };
        let placement_class = match placement {
            TextPlacement::Inside => format!("taskText taskText{n}"),
            TextPlacement::OutsideRight => format!("taskTextOutsideRight taskTextOutside{n}"),
            TextPlacement::OutsideLeft => format!("taskTextOutsideLeft taskTextOutside{n}"),
        };
        let _ = links;
        texts.push(Element::Text(Text {
            common: Common {
                id: Some(format!("{SVG_ID}-{}-text", task.id)),
                class: Some(placement_class),
                style: chart.styles.task_text(placement, n, flags),
                ..Common::default()
            },
            x: text_x,
            y: text_y,
            anchor: None,
            baseline: None,
            font: FontSpec {
                size: Some(*conf.font_size),
                ..FontSpec::default()
            },
            runs: vec![TextRun::new(label)],
        }));
    }
    let mut task_children = rects;
    task_children.extend(texts);
    (group(None, section_rects), group(None, task_children))
}

/// `vertLabels`: one `text` per section with a tspan per `<br>` row, each
/// on the `central` baseline.
fn vert_labels(chart: &Chart<'_>, gap: f64, ctx: &RenderCtx<'_>) -> Element {
    let conf = chart.conf;
    let top_pad = *conf.top_padding;
    let font_size = *conf.section_font_size;
    let spec = MeasureSpec::new(chart.styles.font_family.clone(), font_size as f32);
    let mut prev_gap = 0i64;
    let mut children = Vec::new();
    for (i, (name, rows_count)) in chart.category_heights.iter().enumerate() {
        let y = if i > 0 {
            prev_gap += chart.category_heights[i - 1].1;
            (*rows_count as f64 * gap) / 2.0 + prev_gap as f64 * gap + top_pad
        } else {
            (*rows_count as f64 * gap) / 2.0 + top_pad
        };
        let rows: Vec<String> = LINE_BREAK_RE.split(name).map(xml_collapse).collect();
        let dy_em = -((rows.len() as f64) - 1.0) / 2.0;
        let n = chart
            .categories
            .iter()
            .position(|c| c == name)
            .map_or(usize::MAX, |i| {
                i % (*conf.number_section_styles).max(1.0) as usize
            });
        let class = if n == usize::MAX {
            "sectionTitle".to_string()
        } else {
            format!("sectionTitle sectionTitle{n}")
        };
        // `alignment-baseline: central` moves the baseline down by half the
        // em box, the way Blink computes it from the font's ascent and
        // descent.
        let shaped = ctx
            .metrics
            .shape(rows.first().map_or("", String::as_str), &spec);
        let central = f64::from(shaped.ascent - shaped.descent) / 2.0;
        let runs: Vec<TextRun> = rows
            .iter()
            .enumerate()
            .map(|(j, row)| TextRun {
                text: row.clone(),
                x: Some(10.0),
                dy: Some(crate::mermaid::scene::Length::Px(if j == 0 {
                    dy_em * font_size + central
                } else {
                    font_size
                })),
                ..TextRun::default()
            })
            .collect();
        children.push(Element::Text(Text {
            common: Common {
                class: Some(class),
                style: chart.styles.section_title(n),
                ..Common::default()
            },
            x: 10.0,
            y,
            anchor: None,
            baseline: None,
            font: FontSpec {
                size: Some(font_size),
                ..FontSpec::default()
            },
            runs,
        }));
    }
    group(None, children)
}

/// `drawToday`.
fn draw_today(chart: &Chart<'_>, db: &GanttDb) -> Option<Element> {
    let marker = db.today_marker();
    if marker == "off" {
        return None;
    }
    let conf = chart.conf;
    let today = time::now();
    let x = chart.time(today) + *conf.left_padding;
    let inline = marker.replace(',', ";");
    Some(group(
        Some("today"),
        vec![Element::Line(Line {
            common: Common {
                class: Some("today".to_string()),
                style: chart.styles.today_line(&inline),
                ..Common::default()
            },
            x1: x,
            y1: *conf.title_top_margin,
            x2: x,
            y2: chart.h - *conf.title_top_margin,
        })],
    ))
}

/// The `text.titleText` mermaid appends last. The chart's own `title`
/// statement wins over the front matter title, because `Diagram.fromText`
/// sets the front matter title before the parser runs.
fn title_text(chart: &Chart<'_>, db: &GanttDb, resolved: &Resolved) -> Element {
    let title = if db.diagram_title().is_empty() {
        resolved.title().unwrap_or("")
    } else {
        db.diagram_title()
    };
    Element::Text(Text {
        common: Common {
            class: Some("titleText".to_string()),
            style: chart.styles.title_text(),
            ..Common::default()
        },
        x: chart.w / 2.0,
        y: *chart.conf.title_top_margin,
        anchor: None,
        baseline: None,
        font: FontSpec::default(),
        runs: vec![TextRun::new(xml_collapse(title))],
    })
}

/// The font a label is measured with in `drawRects`, for tests.
pub fn label_measure_spec(styles: &GanttStyles, size: f64) -> MeasureSpec {
    MeasureSpec::new(styles.font_family.clone(), size as f32).style(MeasureStyle::Normal)
}
