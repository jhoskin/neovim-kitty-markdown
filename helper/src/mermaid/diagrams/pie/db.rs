//! Port of `pieDb.ts` and the parts of `commonDb.ts` the pie chart reads.

use indexmap::IndexMap;

use super::parser::Ast;
use crate::mermaid::theme::khroma::js_number;

/// The `addSection` error for a negative value, with the line of the
/// section so the plugin can point at it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionError {
    pub message: String,
    pub line: u32,
}

/// `pieDb`: sections in insertion order, `showData`, the diagram title.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PieDb {
    sections: IndexMap<String, f64>,
    show_data: bool,
    title: String,
    acc_title: String,
    acc_descr: String,
}

impl PieDb {
    pub fn new() -> PieDb {
        PieDb::default()
    }

    /// `setDiagramTitle`: `sanitizeText` with DOMPurify, which leaves plain
    /// text as it is; markup in a title is out of scope here.
    pub fn set_diagram_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn diagram_title(&self) -> &str {
        &self.title
    }

    pub fn set_show_data(&mut self, show: bool) {
        self.show_data = show;
    }

    pub fn show_data(&self) -> bool {
        self.show_data
    }

    /// `addSection`: a negative value throws; a repeated label keeps the
    /// first value.
    pub fn add_section(&mut self, label: &str, value: f64, line: u32) -> Result<(), SectionError> {
        if value < 0.0 {
            return Err(SectionError {
                message: format!(
                    "\"{label}\" has invalid value: {}. Negative values are not allowed in pie charts. All slice values must be >= 0.",
                    js_number(value)
                ),
                line,
            });
        }
        self.sections.entry(label.to_string()).or_insert(value);
        Ok(())
    }

    pub fn sections(&self) -> &IndexMap<String, f64> {
        &self.sections
    }

    /// `pieParser.populateDb`: `populateCommonDb` (a falsy title, that is
    /// an empty one, sets nothing), then `showData`, then every section.
    pub fn populate(&mut self, ast: &Ast) -> Result<(), SectionError> {
        if let Some(d) = ast.acc_descr.as_deref().filter(|s| !s.is_empty()) {
            self.acc_descr = d.to_string();
        }
        if let Some(t) = ast.acc_title.as_deref().filter(|s| !s.is_empty()) {
            self.acc_title = t.trim_start().to_string();
        }
        if let Some(t) = ast.title.as_deref().filter(|s| !s.is_empty()) {
            self.set_diagram_title(t);
        }
        self.set_show_data(ast.show_data);
        for s in &ast.sections {
            self.add_section(&s.label, s.value, s.line)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::diagrams::pie::parser::parse;

    #[test]
    fn duplicates_keep_the_first_value_and_order() {
        let mut db = PieDb::new();
        db.populate(&parse("pie\n \"A\" : 1\n \"B\" : 2\n \"A\" : 3\n").unwrap())
            .unwrap();
        let got: Vec<(&str, f64)> = db
            .sections()
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        assert_eq!(got, vec![("A", 1.0), ("B", 2.0)]);
    }

    #[test]
    fn negative_values_throw_the_oracle_message() {
        let mut db = PieDb::new();
        let err = db
            .populate(&parse("pie\n    \"Dogs\" : -5\n").unwrap())
            .unwrap_err();
        assert_eq!(
            err.message,
            "\"Dogs\" has invalid value: -5. Negative values are not allowed in pie charts. All slice values must be >= 0."
        );
        assert_eq!(err.line, 2);
        assert!(db.sections().is_empty());
    }

    #[test]
    fn front_matter_title_loses_to_a_title_in_the_diagram() {
        let mut db = PieDb::new();
        db.set_diagram_title("From front matter");
        db.populate(&parse("pie\n title\n \"A\" : 1\n").unwrap())
            .unwrap();
        assert_eq!(db.diagram_title(), "From front matter");
        db.populate(&parse("pie title In text\n").unwrap()).unwrap();
        assert_eq!(db.diagram_title(), "In text");
    }
}
