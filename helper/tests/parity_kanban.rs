//! Kanban parity (R-108, R-110): an insta snapshot of the scene SVG and the
//! SSIM against the oracle for every case under `goldens/kanban/`. The
//! shared harness (`helper/oracle/goldens.mjs`, `tests/common/mod.rs`)
//! handles the oracle framing.

mod common;

use image::RgbaImage;
use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::kanban::DIAGRAM;
use kitty_markdown_helper::mermaid::diagrams::{Diagram, RenderCtx};
use kitty_markdown_helper::mermaid::raster::{Background, RasterOpts, render_rgba};
use kitty_markdown_helper::mermaid::scene::{Color, Scene};
use kitty_markdown_helper::mermaid::{self};
use kitty_markdown_helper::text::{self, TextMetrics};

use common::{SSIM_THRESHOLD, assert_ssim, goldens_for};

/// The width `goldens.mjs` renders the helper at.
const PARITY_WIDTH: u32 = 960;

fn render_with_theme(source: &str, theme: &str) -> Result<Scene, HelperError> {
    let resolved = config::resolve(source, &BaseConfig::new(theme))?;
    let fonts = text::font_system();
    let ctx = RenderCtx {
        fonts,
        metrics: TextMetrics::new(fonts),
        target_width: PARITY_WIDTH,
    };
    DIAGRAM.render(&resolved, &ctx)
}

fn render_scene(source: &str) -> Scene {
    render_with_theme(source, "default").expect("kanban renders")
}

fn render_at_harness_width(scene: &Scene) -> RgbaImage {
    let opts = RasterOpts {
        target_width: Some(PARITY_WIDTH),
        background: Some(Background::Color(Color::WHITE)),
    };
    render_rgba(scene, mermaid::fontdb(), &opts).expect("raster")
}

#[test]
#[cfg_attr(not(feature = "phase4"), ignore)]
fn kanban_svg_snapshots() {
    let cases = goldens_for("kanban");
    for case in cases {
        let scene = render_scene(&case.source());
        insta::assert_snapshot!(case.name.as_str(), scene.to_svg());
    }
}

/// Every case is compared before the test fails, so one run reports the
/// SSIM of all of them; the failing ones leave their diff images behind.
#[test]
#[cfg_attr(not(feature = "phase4"), ignore)]
fn kanban_ssim_against_oracle() {
    let cases = skip_if_no_goldens!("kanban");
    let mut failures = Vec::new();
    for case in cases {
        let scene = render_scene(&case.source());
        let goldens = case.goldens();
        let actual = render_at_harness_width(&scene);
        let name = format!("kanban_{}", case.name);
        // `assert_ssim` crops the framed golden and prints the value.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_ssim(&actual, &goldens, SSIM_THRESHOLD, &name);
        }));
        if result.is_err() {
            failures.push(name);
        }
    }
    assert!(
        failures.is_empty(),
        "{} case(s) below {SSIM_THRESHOLD}, see the messages above:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// R-111, R-177: a grammar error carries jison's text and line.
#[test]
fn parse_error_carries_the_oracle_text() {
    let err = render_with_theme("kanban\n  a[b\n", "default").expect_err("unclosed bracket");
    match err {
        HelperError::ParseError {
            diagram_type,
            message,
            ..
        } => {
            assert_eq!(diagram_type, "kanban");
            assert!(message.starts_with("Parse error on line "), "{message}");
        }
        other => panic!("{other:?}"),
    }
}

/// A db error is thrown from the grammar action with the db's own message.
#[test]
fn items_without_a_section_are_the_diagram_error() {
    let err = render_with_theme("kanban\n    a\n  b\n    c\n", "default").expect_err("loose item");
    match err {
        HelperError::Internal(message) => {
            assert_eq!(
                message,
                "Items without section detected, found section (\"b\")"
            );
        }
        other => panic!("{other:?}"),
    }
}

/// The layout the renderer hard codes: columns `sectionWidth` apart plus a
/// 5 px gap, the first card 25 px below the section top, cards 5 px apart,
/// and the mindmap padding of 10 around the `getBBox` of everything.
#[test]
fn columns_and_cards_follow_the_renderer_geometry() {
    let scene = render_scene("kanban\n  a[A]\n    a1[one]\n    a2[two]\n  b[B]\n");
    // Two 200 px columns from x = 100 to x = 505, plus 10 px padding.
    assert_eq!(scene.view_box.x, 90.0);
    assert_eq!(scene.width, 425.0);
    assert_eq!(scene.view_box.y, -310.0);
    // Column A: two 44 px cards (24 px label plus 20 px) from y = -275,
    // each followed by a 5 px gap, and 30 px below them: 44 + 5 + 44 + 5
    // + 30 = 128.
    let svg = scene.to_svg();
    assert!(svg.contains("height=\"128\""), "{svg}");
    // Column B is empty and keeps the 50 px minimum.
    assert!(svg.contains("height=\"50\""), "{svg}");
    assert_eq!(scene.height, 128.0 + 20.0);
}

/// R-104: every built-in theme renders.
#[test]
fn every_builtin_theme_renders() {
    let source = "kanban\n  todo[Todo]\n    t1[Draft]@{ ticket: T-1, assigned: 'sam', priority: 'High' }\n  done[Done]\n    d1[Ship]\n";
    for theme in ["default", "dark", "neutral", "forest", "base"] {
        let scene = render_with_theme(source, theme).unwrap_or_else(|e| panic!("{theme}: {e}"));
        assert!(scene.width > 0.0 && scene.height > 0.0, "{theme}");
    }
}

/// `document` needs one statement, so a bare `kanban` line is a parse
/// error in the oracle too.
#[test]
fn an_empty_board_is_a_parse_error() {
    let err = render_with_theme("kanban\n", "default").expect_err("no statement");
    assert_eq!(err.code(), "parse_error");
}
