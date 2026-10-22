//! The sequence diagram: mermaid 12.2.0's `diagrams/sequence` as a scene.
//!
//! `actions` replays `sequenceDiagram.jison` over the vendored tables into
//! statements, `db` applies them as `SequenceDB` does, `layout` runs
//! `sequenceRenderer.draw` with `draw` standing in for `svgDraw.js`, and
//! `styles::sequence` resolves the stylesheet. `conf` holds the config the
//! renderer reads and the fonts it measures and draws with.

pub mod actions;
pub mod conf;
pub mod db;
pub mod draw;
pub mod layout;

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::jison;
use crate::mermaid::scene::{Common, FontSpec, Group, Scene, Style, ViewBox};
use crate::mermaid::styles::sequence::SequenceStyles;

use actions::{SequenceActions, Val};
use conf::{BODY_DEFAULT_FAMILY, Conf, css_family_valid};
use db::SequenceDb;
use layout::Renderer;

pub struct Sequence;

pub static DIAGRAM: Sequence = Sequence;

/// The participant shapes the classic look draws here. The icon shapes
/// (`boundary`, `control`, `entity`, `database`, `collections`, `queue`)
/// are not ported yet.
const SUPPORTED_KINDS: [&str; 2] = ["participant", "actor"];

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

impl Diagram for Sequence {
    fn types(&self) -> &'static [&'static str] {
        &["sequence"]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        let db = parse(resolved)?;
        if let Some(actor) = db
            .actors
            .values()
            .find(|a| !SUPPORTED_KINDS.contains(&a.kind.as_str()))
        {
            return Err(HelperError::UnsupportedFeature {
                feature: format!("participant type {}", actor.kind),
            });
        }
        let styles = SequenceStyles::resolve(&resolved.theme_variables);
        let root_family = if css_family_valid(&styles.font_family) {
            styles.font_family.clone()
        } else {
            BODY_DEFAULT_FAMILY.to_string()
        };
        let conf = Conf::from_resolved(resolved, &root_family, ctx.fonts);
        let renderer = Renderer::new(conf, &styles, &ctx.metrics, &db);
        let (children, vb) = renderer.draw();

        let mut scene = Scene::new(vb[2], vb[3]);
        scene.view_box = ViewBox::new(vb[0], vb[1], vb[2], vb[3]);
        scene.font_family = root_family;
        scene.defs = draw::markers(&styles);
        // The stylesheet's root rule: `fill: textColor; font-size`.
        scene.root = Group {
            common: Common::with_style(Style {
                fill: Some(styles.text_color.clone()),
                font: FontSpec {
                    size: Some(styles.font_size),
                    ..FontSpec::default()
                },
                ..Style::default()
            }),
            children,
        };
        Ok(scene)
    }
}
