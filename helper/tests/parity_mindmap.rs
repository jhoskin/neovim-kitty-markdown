//! Mindmap parity (R-107, R-110, R-182): an insta snapshot of the scene SVG
//! and the SSIM against the oracle for every case under
//! `goldens/mindmap/`. `goldens.mjs` writes five goldens per case; under
//! the base config's `layout: dagre` the oracle is deterministic, so they
//! are identical and the best-of-five rule reduces to one comparison.

mod common;

use image::RgbaImage;
use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::mindmap::DIAGRAM;
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
    render_with_theme(source, "default").expect("mindmap renders")
}

fn render_at_harness_width(scene: &Scene) -> RgbaImage {
    let opts = RasterOpts {
        target_width: Some(PARITY_WIDTH),
        background: Some(Background::Color(Color::WHITE)),
    };
    render_rgba(scene, mermaid::fontdb(), &opts).expect("raster")
}

#[test]
#[cfg_attr(not(feature = "phase3"), ignore)]
fn mindmap_svg_snapshots() {
    for case in goldens_for("mindmap") {
        let scene = render_scene(&case.source());
        insta::assert_snapshot!(case.name.as_str(), scene.to_svg());
    }
}

/// Every case is compared before the test fails, so one run reports the
/// SSIM of all of them; the failing ones leave their diff images behind.
#[test]
#[cfg_attr(not(feature = "phase3"), ignore)]
fn mindmap_ssim_against_oracle() {
    let cases = skip_if_no_goldens!("mindmap");
    let mut failures = Vec::new();
    for case in cases {
        let scene = render_scene(&case.source());
        let goldens = case.goldens();
        let actual = render_at_harness_width(&scene);
        let name = format!("mindmap_{}", case.name);
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
    let err = render_with_theme("mindmap\n  a[b\n", "default").expect_err("unclosed bracket");
    match err {
        HelperError::ParseError {
            diagram_type,
            message,
            ..
        } => {
            assert_eq!(diagram_type, "mindmap");
            assert!(message.starts_with("Parse error on line "), "{message}");
        }
        other => panic!("{other:?}"),
    }
}

/// A second root is thrown from the grammar action with the db's message.
#[test]
fn a_second_root_is_the_diagram_error() {
    let err = render_with_theme("mindmap\n  a\n  b\n", "default").expect_err("two roots");
    match err {
        HelperError::Internal(message) => assert_eq!(
            message,
            "There can be only one root. No parent could be found for (\"b\")"
        ),
        other => panic!("{other:?}"),
    }
}

/// The layout is dagre top to bottom: the root above its children, which
/// share one rank, and the mindmap padding of 10 around the content.
#[test]
fn the_root_sits_above_one_rank_of_children() {
    let scene = render_scene("mindmap\n  root((R))\n    A\n    B\n");
    let svg = scene.to_svg();
    // dagre's 8 px margin minus the 10 px padding.
    assert_eq!(scene.view_box.x, -2.0);
    assert_eq!(scene.view_box.y, -2.0);
    assert!(svg.contains("section-edge-0 edge-depth-1"), "{svg}");
    assert!(svg.contains("section-edge-1 edge-depth-1"), "{svg}");
}

/// R-104: every built-in theme renders.
#[test]
fn every_builtin_theme_renders() {
    let source =
        "mindmap\n  root((Root))\n    a[A]\n      b(B)\n    c{{C}}\n    d))D((\n    e)E(\n";
    for theme in ["default", "dark", "neutral", "forest", "base"] {
        let scene = render_with_theme(source, theme).unwrap_or_else(|e| panic!("{theme}: {e}"));
        assert!(scene.width > 0.0 && scene.height > 0.0, "{theme}");
    }
}

/// R-175: the cose-bilkent layout mermaid picks without a pinned layout is
/// not one the helper renders.
#[test]
fn an_explicit_cose_bilkent_layout_is_unsupported() {
    let err = render_with_theme(
        "---\nconfig:\n  layout: cose-bilkent\n---\nmindmap\n  a\n",
        "default",
    )
    .expect_err("cose-bilkent");
    assert_eq!(err.code(), "unsupported_feature");
}
