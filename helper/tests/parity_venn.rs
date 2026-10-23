//! Venn parity (R-109, R-110): an insta snapshot of the scene SVG and the
//! SSIM against the oracle for every case under `goldens/venn/`. Venn is a
//! phase 5 type, so the parity gates run under the `phase5` feature. The
//! shared harness (`helper/oracle/goldens.mjs`, `tests/common/mod.rs`)
//! handles the oracle framing.

mod common;

use image::RgbaImage;
use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::venn::DIAGRAM;
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
    render_with_theme(source, "default").expect("venn renders")
}

fn render_at_harness_width(scene: &Scene) -> RgbaImage {
    let opts = RasterOpts {
        target_width: Some(PARITY_WIDTH),
        background: Some(Background::Color(Color::WHITE)),
    };
    render_rgba(scene, mermaid::fontdb(), &opts).expect("raster")
}

#[test]
#[cfg_attr(not(feature = "phase5"), ignore)]
fn venn_svg_snapshots() {
    let cases = goldens_for("venn");
    for case in cases {
        let scene = render_scene(&case.source());
        insta::assert_snapshot!(case.name.as_str(), scene.to_svg());
    }
}

/// Every case is compared before the test fails, so one run reports the
/// SSIM of all of them; the failing ones leave their diff images behind.
#[test]
#[cfg_attr(not(feature = "phase5"), ignore)]
fn venn_ssim_against_oracle() {
    let cases = skip_if_no_goldens!("venn");
    let mut failures = Vec::new();
    for case in cases {
        let scene = render_scene(&case.source());
        let goldens = case.goldens();
        let actual = render_at_harness_width(&scene);
        let name = format!("venn_{}", case.name);
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
    let err = render_with_theme("venn-beta\n  set A\n  set : 3\n", "default")
        .expect_err("missing identifier");
    match err {
        HelperError::ParseError {
            diagram_type,
            message,
            line,
            ..
        } => {
            assert_eq!(diagram_type, "venn");
            assert_eq!(line, 3);
            assert!(message.starts_with("Parse error on line 3:"), "{message}");
        }
        other => panic!("{other:?}"),
    }
}

/// A union of an undeclared set fails in the grammar action with the db's
/// message.
#[test]
fn unknown_union_set_is_the_diagram_error() {
    let err =
        render_with_theme("venn-beta\n  set A\n  union A,B\n", "default").expect_err("unknown set");
    match err {
        HelperError::Internal(message) => assert_eq!(message, "unknown set identifier: B"),
        other => panic!("{other:?}"),
    }
}

/// `configureSvgSize` over the `0 0 width height` viewBox, with the venn
/// config defaults of 800 by 450 and `venn.width` from a directive.
#[test]
fn the_scene_takes_the_configured_size() {
    let scene = render_scene("venn-beta\n  set A\n  set B\n  union A,B\n");
    assert_eq!((scene.width, scene.height), (800.0, 450.0));
    let scene = render_scene(
        "%%{init: {\"venn\": {\"width\": 600, \"height\": 400}}}%%\nvenn-beta\n  set A\n",
    );
    assert_eq!((scene.width, scene.height), (600.0, 400.0));
    assert_eq!(scene.view_box.width, 600.0);
}

/// The circle paths come from the venn.js layout: for the docs example the
/// first circle sits at (319.161, 283.008) below the 24 px title band, as
/// in the oracle SVG.
#[test]
fn the_docs_example_places_circles_like_the_oracle() {
    let scene = render_scene(
        "venn-beta\n  title What makes a good feature\n  set Desirable\n  set Feasible\n  set Viable\n  union Desirable,Feasible[\"Buildable\"]\n  union Feasible,Viable[\"Sustainable\"]\n  union Desirable,Viable[\"Marketable\"]\n  union Desirable,Feasible,Viable[\"Ship it\"]\n",
    );
    let svg = scene.to_svg();
    // `M 319.16 283.008 m -127.992 0` as an absolute move.
    assert!(svg.contains("M191.169,283.008"), "{svg}");
    assert!(svg.contains("translate(0, 24)"), "{svg}");
    // The region label at the floored text centre, as in the oracle.
    assert!(
        svg.contains(r#"x="400" y="236""#) && svg.contains(">Ship it<"),
        "{svg}"
    );
}

/// R-104: every built-in theme renders.
#[test]
fn every_builtin_theme_renders() {
    let source = "venn-beta\n  title T\n  set A\n    text a1\n  set B\n  union A,B[\"AB\"]\n";
    for theme in ["default", "dark", "neutral", "forest", "base"] {
        let scene = render_with_theme(source, theme).expect(theme);
        assert!(!scene.root.children.is_empty(), "{theme}");
    }
}
