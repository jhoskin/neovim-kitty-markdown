//! The quadrant chart (`quadrantChart`), a port of mermaid 12.2.0's
//! `diagrams/quadrant-chart`: the jison grammar's actions, the database,
//! the builder, and the renderer. Phase 2 (R-106).

pub mod actions;
pub mod builder;
pub mod db;
pub mod render;

use super::{Diagram, RenderCtx};
use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::scene::{DEFAULT_FONT_FAMILY, Scene};
use crate::text::fonts::{Style, Weight};

/// The detector id in `quadrantDetector.ts`.
pub const TYPE: &str = "quadrantChart";

pub struct Quadrant;

pub static DIAGRAM: Quadrant = Quadrant;

impl Diagram for Quadrant {
    fn types(&self) -> &'static [&'static str] {
        &[TYPE]
    }

    /// `Diagram.fromText` then `draw`: a front matter title goes in first,
    /// the parse may replace it, and the builder reads the effective
    /// `quadrantChart` section and theme variables.
    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let mut db = db::Db::new();
        if let Some(title) = resolved.title().filter(|t| !t.is_empty()) {
            db.set_diagram_title(title);
        }
        actions::parse_into(&mut db, resolved.code()).map_err(|e| HelperError::ParseError {
            line: u32::try_from(e.line).unwrap_or(u32::MAX),
            column: u32::try_from(e.column).unwrap_or(u32::MAX),
            diagram_type: TYPE.to_string(),
            message: e.message,
        })?;
        let config = &resolved.config.quadrant_chart;
        let build = db.quadrant_data(config, &resolved.theme_variables);
        let chain = resolved
            .theme_variables
            .str("fontFamily")
            .unwrap_or(DEFAULT_FONT_FAMILY);
        // The browser matches `font-family` case-insensitively and walks the
        // chain; fontdb wants the installed spelling, so the scene carries
        // the family the measurer resolved (R-115).
        let font_family = ctx
            .fonts
            .resolve(chain, Weight::NORMAL, Style::Normal)
            .and_then(|id| ctx.fonts.family_name(id))
            .unwrap_or_else(|| chain.to_string());
        let text_color = resolved.theme_variables.str("textColor");
        Ok(render::draw(&build, config, &font_family, text_color))
    }
}
