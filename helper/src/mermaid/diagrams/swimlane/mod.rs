//! The swimlane diagram (`swimlane-beta`, R-109): mermaid 12.2.0's
//! `diagrams/swimlanes/swimlanesDiagram.ts`, which is
//! `createFlowDiagram({ styles: swimlanesStyles })`: the flowchart parser,
//! `FlowDB`, and `flowRenderer-v3-unified.ts` with other styles.
//!
//! The lane layout of `rendering-util/layout-algorithms/swimlanes/` is not
//! ported, because the oracle does not run it. Swimlanes get it only from
//! the schema default `swimlane.layout: swimlane`, and `resolveAppearance`
//! in `config.ts` reads the layers in the order directives, site config
//! delta, defaults. The base config (`layout: dagre`, R-174) is the site
//! config delta, so its top-level `layout` wins over the schema's section
//! default, and the oracle draws a swimlane diagram as a dagre flowchart
//! with plain `rect` subgraphs. A diagram that names `layout: swimlane`
//! itself gets `unsupported: layout: swimlane` (R-175).
//!
//! What differs from a flowchart is the diagram type, which
//! `flowRenderer-v3-unified.ts` hands to the layout as `data4Layout.type`:
//! it names the markers (`my-svg_swimlane-pointEnd`), and
//! `shouldMergeSelfLoopSegments` in `layout-algorithms/dagre/index.js`
//! leaves `swimlane` out, so a self-loop stays three edge paths. The two
//! rules `swimlanes/styles.ts` appends to the flowchart styles select
//! `.swimlane.cluster` and `[data-look="neo"].cluster`; with `rect`
//! subgraphs and the classic look no element matches either, so the
//! flowchart styles apply unchanged.

use crate::error::HelperError;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::flowchart::{self, db, render};
use crate::mermaid::diagrams::{Diagram, RenderCtx};
use crate::mermaid::scene::Scene;

pub struct Swimlane;

pub static DIAGRAM: Swimlane = Swimlane;

/// The detector id in `swimlanes/detector.ts`.
const TYPE: &str = "swimlane";

/// `flowchart::parse_error` with the swimlane type in the error data.
fn parse_error(err: crate::mermaid::jison::ParseError) -> HelperError {
    match flowchart::parse_error(err) {
        HelperError::ParseError {
            line,
            column,
            message,
            ..
        } => HelperError::ParseError {
            line,
            column,
            diagram_type: TYPE.to_string(),
            message,
        },
        other => other,
    }
}

impl Diagram for Swimlane {
    fn types(&self) -> &'static [&'static str] {
        &[TYPE]
    }

    fn render(&self, resolved: &Resolved, ctx: &RenderCtx<'_>) -> Result<Scene, HelperError> {
        // `flow.jison` takes `swimlane-beta` as its `GRAPH` keyword.
        let flow = flowchart::parse(resolved.code(), db::DbConfig::from_config(&resolved.config))
            .map_err(parse_error)?;
        render::draw(&flow, resolved, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::config::{self, BaseConfig};
    use crate::mermaid::diagrams;
    use crate::mermaid::scene::Element;
    use crate::text::{self, TextMetrics};

    fn render_source(source: &str) -> Result<Scene, HelperError> {
        let resolved = config::resolve(source, &BaseConfig::new("default"))?;
        let diagram = diagrams::find(resolved.diagram_type).expect("swimlane is registered");
        let fonts = text::font_system();
        let ctx = RenderCtx {
            fonts,
            metrics: TextMetrics::new(fonts),
            target_width: 960,
        };
        diagram.render(&resolved, &ctx)
    }

    #[test]
    fn the_base_config_layout_wins_over_the_swimlane_default() {
        let r = config::resolve("swimlane-beta LR\n  A-->B", &BaseConfig::new("default")).unwrap();
        assert_eq!(r.diagram_type, "swimlane");
        assert_eq!(r.config_key, "swimlane");
        assert_eq!(r.appearance.layout, "dagre");
        assert_eq!(r.appearance.look, "classic");
        assert_eq!(r.appearance.theme, "default");
    }

    #[test]
    fn an_explicit_swimlane_layout_is_unsupported() {
        for source in [
            "---\nconfig:\n  layout: swimlane\n---\nswimlane-beta LR\n  A-->B",
            "%%{init: {'swimlane': {'layout': 'swimlane'}}}%%\nswimlane-beta LR\n  A-->B",
        ] {
            match render_source(source) {
                Err(HelperError::UnsupportedFeature { feature }) => {
                    assert_eq!(feature, "layout: swimlane");
                }
                other => panic!("{source}: {other:?}"),
            }
        }
    }

    #[test]
    fn markers_carry_the_swimlane_type() {
        let scene = render_source("swimlane-beta LR\n  subgraph L\n    A-->B\n  end").unwrap();
        let ids: Vec<&str> = scene
            .defs
            .iter()
            .filter_map(|e| match e {
                Element::Marker(m) => Some(m.id.as_str()),
                _ => None,
            })
            .collect();
        assert!(ids.contains(&"my-svg_swimlane-pointEnd"), "{ids:?}");
    }

    #[test]
    fn a_self_loop_keeps_three_segments() {
        let svg = render_source("swimlane-beta TB\n  subgraph L\n    a --> a\n  end")
            .unwrap()
            .to_svg();
        for id in [
            "a-cyclic-special-1",
            "a-cyclic-special-mid",
            "a-cyclic-special-2",
        ] {
            assert!(svg.contains(id), "{id} missing");
        }
    }

    #[test]
    fn a_parse_error_names_the_swimlane_type() {
        match render_source("swimlane-beta LR\n  subgraph A\n    x --> \n  end\n") {
            Err(HelperError::ParseError {
                line,
                diagram_type,
                message,
                ..
            }) => {
                assert_eq!(line, 4);
                assert_eq!(diagram_type, "swimlane");
                assert!(message.starts_with("Parse error on line 4:"), "{message}");
                assert!(message.ends_with("got 'end'"), "{message}");
            }
            other => panic!("{other:?}"),
        }
    }
}
