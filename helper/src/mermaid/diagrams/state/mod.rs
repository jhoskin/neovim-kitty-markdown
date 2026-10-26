//! The state diagram: mermaid 12.2.0's `diagrams/state` through the
//! unified renderer and dagre. Both `stateDiagram` and `stateDiagram-v2`
//! reach it, because 12.2.0's one `stateDiagram` detector matches both and
//! loads the v2 diagram.
//!
//! `actions` replays `stateDiagram.jison` over the vendored tables into
//! the document tree, `db` runs `StateDB`'s `docTranslator` and `extract`,
//! `fetch` ports `dataFetcher` into the unified layout data, and `render`
//! runs `stateRenderer-v3-unified.draw` with `styles::state` for the
//! stylesheet.

pub mod actions;
pub mod db;
pub mod fetch;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison;
use crate::mermaid::scene::Scene;

use actions::{StateActions, Val};
use db::StateDb;
use fetch::FetchConfig;

pub struct State;

pub static DIAGRAM: State = State;

/// The SVG id mermaid-cli renders under; DOM ids and marker ids carry it.
const DIAGRAM_ID: &str = "my-svg";

fn parse_error(line: usize, column: usize, message: String) -> HelperError {
    HelperError::ParseError {
        line: line as u32,
        column: column as u32,
        diagram_type: "stateDiagram".to_string(),
        message,
    }
}

/// Parses the diagram text into the db, as `parser.parse` with
/// `yy.setRootDoc` does. Public for tests that look at the db.
pub fn parse_source(code: &str) -> Result<StateDb, HelperError> {
    let grammar = jison::grammar("stateDiagram")
        .map_err(|e| HelperError::internal(format!("state grammar: {e}")))?;
    let mut actions = StateActions::default();
    let value = jison::parse(grammar, code, &mut actions)
        .map_err(|e| parse_error(e.line, e.column, e.to_string()))?;
    let doc = match value {
        Val::Doc(d) => d,
        _ => Vec::new(),
    };
    Ok(StateDb::new(doc))
}

/// [`parse_source`] over the resolved diagram's cleaned text, with the
/// entity codes encoded as `Diagram.fromText` does.
pub fn parse(resolved: &Resolved) -> Result<StateDb, HelperError> {
    parse_source(&resolved.encoded_parse_text())
}

impl Diagram for State {
    fn types(&self) -> &'static [&'static str] {
        &["stateDiagram"]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let db = parse(resolved)?;
        let cfg = &resolved.config;
        let fetch_config = FetchConfig {
            diagram_id: DIAGRAM_ID.to_string(),
            flowchart_padding: *cfg.flowchart.padding,
            wrapping_width: *cfg.state.wrapping_width,
            min_node_width: *cfg.state.min_node_width,
        };
        let fetched = fetch::fetch(&db, &fetch_config).map_err(|message| {
            // `extract` throws from inside `setRootDoc`, so mermaid reports
            // it as the parse failure of the diagram.
            parse_error(1, 0, message)
        })?;
        render::render(resolved, ctx, fetched, DIAGRAM_ID)
    }
}
