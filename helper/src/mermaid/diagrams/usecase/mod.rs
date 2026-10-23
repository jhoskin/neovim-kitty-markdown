//! The use case diagram: mermaid 12.2.0's `diagrams/usecase`
//! (`usecase-beta`) through the unified renderer and dagre.
//!
//! `lexer` and `parser` port the Chevrotain lexer and parser
//! (`usecase.tokens.ts`, `usecase.parser.ts`) with Chevrotain's error
//! texts, `model` ports the visitor and `UsecaseModelBuilder`, `json` the
//! ordered JSON reader, `db` the committed model and `getData`, and
//! `render` the renderer with `styles::usecase` for the stylesheet.

pub mod db;
pub mod json;
pub mod lexer;
pub mod model;
pub mod parser;
pub mod render;

use std::sync::LazyLock;

use regex::Regex;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::scene::Scene;

use db::UsecaseDb;

pub struct Usecase;

pub static DIAGRAM: Usecase = Usecase;

/// The detector id in `usecaseDetector.ts`.
const TYPE: &str = "usecase";

static AT_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bline (\d+)").expect("static regex"));

fn parse_error(line: usize, column: usize, message: String) -> HelperError {
    HelperError::ParseError {
        line: line as u32,
        column: column as u32,
        diagram_type: TYPE.to_string(),
        message,
    }
}

/// Parses the diagram text into the committed model, as `parser.parse`
/// does after `Diagram.fromText` appends a newline. Public for tests that
/// look at the model.
pub fn parse_source(code: &str) -> Result<UsecaseDb, HelperError> {
    let text = format!("{code}\n");
    let statements = parser::parse(&text).map_err(|f| parse_error(f.line, f.column, f.message))?;
    model::build(&text, &statements).map_err(|e| {
        // The builder names the offending location first.
        let line = AT_LINE
            .captures(&e.message)
            .and_then(|c| c[1].parse().ok())
            .unwrap_or(e.line);
        parse_error(line, 0, e.message)
    })
}

impl Diagram for Usecase {
    fn types(&self) -> &'static [&'static str] {
        &[TYPE]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let db = parse_source(resolved.code())?;
        render::draw(&db, resolved, ctx)
    }
}
