//! The class diagram: mermaid 12.2.0's `diagrams/class` through the unified
//! renderer and dagre.
//!
//! `actions` replays `classDiagram.jison` over the vendored tables and
//! makes the `yy` calls into `db` (`classDb.ts`, `classTypes.ts`), `render`
//! is `classRenderer-v3-unified.ts`, and `styles::class` holds the class
//! stylesheet rules the shared renderer does not resolve. Both
//! `classDiagram` and `classDiagram-v2` detect as `classDiagram`.

pub mod actions;
pub mod db;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison;
use crate::mermaid::scene::Scene;

use actions::ClassActions;
use db::ClassDb;

pub struct Class;

pub static DIAGRAM: Class = Class;

/// Parses the diagram text into the db, as `parser.parse` with the db as
/// `yy` does. Public for tests that look at the db.
pub fn parse(resolved: &Resolved) -> Result<ClassDb, HelperError> {
    let grammar = jison::grammar("classDiagram")
        .map_err(|e| HelperError::internal(format!("class grammar: {e}")))?;
    let mut actions = ClassActions::new();
    jison::parse(grammar, resolved.parse_text(), &mut actions).map_err(|e| {
        HelperError::ParseError {
            line: e.line as u32,
            column: e.column as u32,
            diagram_type: "classDiagram".to_string(),
            message: e.to_string(),
        }
    })?;
    Ok(actions.db)
}

impl Diagram for Class {
    fn types(&self) -> &'static [&'static str] {
        &["classDiagram"]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let db = parse(resolved)?;
        render::draw(&db, resolved, ctx)
    }
}
