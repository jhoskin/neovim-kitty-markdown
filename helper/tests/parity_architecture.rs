//! Architecture diagram parity (R-108, R-110): an insta SVG snapshot of the
//! scene for every case under `tests/goldens/architecture/` and an SSIM
//! check against the oracle PNG at the same pixel size (R-181), plus the
//! error paths (R-111, R-112). Phase 4 tests run with `--features phase4`.

mod common;

use std::path::{Path, PathBuf};

use image::RgbaImage;
use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::{self, RenderCtx};
use kitty_markdown_helper::mermaid::raster::Background;
use kitty_markdown_helper::mermaid::{self, RenderRequest};
use kitty_markdown_helper::text::{self, TextMetrics};

use common::{SSIM_THRESHOLD, assert_ssim, golden_dir};

const DIAGRAM_TYPE: &str = "architecture";

/// The width `goldens.mjs` renders at.
const PARITY_WIDTH: u32 = 960;

/// Every `<case>.mmd` under `goldens/architecture/`, with or without a PNG, so
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
    let diagram = diagrams::find(resolved.diagram_type).expect("architecture is registered");
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
fn architecture_scene_snapshots() {
    let cases = sources();
    assert!(
        !cases.is_empty(),
        "no architecture cases under tests/goldens/architecture"
    );
    for (name, mmd) in cases {
        insta::assert_snapshot!(name.as_str(), scene_svg(&mmd));
    }
}

/// An architecture diagram is `useMaxWidth`, so its natural size is its
/// viewBox. The helper renders it at that size when it is narrower than
/// the parity width (R-180) and scales a wider one down to the parity
/// width, on the default white background.
fn render_at_harness_width(source: &str) -> RgbaImage {
    let out = mermaid::render(&RenderRequest {
        source,
        width_px: PARITY_WIDTH,
        theme: "default",
        background: Background::default(),
    })
    .expect("architecture renders");
    image::load_from_memory(&out.png)
        .expect("helper PNG decodes")
        .to_rgba8()
}

#[test]
#[cfg_attr(not(feature = "phase4"), ignore)]
fn architecture_ssim_against_the_oracle() {
    let cases = skip_if_no_goldens!(DIAGRAM_TYPE);
    // Every case runs before the test fails, so one run reports the SSIM
    // of all of them.
    let mut failed = Vec::new();
    for case in cases {
        let name = format!("architecture-{}", case.name);
        let result = std::panic::catch_unwind(|| {
            let goldens = case.goldens();
            let actual = render_at_harness_width(&case.source());
            assert_ssim(&actual, &goldens, SSIM_THRESHOLD, &name);
        });
        if result.is_err() {
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "cases below the threshold: {failed:?}");
}

fn render_error(source: &str) -> HelperError {
    mermaid::render(&RenderRequest {
        source,
        width_px: PARITY_WIDTH,
        theme: "default",
        background: Background::default(),
    })
    .err()
    .unwrap_or_else(|| panic!("{source:?} renders"))
}

/// R-112: an icon outside mermaid's built-in architecture pack needs an
/// icon pack; the built-in names and unknown names in the built-in pack
/// render.
#[test]
fn icon_packs_are_unsupported() {
    for source in [
        "architecture-beta\n  service a(logos:aws-s3)[A]\n",
        "architecture-beta\n  group g(logos:aws-lambda)[G]\n",
    ] {
        match render_error(source) {
            HelperError::UnsupportedFeature { feature } => assert_eq!(feature, "icon pack"),
            other => panic!("{source:?}: {other:?}"),
        }
    }
    for source in [
        "architecture-beta\n  service a(mermaid-architecture:disk)[A]\n",
        "architecture-beta\n  service a(nope)[A]\n",
    ] {
        mermaid::render(&RenderRequest {
            source,
            width_px: PARITY_WIDTH,
            theme: "default",
            background: Background::default(),
        })
        .unwrap_or_else(|e| panic!("{source:?}: {e}"));
    }
}

/// R-111, R-177: a grammar error carries Langium's text and the line of
/// the first error; a db error the line of its statement.
#[test]
fn errors_carry_the_oracle_text_and_line() {
    match render_error("architecture-beta\n    service Lambda(server)[x]\n") {
        HelperError::ParseError {
            line,
            column,
            diagram_type,
            message,
        } => {
            assert_eq!((line, column), (2, 13));
            assert_eq!(diagram_type, "architecture");
            assert_eq!(
                message,
                "Parsing failed:  Parse error on line 2, column 13: Expecting token of type 'ID' but found `L`."
            );
        }
        other => panic!("{other:?}"),
    }
    match render_error("architecture-beta\n  service a\n  service b in a\n") {
        HelperError::ParseError { line, message, .. } => {
            assert_eq!(line, 3);
            assert_eq!(message, "The service [b]'s parent is not a group");
        }
        other => panic!("{other:?}"),
    }
}

/// cytoscape refuses a second element with an edge's `${lhs}-${rhs}` id.
#[test]
fn a_repeated_edge_fails_like_cytoscape() {
    match render_error("architecture-beta\n  service a\n  service b\n  a:L -- R:b\n  a:T -- B:b\n")
    {
        HelperError::Internal(message) => {
            assert_eq!(message, "Can not create second element with ID `a-b`");
        }
        other => panic!("{other:?}"),
    }
}
