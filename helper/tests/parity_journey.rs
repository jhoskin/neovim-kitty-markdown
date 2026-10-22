//! User journey parity: an insta snapshot of the scene SVG and the SSIM
//! against the oracle for every case under `goldens/journey/` (R-107,
//! R-110). The journey is a phase 3 type, so the oracle comparisons run
//! with `--features phase3`.

mod common;

use image::RgbaImage;
use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::journey::{self, DIAGRAM};
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
    render_with_theme(source, "default").expect("journey renders")
}

fn render_at_harness_width(scene: &Scene) -> RgbaImage {
    let opts = RasterOpts {
        target_width: Some(PARITY_WIDTH),
        background: Some(Background::Color(Color::WHITE)),
    };
    render_rgba(scene, mermaid::fontdb(), &opts).expect("raster")
}

#[test]
fn journey_svg_snapshots() {
    let cases = goldens_for("journey");
    for case in cases {
        let scene = render_scene(&case.source());
        insta::assert_snapshot!(case.name.as_str(), scene.to_svg());
    }
}

/// Every case is compared before the test fails, so one run reports the
/// SSIM of all of them; the failing ones leave their diff images behind.
#[test]
#[cfg_attr(not(feature = "phase3"), ignore)]
fn journey_ssim_against_oracle() {
    let cases = skip_if_no_goldens!("journey");
    let mut failures = Vec::new();
    for case in cases {
        let scene = render_scene(&case.source());
        let goldens = case.goldens();
        let actual = render_at_harness_width(&scene);
        let name = format!("journey_{}", case.name);
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

/// The db after the parse: sections, tasks, people, and the sorted actors.
#[test]
fn the_docs_example_parses_into_the_db() {
    let source = "journey\n    title My working day\n    section Go to work\n      Make tea: 5: Me\n      Go upstairs: 3: Me\n      Do work: 1: Me, Cat\n    section Go home\n      Go downstairs: 5: Me\n      Sit down: 5: Me\n";
    let resolved = config::resolve(source, &BaseConfig::new("default")).expect("resolves");
    let db = journey::parse(&resolved).expect("parses");
    assert_eq!(db.title(), Some("My working day"));
    assert_eq!(db.sections, vec!["Go to work", "Go home"]);
    assert_eq!(db.tasks.len(), 5);
    assert_eq!(db.tasks[2].task, "Do work");
    assert_eq!(db.tasks[2].score, 1.0);
    assert_eq!(db.tasks[2].people, vec!["Me", "Cat"]);
    assert_eq!(db.tasks[4].section, "Go home");
    assert_eq!(db.actors(), vec!["Cat", "Me"]);
}

/// R-111, R-177: a grammar error carries jison's text and line.
#[test]
fn parse_error_carries_the_oracle_line() {
    let err = render_with_theme(
        "journey\n  title Day\n  section Work\n    Task without score\n",
        "default",
    )
    .expect_err("a task needs a score");
    match err {
        HelperError::ParseError {
            line,
            diagram_type,
            message,
            ..
        } => {
            assert_eq!(diagram_type, "journey");
            assert!(message.starts_with("Parse error on line "), "{message}");
            assert!(line >= 4, "line {line}");
        }
        other => panic!("{other:?}"),
    }
}

/// The front matter title stands in when the text has no `title`.
#[test]
fn the_front_matter_title_is_drawn() {
    let scene = render_scene("---\ntitle: From the front matter\n---\njourney\n  Task: 3: Me\n");
    let svg = scene.to_svg();
    assert!(svg.contains("From the front matter"), "{svg}");
    // The title adds 70 px to the viewBox height and the box starts at -25.
    assert_eq!(scene.view_box.y, -25.0);
    assert_eq!(scene.view_box.height, 470.0 + 70.0);
}

/// Without a title the drawing is the grid alone: two margins of 10 around
/// the 450 px the tasks reserve, and 200 px of width per task.
#[test]
fn the_grid_sizes_the_viewbox() {
    let scene = render_scene("journey\n  A: 3: Me\n  B: 4: Me\n");
    assert_eq!(scene.view_box.height, 470.0);
    // leftMargin 150 (the legend name `Me` is narrower than the slack),
    // two tasks, and the right margin: 150 + (200 + 100) + 100.
    assert_eq!(scene.view_box.width, 2.0 * 150.0 + 2.0 * 200.0);
}

/// R-104: every built-in theme renders.
#[test]
fn every_builtin_theme_renders() {
    let source =
        "journey\n  title T\n  section S\n    A: 1: Me\n    B: 3: Me, You\n    C: 5: You\n";
    for theme in ["default", "dark", "neutral", "forest", "base"] {
        let scene = render_with_theme(source, theme).unwrap_or_else(|e| panic!("{theme}: {e}"));
        assert!(scene.width > 0.0 && scene.height > 0.0, "{theme}");
    }
}
