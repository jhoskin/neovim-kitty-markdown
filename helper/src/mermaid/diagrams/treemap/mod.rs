//! The treemap diagram (phase 5): `treemap.langium` through a hand-written
//! parser, `db.ts` and `utils.ts` as [`db::TreemapDb`], `renderer.ts` as
//! [`render::draw`] over the d3-hierarchy port, `styles.ts` in
//! `styles::treemap`, and the d3-format slice `valueFormat` needs.

pub mod db;
pub mod format;
pub mod parser;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::scene::Scene;

pub struct Treemap;

pub static DIAGRAM: Treemap = Treemap;

/// The detector id in `detector.ts`.
pub const TYPE: &str = "treemap";

impl Diagram for Treemap {
    fn types(&self) -> &'static [&'static str] {
        &[TYPE]
    }

    /// `Diagram.fromText` followed by `renderer.draw`: the front matter
    /// title is set before the parser runs, so a `title` line in the text
    /// wins, and the text the parser sees ends with the newline `fromText`
    /// appends.
    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let text = format!("{}\n", resolved.code());
        let ast = parser::parse(&text).map_err(|f| HelperError::ParseError {
            line: f.line,
            column: f.column,
            diagram_type: TYPE.to_string(),
            message: f.message,
        })?;
        let mut db = db::TreemapDb::new();
        if let Some(title) = resolved.title().filter(|t| !t.is_empty()) {
            db.set_diagram_title(title);
        }
        db.populate(&ast);
        let cfg = db::TreemapConfig::from_resolved(resolved);
        Ok(render::draw(
            &db,
            &cfg,
            &resolved.theme_variables,
            ctx,
            &resolved.config.font_family,
        ))
    }
}
