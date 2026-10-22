//! Packet diagram parity (R-108, R-110): an insta SVG snapshot of the scene
//! for every case under `tests/goldens/packet/` and an SSIM check against
//! the oracle PNG at the same pixel size (R-181). Phase 4 tests run with
//! `--features phase4`.

mod common;

use std::path::{Path, PathBuf};

use image::RgbaImage;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::{self, RenderCtx};
use kitty_markdown_helper::mermaid::raster::Background;
use kitty_markdown_helper::mermaid::{self, RenderRequest};
use kitty_markdown_helper::text::{self, TextMetrics};

use common::{SSIM_THRESHOLD, assert_ssim, golden_dir};

/// The width `goldens.mjs` renders at.
const PARITY_WIDTH: u32 = 960;

/// Every `<case>.mmd` under `goldens/packet/`, with or without a PNG, so
/// the snapshots cover a case before its golden exists.
fn sources() -> Vec<(String, PathBuf)> {
    let dir = golden_dir().join("packet");
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
    let diagram = diagrams::find(resolved.diagram_type).expect("packet is registered");
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
#[cfg_attr(not(feature = "phase4"), ignore)]
fn packet_scene_snapshots() {
    let cases = sources();
    assert!(
        !cases.is_empty(),
        "no packet cases under tests/goldens/packet"
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
        background: Background::default(),
    })
    .expect("packet renders");
    image::load_from_memory(&out.png)
        .expect("helper PNG decodes")
        .to_rgba8()
}

#[test]
#[cfg_attr(not(feature = "phase4"), ignore)]
fn packet_ssim_against_the_oracle() {
    let cases = skip_if_no_goldens!("packet");
    for case in cases {
        let goldens = case.goldens();
        let actual = render_at_parity_width(&case.source());
        assert_ssim(
            &actual,
            &goldens,
            SSIM_THRESHOLD,
            &format!("packet-{}", case.name),
        );
    }
}
