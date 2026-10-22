//! The git graph: `gitGraphDiagram.ts` with its langium parser, db,
//! renderer, and styles ported from mermaid 12.2.0. Phase 3 (R-107).

pub mod db;
pub mod parser;
pub mod render;

use serde_json::Value;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::scene::Scene;
use crate::mermaid::styles::gitgraph::GitGraphStyles;

pub struct GitGraph;

pub static DIAGRAM: GitGraph = GitGraph;

/// The detector id in `gitGraphDetector.ts`.
pub const TYPE: &str = "gitGraph";

/// `GitGraphDiagramConfig` as `db.getConfig()` returns it: the defaults of
/// `config.schema.yaml` merged with the `gitGraph` section of the
/// effective config.
#[derive(Debug, Clone, PartialEq)]
pub struct GitGraphConfig {
    pub main_branch_name: String,
    /// `None` for a `null` override; the db then numbers the branch by
    /// its position.
    pub main_branch_order: Option<f64>,
    pub show_commit_label: bool,
    pub show_branches: bool,
    pub rotate_commit_label: bool,
    pub parallel_commits: bool,
    pub diagram_padding: f64,
    pub title_top_margin: f64,
}

impl GitGraphConfig {
    /// Reads the `gitGraph` section of the effective config JSON.
    pub fn from_raw(raw: &Value) -> GitGraphConfig {
        let section = raw.get("gitGraph").cloned().unwrap_or(Value::Null);
        let truthy = |name: &str, default: bool| match section.get(name) {
            None | Some(Value::Null) => default,
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_f64().is_some_and(|v| v != 0.0),
            Some(Value::String(s)) => !s.is_empty(),
            Some(_) => true,
        };
        let number =
            |name: &str, default: f64| section.get(name).and_then(Value::as_f64).unwrap_or(default);
        GitGraphConfig {
            main_branch_name: section
                .get("mainBranchName")
                .and_then(Value::as_str)
                .unwrap_or("main")
                .to_string(),
            main_branch_order: match section.get("mainBranchOrder") {
                None => Some(0.0),
                Some(v) => v.as_f64(),
            },
            show_commit_label: truthy("showCommitLabel", true),
            show_branches: truthy("showBranches", true),
            rotate_commit_label: truthy("rotateCommitLabel", true),
            parallel_commits: truthy("parallelCommits", false),
            diagram_padding: number("diagramPadding", 8.0),
            title_top_margin: number("titleTopMargin", 25.0),
        }
    }
}

impl Diagram for GitGraph {
    fn types(&self) -> &'static [&'static str] {
        &[TYPE]
    }

    /// `Diagram.fromText` followed by `renderer.draw`: the front matter
    /// title is set before the parser runs, so a `title` line in the text
    /// wins, and the text the parser sees ends with the newline `fromText`
    /// appends.
    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let text = format!("{}\n", resolved.code());
        let ast = parser::parse(&text).map_err(|f| HelperError::ParseError {
            line: f.line,
            column: f.column,
            diagram_type: TYPE.to_string(),
            message: f.message,
        })?;
        let cfg = GitGraphConfig::from_raw(&resolved.raw);
        let mut db = db::GitGraphDb::new(&cfg.main_branch_name, cfg.main_branch_order);
        if let Some(title) = resolved.title() {
            db.set_diagram_title(title);
        }
        db.populate(&ast).map_err(|e| HelperError::ParseError {
            line: e.line,
            column: 0,
            diagram_type: TYPE.to_string(),
            message: e.message,
        })?;
        let styles =
            GitGraphStyles::resolve(&resolved.theme_variables, &resolved.config.font_family);
        render::draw(&db, &cfg, &styles, &resolved.config.theme, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn config_reads_the_section_with_defaults() {
        let cfg = GitGraphConfig::from_raw(&json!({
            "gitGraph": { "showBranches": false, "rotateCommitLabel": false, "mainBranchName": "trunk", "mainBranchOrder": 2 }
        }));
        assert_eq!(cfg.main_branch_name, "trunk");
        assert_eq!(cfg.main_branch_order, Some(2.0));
        assert!(!cfg.show_branches);
        assert!(!cfg.rotate_commit_label);
        assert!(cfg.show_commit_label);
        assert!(!cfg.parallel_commits);
        assert_eq!(cfg.diagram_padding, 8.0);
        assert_eq!(cfg.title_top_margin, 25.0);
        let empty = GitGraphConfig::from_raw(&json!({}));
        assert_eq!(empty.main_branch_name, "main");
        assert!(empty.rotate_commit_label);
    }
}
