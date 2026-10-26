//! The pie chart: `pieDiagram.ts` with its langium parser, db, renderer,
//! and styles ported from mermaid 12.2.0.

pub mod db;
pub mod parser;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::scene::Scene;

pub struct Pie;

pub const DIAGRAM: Pie = Pie;

/// The detector id in `pieDetector.ts`.
const TYPE: &str = "pie";

impl Diagram for Pie {
    fn types(&self) -> &'static [&'static str] {
        &[TYPE]
    }

    /// `Diagram.fromText` followed by `renderer.draw`: the front matter
    /// title is set before the parser runs, so a `title` line in the text
    /// wins, and the text the parser sees ends with the newline
    /// `fromText` appends.
    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let text = resolved.parse_text();
        let ast = parser::parse(text).map_err(|f| HelperError::ParseError {
            line: f.line,
            column: f.column,
            diagram_type: TYPE.to_string(),
            message: f.message,
        })?;
        let mut db = db::PieDb::new();
        if let Some(title) = resolved.title() {
            db.set_diagram_title(title);
        }
        db.populate(&ast).map_err(|e| HelperError::ParseError {
            line: e.line,
            column: 0,
            diagram_type: TYPE.to_string(),
            message: e.message,
        })?;
        Ok(render::draw(&db, resolved, ctx))
    }
}
