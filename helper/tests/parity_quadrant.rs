//! Quadrant chart parity (phase 2, R-106, R-110): an SVG snapshot per case
//! under `goldens/quadrant/` and the SSIM check against the oracle PNGs.

mod common;

use std::path::PathBuf;

use kitty_markdown_helper::mermaid::config::{BaseConfig, resolve};
use kitty_markdown_helper::mermaid::diagrams::{RenderCtx, find};
use kitty_markdown_helper::mermaid::raster::Background;
use kitty_markdown_helper::mermaid::scene::{Color, Scene};
use kitty_markdown_helper::mermaid::{RenderRequest, render};
use kitty_markdown_helper::text::{TextMetrics, font_system};

use common::{SSIM_THRESHOLD, assert_ssim, golden_dir};

/// The width `helper/oracle/goldens.mjs` renders at.
const PARITY_WIDTH: u32 = 960;

/// Every `.mmd` under `goldens/quadrant/`, with or without a PNG, sorted.
fn sources() -> Vec<(String, PathBuf)> {
    let dir = golden_dir().join("quadrant");
    let mut out: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "mmd"))
        .filter_map(|p| Some((p.file_stem()?.to_str()?.to_string(), p)))
        .collect();
    out.sort();
    out
}

fn scene_of(source: &str) -> Scene {
    let resolved = resolve(source, &BaseConfig::new("default")).expect("config resolves");
    let diagram = find(resolved.diagram_type).expect("quadrantChart is registered");
    let fonts = font_system();
    let ctx = RenderCtx {
        fonts,
        metrics: TextMetrics::new(fonts),
        target_width: PARITY_WIDTH,
    };
    diagram.render(&resolved, &ctx).expect("renders")
}

fn svg_of(source: &str) -> String {
    scene_of(source).to_svg()
}

#[test]
fn svg_snapshots() {
    let sources = sources();
    assert!(!sources.is_empty(), "no quadrant cases");
    for (name, path) in sources {
        let source = std::fs::read_to_string(&path).expect("case source");
        insta::assert_snapshot!(name, svg_of(&source));
    }
}

#[test]
fn registered_as_quadrant_chart() {
    let out = render(&RenderRequest {
        source: "quadrantChart\n  title t\n  A: [0.5, 0.5]\n",
        width_px: PARITY_WIDTH,
        theme: "default",
        background: Background::Color(Color::WHITE),
    })
    .expect("renders");
    assert_eq!(out.diagram_type, "quadrantChart");
    assert_eq!((out.width, out.height), (500, 500));
}

#[test]
fn parse_errors_carry_the_oracle_text() {
    let err = match render(&RenderRequest {
        source: "quadrantChart\n  x-axis\n",
        width_px: PARITY_WIDTH,
        theme: "default",
        background: Background::Transparent,
    }) {
        Ok(_) => panic!("bad input rendered"),
        Err(e) => e,
    };
    assert_eq!(err.code(), "parse_error");
    assert!(
        err.to_string().starts_with("Parse error on line 2:\n"),
        "{err}"
    );
}

#[test]
#[cfg_attr(not(feature = "phase2"), ignore)]
fn ssim_against_the_oracle() {
    let cases = skip_if_no_goldens!("quadrant");
    let mut failures = Vec::new();
    for case in cases {
        let out = render(&RenderRequest {
            source: &case.source(),
            width_px: PARITY_WIDTH,
            theme: "default",
            background: Background::Color(Color::WHITE),
        })
        .unwrap_or_else(|e| panic!("{}: {e}", case.name));
        let actual = image::load_from_memory(&out.png)
            .expect("helper PNG decodes")
            .to_rgba8();
        let name = format!("quadrant_{}", case.name);
        // Every case is checked so one failure does not hide the others.
        let result = std::panic::catch_unwind(|| {
            assert_ssim(&actual, &case.goldens(), SSIM_THRESHOLD, &name);
        });
        if let Err(payload) = result {
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "non-string panic".to_string());
            failures.push(message);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
