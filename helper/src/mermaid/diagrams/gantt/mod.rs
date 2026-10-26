//! The gantt diagram (R-106): `gantt.jison` through the jison driver,
//! `ganttDb.js` as [`db::GanttDb`], `ganttRenderer.js` as [`render::draw`],
//! and `styles.js` in `styles::gantt`.

pub mod actions;
pub mod dates;
pub mod db;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison::{self, ErrorKind, ParseError};
use crate::mermaid::scene::Scene;

pub struct Gantt;

pub static DIAGRAM: Gantt = Gantt;

/// `parser.parse(text)` with a fresh db, as `Diagram.fromText` does.
pub fn parse(code: &str) -> Result<db::GanttDb, ParseError> {
    let grammar = jison::grammar("gantt").map_err(|e| ParseError::action(1, 0, e.to_string()))?;
    let mut db = db::GanttDb::new();
    jison::parse(grammar, code, &mut db)?;
    Ok(db)
}

fn parse_error(err: ParseError) -> HelperError {
    match err.kind {
        // A grammar action threw (`addTask` on bad data): mermaid reports
        // it as the diagram's error, not a parse error.
        ErrorKind::Action => HelperError::internal(err.message),
        ErrorKind::Parse | ErrorKind::Lexical => HelperError::ParseError {
            line: err.line as u32,
            column: err.column as u32,
            diagram_type: "gantt".to_string(),
            message: err.message,
        },
    }
}

impl Diagram for Gantt {
    fn types(&self) -> &'static [&'static str] {
        &["gantt"]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let mut db = parse(resolved.parse_text()).map_err(parse_error)?;
        render::draw(&mut db, resolved, ctx)
    }
}
