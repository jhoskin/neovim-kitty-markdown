//! Port of `ganttDb.js` (mermaid 12.2.0): the statements the grammar
//! actions record and `getTasks`, which resolves every task's start and end.

use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;

use super::dates::{self, Unit};
use crate::mermaid::jison::lexer::js_trim;
use crate::mermaid::layout::d3::time::Ms;

/// `WEEKEND_START_DAY`.
fn weekend_start_day(weekend: &str) -> i64 {
    match weekend {
        "friday" => 5,
        _ => 6,
    }
}

const TAGS: [&str; 5] = ["active", "done", "crit", "milestone", "vert"];

/// What the db throws. The messages are mermaid's; the preview shows them
/// in `:messages` under the `internal` code because they are not parser
/// errors in mermaid either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GanttError {
    /// `getStartDate` with a string neither dayjs nor `Date` understands.
    InvalidDate(String),
    /// `fixTaskDates` ran out of days.
    ExcludesExhausted,
    /// A `TypeError` the JavaScript would raise on malformed task data.
    TypeError(String),
    /// Tasks whose dependencies never resolved after mermaid's ten passes.
    Unresolved(Vec<String>),
}

impl fmt::Display for GanttError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GanttError::InvalidDate(s) => write!(f, "Invalid date:{s}"),
            GanttError::ExcludesExhausted => f.write_str(
                "Failed to find a valid date that was not excluded by `excludes` after 10,000 iterations.",
            ),
            GanttError::TypeError(what) => write!(f, "TypeError: {what}"),
            GanttError::Unresolved(ids) => {
                write!(f, "Gantt: tasks could not be resolved: {}", ids.join(", "))
            }
        }
    }
}

impl std::error::Error for GanttError {}

/// `rawTask.raw.startTime`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StartSpec {
    /// `{ type: 'prevTaskEnd', id }`.
    PrevTaskEnd(Option<String>),
    /// `{ type: 'getStartDate', startData }`.
    GetStartDate(String),
}

#[derive(Debug, Clone, PartialEq)]
struct RawTask {
    id: Option<String>,
    section: String,
    task: String,
    processed: bool,
    manual_end_time: bool,
    render_end_time: Option<Ms>,
    raw_start: Option<StartSpec>,
    raw_end: Option<String>,
    prev_task_id: Option<String>,
    classes: Vec<String>,
    active: bool,
    done: bool,
    crit: bool,
    milestone: bool,
    vert: bool,
    order: i64,
    start_time: Option<Ms>,
    end_time: Option<Ms>,
}

/// A resolved task as the renderer reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub id: String,
    /// `section` and `type` are the same string in mermaid.
    pub section: String,
    /// The label text.
    pub task: String,
    pub start_time: Ms,
    pub end_time: Ms,
    /// `renderEndTime`: the end before `excludes` pushed it out.
    pub render_end_time: Option<Ms>,
    pub classes: Vec<String>,
    pub active: bool,
    pub done: bool,
    pub crit: bool,
    pub milestone: bool,
    pub vert: bool,
    /// The row; the renderer rewrites it in compact mode.
    pub order: i64,
}

#[derive(Debug, Clone, Default)]
pub struct GanttDb {
    date_format: String,
    axis_format: String,
    tick_interval: Option<String>,
    today_marker: String,
    includes: Vec<String>,
    excludes: Vec<String>,
    links: IndexMap<String, String>,
    sections: Vec<String>,
    current_section: String,
    display_mode: String,
    inclusive_end_dates: bool,
    top_axis: bool,
    weekday: String,
    weekend: String,
    last_order: i64,
    title: String,
    acc_title: String,
    acc_description: String,
    raw_tasks: Vec<RawTask>,
    /// `taskDb`: id to position; a later task with the same id wins.
    task_db: HashMap<String, usize>,
    last_task_id: Option<String>,
    task_cnt: usize,
}

static AFTER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^after\s+(?P<ids>[\d\w\- ]+)").expect("valid regex"));
static UNTIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^until\s+(?P<ids>[\d\w\- ]+)").expect("valid regex"));
static DURATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+(?:\.\d+)?)([Mdhmswy]|ms)$").expect("valid regex"));
static TOKEN_SPLIT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\s,]+").expect("valid regex"));

impl GanttDb {
    pub fn new() -> GanttDb {
        GanttDb {
            weekday: "sunday".to_string(),
            weekend: "saturday".to_string(),
            ..GanttDb::default()
        }
    }

