//! The event modeling diagram (phase 5): `event-modeling.langium` through
//! a hand-written parser, `db.ts` as [`db::state`], `renderer.ts` as
//! [`render::draw`], and the theme colors in `styles::eventmodeling`
//! (mermaid 12.2.0).

pub mod db;
pub mod parser;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::scene::Scene;
use crate::mermaid::styles::eventmodeling::EventModelingStyles;

pub struct EventModeling;

pub static DIAGRAM: EventModeling = EventModeling;

/// The detector id in `detector.ts`.
pub const TYPE: &str = "eventmodeling";

impl Diagram for EventModeling {
    fn types(&self) -> &'static [&'static str] {
        &[TYPE]
    }

    /// `Diagram.fromText` followed by `renderer.draw`. The db sets
    /// `preserveCommentsWhenParsing`, so the parser sees the text with its
    /// comments, which keeps the line numbers of the source, plus the
    /// newline `fromText` appends. The title and the accessibility values
    /// reach the common db only; the renderer draws none of them.
    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let text = resolved.parse_text();
        let ast = parser::parse(text).map_err(|f| HelperError::ParseError {
            line: f.line,
            column: f.column,
            diagram_type: TYPE.to_string(),
            message: f.message,
        })?;
        db::check_frames(&ast).map_err(|e| HelperError::ParseError {
            line: e.line,
            column: 0,
            diagram_type: TYPE.to_string(),
            message: e.message,
        })?;
        let state = db::state(&ast, &ctx.metrics);
        let styles =
            EventModelingStyles::resolve(&resolved.theme_variables, &resolved.config.font_family);
        let cfg = render::EventModelingConfig::from_raw(&resolved.raw);
        Ok(render::draw(&state, &cfg, &styles, ctx))
    }
}
