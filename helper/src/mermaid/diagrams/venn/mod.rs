//! The venn diagram (R-109): `venn.jison` through the jison driver,
//! `vennDB.ts` as [`db::VennDb`], `vennRenderer.ts` as [`render::draw`],
//! `styles.ts` in `styles::venn`, and the `@upsetjs/venn.js` 2.0.0 layout
//! the renderer imports in [`layout`].

pub mod actions;
pub mod db;
pub mod layout;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison::{self, ErrorKind, ParseError};
use crate::mermaid::scene::Scene;

pub struct Venn;

pub static DIAGRAM: Venn = Venn;

/// The detector id in `vennDetector.ts`.
const TYPE: &str = "venn";

/// `parser.parse(text)` with a fresh db. `Diagram.fromText` sets the front
/// matter title before the parse, so a `title` statement replaces it.
pub fn parse(code: &str, title: Option<&str>) -> Result<db::VennDb, ParseError> {
    let grammar = jison::grammar(TYPE).map_err(|e| ParseError::action(1, 0, e.to_string()))?;
    let mut db = db::VennDb::new();
    if let Some(title) = title {
        db.set_diagram_title(title);
    }
    jison::parse(grammar, code, &mut db)?;
    Ok(db)
}

fn parse_error(err: ParseError) -> HelperError {
    match err.kind {
        // A grammar action threw (an unknown set in a union): mermaid
        // reports it as the diagram's error, not a parse error.
        ErrorKind::Action => HelperError::internal(err.message),
        ErrorKind::Parse | ErrorKind::Lexical => HelperError::ParseError {
            line: err.line as u32,
            column: err.column as u32,
            diagram_type: TYPE.to_string(),
            message: err.message,
        },
    }
}

impl Diagram for Venn {
    fn types(&self) -> &'static [&'static str] {
        &[TYPE]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let db = parse(resolved.parse_text(), resolved.title()).map_err(parse_error)?;
        render::draw(&db, resolved, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn docs_example_parses_sets_unions_and_title() {
        let db = parse(
            "venn-beta\n  title What makes a good feature\n  set Desirable\n  set Feasible\n  set Viable\n  union Desirable,Feasible[\"Buildable\"]\n  union Desirable,Feasible,Viable[\"Ship it\"]\n",
            None,
        )
        .expect("parses");
        assert_eq!(db.diagram_title(), Some("What makes a good feature"));
        let s = db.subset_data();
        assert_eq!(s.len(), 5);
        assert_eq!(s[3].sets, strings(&["Desirable", "Feasible"]));
        assert_eq!(s[3].label.as_deref(), Some("Buildable"));
        assert_eq!(s[4].size, 10.0 / 9.0);
    }

    #[test]
    fn indented_text_attaches_to_the_last_set_and_styles_parse() {
        let db = parse(
            "venn-beta\n  set A[\"Alpha\"]:20\n    text A1[\"React\"]\n    text A2[\"Design Systems\"]\n  set B[\"Beta\"]:12\n  union A,B[\"AB\"]:3\n    text AB1\ntext A,B \"Q\"\n  style A fill:#ff6b6b, stroke-width: 4px\n  style A,B color:#333\n  style A1 color:red\n",
            None,
        )
        .expect("parses");
        let s = db.subset_data();
        assert_eq!(s[0].size, 20.0);
        assert_eq!(s[0].label.as_deref(), Some("Alpha"));
        assert_eq!(s[2].size, 3.0);
        let t = db.text_data();
        assert_eq!(t.len(), 4);
        assert_eq!(t[0].sets, strings(&["A"]));
        assert_eq!(t[0].id, "A1");
        assert_eq!(t[1].label.as_deref(), Some("Design Systems"));
        assert_eq!(t[2].sets, strings(&["A", "B"]));
        assert_eq!(t[2].id, "AB1");
        assert_eq!(t[3].id, "Q");
        let st = db.style_data();
        assert_eq!(st[0].targets, strings(&["A"]));
        assert_eq!(st[0].styles["fill"], "#ff6b6b");
        // `valueTokens` joins `4` and `px` with a space, which CSS rejects.
        assert_eq!(st[0].styles["stroke-width"], "4 px");
        assert_eq!(st[1].targets, strings(&["A", "B"]));
    }

    #[test]
    fn unknown_union_sets_are_action_errors_and_bad_tokens_parse_errors() {
        let err = parse("venn-beta\n  set A\n  union A,B\n", None).expect_err("unknown set");
        assert_eq!(err.kind, ErrorKind::Action);
        assert_eq!(err.message, "unknown set identifier: B");
        let err = parse("venn-beta\n  set A\n  union A\n", None).expect_err("one set");
        assert_eq!(err.message, "union requires multiple identifiers");
        let err = parse("venn-beta\n  set : 3\n", None).expect_err("no identifier");
        assert_eq!(err.kind, ErrorKind::Parse);
    }

    #[test]
    fn the_title_statement_replaces_the_front_matter_title() {
        let db = parse("venn-beta\n  title Mine\n  set A\n", Some("Front")).expect("parses");
        assert_eq!(db.diagram_title(), Some("Mine"));
        let db = parse("venn-beta\n  set A\n", Some("Front")).expect("parses");
        assert_eq!(db.diagram_title(), Some("Front"));
    }
}
