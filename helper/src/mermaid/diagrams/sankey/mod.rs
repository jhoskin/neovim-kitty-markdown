//! The sankey diagram (R-108): `sankey.jison` through the jison driver on
//! the text `prepareTextForParsing` returns, `sankeyDB.ts` as
//! [`db::SankeyDb`], `sankeyRenderer.ts` as [`render::draw`] over the
//! d3-sankey port in `layout::d3::sankey`, and `styles.js` in
//! `styles::sankey`.

pub mod actions;
pub mod db;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison::lexer::js_trim;
use crate::mermaid::jison::{self, ErrorKind, ParseError};
use crate::mermaid::scene::Scene;

pub struct Sankey;

pub static DIAGRAM: Sankey = Sankey;

/// The detector id in `sankeyDetector.ts`.
const TYPE: &str = "sankey";

/// JS `\s` without the line breaks: `[^\S\n\r]`.
fn is_inline_space(c: char) -> bool {
    c != '\n' && c != '\r' && js_trim(c.encode_utf8(&mut [0; 4])).is_empty()
}

/// `prepareTextForParsing` (`sankeyUtils.ts`). The first regex has no `m`
/// flag, so `^` and `$` anchor at the ends of the whole text: only the
/// first row loses its leading spaces and only the last its trailing
/// ones. Runs of `\n` and `\r` become one `\n`, then the text is trimmed.
pub fn prepare_text_for_parsing(text: &str) -> String {
    let text = text.trim_start_matches(is_inline_space);
    let text = text.trim_end_matches(is_inline_space);
    let mut out = String::with_capacity(text.len());
    let mut in_break = false;
    for c in text.chars() {
        if c == '\n' || c == '\r' {
            if !in_break {
                out.push('\n');
            }
            in_break = true;
        } else {
            out.push(c);
            in_break = false;
        }
    }
    js_trim(&out).to_string()
}

/// `parser.parse(text)` with a fresh db.
pub fn parse(code: &str) -> Result<db::SankeyDb, ParseError> {
    let grammar = jison::grammar("sankey").map_err(|e| ParseError::action(1, 0, e.to_string()))?;
    let mut db = db::SankeyDb::new();
    jison::parse(grammar, &prepare_text_for_parsing(code), &mut db)?;
    Ok(db)
}

fn parse_error(err: ParseError) -> HelperError {
    match err.kind {
        ErrorKind::Action => HelperError::internal(err.message),
        ErrorKind::Parse | ErrorKind::Lexical => HelperError::ParseError {
            line: err.line as u32,
            column: err.column as u32,
            diagram_type: TYPE.to_string(),
            message: err.message,
        },
    }
}

impl Diagram for Sankey {
    fn types(&self) -> &'static [&'static str] {
        &[TYPE]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let db = parse(resolved.code()).map_err(parse_error)?;
        render::draw(&db, resolved, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_text_trims_the_ends_and_folds_blank_lines() {
        assert_eq!(
            prepare_text_for_parsing("  sankey\n\n\n  a,b,1  \r\n\nc,d,2 \n\n"),
            "sankey\n  a,b,1  \nc,d,2"
        );
    }
}
