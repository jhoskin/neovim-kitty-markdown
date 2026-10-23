//! Ishikawa parity (R-109, R-110): an insta snapshot of the scene SVG and
//! the SSIM against the oracle for every case under `goldens/ishikawa/`.
//! Ishikawa is a phase 5 type, so the parity gates run under the `phase5`
//! feature. The shared harness (`helper/oracle/goldens.mjs`,
//! `tests/common/mod.rs`) handles the oracle framing.

mod common;

use image::RgbaImage;
use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::ishikawa::DIAGRAM;
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
    render_with_theme(source, "default").expect("ishikawa renders")
}

fn render_at_harness_width(scene: &Scene) -> RgbaImage {
    let opts = RasterOpts {
        target_width: Some(PARITY_WIDTH),
        background: Some(Background::Color(Color::WHITE)),
    };
    render_rgba(scene, mermaid::fontdb(), &opts).expect("raster")
}

const DOCS_EXAMPLE: &str = "ishikawa-beta\n    Blurry Photo\n    Process\n        Out of focus\n        Shutter speed too slow\n        Protective film not removed\n        Beautification filter applied\n    User\n        Shaky hands\n    Equipment\n        LENS\n            Inappropriate lens\n            Damaged lens\n            Dirty lens\n        SENSOR\n            Damaged sensor\n            Dirty sensor\n    Environment\n        Subject moved too quickly\n        Too dark\n";

#[test]
#[cfg_attr(not(feature = "phase5"), ignore)]
fn ishikawa_svg_snapshots() {
    let cases = goldens_for("ishikawa");
    for case in cases {
        let scene = render_scene(&case.source());
        insta::assert_snapshot!(case.name.as_str(), scene.to_svg());
    }
}

/// Every case is compared before the test fails, so one run reports the
/// SSIM of all of them; the failing ones leave their diff images behind.
#[test]
#[cfg_attr(not(feature = "phase5"), ignore)]
fn ishikawa_ssim_against_oracle() {
    let cases = skip_if_no_goldens!("ishikawa");
    let mut failures = Vec::new();
    for case in cases {
        let scene = render_scene(&case.source());
        let goldens = case.goldens();
        let actual = render_at_harness_width(&scene);
        let name = format!("ishikawa_{}", case.name);
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

/// The oracle SVG of the docs example has the viewBox `-379.765 -39.989
/// 471.765 580.134`: the spine ends at the leftmost text, the head path
/// reaches 1.2 times its width, and the cause boxes set the top and the
/// bottom. The spine sits at `500 * 11 / 14` below the origin.
#[test]
fn docs_example_frames_like_the_oracle() {
    let scene = render_scene(DOCS_EXAMPLE);
    let vb = scene.view_box;
    let close = |a: f64, b: f64| (a - b).abs() < 1.0;
    assert!(close(vb.x, -379.765), "{vb:?}");
    assert!(close(vb.y, -39.989), "{vb:?}");
    assert!(close(vb.width, 471.765), "{vb:?}");
    assert!(close(vb.height, 580.134), "{vb:?}");
    assert_eq!((scene.width, scene.height), (vb.width, vb.height));
    let svg = scene.to_svg();
    assert!(svg.contains("translate(0, 392.857"), "{svg}");
}

/// R-111, R-177: a grammar error carries jison's text and line.
#[test]
fn parse_error_carries_the_oracle_text() {
    let err = render_with_theme("ishikawa\n  Effect\n  ishikawa\n", "default")
        .expect_err("keyword as a cause");
    match err {
        HelperError::ParseError {
            diagram_type,
            message,
            line,
            ..
        } => {
            assert_eq!(diagram_type, "ishikawa");
            assert_eq!(line, 3);
            assert!(message.starts_with("Parse error on line 3:"), "{message}");
        }
        other => panic!("{other:?}"),
    }
}

/// An effect without causes draws the head alone, framed by the padding.
#[test]
fn an_effect_alone_is_the_head() {
    let scene = render_scene("ishikawa\n  Effect\n");
    assert!(scene.view_box.width > 40.0 && scene.view_box.width < 140.0);
    assert!(scene.to_svg().contains(">Effect<"));
}

/// R-104: every built-in theme renders.
#[test]
fn every_builtin_theme_renders() {
    for theme in ["default", "dark", "neutral", "forest", "base"] {
        let scene = render_with_theme(DOCS_EXAMPLE, theme).expect(theme);
        assert!(!scene.root.children.is_empty(), "{theme}");
    }
}
