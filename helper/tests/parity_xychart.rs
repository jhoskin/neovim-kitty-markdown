//! xychart parity: an insta SVG snapshot and an SSIM check against the
//! oracle golden for every case under `tests/goldens/xychart/` (R-106,
//! R-110). Phase 2, so the SSIM tests are gated by the `phase2` feature.
//!
//! Framing: mermaid-cli 12.0.0 measures the SVG clip in a `size + 16`
//! viewport and then sets the viewport to the clip width, which shrinks the
//! `width="100%"` SVG by 8 px before the screenshot while the clip keeps
//! its size (the coordinator's note from the quadrant package). Every
//! golden is therefore the chart scaled by `(size - 8) / size` with white
//! on the right and bottom. The xychart's fixed 700 by 500 SVG shrinks the
//! same way, so the helper output is rendered at `size - 8` and padded to
//! the golden's size here. The shared harness takes this over at
//! integration.

mod common;

use image::{Rgba, RgbaImage};
use kitty_markdown_helper::mermaid::config::{BaseConfig, resolve};
use kitty_markdown_helper::mermaid::diagrams::{self, RenderCtx};
use kitty_markdown_helper::mermaid::raster::{Background, OutputSize, RasterOpts, render_rgba};
use kitty_markdown_helper::mermaid::scene::{Color, Scene};
use kitty_markdown_helper::mermaid::{self, raster};
use kitty_markdown_helper::text::{self, TextMetrics};

use common::{Case, SSIM_THRESHOLD, assert_ssim};

/// The oracle's `--size`: the longer side of the helper's natural render.
const SHRINK_PX: u32 = 8;

fn scene_for(source: &str) -> Scene {
    let resolved = resolve(source, &BaseConfig::new("default")).expect("config resolves");
    let diagram = diagrams::find(resolved.diagram_type).expect("xychart is registered");
    let fonts = text::font_system();
    let ctx = RenderCtx {
        fonts,
        metrics: TextMetrics::new(fonts),
        target_width: 960,
    };
    diagram.render(&resolved, &ctx).expect("xychart renders")
}

/// The helper's PNG framed the way the oracle frames its screenshot.
fn framed_render(scene: &Scene) -> RgbaImage {
    let natural = OutputSize::natural(scene.width, scene.height);
    let size = natural.oracle_size();
    let target = OutputSize::for_target(scene.width, scene.height, size - SHRINK_PX);
    let inner = render_rgba(
        scene,
        mermaid::fontdb(),
        &RasterOpts {
            target_width: Some(target.width),
            background: Some(Background::Color(Color::WHITE)),
        },
    )
    .expect("raster");
    let mut framed =
        RgbaImage::from_pixel(natural.width, natural.height, Rgba([255, 255, 255, 255]));
    image::imageops::overlay(&mut framed, &inner, 0, 0);
    framed
}

fn snapshot(case: &Case) {
    let scene = scene_for(&case.source());
    insta::assert_snapshot!(case.name.as_str(), scene.to_svg());
}

#[test]
fn svg_snapshots() {
    let cases = skip_if_no_goldens!("xychart");
    for case in &cases {
        snapshot(case);
    }
}

#[test]
#[cfg_attr(not(feature = "phase2"), ignore)]
fn ssim_against_oracle() {
    let cases = skip_if_no_goldens!("xychart");
    for case in &cases {
        let scene = scene_for(&case.source());
        let actual = framed_render(&scene);
        assert_ssim(
            &actual,
            &case.goldens(),
            SSIM_THRESHOLD,
            &format!("xychart_{}", case.name),
        );
    }
}

#[test]
fn natural_size_is_the_configured_chart_size() {
    let scene = scene_for("xychart\n    line [1, 2, 3]\n");
    assert_eq!((scene.width, scene.height), (700.0, 500.0));
    let out = raster::render_png(
        &scene,
        mermaid::fontdb(),
        &RasterOpts {
            target_width: Some(960),
            background: None,
        },
    )
    .expect("raster");
    assert_eq!((out.width, out.height), (700, 500));
}
