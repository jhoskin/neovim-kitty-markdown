//! The Wardley map (phase 5): the `wardley-beta` diagram with its langium
//! parser, `populateDb` and `WardleyBuilder` as [`db::WardleyDb`], and
//! `wardleyRenderer.ts` as [`render::draw`], ported from mermaid 12.2.0.

pub mod db;
pub mod parser;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::scene::Scene;
use crate::mermaid::styles::wardley::WardleyStyles;

pub struct Wardley;

pub static DIAGRAM: Wardley = Wardley;

/// The detector id in `wardleyDetector.ts`.
pub const TYPE: &str = "wardley";

impl Diagram for Wardley {
    fn types(&self) -> &'static [&'static str] {
        &[TYPE]
    }

    /// `Diagram.fromText` followed by `renderer.draw`: the front matter
    /// title is set before the parser runs, so a `title` line in the text
    /// wins, and the text the parser sees ends with the newline `fromText`
    /// appends. An error `populateDb` throws is reported like a parse error
    /// at the line of its statement.
    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let text = format!("{}\n", resolved.code());
        let ast = parser::parse(&text).map_err(|f| HelperError::ParseError {
            line: f.line,
            column: f.column,
            diagram_type: TYPE.to_string(),
            message: f.message,
        })?;
        let mut db = db::WardleyDb::new();
        if let Some(title) = resolved.title().filter(|t| !t.is_empty()) {
            db.set_diagram_title(title);
        }
        db.populate(&ast).map_err(|e| HelperError::ParseError {
            line: e.line,
            column: 0,
            diagram_type: TYPE.to_string(),
            message: e.message,
        })?;
        let cfg = render::WardleyConfig::from_raw(&resolved.raw);
        let styles = WardleyStyles::resolve(&resolved.theme_variables);
        Ok(render::draw(
            &db,
            &cfg,
            &styles,
            ctx,
            &resolved.config.font_family,
        ))
    }
}
