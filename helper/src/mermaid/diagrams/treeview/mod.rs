//! The treeView diagram (phase 5): `boxDrawingPreprocessor.ts` in
//! [`box_drawing`], `treeView.langium` through a hand-written parser,
//! `db.ts` and `icons.ts` as [`db`], `renderer.ts` as [`render::draw`],
//! and `styles.ts` in `styles::treeview`.

pub mod box_drawing;
pub mod db;
pub mod parser;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::scene::Scene;
use crate::mermaid::styles::treeview::TreeViewStyles;

pub struct TreeView;

pub static DIAGRAM: TreeView = TreeView;

/// The detector id in `detector.ts`.
pub const TYPE: &str = "treeView";

fn parse_error(line: u32, column: u32, message: String) -> HelperError {
    HelperError::ParseError {
        line,
        column,
        diagram_type: TYPE.to_string(),
        message,
    }
}

impl Diagram for TreeView {
    fn types(&self) -> &'static [&'static str] {
        &[TYPE]
    }

    /// `Diagram.fromText` followed by `renderer.draw`. The text the parser
    /// sees ends with the newline `fromText` appends. `parser.parse` runs
    /// the box-drawing preprocessor first, and a Langium error on a
    /// rewritten text names the source lines again.
    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let input = resolved.parse_text();
        let pre = box_drawing::preprocess(input).map_err(|e| parse_error(e.line, 0, e.message))?;
        let ast = parser::parse(&pre.text).map_err(|f| {
            if pre.line_map.is_empty() {
                return parse_error(f.line, f.column, f.message);
            }
            let line = pre.line_map.get(&f.line).copied().unwrap_or(f.line);
            let message = box_drawing::remap_error_lines(&f.message, &pre.line_map);
            parse_error(line, f.column, message)
        })?;
        let mut db = db::TreeViewDb::new();
        if let Some(title) = resolved.title().filter(|t| !t.is_empty()) {
            db.title = title.to_string();
        }
        db.populate(&ast);
        let cfg = db::TreeViewConfig::from_resolved(resolved);
        let styles =
            TreeViewStyles::resolve(&resolved.theme_variables, &resolved.config.font_family);
        render::draw(&db, &cfg, &styles, ctx)
    }
}
