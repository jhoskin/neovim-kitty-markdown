//! The Cynefin framework: the `cynefin-beta` diagram with its langium
//! parser, db, renderer, and styles ported from mermaid 12.2.0.

pub mod db;
pub mod parser;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::scene::Scene;
use crate::mermaid::styles::cynefin::CynefinStyles;

pub struct Cynefin;

pub const DIAGRAM: Cynefin = Cynefin;

/// The detector id in `cynefinDetector.ts`.
const TYPE: &str = "cynefin";

impl Diagram for Cynefin {
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
        let mut db = db::CynefinDb::new();
        if let Some(title) = resolved.title() {
            db.set_diagram_title(title);
        }
        db.populate(&ast);
        let cfg = render::CynefinConfig::from_raw(&resolved.raw);
        let styles =
            CynefinStyles::resolve(&resolved.theme_variables, &resolved.config.font_family);
        Ok(render::draw(&db, &cfg, &styles, ctx))
    }
}
