//! Sankey diagram parity: an insta snapshot of the scene SVG and the SSIM
//! against the oracle for every case under `goldens/sankey/` (R-108,
//! R-110). Sankey is a phase 4 type, so the parity gates run under the
//! `phase4` feature.

mod common;

use image::RgbaImage;
use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::sankey::{self, DIAGRAM};
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
    render_with_theme(source, "default").expect("sankey renders")
}

/// The diagram is `useMaxWidth`, 600 wide by default, so it renders at its
/// natural size inside the 960 target (R-180) on the oracle's white page.
fn render_at_harness_width(scene: &Scene) -> RgbaImage {
    let opts = RasterOpts {
        target_width: Some(PARITY_WIDTH),
        background: Some(Background::Color(Color::WHITE)),
    };
    render_rgba(scene, mermaid::fontdb(), &opts).expect("raster")
}

#[test]
#[cfg_attr(not(feature = "phase4"), ignore)]
fn sankey_svg_snapshots() {
    for case in goldens_for("sankey") {
        let scene = render_scene(&case.source());
        insta::assert_snapshot!(case.name.as_str(), scene.to_svg());
    }
}

/// Every case is compared before the test fails, so one run reports the
/// SSIM of all of them; the failing ones leave their diff images behind.
#[test]
#[cfg_attr(not(feature = "phase4"), ignore)]
fn sankey_ssim_against_oracle() {
    let cases = skip_if_no_goldens!("sankey");
    let mut failures = Vec::new();
    for case in cases {
        let scene = render_scene(&case.source());
        let goldens = case.goldens();
        let actual = render_at_harness_width(&scene);
        let name = format!("sankey_{}", case.name);
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

/// The CSV rows: quoted fields keep commas, `""` is a quote, and blank
/// lines between rows are allowed.
#[test]
fn parser_reads_csv_rows() {
    let db = sankey::parse("sankey\n\n\"a, b\",c,1.5\n\n\"say \"\"hi\"\"\",c,2\nc,d,3\n")
        .expect("parses");
    assert_eq!(db.nodes(), ["a, b", "c", "say \"hi\"", "d"]);
    assert_eq!(db.links(), [(0, 1, 1.5), (2, 1, 2.0), (1, 3, 3.0)]);
}

/// R-111, R-177: a row without three fields is a jison parse error with
/// the oracle's line.
#[test]
fn a_short_row_is_a_parse_error() {
    let err = render_with_theme("sankey\na,b,1\nc,d\n", "default").expect_err("two fields");
    match err {
        HelperError::ParseError {
            line,
            diagram_type,
            message,
            ..
        } => {
            assert_eq!(diagram_type, "sankey");
            assert!(message.starts_with("Parse error on line 3:"), "{message}");
            assert_eq!(line, 3);
        }
        other => panic!("{other:?}"),
    }
}

/// d3-sankey throws on a cycle; mermaid reports the thrown message.
#[test]
fn a_cycle_fails_with_the_d3_message() {
    let err = render_with_theme("sankey\na,b,1\nb,a,1\n", "default").expect_err("cycle");
    assert!(err.to_string().contains("circular link"), "{err}");
}

/// R-104: every built-in theme renders.
#[test]
fn every_builtin_theme_renders() {
    for theme in ["default", "dark", "neutral", "forest", "base"] {
        let scene = render_with_theme("sankey\na,b,1\nb,c,2\n", theme)
            .unwrap_or_else(|e| panic!("{theme}: {e}"));
        assert!(scene.width > 0.0 && scene.height > 0.0, "{theme}");
    }
}

/// The default extent is 600 by 400 and the default link color is a
/// gradient from the source color to the target color.
#[test]
fn default_layout_fills_the_extent_with_gradients() {
    let scene = render_scene("sankey\na,b,10\na,c,5\n");
    assert_eq!((scene.width, scene.height), (600.0, 400.0));
    let svg = scene.to_svg();
    assert!(
        svg.contains("<linearGradient id=\"linearGradient-4\""),
        "{svg}"
    );
    assert!(svg.contains("stroke=\"url(#linearGradient-4)\""), "{svg}");
    assert!(svg.contains(">a 15</text>"), "{svg}");
}
