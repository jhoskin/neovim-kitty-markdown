//! Port of `timelineDb.js` and the `commonDb.ts` fields it exposes through
//! `getCommonDb()` (mermaid 12.2.0).
//!
//! `getTasks` runs `compileTasks` up to 100 times, but nothing ever sets
//! `rawTask.processed`, so the loop changes nothing and the result is the
//! raw tasks in insertion order. `tasks()` returns them directly.

use std::fmt;

/// A period with its events: `addTask`'s `rawTask`.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub id: usize,
    /// The section the period was declared in, `''` before any `section`.
    pub section: String,
    /// The period text as the lexer matched it (`[^#:\n]+`), surrounding
    /// spaces included.
    pub task: String,
    /// `length ? length : 0`; the grammar always passes 0.
    pub score: f64,
    pub events: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineError(pub String);

impl fmt::Display for TimelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimelineDb {
    current_section: String,
    current_task_id: usize,
    direction: String,
    sections: Vec<String>,
    raw_tasks: Vec<Task>,
    diagram_title: String,
    acc_title: String,
    acc_description: String,
}

impl TimelineDb {
    /// The state after `clear()`.
    pub fn new() -> TimelineDb {
        TimelineDb {
            direction: "LR".to_string(),
            ..TimelineDb::default()
        }
    }

    pub fn set_direction(&mut self, dir: &str) {
        self.direction = dir.to_string();
    }

    pub fn direction(&self) -> &str {
        &self.direction
    }

    /// `addSection`: every `section` statement is pushed, duplicates too.
    pub fn add_section(&mut self, txt: &str) {
        self.current_section = txt.to_string();
        self.sections.push(txt.to_string());
    }

    pub fn sections(&self) -> &[String] {
        &self.sections
    }

    /// `getTasks()`.
    pub fn tasks(&self) -> &[Task] {
        &self.raw_tasks
    }

    /// `addTask(period, length, event)`.
    pub fn add_task(&mut self, period: &str, length: f64, event: &str) {
        let task = Task {
            id: self.current_task_id,
            section: self.current_section.clone(),
            task: period.to_string(),
            score: if length != 0.0 { length } else { 0.0 },
            events: if event.is_empty() {
                Vec::new()
            } else {
                vec![event.to_string()]
            },
        };
        self.current_task_id += 1;
        self.raw_tasks.push(task);
    }

    /// `addEvent`: appends to the task with id `currentTaskId - 1`. Before
    /// any period that lookup is `undefined` and mermaid throws while
    /// reading `.events`.
    pub fn add_event(&mut self, event: &str) -> Result<(), TimelineError> {
        let wanted = self.current_task_id.checked_sub(1);
        let current = wanted.and_then(|id| self.raw_tasks.iter_mut().find(|t| t.id == id));
        match current {
            Some(task) => {
                task.events.push(event.to_string());
                Ok(())
            }
            None => Err(TimelineError(
                "TypeError: Cannot read properties of undefined (reading 'events')".to_string(),
            )),
        }
    }

    /// `commonDb.setDiagramTitle`. `sanitizeText` leaves plain text as it
    /// is; HTML in a title is not sanitized here.
    pub fn set_diagram_title(&mut self, txt: &str) {
        self.diagram_title = txt.to_string();
    }

    pub fn diagram_title(&self) -> &str {
        &self.diagram_title
    }

    /// `commonDb.setAccTitle`: leading whitespace removed.
    pub fn set_acc_title(&mut self, txt: &str) {
        self.acc_title = txt.trim_start().to_string();
    }

    pub fn acc_title(&self) -> &str {
        &self.acc_title
    }

    /// `commonDb.setAccDescription`: `\n` followed by whitespace becomes a
    /// bare `\n`.
    pub fn set_acc_description(&mut self, txt: &str) {
        let mut out = String::with_capacity(txt.len());
        let mut chars = txt.chars().peekable();
        while let Some(c) = chars.next() {
            out.push(c);
            if c == '\n' {
                while chars.peek().is_some_and(|n| n.is_whitespace()) {
                    chars.next();
                }
            }
        }
        self.acc_description = out;
    }

    pub fn acc_description(&self) -> &str {
        &self.acc_description
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tasks_keep_their_section_and_events() {
        let mut db = TimelineDb::new();
        db.add_task("2002 ", 0.0, "");
        db.add_event("LinkedIn").expect("has a task");
        db.add_section("Later");
        db.add_task("2004 ", 0.0, "");
        db.add_event("Facebook").expect("has a task");
        db.add_event("Google").expect("has a task");
        let tasks = db.tasks();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].section, "");
        assert_eq!(tasks[0].events, vec!["LinkedIn"]);
        assert_eq!(tasks[1].section, "Later");
        assert_eq!(tasks[1].events, vec!["Facebook", "Google"]);
        assert_eq!(db.sections(), ["Later"]);
    }

    #[test]
    fn event_before_any_period_is_an_error() {
        let mut db = TimelineDb::new();
        assert!(db.add_event("x").is_err());
    }

    #[test]
    fn acc_description_collapses_indented_lines() {
        let mut db = TimelineDb::new();
        db.set_acc_description("one\n   two\n\tthree");
        assert_eq!(db.acc_description(), "one\ntwo\nthree");
        db.set_acc_title("  t");
        assert_eq!(db.acc_title(), "t");
    }
}
