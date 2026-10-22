//! Port of `diagrams/user-journey/journeyDb.js` (mermaid 12.2.0).
//!
//! The grammar actions call the db directly (`yy.addSection`,
//! `yy.addTask`, `yy.setDiagramTitle`), so there is no statement list to
//! apply afterwards; [`JourneyDb`] holds the sections and tasks in source
//! order and derives the actors the way `updateActors` does.

use std::cmp::Ordering;

use crate::mermaid::directive::js_trim;

/// One `rawTask` entry: `addTask(descr, taskData)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub section: String,
    pub people: Vec<String>,
    pub task: String,
    /// `Number(pieces[0])`: `NaN` when the score is not a number, which
    /// the renderer treats as JavaScript does.
    pub score: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct JourneyDb {
    current_section: String,
    pub sections: Vec<String>,
    pub tasks: Vec<Task>,
    /// `commonDb.setDiagramTitle`; the front matter title arrives first and
    /// a `title` statement replaces it.
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_description: Option<String>,
}

/// `Number(text)` for the strings the lexer hands over: whitespace-only
/// is `0`, otherwise a decimal literal or `NaN`.
pub fn js_number(text: &str) -> f64 {
    let trimmed = js_trim(text);
    if trimmed.is_empty() {
        return 0.0;
    }
    match trimmed {
        "Infinity" | "+Infinity" => f64::INFINITY,
        "-Infinity" => f64::NEG_INFINITY,
        _ => {
            if let Some(hex) = trimmed
                .strip_prefix("0x")
                .or_else(|| trimmed.strip_prefix("0X"))
            {
                return u64::from_str_radix(hex, 16).map_or(f64::NAN, |v| v as f64);
            }
            trimmed.parse::<f64>().unwrap_or(f64::NAN)
        }
    }
}

/// `Array.prototype.sort()` with no comparator: by UTF-16 code units.
fn js_string_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

impl JourneyDb {
    pub fn new(front_matter_title: Option<&str>) -> JourneyDb {
        JourneyDb {
            title: front_matter_title
                .filter(|t| !t.is_empty())
                .map(str::to_string),
            ..JourneyDb::default()
        }
    }

    /// `addSection(txt)`.
    pub fn add_section(&mut self, text: &str) {
        self.current_section = text.to_string();
        self.sections.push(text.to_string());
    }

    /// `addTask(descr, taskData)`: `taskData` still carries its leading
    /// `:`; the score and the comma list of people follow.
    pub fn add_task(&mut self, descr: &str, task_data: &str) {
        let body: String = task_data.chars().skip(1).collect();
        let pieces: Vec<&str> = body.split(':').collect();
        let score = js_number(pieces[0]);
        let people: Vec<String> = match pieces.get(1) {
            None => Vec::new(),
            Some(list) => list.split(',').map(|s| js_trim(s).to_string()).collect(),
        };
        self.tasks.push(Task {
            section: self.current_section.clone(),
            people,
            task: descr.to_string(),
            score,
        });
    }

    pub fn set_diagram_title(&mut self, title: &str) {
        self.title = Some(title.to_string());
    }

    pub fn set_acc_title(&mut self, title: &str) {
        self.acc_title = Some(title.to_string());
    }

    pub fn set_acc_description(&mut self, description: &str) {
        self.acc_description = Some(description.to_string());
    }

    /// `getDiagramTitle()` as `draw` tests it: a non-empty string.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref().filter(|t| !t.is_empty())
    }

    /// `getActors()`: every person named by a task, once, in
    /// `Array.prototype.sort` order.
    pub fn actors(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for task in &self.tasks {
            for person in &task.people {
                if !names.contains(person) {
                    names.push(person.clone());
                }
            }
        }
        names.sort_by(|a, b| js_string_cmp(a, b));
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_splits_score_and_people() {
        let mut db = JourneyDb::default();
        db.add_section("Go to work");
        db.add_task("Do work", ": 1: Me, Cat");
        db.add_task("Rest", ": 3");
        assert_eq!(db.tasks[0].section, "Go to work");
        assert_eq!(db.tasks[0].score, 1.0);
        assert_eq!(db.tasks[0].people, vec!["Me", "Cat"]);
        assert!(db.tasks[1].people.is_empty());
        assert_eq!(db.tasks[1].score, 3.0);
    }

    #[test]
    fn actors_are_unique_and_sorted() {
        let mut db = JourneyDb::default();
        db.add_task("a", ": 5: Me, Cat");
        db.add_task("b", ": 5: Cat, Alice");
        assert_eq!(db.actors(), vec!["Alice", "Cat", "Me"]);
    }

    #[test]
    fn a_bad_score_is_nan() {
        assert!(js_number("five").is_nan());
        assert_eq!(js_number("  "), 0.0);
        assert_eq!(js_number(" 4 "), 4.0);
    }

    #[test]
    fn the_title_statement_replaces_the_front_matter_title() {
        let mut db = JourneyDb::new(Some("From front matter"));
        assert_eq!(db.title(), Some("From front matter"));
        db.set_diagram_title("My working day");
        assert_eq!(db.title(), Some("My working day"));
    }
}
