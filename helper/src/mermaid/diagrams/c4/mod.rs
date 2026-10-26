//! The C4 diagrams (R-108): mermaid 12.2.0's `diagrams/c4` as a scene, for
//! `C4Context`, `C4Container`, `C4Component`, `C4Dynamic`, and
//! `C4Deployment`.
//!
//! `actions` replays `c4Diagram.jison` over the vendored tables straight
//! into `db` (`c4Db.ts`), `conf` reads the `c4` config section, `shapes`
//! draws the elements as the unified shapes do through
//! `c4ShapeAdapter.ts` and `c4LabelHelper.ts`, `render` runs
//! `c4Renderer.ts` with `svgDraw.ts` folded in, and `styles::c4` resolves
//! the stylesheet.

pub mod actions;
pub mod conf;
pub mod db;
pub mod render;
pub mod shapes;

use serde_json::Value;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison::{self, ErrorKind};
use crate::mermaid::scene::{Common, Group, Scene, ViewBox};
use crate::mermaid::styles::c4::C4Styles;

use actions::C4Actions;
use conf::Conf;
use db::C4Db;
use render::Renderer;

pub struct C4;

pub static DIAGRAM: C4 = C4;

/// Parses the diagram text into the db, as `parser.parse` does with
/// `yy = db` after `init` handed the root `wrap` to `setWrap`. Public for
/// tests that look at the db.
pub fn parse(resolved: &Resolved) -> Result<C4Db, HelperError> {
    let grammar = jison::grammar("c4Diagram")
        .map_err(|e| HelperError::internal(format!("c4Diagram grammar: {e}")))?;
    let wrap = resolved.raw.get("wrap").and_then(Value::as_bool) == Some(true);
    let mut actions = C4Actions::new(C4Db::new(wrap));
    jison::parse(grammar, resolved.parse_text(), &mut actions).map_err(|e| match e.kind {
        // A lexer `throw` is the diagram's error, not a parse error.
        ErrorKind::Action => HelperError::internal(e.message),
        ErrorKind::Parse | ErrorKind::Lexical => HelperError::ParseError {
            line: e.line as u32,
            column: e.column as u32,
            diagram_type: "c4".to_string(),
            message: e.to_string(),
        },
    })?;
    Ok(actions.db)
}

impl Diagram for C4 {
    fn types(&self) -> &'static [&'static str] {
        &["c4"]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let db = parse(resolved)?;
        let conf = Conf::from_resolved(resolved);
        let styles = C4Styles::resolve(&resolved.theme_variables);
        let drawn = Renderer::new(&conf, &styles, &ctx.metrics, &db).draw()?;
        let [x, y, width, height] = drawn.view_box;

        let mut scene = Scene::new(drawn.width, drawn.height);
        scene.view_box = ViewBox::new(x, y, width, height);
        scene.font_family = styles.font_family.clone();
        scene.defs = drawn.defs;
        scene.root = Group {
            common: Common::with_style(styles.root()),
            children: drawn.children,
        };
        Ok(scene)
    }
}
