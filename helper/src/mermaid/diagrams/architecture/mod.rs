//! The architecture diagram (phase 4): `architecture-beta` with its
//! langium parser, `ArchitectureDB`, the fcose layout mermaid runs through
//! cytoscape, and `svgDraw.ts`, ported from mermaid 12.2.0.

pub mod db;
pub mod icons;
pub mod layout;
pub mod parser;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::scene::Scene;
use crate::mermaid::styles::architecture::ArchitectureStyles;

pub struct Architecture;

pub static DIAGRAM: Architecture = Architecture;

/// The detector id in `architectureDetector.ts`.
pub const TYPE: &str = "architecture";

impl Diagram for Architecture {
    fn types(&self) -> &'static [&'static str] {
        &[TYPE]
    }

    /// `Diagram.fromText` followed by `renderer.draw`: the front matter
    /// title is set before the parser runs, and the text the parser sees
    /// ends with the newline `fromText` appends. An error `populateDb`
    /// throws is reported like a parse error at the line of its statement.
    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let text = format!("{}\n", resolved.code());
        let ast = parser::parse(&text).map_err(|f| HelperError::ParseError {
            line: f.line,
            column: f.column,
            diagram_type: TYPE.to_string(),
            message: f.message,
        })?;
        let mut db = db::ArchitectureDb::new();
        if let Some(title) = resolved.title().filter(|t| !t.is_empty()) {
            db.set_diagram_title(title);
        }
        db.populate(&ast).map_err(|e| HelperError::ParseError {
            line: e.line,
            column: 0,
            diagram_type: TYPE.to_string(),
            message: e.message,
        })?;
        let cfg = layout::ArchConfig::from_raw(&resolved.raw);
        if cfg.randomize {
            // `randomize: true` starts fcose from its spectral layout,
            // which is not ported.
            return Err(HelperError::UnsupportedFeature {
                feature: "randomize".to_string(),
            });
        }
        let styles =
            ArchitectureStyles::resolve(&resolved.theme_variables, &resolved.config.font_family);
        render::draw(&db, &cfg, &styles, ctx)
    }
}
