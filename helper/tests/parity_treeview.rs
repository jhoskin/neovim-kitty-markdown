//! TreeView parity (R-109, R-110): an insta snapshot of the scene SVG and
//! the SSIM against the oracle for every case under `goldens/treeview/`.
//! The shared harness (`helper/oracle/goldens.mjs`, `tests/common/mod.rs`)
//! handles the oracle framing.

mod common;

use image::RgbaImage;
use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::treeview::DIAGRAM;
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
    render_with_theme(source, "default").expect("treeView renders")
}

fn render_at_harness_width(scene: &Scene) -> RgbaImage {
    let opts = RasterOpts {
        target_width: Some(PARITY_WIDTH),
        background: Some(Background::Color(Color::WHITE)),
    };
    render_rgba(scene, mermaid::fontdb(), &opts).expect("raster")
}

fn parse_error(source: &str) -> (u32, u32, String) {
    match render_with_theme(source, "default") {
        Err(HelperError::ParseError {
            diagram_type,
            message,
            line,
            column,
        }) => {
            assert_eq!(diagram_type, "treeView");
            (line, column, message)
        }
        other => panic!("{other:?}"),
    }
}

#[test]
#[cfg_attr(not(feature = "phase5"), ignore)]
fn treeview_svg_snapshots() {
    let cases = goldens_for("treeview");
    for case in cases {
        let scene = render_scene(&case.source());
        insta::assert_snapshot!(case.name.as_str(), scene.to_svg());
    }
}

/// Every case is compared before the test fails, so one run reports the
/// SSIM of all of them; the failing ones leave their diff images behind.
#[test]
#[cfg_attr(not(feature = "phase5"), ignore)]
fn treeview_ssim_against_oracle() {
    let cases = skip_if_no_goldens!("treeview");
    let mut failures = Vec::new();
    for case in cases {
        let scene = render_scene(&case.source());
        let goldens = case.goldens();
        let actual = render_at_harness_width(&scene);
        let name = format!("treeview_{}", case.name);
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

/// The oracle's viewBox for the docs example is `-0.5 0 134.03125 174`:
/// six 29 px rows, and the widest row is `package.json` at depth 2.
#[test]
fn the_view_box_is_the_tree_size() {
    let scene = render_scene(
        "treeView-beta\n    my-project/\n        src/\n            index.js\n        package.json\n        README.md\n",
    );
    assert_eq!((scene.view_box.x, scene.view_box.y), (-0.5, 0.0));
    assert_eq!(scene.height, 174.0);
    assert!((scene.width - 134.03125).abs() < 0.5, "{}", scene.width);
}

/// R-111: a grammar error carries the Langium text and position.
#[test]
fn parse_error_carries_the_oracle_text() {
    let (line, column, message) = parse_error("treeView-beta\n  a\ntitle T\n  b\n");
    assert_eq!((line, column), (3, 1));
    assert_eq!(
        message,
        "Parsing failed:  Parse error on line 3, column 1: Expecting token of type 'EOF' but found `title T`."
    );
}

/// A Langium error on box-drawing input names the source line, and the
/// preprocessor's own errors name it too.
#[test]
fn box_drawing_errors_name_the_source_line() {
    let (line, _, message) = parse_error("treeView-beta\n├── a/\n│\n│   └── \"b\n");
    assert_eq!(line, 4);
    assert!(
        message.starts_with("Parsing failed: Lexer error on line 4, column 9:"),
        "{message}"
    );
    let (line, _, message) = parse_error("treeView-beta\n├── a\n├──\n");
    assert_eq!(line, 3);
    assert!(message.starts_with("Line 3: Empty node"), "{message}");
}

/// R-112: an icon outside the built-in file and folder icons needs an icon
/// pack.
#[test]
fn an_icon_pack_is_an_unsupported_feature() {
    for source in [
        "treeView-beta\n  App.tsx icon(logos:react)\n",
        "---\nconfig:\n  treeView:\n    showIcons: true\n    extensionIcons:\n      .ts: typescript\n---\ntreeView-beta\n  a.ts\n",
    ] {
        match render_with_theme(source, "default") {
            Err(HelperError::UnsupportedFeature { feature }) => assert_eq!(feature, "icon pack"),
            other => panic!("{other:?}"),
        }
    }
}

/// R-104: every built-in theme renders.
#[test]
fn every_builtin_theme_renders() {
    let source = "treeView-beta\n  src/ :::highlight\n    a.ts ## d\n  b\n";
    for theme in ["default", "dark", "neutral", "forest", "base"] {
        let scene = render_with_theme(source, theme).unwrap_or_else(|e| panic!("{theme}: {e}"));
        assert!(scene.width > 0.0 && scene.height > 0.0, "{theme}");
    }
}

/// A bare keyword draws the root row alone.
#[test]
fn an_empty_tree_draws_the_root() {
    let scene = render_scene("treeView-beta\n");
    assert!(scene.width > 0.0);
    assert!(scene.height > 0.0);
}
