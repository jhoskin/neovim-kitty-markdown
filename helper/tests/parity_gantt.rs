//! Gantt parity (R-106, R-110): an insta snapshot of the scene SVG and an
//! SSIM check against the oracle PNG for every case under
//! `tests/goldens/gantt/`.
//!
//! Time zone: the gantt renders in local time on both sides. The goldens
//! were produced with the machine zone `America/Los_Angeles`; the test pins
//! `TZ` to the same zone before the first time zone lookup, so the helper
//! sees the dates the oracle saw. A rerun of `just goldens` under another
//! zone must update the zone here.
//!
//! Framing: mermaid-cli 12.0.0 measures the SVG clip in a `size + 16`
//! viewport and then shrinks the viewport to `clip.x + clip.width`, which
//! narrows the `width="100%"` SVG by 8 px while the clip keeps its size.
//! Every golden is therefore the chart scaled by `(size - 8) / size` with
//! 8 px of white on the right and bottom. Per the coordinator's note from
//! the quadrant package, the test frames the helper output the same way
//! until the shared harness (`helper/oracle/goldens.mjs`,
//! `tests/common/mod.rs`) does it for every type.

mod common;

use std::sync::Once;

use image::{Rgba, RgbaImage};
use kitty_markdown_helper::mermaid::config::{BaseConfig, resolve};
use kitty_markdown_helper::mermaid::diagrams::{RenderCtx, find};
use kitty_markdown_helper::mermaid::raster::{Background, RasterOpts, render_rgba};
use kitty_markdown_helper::mermaid::scene::Color;
use kitty_markdown_helper::mermaid::{self, parse_background};
use kitty_markdown_helper::ssim::{diff_image, ssim_with_tolerance};
use kitty_markdown_helper::text::{self, TextMetrics};

/// The width `goldens.mjs` renders at.
const PARITY_WIDTH: u32 = 960;
/// The zone the goldens were rendered under.
const GOLDEN_TZ: &str = "America/Los_Angeles";
/// The viewport shrink mermaid-cli applies before the screenshot.
const CLI_SHRINK_PX: u32 = 8;

fn pin_zone() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // Safety: called before any other thread reads the environment; the
        // test binary is single-threaded until the first test body runs.
        unsafe { std::env::set_var("TZ", GOLDEN_TZ) };
        let _ = kitty_markdown_helper::mermaid::layout::d3::time::zone();
    });
}

fn scene_for(source: &str) -> kitty_markdown_helper::mermaid::scene::Scene {
    let resolved = resolve(source, &BaseConfig::new("default")).expect("config resolves");
    let diagram = find(resolved.diagram_type).expect("gantt is registered");
    let fonts = text::font_system();
    let ctx = RenderCtx {
        fonts,
        metrics: TextMetrics::new(fonts),
        target_width: PARITY_WIDTH,
    };
    diagram.render(&resolved, &ctx).expect("gantt renders")
}

/// The chart as the oracle screenshot shows it: scaled by
/// `(size - 8) / size` and padded to the golden's size with white.
fn framed(scene: &kitty_markdown_helper::mermaid::scene::Scene, golden: &RgbaImage) -> RgbaImage {
    let size = PARITY_WIDTH.max(scene.height.ceil() as u32);
    let shrunk_width = (scene.width * f64::from(size - CLI_SHRINK_PX) / f64::from(size)) as u32;
    let opts = RasterOpts {
        target_width: Some(shrunk_width),
        background: Some(Background::Color(Color::WHITE)),
    };
    let inner = render_rgba(scene, mermaid::fontdb(), &opts).expect("raster");
    let mut out =
        RgbaImage::from_pixel(golden.width(), golden.height(), Rgba([255, 255, 255, 255]));
    for (x, y, px) in inner.enumerate_pixels() {
        if x < out.width() && y < out.height() {
            out.put_pixel(x, y, *px);
        }
    }
    out
}

#[test]
fn gantt_cases_match_the_oracle() {
    pin_zone();
    let cases = skip_if_no_goldens!("gantt");
    // Every case is measured before the first failure is reported, so one
    // run shows the SSIM of all of them.
    let mut failures = Vec::new();
    for case in cases {
        let scene = scene_for(&case.source());
        insta::assert_snapshot!(format!("gantt_{}", case.name), scene.to_svg());
        let goldens = case.goldens();
        let actual = framed(&scene, &goldens[0]);
        let best = goldens
            .iter()
            .filter_map(|g| ssim_with_tolerance(&actual, g, common::SIZE_TOLERANCE_PX).ok())
            .fold(f64::NAN, f64::max);
        eprintln!("gantt/{}: SSIM {best:.4}", case.name);
        if best.is_nan() || best < common::SSIM_THRESHOLD {
            failures.push((case.name.clone(), actual, goldens));
        }
    }
    // Every failure gets its images before the first panic, so the diffs of
    // all of them can be read from one run.
    for (name, actual, goldens) in &failures {
        let dir = common::parity_out_dir();
        actual
            .save(dir.join(format!("gantt_{name}.actual.png")))
            .expect("write actual");
        diff_image(actual, &goldens[0])
            .save(dir.join(format!("gantt_{name}.diff.png")))
            .expect("write diff");
    }
    for (name, actual, goldens) in &failures {
        common::assert_ssim(
            actual,
            goldens,
            common::SSIM_THRESHOLD,
            &format!("gantt_{name}"),
        );
    }
}

#[test]
fn gantt_background_parameter_parses() {
    assert!(matches!(parse_background("white"), Background::Color(_)));
}
