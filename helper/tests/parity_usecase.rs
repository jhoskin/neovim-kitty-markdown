//! Use case diagram parity: an insta snapshot of the scene SVG and the
//! SSIM against the oracle for every case under `goldens/usecase/`
//! (R-109, R-110). Use case is a phase 5 type, so the parity gates run
//! under the `phase5` feature.

mod common;

use image::RgbaImage;
use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::usecase::{self, DIAGRAM};
use kitty_markdown_helper::mermaid::diagrams::{Diagram, RenderCtx};
use kitty_markdown_helper::mermaid::raster::{Background, RasterOpts, render_rgba};
use kitty_markdown_helper::mermaid::scene::{Color, Scene};
use kitty_markdown_helper::mermaid::{self};
use kitty_markdown_helper::ssim::ssim_with_tolerance;
use kitty_markdown_helper::text::{self, TextMetrics};

use common::{SIZE_TOLERANCE_PX, SSIM_THRESHOLD, assert_ssim, golden_dir};

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
    render_with_theme(source, "default").expect("use case renders")
}

/// The scene at the parity width on the oracle's white page.
fn render_at_harness_width(scene: &Scene) -> RgbaImage {
    let opts = RasterOpts {
        target_width: Some(PARITY_WIDTH),
        background: Some(Background::Color(Color::WHITE)),
    };
    render_rgba(scene, mermaid::fontdb(), &opts).expect("raster")
}

/// Every `<case>.mmd` under `goldens/usecase/`, with or without a PNG, so
/// the snapshots cover a case before its golden exists.
fn sources() -> Vec<(String, String)> {
    let dir = golden_dir().join("usecase");
    let mut cases: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "mmd"))
        .filter_map(|p| {
            let name = p.file_stem()?.to_str()?.to_string();
            let source = std::fs::read_to_string(&p).ok()?;
            Some((name, source))
        })
        .collect();
    cases.sort();
    cases
}

#[test]
#[cfg_attr(not(feature = "phase5"), ignore)]
fn usecase_svg_snapshots() {
    let cases = sources();
    assert!(!cases.is_empty(), "no cases under tests/goldens/usecase");
    for (name, source) in cases {
        let scene = render_scene(&source);
        insta::assert_snapshot!(name.as_str(), scene.to_svg());
    }
}

/// Every case is compared before the test fails, so one run reports the
/// SSIM of all of them; the failing ones leave their diff images behind.
#[test]
#[cfg_attr(not(feature = "phase5"), ignore)]
fn usecase_ssim_against_oracle() {
    let cases = skip_if_no_goldens!("usecase");
    let mut failures = Vec::new();
    for case in cases {
        let scene = render_scene(&case.source());
        let goldens = case.goldens();
        let actual = render_at_harness_width(&scene);
        let name = format!("usecase_{}", case.name);
        let value = ssim_with_tolerance(
            &actual,
            &common::golden_frame(&actual, &goldens[0]),
            SIZE_TOLERANCE_PX,
        )
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

fn parse_failure(source: &str) -> (u32, String) {
    match render_with_theme(source, "default").expect_err("does not parse") {
        HelperError::ParseError {
            line,
            diagram_type,
            message,
            ..
        } => {
            assert_eq!(diagram_type, "usecase");
            (line, message)
        }
        other => panic!("{other:?}"),
    }
}

/// R-111, R-177, R-178: a Chevrotain parse error carries the oracle text
/// and its line.
#[test]
fn a_parser_error_reports_the_oracle_line() {
    let (line, message) = parse_failure("usecase-beta\nactor A\nA --> B -->\n");
    assert_eq!(line, 3);
    assert_eq!(
        message,
        "Error parsing usecase diagram: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '-->' at line 3, column 9 [29,32)"
    );
}

/// The model builder's semantic errors name the offending line first.
#[test]
fn a_semantic_error_reports_its_line() {
    let (line, message) = parse_failure("usecase-beta\nA --> B\nA ..> : include C\nactor A\n");
    assert_eq!(line, 3);
    assert_eq!(
        message,
        "include relationship requires use-case endpoints at line 3, column 3 [23,38)"
    );
}

/// R-112: an icon actor needs an icon pack.
#[test]
fn an_icon_actor_is_unsupported() {
    let err = render_with_theme("usecase-beta\nactor A@{ icon: \"fa:user\" }\n", "default")
        .expect_err("icon pack");
    assert!(
        matches!(err, HelperError::UnsupportedFeature { .. }),
        "{err:?}"
    );
}

/// Endpoints that are never declared become ellipse use cases, ordered
/// by first appearance.
#[test]
fn undeclared_endpoints_become_use_cases() {
    let db = usecase::parse_source("usecase-beta\nCustomer --> Login\nactor Customer\n")
        .expect("parses");
    assert_eq!(db.actors.keys().collect::<Vec<_>>(), ["Customer"]);
    assert_eq!(db.use_cases.keys().collect::<Vec<_>>(), ["Login"]);
    assert_eq!(db.direction, "LR");
}

/// R-104: every built-in theme renders.
#[test]
fn every_builtin_theme_renders() {
    for theme in ["default", "dark", "neutral", "forest", "base"] {
        let scene = render_with_theme("usecase-beta\nactor A\nA --> B\n", theme)
            .unwrap_or_else(|e| panic!("{theme}: {e}"));
        assert!(scene.width > 0.0 && scene.height > 0.0, "{theme}");
    }
}
