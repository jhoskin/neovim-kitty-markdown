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

use image::RgbaImage;
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

/// The helper's PNG at the width the golden harness renders with; the
/// shared `assert_ssim` crops the oracle image to the helper's size. A gantt
/// lays its time scale over the available width, so the oracle runs at the
/// helper's width and its screenshot then reflows the drawing 8 px
/// narrower (see `helper/oracle/goldens.mjs`); the render scales the same.
const ORACLE_REFLOW_PX: u32 = 8;

fn render_at_harness_width(scene: &kitty_markdown_helper::mermaid::scene::Scene) -> RgbaImage {
    let opts = RasterOpts {
        target_width: Some(PARITY_WIDTH - ORACLE_REFLOW_PX),
        background: Some(Background::Color(Color::WHITE)),
    };
    render_rgba(scene, mermaid::fontdb(), &opts).expect("raster")
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
        let actual = render_at_harness_width(&scene);
        let best = goldens
            .iter()
            .filter_map(|g| {
                let framed = common::golden_frame(&actual, g);
                ssim_with_tolerance(&actual, &framed, common::SIZE_TOLERANCE_PX).ok()
            })
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
