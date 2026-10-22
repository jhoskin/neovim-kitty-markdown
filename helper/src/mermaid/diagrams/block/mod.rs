//! The block diagram (R-108): `block.jison` through the jison driver,
//! `blockDB.ts` as [`db::BlockDb`], `layout.ts` as [`layout`],
//! `blockRenderer.ts` and `renderHelpers.ts` as [`render::draw`], and
//! `styles.ts` in `styles::block`.

pub mod actions;
pub mod db;
pub mod layout;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison::{self, ErrorKind, ParseError};
use crate::mermaid::scene::Scene;

pub struct Block;

pub static DIAGRAM: Block = Block;

/// `parser.parse(text)` followed by `setHierarchy`, as `Diagram.fromText`
/// runs them. Public for tests that look at the db.
pub fn parse(code: &str) -> Result<db::BlockDb, ParseError> {
    let grammar = jison::grammar("block").map_err(|e| ParseError::action(1, 0, e.to_string()))?;
    let mut actions = actions::BlockActions::default();
    jison::parse(grammar, code, &mut actions)?;
    actions
        .into_db()
        .map_err(|e| ParseError::action(1, 0, e.to_string()))
}

fn parse_error(err: ParseError) -> HelperError {
    match err.kind {
        // `setHierarchy` threw inside the start production: mermaid reports
        // the TypeError as the diagram's error, not a parse error.
        ErrorKind::Action => HelperError::internal(err.message),
        ErrorKind::Parse | ErrorKind::Lexical => HelperError::ParseError {
            line: err.line as u32,
            column: err.column as u32,
            diagram_type: "block".to_string(),
            message: err.message,
        },
    }
}

impl Diagram for Block {
    fn types(&self) -> &'static [&'static str] {
        &["block"]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let mut db = parse(resolved.code()).map_err(parse_error)?;
        render::draw(&mut db, resolved, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::db::BlockType;
    use super::*;

    #[test]
    fn parses_the_docs_three_column_example() {
        let db = parse("block\n  columns 3\n  a b c d\n").expect("parses");
        let root = db.block(db.root());
        assert_eq!(root.columns, Some(3));
        let ids: Vec<&str> = db
            .blocks()
            .iter()
            .map(|&i| db.block(i).id.as_str())
            .collect();
        assert_eq!(ids, vec!["a", "b", "c", "d"]);
        assert_eq!(db.block(db.blocks()[0]).kind, BlockType::Na);
    }

    #[test]
    fn parses_shapes_widths_spaces_and_edges() {
        let db = parse(
            "block-beta\n  columns 3\n  a[\"A label\"] b:2 c(\"round\")\n  space:2 d[(\"db\")]\n  a --> d\n  a -- \"x\" --> b\n",
        )
        .expect("parses");
        let kinds: Vec<(String, BlockType, i64)> = db
            .blocks()
            .iter()
            .map(|&i| {
                let b = db.block(i);
                (b.id.clone(), b.kind, b.width_in_columns())
            })
            .collect();
        assert_eq!(kinds[0], ("a".to_string(), BlockType::Square, 1));
        assert_eq!(kinds[1], ("b".to_string(), BlockType::Na, 2));
        assert_eq!(kinds[2], ("c".to_string(), BlockType::Round, 1));
        assert_eq!(kinds[3].1, BlockType::Space);
        assert_eq!(kinds[4].1, BlockType::Space);
        assert_eq!(kinds[5], ("d".to_string(), BlockType::Cylinder, 1));
        assert_eq!(db.block(db.blocks()[0]).label, "A label");
        let edges = db.edges();
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].id, "1-a-d");
        assert_eq!(edges[0].arrow_type_end, "arrow_point");
        assert_eq!(edges[0].arrow_type_start, "arrow_open");
        assert_eq!(edges[1].label, "x");
    }

    #[test]
    fn parses_composites_and_block_arrows() {
        let db = parse(
            "block\ncolumns 1\n  db((\"DB\"))\n  blockArrowId6<[\"&nbsp;\"]>(down)\n  block:ID\n    A\n    B[\"wide\"]\n  end\n  style B fill:#969,stroke:#333\n  classDef blue fill:#6e6ce6;\n  class A blue\n",
        )
        .expect("parses");
        let top: Vec<&Block_> = db.blocks().iter().map(|&i| db.block(i)).collect();
        assert_eq!(top[0].kind, BlockType::Circle);
        assert_eq!(top[1].kind, BlockType::BlockArrow);
        assert_eq!(top[1].directions, vec!["down".to_string()]);
        assert_eq!(top[2].kind, BlockType::Composite);
        // `block:ID` without a bracket label keeps the empty composite
        // label; only `block:ID["Title"]` names it.
        assert_eq!(top[2].label, "");
        assert_eq!(top[2].children.len(), 2);
        let b = db.block(top[2].children[1]);
        assert_eq!(b.styles, vec!["fill:#969", "stroke:#333"]);
        let a = db.block(top[2].children[0]);
        assert_eq!(a.classes, vec!["blue".to_string()]);
    }

    type Block_ = super::db::Block;

    #[test]
    fn a_bad_token_is_a_parse_error() {
        let err = parse("block\n  a -->\n").expect_err("dangling link");
        assert_eq!(err.kind, ErrorKind::Parse);
        assert!(
            err.message.starts_with("Parse error on line "),
            "{}",
            err.message
        );
    }
}
