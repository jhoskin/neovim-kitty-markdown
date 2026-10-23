//! The flowchart (R-105): `flow.jison` through the jison driver with the
//! source cleanup of `parser/flowParser.ts`, `flowDb.ts` as
//! [`db::FlowDb`], `flowRenderer-v3-unified.ts` as [`render::draw`] over
//! the unified renderer and the dagre port, and `styles.ts` in
//! `styles::flowchart`.

pub mod actions;
pub mod db;
pub mod render;

use std::sync::LazyLock;

use regex::Regex;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison::{self, ErrorKind, ParseError};
use crate::mermaid::scene::Scene;

pub struct Flowchart;

pub static DIAGRAM: Flowchart = Flowchart;

/// `/}\s*\n/g` of `flowParser.ts`: the whitespace after a closing brace at
/// the end of a line goes, so shape data can end a line.
static BRACE_EOL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\}\s*\n").expect("valid regex"));

/// `parser.parse(text)` with a fresh db, as `Diagram.fromText` does.
pub fn parse(code: &str, config: db::DbConfig) -> Result<db::FlowDb, ParseError> {
    let grammar = jison::grammar("flow").map_err(|e| ParseError::action(1, 0, e.to_string()))?;
    let source = BRACE_EOL.replace_all(code, "}\n");
    let mut flow = db::FlowDb::new(config);
    jison::parse(grammar, &source, &mut flow)?;
    Ok(flow)
}

/// The error a failed `parse` reports. Public for swimlane, which parses
/// with the same grammar.
pub fn parse_error(err: ParseError) -> HelperError {
    match err.kind {
        // A db call threw (an unknown shape, a `linkStyle` index past the
        // end): mermaid reports the error text, not a parse error.
        ErrorKind::Action => HelperError::internal(err.message),
        ErrorKind::Parse | ErrorKind::Lexical => HelperError::ParseError {
            line: err.line as u32,
            column: err.column as u32,
            diagram_type: "flowchart".to_string(),
            message: err.message,
        },
    }
}

impl Diagram for Flowchart {
    /// `graph` and `flowchart` text both detect as `flowchart-v2` in
    /// mermaid 12.2.0; `flowchart-elk` is the ELK layout (R-175).
    fn types(&self) -> &'static [&'static str] {
        &["flowchart-v2", "flowchart-elk"]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        if resolved.diagram_type == "flowchart-elk" {
            return Err(HelperError::UnsupportedFeature {
                feature: "layout: elk".to_string(),
            });
        }
        let flow = parse(resolved.code(), db::DbConfig::from_config(&resolved.config))
            .map_err(parse_error)?;
        render::draw(&flow, resolved, ctx)
    }
}