    pub fn set_axis_format(&mut self, txt: &str) {
        self.axis_format = txt.to_string();
    }

    pub fn axis_format(&self) -> &str {
        &self.axis_format
    }

    pub fn set_tick_interval(&mut self, txt: &str) {
        self.tick_interval = Some(txt.to_string());
    }

    pub fn tick_interval(&self) -> Option<&str> {
        self.tick_interval.as_deref()
    }

    pub fn set_today_marker(&mut self, txt: &str) {
        self.today_marker = txt.to_string();
    }

    pub fn today_marker(&self) -> &str {
        &self.today_marker
    }

    pub fn set_date_format(&mut self, txt: &str) {
        self.date_format = txt.to_string();
    }

    pub fn date_format(&self) -> &str {
        &self.date_format
    }

    pub fn enable_inclusive_end_dates(&mut self) {
        self.inclusive_end_dates = true;
    }

    pub fn enable_top_axis(&mut self) {
        self.top_axis = true;
    }

    pub fn top_axis_enabled(&self) -> bool {
        self.top_axis
    }

    pub fn set_display_mode(&mut self, txt: &str) {
        self.display_mode = txt.to_string();
    }

    pub fn display_mode(&self) -> &str {
        &self.display_mode
    }

    /// `mergeTokens`: lower-cased, split on spaces and commas, deduplicated
    /// in first-seen order.
    fn merge_tokens(existing: &mut Vec<String>, txt: &str) {
        for token in TOKEN_SPLIT_RE.split(&txt.to_lowercase()) {
            if !token.is_empty() && !existing.iter().any(|e| e == token) {
                existing.push(token.to_string());
            }
        }
    }

    pub fn set_includes(&mut self, txt: &str) {
        Self::merge_tokens(&mut self.includes, txt);
    }

    pub fn includes(&self) -> &[String] {
        &self.includes
    }

    pub fn set_excludes(&mut self, txt: &str) {
        Self::merge_tokens(&mut self.excludes, txt);
    }

    pub fn excludes(&self) -> &[String] {
        &self.excludes
    }

    pub fn links(&self) -> &IndexMap<String, String> {
        &self.links
    }

    pub fn add_section(&mut self, txt: &str) {
        self.current_section = txt.to_string();
        self.sections.push(txt.to_string());
    }

    pub fn set_weekday(&mut self, txt: &str) {
        self.weekday = txt.to_string();
    }

    pub fn weekday(&self) -> &str {
        &self.weekday
    }

    pub fn set_weekend(&mut self, start_day: &str) {
        self.weekend = start_day.to_string();
    }

    /// `setDiagramTitle` from `commonDb`.
    pub fn set_diagram_title(&mut self, txt: &str) {
        self.title = txt.to_string();
    }

    pub fn diagram_title(&self) -> &str {
        &self.title
    }

    pub fn set_acc_title(&mut self, txt: &str) {
        self.acc_title = txt.to_string();
    }

    pub fn set_acc_description(&mut self, txt: &str) {
        self.acc_description = txt.to_string();
    }

    /// `isInvalidDate(date, dateFormat, excludes, includes)`: `date` is a
    /// dayjs in mermaid, a time value here.
    pub fn is_invalid_date(
        &self,
        date: Ms,
        date_format: &str,
        excludes: &[String],
        includes: &[String],
    ) -> bool {
        let formatted = dates::format(date, js_trim(date_format));
        let date_only = dates::format(date, "YYYY-MM-DD");
        let has = |list: &[String], s: &str| list.iter().any(|e| e == s);
        if has(includes, &formatted) || has(includes, &date_only) {
            return false;
        }
        let weekend_start = weekend_start_day(&self.weekend);
        let iso = dates::iso_weekday(date);
        if has(excludes, "weekends") && (iso == weekend_start || iso == weekend_start + 1) {
            return true;
        }
        if has(excludes, &dates::format(date, "dddd").to_lowercase()) {
            return true;
        }
        has(excludes, &formatted) || has(excludes, &date_only)
    }

    fn find_task_by_id(&self, id: &str) -> Option<&RawTask> {
        self.task_db
            .get(id)
            .and_then(|&pos| self.raw_tasks.get(pos))
    }

