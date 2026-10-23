//! C4 parity: an insta snapshot of the scene SVG and the SSIM against the
//! oracle for every case under `goldens/c4/` (R-108, R-110). C4 is a phase
//! 4 type, so the oracle comparisons run with `--features phase4`.

mod common;

use image::RgbaImage;
use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::c4::{self, DIAGRAM};
use kitty_markdown_helper::mermaid::diagrams::{Diagram, RenderCtx};
use kitty_markdown_helper::mermaid::raster::{Background, RasterOpts, render_rgba};
use kitty_markdown_helper::mermaid::scene::{Color, Scene};
use kitty_markdown_helper::mermaid::{self};
use kitty_markdown_helper::ssim::ssim_with_tolerance;
use kitty_markdown_helper::text::{self, TextMetrics};

use common::{SIZE_TOLERANCE_PX, SSIM_THRESHOLD, assert_ssim, goldens_for};

/// The width `goldens.mjs` renders the helper at; the shared `assert_ssim`
/// crops the golden to the helper's size.
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
    render_with_theme(source, "default").expect("c4 renders")
}

fn render_at_harness_width(scene: &Scene) -> RgbaImage {
    let opts = RasterOpts {
        target_width: Some(PARITY_WIDTH),
        background: Some(Background::Color(Color::WHITE)),
    };
    render_rgba(scene, mermaid::fontdb(), &opts).expect("raster")
}

#[test]
fn c4_svg_snapshots() {
    let cases = goldens_for("c4");
    for case in cases {
        let scene = render_scene(&case.source());
        insta::assert_snapshot!(case.name.as_str(), scene.to_svg());
    }
}

/// Every case is compared before the test fails, so one run reports the
/// SSIM of all of them; the failing ones leave their diff images behind.
#[test]
#[cfg_attr(not(feature = "phase4"), ignore)]
fn c4_ssim_against_oracle() {
    let cases = skip_if_no_goldens!("c4");
    let mut failures = Vec::new();
    for case in cases {
        let scene = render_scene(&case.source());
        let goldens = case.goldens();
        let actual = render_at_harness_width(&scene);
        let name = format!("c4_{}", case.name);
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

/// The db after the parse: the boundary tree, the spliced boundary types,
/// the `RelIndex` index dropped, and the style and layout updates.
#[test]
fn statements_fill_the_db() {
    let source = "C4Dynamic\n  title T\n  Enterprise_Boundary(b0, \"Bank\") {\n    Person(a, \"A\", \"desc\")\n    System_Boundary(b1, \"Inner\") {\n      SystemDb(d, \"D\")\n    }\n  }\n  System_Ext(x, \"X\", $sprite=\"cloud\")\n  RelIndex(1, a, d, \"Reads\", \"SQL\")\n  UpdateElementStyle(a, $bgColor=\"grey\")\n  UpdateRelStyle(a, d, $offsetX=\"5\")\n  UpdateLayoutConfig($c4ShapeInRow=\"3\", $c4BoundaryInRow=\"1\")\n";
    let resolved = config::resolve(source, &BaseConfig::new("default")).expect("resolves");
    let db = c4::parse(&resolved).expect("parses");
    assert_eq!(db.c4_type.as_deref(), Some("C4Dynamic"));
    assert_eq!(db.title, "T");
    let aliases: Vec<&str> = db.boundaries.iter().map(|b| b.alias.as_str()).collect();
    assert_eq!(aliases, vec!["global", "b0", "b1"]);
    assert_eq!(db.boundaries[1].fields.text("type"), Some("ENTERPRISE"));
    assert_eq!(db.boundaries[2].parent_boundary, "b0");
    assert_eq!(db.shapes[0].parent_boundary, "b0");
    assert_eq!(db.shapes[1].parent_boundary, "b1");
    assert_eq!(db.shapes[2].parent_boundary, "global");
    assert_eq!(db.shapes[2].fields.str("sprite"), Some("cloud"));
    assert_eq!(db.shapes[0].fields.str("bgColor"), Some("grey"));
    assert_eq!(db.rels.len(), 1);
    assert_eq!(db.rels[0].from, "a");
    assert_eq!(db.rels[0].fields.text("label"), Some("Reads"));
    assert_eq!(db.rels[0].fields.text("techn"), Some("SQL"));
    assert_eq!(db.c4_shape_in_row, 3);
    assert_eq!(db.c4_boundary_in_row, 1);
}

/// R-111, R-177: a grammar error carries jison's text and line.
#[test]
fn parse_error_carries_the_oracle_line() {
    let err = render_with_theme("C4Context\n  Person(a, \"A\")\n  }\n", "default")
        .expect_err("a stray brace does not parse");
    match err {
        HelperError::ParseError {
            line,
            diagram_type,
            message,
            ..
        } => {
            assert_eq!(diagram_type, "c4");
            assert!(message.starts_with("Parse error on line "), "{message}");
            assert_eq!(line, 3, "{message}");
        }
        other => panic!("{other:?}"),
    }
}

/// A relationship to an alias nobody declared fails like mermaid's throw.
#[test]
fn a_rel_to_an_unknown_alias_is_an_error() {
    let err = render_with_theme(
        "C4Context\n  Person(a, \"A\")\n  Rel(a, nobody, \"Uses\")\n",
        "default",
    )
    .expect_err("unknown endpoint");
    assert!(err.to_string().contains("nobody"), "{err}");
}

/// The grid: one 216 px wide shape inside the global boundary, and the 60 px
/// the title adds above the drawing.
#[test]
fn the_grid_sizes_the_viewbox() {
    let scene = render_scene("C4Context\n  title T\n  System(s, \"S\")\n");
    assert_eq!(scene.view_box.x, 0.0);
    assert_eq!(scene.view_box.y, -70.0);
    // The global boundary starts one diagramMarginX in (100); the shape sits
    // a c4ShapeMargin further (150) and is 216 wide; bumpLastMargin and the
    // parent's c4ShapeMargin add 50 each (466). The box starts at 50 and
    // the viewBox adds diagramMarginX on both sides.
    assert_eq!(scene.view_box.width, (466.0 - 50.0) + 2.0 * 50.0);
}

/// R-104: every built-in theme renders.
#[test]
fn every_builtin_theme_renders() {
    let source = "C4Context\n  Person(a, \"A\")\n  System(b, \"B\")\n  Rel(a, b, \"Uses\")\n";
    for theme in ["default", "dark", "neutral", "forest", "base"] {
        let scene = render_with_theme(source, theme).unwrap_or_else(|e| panic!("{theme}: {e}"));
        assert!(scene.width > 0.0 && scene.height > 0.0, "{theme}");
    }
}
