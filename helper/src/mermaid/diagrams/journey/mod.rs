//! The user journey diagram: mermaid 12.2.0's `diagrams/user-journey` as
//! a scene.
//!
//! `actions` replays `journey.jison` over the vendored tables straight into
//! `db` (the grammar calls the db from its actions), `conf` reads the
//! `journey` config section, `render` runs `journeyRenderer.draw` with
//! `svgDraw.js` folded in, and `styles::journey` resolves the stylesheet.

pub mod actions;
pub mod conf;
pub mod db;
pub mod render;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison;
use crate::mermaid::scene::{Common, Group, Scene, ViewBox};
use crate::mermaid::styles::journey::JourneyStyles;

use actions::JourneyActions;
use conf::Conf;
use db::JourneyDb;
use render::Renderer;

pub struct Journey;

pub static DIAGRAM: Journey = Journey;

/// Parses the diagram text into the db, as `parser.parse` does with
/// `yy = db`. Public for tests that look at the db.
pub fn parse(resolved: &Resolved) -> Result<JourneyDb, HelperError> {
    let grammar = jison::grammar("journey")
        .map_err(|e| HelperError::internal(format!("journey grammar: {e}")))?;
    // `Diagram.fromText` sets the front matter title before the parse, so a
    // `title` statement in the text wins.
    let mut actions = JourneyActions::new(JourneyDb::new(resolved.title()));
    jison::parse(grammar, resolved.code(), &mut actions).map_err(|e| HelperError::ParseError {
        line: e.line as u32,
        column: e.column as u32,
        diagram_type: "journey".to_string(),
        message: e.to_string(),
    })?;
    Ok(actions.db)
}

impl Diagram for Journey {
    fn types(&self) -> &'static [&'static str] {
        &["journey"]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let db = parse(resolved)?;
        let styles = JourneyStyles::resolve(&resolved.theme_variables);
        let conf = Conf::from_resolved(resolved);
        let drawn = Renderer::new(&conf, &styles, &ctx.metrics, &db).draw();
        let [x, y, width, height] = drawn.view_box;

        let mut scene = Scene::new(width, height);
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