    /// `getStartDate`: `Ok(None)` when an `after` target has no end yet.
    fn get_start_date(&self, date_format: &str, str_: &str) -> Result<Option<Ms>, GanttError> {
        let s = js_trim(str_);
        let fmt = js_trim(date_format);
        if (fmt == "x" || fmt == "X") && !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
            return Ok(Some(s.parse::<i64>().unwrap_or(i64::MAX)));
        }
        if let Some(caps) = AFTER_RE.captures(s) {
            let mut latest: Option<&RawTask> = None;
            for id in caps["ids"].split(' ').filter(|id| !id.is_empty()) {
                let Some(task) = self.find_task_by_id(id) else {
                    // Unknown ids fall through to today, with a warning in
                    // mermaid's log.
                    continue;
                };
                // `task.endTime > latestTask.endTime` is false while either
                // is still undefined.
                let later = match (task.end_time, latest.and_then(|l| l.end_time)) {
                    (Some(a), Some(b)) => a > b,
                    _ => false,
                };
                if latest.is_none() || later {
                    latest = Some(task);
                }
            }
            return Ok(match latest {
                Some(task) => task.end_time,
                None => Some(dates::today_midnight()),
            });
        }
        if let Some(ms) = dates::parse_strict(s, fmt) {
            return Ok(Some(ms));
        }
        match dates::js_date_parse(s) {
            Some(d) => {
                let year = crate::mermaid::layout::d3::time::civil(d).year;
                if !(-10000..=10000).contains(&year) {
                    return Err(GanttError::InvalidDate(s.to_string()));
                }
                Ok(Some(d))
            }
            None => Err(GanttError::InvalidDate(s.to_string())),
        }
    }

    /// `parseDuration`: `None` is the `NaN` case.
    fn parse_duration(str_: &str) -> Option<(f64, Unit)> {
        let caps = DURATION_RE.captures(js_trim(str_))?;
        let value: f64 = caps[1].parse().ok()?;
        Some((value, Unit::from_shorthand(&caps[2])?))
    }

    /// `getEndDate`: `None` when an `until` target has no start yet.
    fn get_end_date(
        &self,
        prev_time: Ms,
        date_format: &str,
        str_: &str,
        inclusive: bool,
    ) -> Option<Ms> {
        let s = js_trim(str_);
        let fmt = js_trim(date_format);
        if let Some(caps) = UNTIL_RE.captures(s) {
            let mut earliest: Option<&RawTask> = None;
            for id in caps["ids"].split(' ').filter(|id| !id.is_empty()) {
                let Some(task) = self.find_task_by_id(id) else {
                    continue;
                };
                let earlier = match (task.start_time, earliest.and_then(|e| e.start_time)) {
                    (Some(a), Some(b)) => a < b,
                    _ => false,
                };
                if earliest.is_none() || earlier {
                    earliest = Some(task);
                }
            }
            return match earliest {
                Some(task) => task.start_time,
                None => Some(dates::today_midnight()),
            };
        }
        if let Some(parsed) = dates::parse_strict(s, fmt) {
            return Some(if inclusive {
                dates::add(parsed, 1.0, Unit::Day)
            } else {
                parsed
            });
        }
        let mut end_time = prev_time;
        if let Some((value, unit)) = Self::parse_duration(s) {
            end_time = dates::add(end_time, value, unit);
        }
        // A duration that is not one logs a warning and keeps a zero length.
        Some(end_time)
    }

    fn parse_id(&mut self, id: Option<&str>) -> String {
        match id {
            Some(id) => id.to_string(),
            None => {
                self.task_cnt += 1;
                format!("task{}", self.task_cnt)
            }
        }
    }

    /// `getTaskTags`: strips leading tag words from the data list.
    fn take_tags(data: &mut Vec<String>, task: &mut RawTask) -> Result<(), GanttError> {
        let mut match_found = true;
        while match_found {
            match_found = false;
            for tag in TAGS {
                let Some(first) = data.first() else {
                    return Err(GanttError::TypeError(
                        "Cannot read properties of undefined (reading 'match')".to_string(),
                    ));
                };
                if js_trim(first) == tag {
                    match tag {
                        "active" => task.active = true,
                        "done" => task.done = true,
                        "crit" => task.crit = true,
                        "milestone" => task.milestone = true,
                        _ => task.vert = true,
                    }
                    data.remove(0);
                    match_found = true;
                }
            }
        }
        Ok(())
    }

    /// `addTask(descr, data)` with `parseData`.
    pub fn add_task(&mut self, descr: &str, data: &str) -> Result<(), GanttError> {
        let ds = data.strip_prefix(':').unwrap_or(data);
        let mut fields: Vec<String> = ds.split(',').map(str::to_string).collect();
        let mut task = RawTask {
            id: None,
            section: self.current_section.clone(),
            task: descr.to_string(),
            processed: false,
            manual_end_time: false,
            render_end_time: None,
            raw_start: None,
            raw_end: None,
            prev_task_id: self.last_task_id.clone(),
            classes: Vec::new(),
            active: false,
            done: false,
            crit: false,
            milestone: false,
            vert: false,
            order: 0,
            start_time: None,
            end_time: None,
        };
        Self::take_tags(&mut fields, &mut task)?;
        let fields: Vec<String> = fields.iter().map(|f| js_trim(f).to_string()).collect();
        match fields.len() {
            1 => {
                task.id = Some(self.parse_id(None));
                task.raw_start = Some(StartSpec::PrevTaskEnd(self.last_task_id.clone()));
                task.raw_end = Some(fields[0].clone());
            }
            2 => {
                task.id = Some(self.parse_id(None));
                task.raw_start = Some(StartSpec::GetStartDate(fields[0].clone()));
                task.raw_end = Some(fields[1].clone());
            }
            3 => {
                task.id = Some(self.parse_id(Some(&fields[0])));
                task.raw_start = Some(StartSpec::GetStartDate(fields[1].clone()));
                task.raw_end = Some(fields[2].clone());
            }
            _ => {}
        }
        if task.vert {
            task.order = -1;
        } else {
            task.order = self.last_order;
            self.last_order += 1;
        }
        let pos = self.raw_tasks.len();
        self.last_task_id = task.id.clone();
        if let Some(id) = &task.id {
            self.task_db.insert(id.clone(), pos);
        }
        self.raw_tasks.push(task);
        Ok(())
    }

    /// `setClass(ids, className)`.
    fn set_class(&mut self, ids: &str, class_name: &str) {
        for id in ids.split(',') {
            if let Some(&pos) = self.task_db.get(id)
                && let Some(task) = self.raw_tasks.get_mut(pos)
            {
                task.classes.push(class_name.to_string());
            }
        }
    }

    /// `setLink(ids, url)`: records the link and marks the tasks clickable.
    pub fn set_link(&mut self, ids: &str, link: &str) {
        for id in ids.split(',') {
            if self.task_db.contains_key(id) {
                self.links.insert(id.to_string(), link.to_string());
            }
        }
        self.set_class(ids, "clickable");
    }

    /// `setClickEvent(ids, functionName, functionArgs)`: callbacks never run
    /// here; the tasks still become clickable.
    pub fn set_click_event(&mut self, ids: &str, _function_name: &str, _args: Option<&str>) {
        self.set_class(ids, "clickable");
    }

    /// `checkTaskDates`: pushes the end past excluded days.
    fn check_task_dates(&self, pos: usize) -> Result<(Option<Ms>, Option<Ms>), GanttError> {
        let task = &self.raw_tasks[pos];
        if self.excludes.is_empty() || task.manual_end_time {
            return Ok((task.end_time, task.render_end_time));
        }
        let (Some(start), Some(end)) = (task.start_time, task.end_time) else {
            return Ok((task.end_time, task.render_end_time));
        };
        let start_time = dates::add(start, 1.0, Unit::Day);
        let (fixed, render) = self.fix_task_dates(start_time, end)?;
        Ok((Some(fixed), render))
    }

    /// `fixTaskDates`.
    fn fix_task_dates(&self, start: Ms, end: Ms) -> Result<(Ms, Option<Ms>), GanttError> {
        let mut start_time = start;
        let mut end_time = end;
        let mut invalid = false;
        let mut render_end_time = None;
        let mut extensions = 0u32;
        while start_time <= end_time {
            if !invalid {
                render_end_time = Some(end_time);
            }
            invalid = self.is_invalid_date(
                start_time,
                &self.date_format,
                &self.excludes,
                &self.includes,
            );
            if invalid {
                end_time = dates::add(end_time, 1.0, Unit::Day);
                extensions += 1;
                if extensions > 10_000 {
                    return Err(GanttError::ExcludesExhausted);
                }
            }
            start_time = dates::add(start_time, 1.0, Unit::Day);
        }
        Ok((end_time, render_end_time))
    }

    /// One `compileTask(pos)`; returns `processed`.
    fn compile_task(&mut self, pos: usize) -> Result<bool, GanttError> {
        let raw_start = self.raw_tasks[pos].raw_start.clone();
        let Some(raw_start) = raw_start else {
            return Err(GanttError::TypeError(
                "Cannot read properties of undefined (reading 'type')".to_string(),
            ));
        };
        match raw_start {
            StartSpec::PrevTaskEnd(prev_id) => {
                let prev = prev_id.as_deref().and_then(|id| self.find_task_by_id(id));
                let Some(prev) = prev else {
                    return Err(GanttError::TypeError(
                        "Cannot read properties of undefined (reading 'endTime')".to_string(),
                    ));
                };
                let end = prev.end_time;
                self.raw_tasks[pos].start_time = end;
            }
            StartSpec::GetStartDate(data) => {
                if let Some(start) = self.get_start_date(&self.date_format, &data)? {
                    self.raw_tasks[pos].start_time = Some(start);
                }
            }
        }
        let Some(start) = self.raw_tasks[pos].start_time else {
            return Ok(self.raw_tasks[pos].processed);
        };
        let raw_end = self.raw_tasks[pos].raw_end.clone().unwrap_or_default();
        let end = self.get_end_date(start, &self.date_format, &raw_end, self.inclusive_end_dates);
        self.raw_tasks[pos].end_time = end;
        if end.is_some() {
            self.raw_tasks[pos].processed = true;
            self.raw_tasks[pos].manual_end_time =
                dates::parse_strict(&raw_end, "YYYY-MM-DD").is_some();
            let (fixed_end, render_end) = self.check_task_dates(pos)?;
            self.raw_tasks[pos].end_time = fixed_end;
            self.raw_tasks[pos].render_end_time = render_end;
        }
        Ok(self.raw_tasks[pos].processed)
    }

    fn compile_tasks(&mut self) -> Result<bool, GanttError> {
        let mut all_processed = true;
        for pos in 0..self.raw_tasks.len() {
            let processed = self.compile_task(pos)?;
            all_processed = all_processed && processed;
        }
        Ok(all_processed)
    }

    /// `getTasks()`: up to eleven compile passes, then the resolved tasks in
    /// source order.
    pub fn get_tasks(&mut self) -> Result<Vec<Task>, GanttError> {
        let mut all = self.compile_tasks()?;
        let mut iterations = 0;
        while !all && iterations < 10 {
            all = self.compile_tasks()?;
            iterations += 1;
        }
        let unresolved: Vec<String> = self
            .raw_tasks
            .iter()
            .filter(|t| !t.processed)
            .map(|t| t.id.clone().unwrap_or_default())
            .collect();
        if !unresolved.is_empty() {
            return Err(GanttError::Unresolved(unresolved));
        }
        Ok(self
            .raw_tasks
            .iter()
            .filter_map(|t| {
                Some(Task {
                    id: t.id.clone()?,
                    section: t.section.clone(),
                    task: t.task.clone(),
                    start_time: t.start_time?,
                    end_time: t.end_time?,
                    render_end_time: t.render_end_time,
                    classes: t.classes.clone(),
                    active: t.active,
                    done: t.done,
                    crit: t.crit,
                    milestone: t.milestone,
                    vert: t.vert,
                    order: t.order,
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::d3::time::make;

    fn d(y: i64, m: i64, day: i64) -> Ms {
        make(y, m, day, 0, 0, 0, 0)
    }

    fn db_with(tasks: &[(&str, &str)]) -> GanttDb {
        let mut db = GanttDb::new();
        db.set_date_format("YYYY-MM-DD");
        for (name, data) in tasks {
            db.add_task(name, data).expect("task data parses");
        }
        db
    }

    #[test]
    fn ids_starts_and_durations() {
        let mut db = db_with(&[
            ("A task", ":a1, 2014-01-01, 30d"),
            ("Another task", ":after a1, 20d"),
            ("Task in Another", ":2014-01-12, 12d"),
            ("another task", ":24d"),
        ]);
        let tasks = db.get_tasks().expect("resolves");
        let ids: Vec<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["a1", "task1", "task2", "task3"]);
        assert_eq!(
            (tasks[0].start_time, tasks[0].end_time),
            (d(2014, 0, 1), d(2014, 0, 31))
        );
        assert_eq!(
            (tasks[1].start_time, tasks[1].end_time),
            (d(2014, 0, 31), d(2014, 1, 20))
        );
        assert_eq!(tasks[3].start_time, tasks[2].end_time);
        assert_eq!(tasks[3].end_time, d(2014, 1, 17));
        assert_eq!(
            tasks.iter().map(|t| t.order).collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
    }

    #[test]
    fn after_until_and_tags() {
        let mut db = db_with(&[
            ("apple", ":a, 2017-07-20, 1w"),
            ("banana", ":crit, b, 2017-07-23, 1d"),
            ("cherry", ":active, c, after b a, 1d"),
            ("kiwi", ":d, 2017-07-20, until b c"),
        ]);
        let tasks = db.get_tasks().expect("resolves");
        assert!(tasks[1].crit && tasks[2].active);
        assert_eq!(tasks[2].start_time, d(2017, 6, 27));
        assert_eq!(tasks[3].end_time, d(2017, 6, 23));
    }

    #[test]
    fn excludes_push_the_end_and_mark_render_end() {
        let mut db = GanttDb::new();
        db.set_date_format("YYYY-MM-DD");
        db.set_excludes("weekends");
        db.add_task("Active", ":active, des2, 2014-01-09, 3d")
            .expect("parses");
        let tasks = db.get_tasks().expect("resolves");
        // Thursday the 9th plus 3 days skips the weekend: ends Tuesday, and
        // `renderEndTime` follows the end while the day before was valid.
        assert_eq!(tasks[0].end_time, d(2014, 0, 14));
        assert_eq!(tasks[0].render_end_time, Some(d(2014, 0, 14)));
        assert!(db.is_invalid_date(d(2014, 0, 11), "YYYY-MM-DD", db.excludes(), db.includes()));
        db.set_weekend("friday");
        assert!(db.is_invalid_date(d(2014, 0, 10), "YYYY-MM-DD", db.excludes(), db.includes()));
        assert!(!db.is_invalid_date(d(2014, 0, 12), "YYYY-MM-DD", db.excludes(), db.includes()));
    }

    #[test]
    fn manual_end_dates_are_not_pushed() {
        let mut db = GanttDb::new();
        db.set_date_format("YYYY-MM-DD");
        db.set_excludes("weekends");
        db.add_task("Done", ":done, des1, 2014-01-10, 2014-01-12")
            .expect("parses");
        let tasks = db.get_tasks().expect("resolves");
        assert_eq!(tasks[0].end_time, d(2014, 0, 12));
        assert_eq!(tasks[0].render_end_time, None);
    }

    #[test]
    fn errors_match_mermaid() {
        let mut db = db_with(&[("Bad", ":x1, nope, 3d")]);
        assert_eq!(db.get_tasks().unwrap_err().to_string(), "Invalid date:nope");
        let mut db = GanttDb::new();
        assert!(db.add_task("Only tags", ":milestone").is_err());
        db.add_task("First", ":3d").expect("parses");
        assert!(matches!(db.get_tasks(), Err(GanttError::TypeError(_))));
    }

    #[test]
    fn links_and_clicks_mark_tasks_clickable() {
        let mut db = db_with(&[("Task", ":t1, 2020-02-03, 5d")]);
        db.set_link("t1,missing", "https://example.com");
        db.set_click_event("t1", "cb", None);
        let tasks = db.get_tasks().expect("resolves");
        assert_eq!(tasks[0].classes, ["clickable", "clickable"]);
        assert_eq!(
            db.links().get("t1").map(String::as_str),
            Some("https://example.com")
        );
        assert!(!db.links().contains_key("missing"));
    }

    #[test]
    fn vert_tasks_take_no_row_and_tokens_merge() {
        let mut db = db_with(&[("v", ":vert, v1, 2020-02-03, 1d"), ("t", ":2020-02-03, 2d")]);
        db.set_includes("2020-02-05, 2020-02-06");
        db.set_includes("2020-02-05 Monday");
        assert_eq!(db.includes(), ["2020-02-05", "2020-02-06", "monday"]);
        let tasks = db.get_tasks().expect("resolves");
        assert_eq!(tasks[0].order, -1);
        assert_eq!(tasks[1].order, 0);
    }
}
