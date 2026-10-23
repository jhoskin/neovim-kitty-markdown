//! Requirement diagram parity (R-108, R-110): an insta SVG snapshot of the
//! scene for every case under `tests/goldens/requirement/` and an SSIM
//! check against the oracle PNG at the same pixel size (R-181).
//! Requirement is a phase 4 type, so the gates run under the `phase4`
//! feature.

mod common;

use std::path::{Path, PathBuf};

use image::RgbaImage;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::requirement::{self, DIAGRAM};
use kitty_markdown_helper::mermaid::diagrams::{Diagram, RenderCtx};
use kitty_markdown_helper::mermaid::raster::Background;
use kitty_markdown_helper::mermaid::{self, RenderRequest};
use kitty_markdown_helper::text::{self, TextMetrics};

use common::{SSIM_THRESHOLD, assert_ssim, golden_dir};

/// The width `goldens.mjs` renders at.
const PARITY_WIDTH: u32 = 960;

/// Every `<case>.mmd` under `goldens/requirement/`, with or without a PNG,
/// so the snapshots cover a case before its golden exists.
fn sources() -> Vec<(String, PathBuf)> {
    let dir = golden_dir().join("requirement");
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
    assert_eq!(resolved.diagram_type, "requirement");
    let fonts = text::font_system();
    let ctx = RenderCtx {
        fonts,
        metrics: TextMetrics::new(fonts),
        target_width: PARITY_WIDTH,
    };
    let scene = DIAGRAM
        .render(&resolved, &ctx)
        .unwrap_or_else(|e| panic!("{}: {e}", mmd.display()));
    scene.to_svg()
}

#[test]
#[cfg_attr(not(feature = "phase4"), ignore)]
fn requirement_scene_snapshots() {
    let cases = sources();
    assert!(
        !cases.is_empty(),
        "no requirement cases under tests/goldens/requirement"
    );
    for (name, mmd) in cases {
        insta::assert_snapshot!(name.as_str(), scene_svg(&mmd));
    }
}

#[test]
fn the_docs_example_parses_into_the_db() {
    let source =
        std::fs::read_to_string(golden_dir().join("requirement/docs.mmd")).expect("docs case");
    let resolved = config::resolve(&source, &BaseConfig::new("default")).expect("resolves");
    let db = requirement::parse(&resolved).expect("parses");
    let req = &db.requirements["test_req"];
    assert_eq!(req.type_name, "Requirement");
    assert_eq!(req.requirement_id, "1");
    assert_eq!(req.text, "the test text.");
    assert_eq!(req.risk, "High");
    assert_eq!(req.verify_method, "Test");
    assert_eq!(db.elements["test_entity"].type_name, "simulation");
    assert_eq!(db.relations.len(), 1);
    assert_eq!(db.relations[0].type_name, "satisfies");
    assert_eq!(db.relations[0].src, "test_entity");
    assert_eq!(db.relations[0].dst, "test_req");
}

#[test]
fn svg_labels_decode_entities_and_the_title_is_centred() {
    let source = "---\ntitle: Orders\nconfig:\n  htmlLabels: false\n---\n\
        requirementDiagram\nelement e {\ntype: t\n}\nrequirement r {\nid: 1\n}\n\
        e - satisfies -> r\n";
    let resolved = config::resolve(source, &BaseConfig::new("default")).expect("resolves");
    let fonts = text::font_system();
    let ctx = RenderCtx {
        fonts,
        metrics: TextMetrics::new(fonts),
        target_width: PARITY_WIDTH,
    };
    let svg = DIAGRAM.render(&resolved, &ctx).expect("renders").to_svg();
    assert!(svg.contains(">&lt;&lt;Element&gt;&gt;<"), "{svg}");
    assert!(svg.contains(">&lt;&lt;satisfies&gt;&gt;<"), "{svg}");
    assert!(!svg.contains("&amp;lt;"), "{svg}");
    let title = svg
        .lines()
        .find(|l| l.contains("requirementDiagramTitleText"))
        .expect("title text");
    assert!(title.contains("text-anchor=\"middle\""), "{title}");
}

#[test]
fn a_bad_body_line_is_a_parse_error() {
    let source = "requirementDiagram\nrequirement r {\nid 1\n}\n";
    let resolved = config::resolve(source, &BaseConfig::new("default")).expect("resolves");
    let err = requirement::parse(&resolved).expect_err("does not parse");
    assert!(err.to_string().contains("Parse error"), "{err}");
}

fn render_at_parity_width(source: &str) -> RgbaImage {
    let out = mermaid::render(&RenderRequest {
        source,
        width_px: PARITY_WIDTH,
        theme: "default",
        background: Background::default(),
    })
    .expect("requirement renders");
    image::load_from_memory(&out.png)
        .expect("helper PNG decodes")
        .to_rgba8()
}

#[test]
#[cfg_attr(not(feature = "phase4"), ignore)]
fn requirement_ssim_against_the_oracle() {
    let cases = skip_if_no_goldens!("requirement");
    // Every case is checked before the test fails, so one run reports the
    // score of each and writes every diff image.
    let mut failures = Vec::new();
    for case in cases {
        let goldens = case.goldens();
        let actual = render_at_parity_width(&case.source());
        let name = format!("requirement-{}", case.name);
        let result = std::panic::catch_unwind(|| {
            assert_ssim(&actual, &goldens, SSIM_THRESHOLD, &name);
        });
        if result.is_err() {
            failures.push(case.name.clone());
        }
    }
    assert!(
        failures.is_empty(),
        "below the SSIM threshold: {failures:?}"
    );
}
