//! Sequence diagram parity: an insta snapshot of the scene SVG and the SSIM
//! against the oracle for every case under `goldens/sequence/` (R-105,
//! R-110).

mod common;

use image::RgbaImage;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::sequence::DIAGRAM;
use kitty_markdown_helper::mermaid::diagrams::{Diagram, RenderCtx};
use kitty_markdown_helper::mermaid::raster::{Background, RasterOpts, render_rgba};
use kitty_markdown_helper::mermaid::scene::{Color, Scene};
use kitty_markdown_helper::mermaid::{self};
use kitty_markdown_helper::text::{self, TextMetrics};

use kitty_markdown_helper::ssim::ssim_with_tolerance;

use common::{SIZE_TOLERANCE_PX, SSIM_THRESHOLD, assert_ssim, goldens_for};

/// The width `goldens.mjs` renders the helper at.
const PARITY_WIDTH: u32 = 960;

fn render_scene(source: &str) -> Scene {
    let resolved = config::resolve(source, &BaseConfig::new("default")).expect("config resolves");
    let fonts = text::font_system();
    let ctx = RenderCtx {
        fonts,
        metrics: TextMetrics::new(fonts),
        target_width: PARITY_WIDTH,
    };
    DIAGRAM.render(&resolved, &ctx).expect("sequence renders")
}

/// The width `goldens.mjs` renders the helper with before it sizes the
/// oracle; the shared `assert_ssim` crops the golden to the helper's size.
const HARNESS_WIDTH: u32 = 960;

fn render_at_harness_width(scene: &Scene) -> RgbaImage {
    let opts = RasterOpts {
        target_width: Some(HARNESS_WIDTH),
        background: Some(Background::Color(Color::WHITE)),
    };
    render_rgba(scene, mermaid::fontdb(), &opts).expect("raster")
}

#[test]
fn sequence_svg_snapshots() {
    let cases = goldens_for("sequence");
    for case in cases {
        let scene = render_scene(&case.source());
        insta::assert_snapshot!(case.name.as_str(), scene.to_svg());
    }
}

/// Every case is compared before the test fails, so one run reports the
/// SSIM of all of them; the failing ones leave their diff images behind.
#[test]
fn sequence_ssim_against_oracle() {
    let cases = skip_if_no_goldens!("sequence");
    let mut failures = Vec::new();
    for case in cases {
        let scene = render_scene(&case.source());
        let goldens = case.goldens();
        let actual = render_at_harness_width(&scene);
        let name = format!("sequence_{}", case.name);
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

fn render_with_theme(
    source: &str,
    theme: &str,
) -> Result<Scene, kitty_markdown_helper::error::HelperError> {
    let resolved = config::resolve(source, &BaseConfig::new(theme))?;
    let fonts = text::font_system();
    let ctx = RenderCtx {
        fonts,
        metrics: TextMetrics::new(fonts),
        target_width: PARITY_WIDTH,
    };
    DIAGRAM.render(&resolved, &ctx)
}

/// R-111, R-177: a grammar error carries jison's text and line.
#[test]
fn parse_error_carries_the_oracle_line() {
    let err = render_with_theme("sequenceDiagram\n  A->>B: hi\n  loop\n", "default")
        .expect_err("unterminated loop");
    match err {
        kitty_markdown_helper::error::HelperError::ParseError {
            line,
            diagram_type,
            message,
            ..
        } => {
            assert_eq!(diagram_type, "sequence");
            assert!(message.starts_with("Parse error on line "), "{message}");
            assert!(line >= 3, "line {line}");
        }
        other => panic!("{other:?}"),
    }
}

/// A db error is thrown from `yy.apply` with the db's own message.
#[test]
fn unbalanced_activation_is_a_parse_error() {
    let err = render_with_theme("sequenceDiagram\n  A->>B: hi\n  B-->>-A: bye\n", "default")
        .expect_err("inactive participant");
    match err {
        kitty_markdown_helper::error::HelperError::ParseError { line, message, .. } => {
            assert_eq!(message, "Trying to inactivate an inactive participant (B)");
            assert_eq!(line, 3);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn icon_participant_shapes_are_unsupported_features() {
    let err = render_with_theme(
        "sequenceDiagram\n  participant DB@{ \"type\": \"database\" }\n  A->>DB: query\n",
        "default",
    )
    .expect_err("database shape");
    match err {
        kitty_markdown_helper::error::HelperError::UnsupportedFeature { feature } => {
            assert_eq!(feature, "participant type database");
        }
        other => panic!("{other:?}"),
    }
}

/// R-104: every built-in theme renders.
#[test]
fn every_builtin_theme_renders() {
    let source = "sequenceDiagram\n  autonumber\n  Alice->>+John: Hello\n  John-->>-Alice: Hi\n  Note over Alice,John: ok\n  loop x\n    Alice->>Alice: y\n  end\n";
    for theme in ["default", "dark", "neutral", "forest", "base"] {
        let scene = render_with_theme(source, theme).unwrap_or_else(|e| panic!("{theme}: {e}"));
        assert!(scene.width > 0.0 && scene.height > 0.0, "{theme}");
    }
}
