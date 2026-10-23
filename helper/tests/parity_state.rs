//! State diagram parity (R-105, R-110): an insta SVG snapshot of the scene
//! for every case under `tests/goldens/state/` and an SSIM check against
//! the oracle PNG at the same pixel size (R-181). State is a phase 1 type,
//! so nothing here is feature-gated.

mod common;

use std::path::{Path, PathBuf};

use image::RgbaImage;
use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::state::DIAGRAM;
use kitty_markdown_helper::mermaid::diagrams::{Diagram, RenderCtx};
use kitty_markdown_helper::mermaid::raster::Background;
use kitty_markdown_helper::mermaid::scene::{Color, Scene};
use kitty_markdown_helper::mermaid::{self, RenderRequest};
use kitty_markdown_helper::ssim::ssim_with_tolerance;
use kitty_markdown_helper::text::{self, TextMetrics};

use common::{SIZE_TOLERANCE_PX, SSIM_THRESHOLD, assert_ssim, golden_dir};

/// The width `goldens.mjs` renders at.
const PARITY_WIDTH: u32 = 960;

/// Every `<case>.mmd` under `goldens/state/`, with or without a PNG, so
/// the snapshots cover a case before its golden exists.
fn sources() -> Vec<(String, PathBuf)> {
    let dir = golden_dir().join("state");
    let mut cases: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "mmd"))
        .filter_map(|p| Some((p.file_stem()?.to_str()?.to_string(), p)))
        .collect();
    cases.sort();
    cases
}

fn render_scene(source: &str) -> Result<Scene, HelperError> {
    let resolved = config::resolve(source, &BaseConfig::new("default"))?;
    let fonts = text::font_system();
    let ctx = RenderCtx {
        fonts,
        metrics: TextMetrics::new(fonts),
        target_width: PARITY_WIDTH,
    };
    DIAGRAM.render(&resolved, &ctx)
}

fn scene_svg(mmd: &Path) -> String {
    let source = std::fs::read_to_string(mmd).unwrap_or_else(|e| panic!("{}: {e}", mmd.display()));
    render_scene(&source)
        .unwrap_or_else(|e| panic!("{}: {e}", mmd.display()))
        .to_svg()
}

#[test]
fn state_scene_snapshots() {
    let cases = sources();
    assert!(
        !cases.is_empty(),
        "no state cases under tests/goldens/state"
    );
    for (name, mmd) in cases {
        insta::assert_snapshot!(name.as_str(), scene_svg(&mmd));
    }
}

fn render_at_parity_width(source: &str) -> RgbaImage {
    let out = mermaid::render(&RenderRequest {
        source,
        width_px: PARITY_WIDTH,
        theme: "default",
        background: Background::Color(Color::WHITE),
    })
    .expect("state renders");
    image::load_from_memory(&out.png)
        .expect("helper PNG decodes")
        .to_rgba8()
}

#[test]
fn state_ssim_against_the_oracle() {
    let cases = skip_if_no_goldens!("state");
    let mut failures = Vec::new();
    for case in cases {
        let goldens = case.goldens();
        let actual = render_at_parity_width(&case.source());
        let name = format!("state-{}", case.name);
        // Kept for every case, not only failures, so a passing case can be
        // inspected next to its golden too.
        let keep = common::parity_out_dir().join(format!("{name}.actual.png"));
        actual
            .save(&keep)
            .unwrap_or_else(|e| panic!("cannot write {}: {e}", keep.display()));
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

#[test]
fn both_headers_reach_the_state_renderer() {
    for header in ["stateDiagram", "stateDiagram-v2"] {
        let source = format!("{header}\n  [*] --> A\n  A --> [*]\n");
        let resolved = config::resolve(&source, &BaseConfig::new("default")).expect("resolves");
        assert_eq!(resolved.diagram_type, "stateDiagram");
        render_scene(&source).expect("renders");
    }
}

#[test]
fn parse_error_carries_the_oracle_text() {
    let err = render_scene("stateDiagram-v2\n  A --> B\n  A -->\n").expect_err("dangling arrow");
    match err {
        HelperError::ParseError {
            line,
            diagram_type,
            message,
            ..
        } => {
            assert_eq!(diagram_type, "stateDiagram");
            assert!(message.starts_with("Parse error on line 3:"), "{message}");
            assert_eq!(line, 3);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_note_on_an_undeclared_state_has_no_shape() {
    // mermaid 12.2.0 fails the same way: the note statement creates the
    // node without a type, so `insertNode` finds no shape.
    let err = render_scene("stateDiagram-v2\n  note right of A : first\n  A --> B\n")
        .expect_err("no shape");
    assert!(
        err.to_string()
            .contains("No such shape: undefined. Please check your syntax."),
        "{err}"
    );
}
