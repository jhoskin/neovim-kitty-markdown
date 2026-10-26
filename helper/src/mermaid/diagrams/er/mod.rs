//! The entity relationship diagram: mermaid 12.2.0's `diagrams/er` through
//! the unified renderer and dagre.
//!
//! `actions` replays `erDiagram.jison` over the vendored tables and makes
//! the `yy` calls into `db` (`erDb.ts`, `erTypes.ts`), `render` is
//! `erRenderer-unified.ts`, and `styles::er` holds the ER stylesheet
//! rules. The entity boxes are the shared `erBox` shape.

pub mod actions;
pub mod db;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison;
use crate::mermaid::scene::Scene;

use actions::ErActions;
use db::ErDb;

pub struct Er;

pub static DIAGRAM: Er = Er;

/// Parses the diagram text into the db, as `parser.parse` with the db as
/// `yy` does. Public for tests that look at the db.
pub fn parse(resolved: &Resolved) -> Result<ErDb, HelperError> {
    let grammar = jison::grammar("erDiagram")
        .map_err(|e| HelperError::internal(format!("er grammar: {e}")))?;
    let mut actions = ErActions::new();
    jison::parse(grammar, resolved.parse_text(), &mut actions).map_err(|e| {
        HelperError::ParseError {
            line: e.line as u32,
            column: e.column as u32,
            diagram_type: "er".to_string(),
            message: e.to_string(),
        }
    })?;
    Ok(actions.db)
}

impl Diagram for Er {
    fn types(&self) -> &'static [&'static str] {
        &["er"]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let db = parse(resolved)?;
        render::draw(&db, resolved, ctx)
    }
}
