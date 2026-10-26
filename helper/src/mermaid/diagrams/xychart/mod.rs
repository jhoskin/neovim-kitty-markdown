//! The xychart renderer: mermaid 12.2.0's `diagrams/xychart/` as a port.
//!
//! `actions` fills the `db` from the vendored jison grammar, `builder` lays
//! the chart out into drawable elements, and `render` turns them into the
//! scene. The detector id is `xychart` (`xychartDetector.ts`).

pub mod actions;
pub mod builder;
pub mod db;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison;
use crate::mermaid::scene::{DEFAULT_FONT_FAMILY, Scene};
use crate::mermaid::styles::xychart::theme_config;

use actions::XyActions;
use db::Db;

pub struct XyChart;

pub static DIAGRAM: XyChart = XyChart;

/// The diagram id the detector answers with and the error data carries.
pub const DIAGRAM_TYPE: &str = "xychart";

/// Parses `code` into a db with the resolved config and theme. The front
/// matter title is set before the parse, as `Diagram.fromText` does, so a
/// `title` statement in the body wins.
pub fn parse(resolved: &Resolved) -> Result<Db, HelperError> {
    let theme = theme_config(&resolved.theme_variables);
    let mut db = Db::new(&resolved.config.xy_chart, theme);
    if let Some(title) = resolved.title() {
        db.set_diagram_title(title);
    }
    let grammar = jison::grammar("xychart").map_err(|e| HelperError::internal(e.to_string()))?;
    let mut actions = XyActions { db: &mut db };
    jison::parse(grammar, resolved.parse_text(), &mut actions).map_err(|e| {
        HelperError::ParseError {
            line: e.line as u32,
            column: e.column as u32,
            diagram_type: DIAGRAM_TYPE.to_string(),
            message: e.message,
        }
    })?;
    Ok(db)
}

impl Diagram for XyChart {
    fn types(&self) -> &'static [&'static str] {
        &[DIAGRAM_TYPE]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let db = parse(resolved)?;
        let family = resolved
            .theme_variables
            .str("fontFamily")
            .unwrap_or(DEFAULT_FONT_FAMILY);
        // The shared font database carries lower-case aliases, so the scene
        // keeps mermaid's CSS chain and resvg resolves it like a browser.
        render::draw(&db, &ctx.metrics, family, family)
    }
}
