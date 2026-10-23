//! Cynefin parity (R-109, R-110): an insta SVG snapshot of the scene for
//! every case under `tests/goldens/cynefin/` and an SSIM check against the
//! best of the five oracle PNGs per case at the same pixel size (R-181,
//! R-182). Phase 5 tests run with `--features phase5`.

mod common;

use std::path::{Path, PathBuf};

use image::RgbaImage;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::{self, RenderCtx};
use kitty_markdown_helper::mermaid::raster::Background;
use kitty_markdown_helper::mermaid::{self, RenderRequest};
use kitty_markdown_helper::text::{self, TextMetrics};

use common::{SIZE_TOLERANCE_PX, SSIM_THRESHOLD, assert_ssim, golden_dir, golden_frame};
use kitty_markdown_helper::ssim::ssim_with_tolerance;

const DIAGRAM_TYPE: &str = "cynefin";

/// The width `goldens.mjs` renders at.
const PARITY_WIDTH: u32 = 960;

/// Every `<case>.mmd` under `goldens/cynefin/`, with or without a PNG, so
/// the snapshots cover a case before its golden exists.
fn sources() -> Vec<(String, PathBuf)> {
    let dir = golden_dir().join(DIAGRAM_TYPE);
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

fn scene_svg(mmd: &Path) -> String {
    let source = std::fs::read_to_string(mmd).unwrap_or_else(|e| panic!("{}: {e}", mmd.display()));
    let resolved = config::resolve(&source, &BaseConfig::new("default"))
        .unwrap_or_else(|e| panic!("{}: {e}", mmd.display()));
    let diagram = diagrams::find(resolved.diagram_type).expect("cynefin is registered");
    let fonts = text::font_system();
    let ctx = RenderCtx {
        fonts,
        metrics: TextMetrics::new(fonts),
        target_width: PARITY_WIDTH,
    };
    let scene = diagram
        .render(&resolved, &ctx)
        .unwrap_or_else(|e| panic!("{}: {e}", mmd.display()));
    scene.to_svg()
}

#[test]
#[cfg_attr(not(feature = "phase5"), ignore)]
fn cynefin_scene_snapshots() {
    let cases = sources();
    assert!(
        !cases.is_empty(),
        "no cynefin cases under tests/goldens/cynefin"
    );
    for (name, mmd) in cases {
        insta::assert_snapshot!(name.as_str(), scene_svg(&mmd));
    }
}

/// A cynefin diagram has a fixed natural size (880 by 680 with the default
/// config). One narrower than the parity width renders at that size
/// (R-180); a wider one is scaled down to it. The background is the
/// default white.
fn render_at_harness_width(source: &str) -> RgbaImage {
    let out = mermaid::render(&RenderRequest {
        source,
        width_px: PARITY_WIDTH,
        theme: "default",
        background: Background::default(),
    })
    .expect("cynefin renders");
    image::load_from_memory(&out.png)
        .expect("helper PNG decodes")
        .to_rgba8()
}

#[test]
#[cfg_attr(not(feature = "phase5"), ignore)]
fn cynefin_ssim_against_the_oracle() {
    let cases = skip_if_no_goldens!(DIAGRAM_TYPE);
    // Every case runs before the test fails, so one run reports the SSIM
    // of all of them.
    let mut failed = Vec::new();
    for case in cases {
        let name = format!("cynefin-{}", case.name);
        let result = std::panic::catch_unwind(|| {
            let goldens = case.goldens();
            let actual = render_at_harness_width(&case.source());
            // The score against each golden, for the spread over the runs.
            let scores: Vec<String> = goldens
                .iter()
                .map(|g| {
                    ssim_with_tolerance(&actual, &golden_frame(&actual, g), SIZE_TOLERANCE_PX)
                        .map_or_else(|e| e.to_string(), |v| format!("{v:.4}"))
                })
                .collect();
            eprintln!("{name}: per golden [{}]", scores.join(", "));
            assert_ssim(&actual, &goldens, SSIM_THRESHOLD, &name);
        });
        if result.is_err() {
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "cases below the threshold: {failed:?}");
}
