//! Block diagram parity (R-108, R-110): an insta snapshot of the scene SVG
//! and the SSIM against the oracle for every case under `goldens/block/`.
//! Phase 4 tests; they run with `--features phase4`.

mod common;

use image::RgbaImage;
use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::block::{DIAGRAM, parse};
use kitty_markdown_helper::mermaid::diagrams::{Diagram, RenderCtx};
use kitty_markdown_helper::mermaid::raster::{Background, RasterOpts, render_rgba};
use kitty_markdown_helper::mermaid::scene::{Color, Scene};
use kitty_markdown_helper::mermaid::{self};
use kitty_markdown_helper::ssim::ssim_with_tolerance;
use kitty_markdown_helper::text::{self, TextMetrics};

use common::{SIZE_TOLERANCE_PX, SSIM_THRESHOLD, assert_ssim, goldens_for};

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
    render_with_theme(source, "default").expect("block renders")
}

/// The helper's PNG at the harness width; the shared `assert_ssim` crops
/// the golden to the helper's size.
fn render_at_harness_width(scene: &Scene) -> RgbaImage {
    let opts = RasterOpts {
        target_width: Some(PARITY_WIDTH),
        background: Some(Background::Color(Color::WHITE)),
    };
    render_rgba(scene, mermaid::fontdb(), &opts).expect("raster")
}

#[test]
#[cfg_attr(not(feature = "phase4"), ignore)]
fn block_svg_snapshots() {
    let cases = goldens_for("block");
    for case in cases {
        let scene = render_scene(&case.source());
        insta::assert_snapshot!(case.name.as_str(), scene.to_svg());
    }
}

/// Every case is compared before the test fails, so one run reports the
/// SSIM of all of them; the failing ones leave their diff images behind.
#[test]
#[cfg_attr(not(feature = "phase4"), ignore)]
fn block_ssim_against_oracle() {
    let cases = skip_if_no_goldens!("block");
    let mut failures = Vec::new();
    for case in cases {
        let scene = render_scene(&case.source());
        let goldens = case.goldens();
        let actual = render_at_harness_width(&scene);
        let name = format!("block_{}", case.name);
        let value = ssim_with_tolerance(&actual, &goldens[0], SIZE_TOLERANCE_PX)
            .map(|v| format!("{v:.4}"))
            .unwrap_or_else(|e| format!("size mismatch: {e}"));
        eprintln!("{name}: SSIM {value}");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_ssim(&actual, &goldens, SSIM_THRESHOLD, &name);
        }));
        if result.is_err() {
            failures.push(format!("{name}: {value}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} case(s) below {SSIM_THRESHOLD}:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// R-111, R-177: a grammar error carries jison's text and line.
#[test]
fn parse_error_carries_the_oracle_line() {
    let err = render_with_theme("block\n  a\n  end\n", "default").expect_err("stray end");
    match err {
        HelperError::ParseError {
            line,
            diagram_type,
            message,
            ..
        } => {
            assert_eq!(diagram_type, "block");
            assert!(message.starts_with("Parse error on line "), "{message}");
            assert!(line >= 2, "line {line}");
        }
        other => panic!("{other:?}"),
    }
}

/// `block-beta` and `block` both open a diagram; the db sees the same tree.
#[test]
fn block_beta_and_block_parse_alike() {
    let a = parse("block-beta\n  columns 2\n  a b c\n").expect("parses");
    let b = parse("block\n  columns 2\n  a b c\n").expect("parses");
    assert_eq!(a.blocks().len(), b.blocks().len());
    assert_eq!(a.block(a.root()).columns, Some(2));
}

/// R-104: every built-in theme renders.
#[test]
fn every_builtin_theme_renders() {
    let source =
        "block\n  columns 2\n  a[\"A\"] b((\"B\"))\n  block:c\n    d e\n  end\n  a --> b\n";
    for theme in ["default", "dark", "neutral", "forest", "base"] {
        let scene = render_with_theme(source, theme).unwrap_or_else(|e| panic!("{theme}: {e}"));
        assert!(scene.width > 0.0 && scene.height > 0.0, "{theme}");
    }
}
