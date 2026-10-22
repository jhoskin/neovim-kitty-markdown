//! The timeline diagram (R-107): `timeline.jison` through the jison driver,
//! `timelineDb.js` as [`db::TimelineDb`], `timelineRenderer.ts` and
//! `svgDraw.js` as [`render::draw`], and `styles.js` in `styles::timeline`.
//!
//! `timeline TD` selects `timelineRendererVertical.ts` in mermaid; that
//! renderer is not ported, so the vertical direction is an unsupported
//! feature.

pub mod actions;
pub mod db;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison::{self, ErrorKind, ParseError};
use crate::mermaid::scene::Scene;

pub struct Timeline;

pub static DIAGRAM: Timeline = Timeline;

/// `parser.parse(text)` with a fresh db, as `Diagram.fromText` does.
pub fn parse(code: &str) -> Result<db::TimelineDb, ParseError> {
    let grammar =
        jison::grammar("timeline").map_err(|e| ParseError::action(1, 0, e.to_string()))?;
    let mut db = db::TimelineDb::new();
    jison::parse(grammar, code, &mut db)?;
    Ok(db)
}

fn parse_error(err: ParseError) -> HelperError {
    match err.kind {
        // A grammar action threw (an event before any period): mermaid
        // reports it as the diagram's error, not a parse error.
        ErrorKind::Action => HelperError::internal(err.message),
        ErrorKind::Parse | ErrorKind::Lexical => HelperError::ParseError {
            line: err.line as u32,
            column: err.column as u32,
            diagram_type: "timeline".to_string(),
            message: err.message,
        },
    }
}

impl Diagram for Timeline {
    fn types(&self) -> &'static [&'static str] {
        &["timeline"]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let db = parse(resolved.code()).map_err(parse_error)?;
        if db.direction() == "TD" {
            return Err(HelperError::UnsupportedFeature {
                feature: "timeline direction TD".to_string(),
            });
        }
        render::draw(&db, resolved, ctx)
    }
}
