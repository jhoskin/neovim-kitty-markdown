//! The kanban diagram (R-108): `kanban.jison` through the jison driver,
//! `kanbanDb.ts` as [`db::KanbanDb`], `kanbanRenderer.ts` as
//! [`render::draw`], and `styles.ts` in `styles::kanban`.

pub mod actions;
pub mod db;
pub mod render;

use serde_json::Value;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison::{self, ErrorKind, ParseError};
use crate::mermaid::scene::Scene;

pub struct Kanban;

pub static DIAGRAM: Kanban = Kanban;

/// `parser.parse(text)` with a fresh db, as `Diagram.fromText` does.
pub fn parse(code: &str, defaults: db::NodeDefaults) -> Result<db::KanbanDb, ParseError> {
    let grammar = jison::grammar("kanban").map_err(|e| ParseError::action(1, 0, e.to_string()))?;
    let mut db = db::KanbanDb::new(defaults);
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
        // A grammar action threw (`addNode` on bad data): mermaid reports
        // it as the diagram's error, not a parse error.
        ErrorKind::Action => HelperError::internal(err.message),
        ErrorKind::Parse | ErrorKind::Lexical => HelperError::ParseError {
            line: err.line as u32,
            column: err.column as u32,
            diagram_type: "kanban".to_string(),
            message: err.message,
        },
    }
}

impl Diagram for Kanban {
    fn types(&self) -> &'static [&'static str] {
        &["kanban"]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let db = parse(resolved.code(), node_defaults(resolved)).map_err(parse_error)?;
        render::draw(&db, resolved, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_default(code: &str) -> Result<db::KanbanDb, ParseError> {
        parse(code, db::NodeDefaults::default())
    }

    #[test]
    fn docs_example_parses_into_sections_and_items() {
        let db = parse_default(
            "kanban\n  Todo\n    [Create Documentation]\n    docs[Create Blog about the new diagram]\n  [In progress]\n    id6[Create renderer]\n  id9[Ready for deploy]\n    id8[Design grammar]@{ assigned: 'knsv' }\n",
        )
        .expect("parses");
        let data = db.get_data();
        assert_eq!(data.len(), 3);
        assert_eq!(data[0].label, "Todo");
        assert_eq!(data[0].items[0].id, "Create Documentation");
        assert_eq!(data[0].items[1].id, "docs");
        assert_eq!(data[1].id, "In progress");
        assert_eq!(data[2].items[0].assigned.as_deref(), Some("knsv"));
    }

    #[test]
    fn metadata_quotes_icons_and_comments_parse() {
        let db = parse_default(
            "kanban\n  %% a comment\n  s[Section]\n    a[\"Quoted label\"]@{ ticket: \"MC-1\", priority: 'Very High', assigned: K.Sveidqvist }\n    ::icon(fa fa-book)\n    b[Multi]@{\n      ticket: 7\n      assigned: me\n    }\n",
        )
        .expect("parses");
        let data = db.get_data();
        let a = &data[0].items[0];
        assert_eq!(a.label, "Quoted label");
        assert_eq!(a.ticket.as_deref(), Some("MC-1"));
        assert_eq!(a.priority.as_deref(), Some("Very High"));
        assert_eq!(a.assigned.as_deref(), Some("K.Sveidqvist"));
        assert_eq!(a.icon.as_deref(), Some("fa fa-book"));
        let b = &data[0].items[1];
        assert_eq!(b.ticket.as_deref(), Some("7"));
        assert_eq!(b.assigned.as_deref(), Some("me"));
    }

    #[test]
    fn grammar_errors_are_parse_errors_and_db_errors_are_actions() {
        let err = parse_default("kanban\n  a[b\n").expect_err("unclosed bracket");
        assert_eq!(err.kind, ErrorKind::Parse);
        // The `NODE` state has no `<<EOF>>` rule, so jison reports the
        // bare end symbol after the description swallowed the newline.
        assert!(
            err.message.ends_with("Expecting 'NODE_DEND', got '1'"),
            "{}",
            err.message
        );
        let err = parse_default("kanban\n    a\n  b\n    c\n").expect_err("items without section");
        assert_eq!(err.kind, ErrorKind::Action);
        assert_eq!(
            err.message,
            "Items without section detected, found section (\"b\")"
        );
    }
}
