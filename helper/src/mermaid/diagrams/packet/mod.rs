//! The packet diagram: `diagrams/packet` with its langium parser, db,
//! renderer, and styles ported from mermaid 12.2.0. Phase 4 (R-108).

pub mod db;
pub mod parser;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::scene::Scene;
use crate::mermaid::styles::packet::PacketStyles;
use crate::mermaid::styles::pie::css_color;

pub struct Packet;

pub static DIAGRAM: Packet = Packet;

/// The detector id in `detector.ts`.
pub const TYPE: &str = "packet";

impl Diagram for Packet {
    fn types(&self) -> &'static [&'static str] {
        &[TYPE]
    }

    /// `Diagram.fromText` followed by `renderer.draw`: the front matter
    /// title is set before the parser runs, so a `title` line in the text
    /// wins, and the text the parser sees ends with the newline `fromText`
    /// appends. `populate` reads `bitsPerRow` from the db config.
    fn render(&self, resolved: &Resolved, _ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let text = resolved.parse_text();
        let ast = parser::parse(text).map_err(|f| HelperError::ParseError {
            line: f.line,
            column: f.column,
            diagram_type: TYPE.to_string(),
            message: f.message,
        })?;
        let cfg = db::PacketConfig::from_resolved(resolved);
        let mut db = db::PacketDb::new();
        if let Some(title) = resolved.title().filter(|t| !t.is_empty()) {
            db.set_diagram_title(title);
        }
        db.populate(&ast, cfg.bits_per_row)
            .map_err(|e| HelperError::ParseError {
                line: e.line,
                column: 0,
                diagram_type: TYPE.to_string(),
                message: e.message,
            })?;
        let styles = PacketStyles::resolve(&resolved.theme_variables, &resolved.config.font_family);
        let text_color = resolved
            .theme_variables
            .str("textColor")
            .and_then(css_color);
        Ok(render::draw(&db, &cfg, &styles, text_color))
    }
}
