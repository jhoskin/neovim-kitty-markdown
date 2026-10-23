//! The ishikawa (fishbone) diagram (R-109): `ishikawa.jison` through the
//! jison driver, `ishikawaDb.ts` as [`db::IshikawaDb`],
//! `ishikawaRenderer.ts` as [`render::draw`], and `ishikawaStyles.ts` in
//! `styles::ishikawa`.

pub mod actions;
pub mod db;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison::{self, ErrorKind, ParseError};
use crate::mermaid::scene::Scene;

pub struct Ishikawa;

pub static DIAGRAM: Ishikawa = Ishikawa;

/// The detector id in `ishikawaDetector.ts`.
const TYPE: &str = "ishikawa";

/// `parser.parse(text)` with a fresh db, as `Diagram.fromText` does.
pub fn parse(code: &str) -> Result<db::IshikawaDb, ParseError> {
    let grammar = jison::grammar(TYPE).map_err(|e| ParseError::action(1, 0, e.to_string()))?;
    let mut db = db::IshikawaDb::new();
    jison::parse(grammar, code, &mut db)?;
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

impl Diagram for Ishikawa {
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
    fn docs_example_parses_into_a_tree() {
        let db = parse(
            "ishikawa-beta\n    Blurry Photo\n        Process\n            Out of focus\n        User\n            Shaky hands\n",
        )
        .expect("parses");
        let root = db.root().expect("root");
        assert_eq!(root.text, "Blurry Photo");
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].text, "Process");
        assert_eq!(root.children[0].children[0].text, "Out of focus");
        assert_eq!(root.children[1].children[0].text, "Shaky hands");
    }

    #[test]
    fn unindented_lines_comments_and_blank_lines_parse() {
        let db = parse("ishikawa\nProblem\n%% a comment\nCause A\n\n  Subcause A1   \nCause B")
            .expect("parses");
        let root = db.root().expect("root");
        assert_eq!(root.text, "Problem");
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].children[0].text, "Subcause A1");
        assert_eq!(root.children[1].text, "Cause B");
    }

    #[test]
    fn the_keyword_inside_a_cause_line_is_a_parse_error() {
        let err = parse("ishikawa\n  Effect\n  ishikawa\n").expect_err("keyword");
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_eq!(err.line, 3);
    }
}
