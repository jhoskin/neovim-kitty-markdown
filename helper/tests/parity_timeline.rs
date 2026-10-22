//! Timeline diagram parity: an insta snapshot of the scene SVG and the SSIM
//! against the oracle for every case under `goldens/timeline/` (R-107,
//! R-110). The timeline is a phase 3 type, so the parity gates run under
//! the `phase3` feature.

mod common;

use image::RgbaImage;
use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::timeline::{self, DIAGRAM};
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
    render_with_theme(source, "default").expect("timeline renders")
}

/// The shared `assert_ssim` crops the golden to the helper's size, so the
/// helper renders at the harness width with the oracle's white background.
fn render_at_harness_width(scene: &Scene) -> RgbaImage {
    let opts = RasterOpts {
        target_width: Some(PARITY_WIDTH),
        background: Some(Background::Color(Color::WHITE)),
    };
    render_rgba(scene, mermaid::fontdb(), &opts).expect("raster")
}

#[test]
#[cfg_attr(not(feature = "phase3"), ignore)]
fn timeline_svg_snapshots() {
    let cases = goldens_for("timeline");
    for case in cases {
        let scene = render_scene(&case.source());
        insta::assert_snapshot!(case.name.as_str(), scene.to_svg());
    }
}

/// Every case is compared before the test fails, so one run reports the
/// SSIM of all of them; the failing ones leave their diff images behind.
#[test]
#[cfg_attr(not(feature = "phase3"), ignore)]
fn timeline_ssim_against_oracle() {
    let cases = skip_if_no_goldens!("timeline");
    let mut failures = Vec::new();
    for case in cases {
        let scene = render_scene(&case.source());
        let goldens = case.goldens();
        let actual = render_at_harness_width(&scene);
        let name = format!("timeline_{}", case.name);
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

/// The db after the grammar ran: sections, periods, and events in order.
#[test]
fn parser_fills_sections_periods_and_events() {
    let db = timeline::parse(
        "timeline\n    title History\n    section Early\n    2002 : LinkedIn\n    2004 : Facebook : Google\n         : Orkut\n    section Later\n    2005 : YouTube\n",
    )
    .expect("parses");
    assert_eq!(db.diagram_title(), "History");
    assert_eq!(db.sections(), ["Early", "Later"]);
    let tasks = db.tasks();
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].task, "2002 ");
    assert_eq!(tasks[0].section, "Early");
    assert_eq!(tasks[0].events, ["LinkedIn"]);
    // The event rule matches up to the next `:`, trailing space included.
    assert_eq!(tasks[1].events, ["Facebook ", "Google", "Orkut"]);
    assert_eq!(tasks[2].section, "Later");
    assert_eq!(db.direction(), "LR");
}

/// R-111, R-177: a grammar error carries jison's text and line.
#[test]
fn parse_error_carries_the_oracle_line() {
    let err = render_with_theme(
        "timeline\n  title x\n  section a\n  : lonely event\n",
        "default",
    )
    .expect_err("an event before any period throws in the action");
    match err {
        HelperError::Internal(message) => {
            assert!(
                message.contains("Cannot read properties of undefined"),
                "{message}"
            );
        }
        other => panic!("{other:?}"),
    }
    // A `:` not followed by white space is neither an event nor part of a
    // period, so the lexer returns INVALID.
    let err = render_with_theme("timeline\n  2002 : a\n  2003 :b\n", "default")
        .expect_err("a colon without a space is INVALID");
    match err {
        HelperError::ParseError {
            line,
            diagram_type,
            message,
            ..
        } => {
            assert_eq!(diagram_type, "timeline");
            assert!(message.starts_with("Parse error on line "), "{message}");
            assert_eq!(line, 3);
        }
        other => panic!("{other:?}"),
    }
}

/// The vertical renderer is not ported.
#[test]
fn vertical_direction_is_an_unsupported_feature() {
    let err = render_with_theme("timeline TD\n  title x\n  2002 : a\n", "default")
        .expect_err("TD selects timelineRendererVertical");
    match err {
        HelperError::UnsupportedFeature { feature } => {
            assert_eq!(feature, "timeline direction TD");
        }
        other => panic!("{other:?}"),
    }
}

/// R-104: every built-in theme renders, and the title-only diagram too.
#[test]
fn every_builtin_theme_renders() {
    let source = "timeline\n  title T\n  section S\n    2002 : a : b\n    2003 : c\n";
    for theme in ["default", "dark", "neutral", "forest", "base"] {
        let scene = render_with_theme(source, theme).unwrap_or_else(|e| panic!("{theme}: {e}"));
        assert!(scene.width > 0.0 && scene.height > 0.0, "{theme}");
    }
    let bare = render_scene("timeline\n  title Only\n");
    assert!(bare.width > 0.0 && bare.height > 0.0);
    let empty = render_scene("timeline\n");
    assert!(empty.width > 0.0 && empty.height > 0.0);
}

/// `drawNode` sizes every task box from `[object Object]`, so one-line and
/// two-line periods share one height, and a section box spans its tasks.
#[test]
fn boxes_follow_the_renderer_arithmetic() {
    let scene = render_scene("timeline\n  section S\n    2002 : a\n    2003 : b\n");
    let svg = scene.to_svg();
    // Section node at (200, 50): 50 + leftMargin 150; two tasks make it
    // 200 * 2 - 50 + 40 = 390 wide.
    assert!(svg.contains("translate(200, 50)"), "{svg}");
    // The second task sits 200 px right of the first.
    assert!(svg.contains("translate(400,"), "{svg}");
}
