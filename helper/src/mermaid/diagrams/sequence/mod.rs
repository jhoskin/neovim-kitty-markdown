//! The sequence diagram: mermaid 12.2.0's `diagrams/sequence` as a scene.
//!
//! `actions` replays `sequenceDiagram.jison` over the vendored tables into
//! statements and `db` applies them as `SequenceDB` does.

pub mod actions;
pub mod db;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::jison;

use actions::{SequenceActions, Val};
use db::SequenceDb;

fn parse_error(line: usize, column: usize, message: String) -> HelperError {
    HelperError::ParseError {
        line: line as u32,
        column: column as u32,
        diagram_type: "sequence".to_string(),
        message,
    }
}

/// Parses the diagram text into the db, as `parser.parse` followed by
/// `yy.apply` does. Public for tests that look at the db.
pub fn parse(resolved: &Resolved) -> Result<SequenceDb, HelperError> {
    let grammar = jison::grammar("sequenceDiagram")
        .map_err(|e| HelperError::internal(format!("sequence grammar: {e}")))?;
    let mut actions = SequenceActions;
    let value = jison::parse(grammar, resolved.code(), &mut actions)
        .map_err(|e| parse_error(e.line, e.column, e.to_string()))?;
    let stmts = match value {
        Val::Stmts(s) => s,
        _ => Vec::new(),
    };
    let cfg = &resolved.config;
    // `sequenceDiagram.ts` `init`: a top-level `wrap` turns on
    // `sequence.wrap`; the db also keeps it as `setWrap`'s value.
    let config_wrap = cfg.sequence.wrap || cfg.wrap == Some(true);
    let mut db = SequenceDb::new(config_wrap, cfg.wrap);
    db.apply_all(&stmts)
        .map_err(|(line, e)| parse_error(line, 0, e.message))?;
    Ok(db)
}
