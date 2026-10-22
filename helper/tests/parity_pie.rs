//! Pie chart parity (R-106, R-110): an insta SVG snapshot of the scene for
//! every case under `tests/goldens/pie/` and an SSIM check against the
//! oracle PNG at the same pixel size (R-181). Phase 2 tests run with
//! `--features phase2`.

mod common;

use std::path::{Path, PathBuf};

use image::{Rgba, RgbaImage};
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::{self, RenderCtx};
use kitty_markdown_helper::mermaid::raster::Background;
use kitty_markdown_helper::mermaid::{self, RenderRequest};
use kitty_markdown_helper::text::{self, TextMetrics};

use common::{SSIM_THRESHOLD, assert_ssim, golden_dir};

/// The width `goldens.mjs` renders at.
const PARITY_WIDTH: u32 = 960;

/// Every `<case>.mmd` under `goldens/pie/`, with or without a PNG, so the
/// snapshots cover a case before its golden exists.
fn sources() -> Vec<(String, PathBuf)> {
    let dir = golden_dir().join("pie");
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
    let diagram = diagrams::find(resolved.diagram_type).expect("pie is registered");
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
#[cfg_attr(not(feature = "phase2"), ignore)]
fn pie_scene_snapshots() {
    let cases = sources();
    assert!(!cases.is_empty(), "no pie cases under tests/goldens/pie");
    for (name, mmd) in cases {
        insta::assert_snapshot!(name.as_str(), scene_svg(&mmd));
    }
}

/// mermaid-cli 12.0.0 measures the SVG clip in a `size + 16` viewport,
/// then narrows the viewport to the clip, which shrinks the `width="100%"`
/// SVG by 8 px before the screenshot while the clip keeps its size. Every
/// golden is therefore the diagram at `(width - 8) / width` with 8 px of
/// white on the right and the bottom. The helper output is framed the same
/// way here, as the coordinator asked while the quadrant package moves this
/// into the shared harness (`goldens.mjs` and `tests/common`).
const ORACLE_SHRINK_PX: u32 = 8;

fn framed_render(source: &str, golden: &RgbaImage) -> RgbaImage {
    let out = mermaid::render(&RenderRequest {
        source,
        width_px: golden.width() - ORACLE_SHRINK_PX,
        theme: "default",
        background: Background::default(),
    })
    .expect("pie renders");
    let inner = image::load_from_memory(&out.png)
        .expect("helper PNG decodes")
        .to_rgba8();
    let mut framed =
        RgbaImage::from_pixel(golden.width(), golden.height(), Rgba([255, 255, 255, 255]));
    for (x, y, px) in inner.enumerate_pixels() {
        if x < framed.width() && y < framed.height() {
            framed.put_pixel(x, y, *px);
        }
    }
    framed
}

#[test]
#[cfg_attr(not(feature = "phase2"), ignore)]
fn pie_ssim_against_the_oracle() {
    let cases = skip_if_no_goldens!("pie");
    for case in cases {
        let goldens = case.goldens();
        let actual = framed_render(&case.source(), &goldens[0]);
        assert_ssim(
            &actual,
            &goldens,
            SSIM_THRESHOLD,
            &format!("pie-{}", case.name),
        );
    }
}
