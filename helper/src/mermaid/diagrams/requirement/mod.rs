//! The requirement diagram: mermaid 12.2.0's `diagrams/requirement`
//! through the unified renderer and dagre.
//!
//! `actions` replays `requirementDiagram.jison` over the vendored tables
//! and makes the `yy` calls into `db` (`requirementDb.ts`), `render` is
//! `requirementRenderer.ts`, and `styles::requirement` holds the
//! stylesheet rules the shared renderer does not resolve. The boxes are the
//! shared `requirementBox` shape.

pub mod actions;
pub mod db;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison;
use crate::mermaid::scene::Scene;

use actions::RequirementActions;
use db::RequirementDb;

pub struct Requirement;

pub static DIAGRAM: Requirement = Requirement;

/// Parses the diagram text into the db, as `parser.parse` with the db as
/// `yy` does. Public for tests that look at the db.
pub fn parse(resolved: &Resolved) -> Result<RequirementDb, HelperError> {
    let grammar = jison::grammar("requirementDiagram")
        .map_err(|e| HelperError::internal(format!("requirement grammar: {e}")))?;
    let mut actions = RequirementActions::new();
    jison::parse(grammar, resolved.parse_text(), &mut actions).map_err(|e| {
        HelperError::ParseError {
            line: e.line as u32,
            column: e.column as u32,
            diagram_type: "requirement".to_string(),
            message: e.to_string(),
        }
    })?;
    Ok(actions.db)
}

impl Diagram for Requirement {
    fn types(&self) -> &'static [&'static str] {
        &["requirement"]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let db = parse(resolved)?;
        render::draw(&db, resolved, ctx)
    }
}
