//! The mindmap diagram (R-107): `mindmap.jison` through the jison driver,
//! `mindmapDb.ts` as [`db::MindmapDb`], `mindmapRenderer.ts` as
//! [`render::draw`] over the unified dagre renderer, and `styles.ts` in
//! `styles::mindmap`.
//!
//! `::icon(...)` is parsed and kept on the node, but no classic shape
//! draws it: the unified shapes only add the `labelBkg` class, which the
//! mindmap sheet does not style. So an icon changes nothing and is not
//! reported as unsupported.

pub mod actions;
pub mod db;
pub mod render;

use serde_json::Value;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison::{self, ErrorKind, ParseError};
use crate::mermaid::scene::Scene;

pub struct Mindmap;

pub static DIAGRAM: Mindmap = Mindmap;

/// `parser.parse(text)` with a fresh db, as `Diagram.fromText` does.
pub fn parse(code: &str, defaults: db::NodeDefaults) -> Result<db::MindmapDb, ParseError> {
    let grammar = jison::grammar("mindmap").map_err(|e| ParseError::action(1, 0, e.to_string()))?;
    let mut db = db::MindmapDb::new(defaults);
    jison::parse(grammar, code, &mut db)?;
    Ok(db)
}

/// The `mindmap` config values `addNode` reads (`conf.mindmap?.padding ??
/// defaultConfig.mindmap.padding`, same for `maxNodeWidth`).
fn node_defaults(resolved: &Resolved) -> db::NodeDefaults {
    let base = db::NodeDefaults::default();
    let mindmap = &resolved.config.mindmap;
    db::NodeDefaults {
        padding: mindmap
            .get("padding")
            .and_then(Value::as_f64)
            .unwrap_or(base.padding),
        max_node_width: mindmap
            .get("maxNodeWidth")
            .and_then(Value::as_f64)
            .unwrap_or(base.max_node_width),
    }
}

fn parse_error(err: ParseError) -> HelperError {
    match err.kind {
        // A grammar action threw (`addNode` with a second root): mermaid
        // reports it as the diagram's error, not a parse error.
        ErrorKind::Action => HelperError::internal(err.message),
        ErrorKind::Parse | ErrorKind::Lexical => HelperError::ParseError {
            line: err.line as u32,
            column: err.column as u32,
            diagram_type: "mindmap".to_string(),
            message: err.message,
        },
    }
}

impl Diagram for Mindmap {
    fn types(&self) -> &'static [&'static str] {
        &["mindmap"]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let mut db = parse(resolved.parse_text(), node_defaults(resolved)).map_err(parse_error)?;
        render::draw(&mut db, resolved, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::db::node_type;
    use super::*;

    fn parse_default(code: &str) -> Result<db::MindmapDb, ParseError> {
        parse(code, db::NodeDefaults::default())
    }

    #[test]
    fn docs_example_parses_into_a_tree() {
        let db = parse_default(
            "mindmap\n  root((mindmap))\n    Origins\n      Long history\n      ::icon(fa fa-book)\n      Popularisation\n    Research\n      On effectiveness<br/>and features\n",
        )
        .expect("parses");
        let nodes = db.nodes();
        assert_eq!(nodes.len(), 6);
        assert_eq!(nodes[0].node_id, "root");
        assert_eq!(nodes[0].descr, "mindmap");
        assert_eq!(nodes[0].node_type, node_type::CIRCLE);
        assert_eq!(nodes[0].children, vec![1, 4]);
        assert_eq!(nodes[2].icon.as_deref(), Some("fa fa-book"));
        assert_eq!(nodes[5].descr, "On effectiveness<br/>and features");
    }

    #[test]
    fn every_bracket_kind_has_its_shape() {
        let db = parse_default(
            "mindmap\n  r\n    a[square]\n    b(rounded)\n    c((circle))\n    d))bang((\n    e)cloud(\n    f{{hexagon}}\n    g\n    :::urgent large\n",
        )
        .expect("parses");
        let types: Vec<u8> = db.nodes().iter().map(|n| n.node_type).collect();
        assert_eq!(
            types,
            vec![
                node_type::DEFAULT,
                node_type::RECT,
                node_type::ROUNDED_RECT,
                node_type::CIRCLE,
                node_type::BANG,
                node_type::CLOUD,
                node_type::HEXAGON,
                node_type::DEFAULT,
            ]
        );
        assert_eq!(db.nodes()[1].descr, "square");
        assert_eq!(db.nodes()[7].class.as_deref(), Some("urgent large"));
    }

    #[test]
    fn a_second_root_is_the_diagram_error() {
        let err = parse_default("mindmap\n  a\n  b\n").expect_err("two roots");
        assert_eq!(err.kind, ErrorKind::Action);
        assert_eq!(
            err.message,
            "There can be only one root. No parent could be found for (\"b\")"
        );
    }

    #[test]
    fn an_unclosed_bracket_is_a_parse_error() {
        let err = parse_default("mindmap\n  a[b\n").expect_err("unclosed");
        assert_eq!(err.kind, ErrorKind::Parse);
        assert!(
            err.message.starts_with("Parse error on line"),
            "{}",
            err.message
        );
    }
}
