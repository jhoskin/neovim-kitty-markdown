//! ER diagram parity: an insta snapshot of the scene SVG and the SSIM
//! against the oracle for every case under `goldens/er/` (R-106, R-110).
//! ER is a phase 2 type, so the parity gates run under the `phase2`
//! feature.

mod common;

use image::RgbaImage;
use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::config::{self, BaseConfig};
use kitty_markdown_helper::mermaid::diagrams::er::{self, DIAGRAM};
use kitty_markdown_helper::mermaid::diagrams::{Diagram, RenderCtx};
use kitty_markdown_helper::mermaid::raster::{Background, RasterOpts, render_rgba};
use kitty_markdown_helper::mermaid::scene::{Color, Scene};
use kitty_markdown_helper::mermaid::{self};
use kitty_markdown_helper::text::{self, TextMetrics};

use common::{SSIM_THRESHOLD, assert_ssim, golden_dir};

/// The width `goldens.mjs` renders the helper at.
const PARITY_WIDTH: u32 = 960;

fn render_with_theme(source: &str, theme: &str) -> Result<Scene, HelperError> {
    let resolved = config::resolve(source, &BaseConfig::new(theme))?;
    let fonts = text::font_system();
    let ctx = RenderCtx {
        fonts,
        metrics: TextMetrics::new(fonts),
        target_width: PARITY_WIDTH,
    };
    DIAGRAM.render(&resolved, &ctx)
}

fn render_scene(source: &str) -> Scene {
    render_with_theme(source, "default").expect("er renders")
}

/// The diagram is `useMaxWidth`, so it renders at its natural size inside
/// the 960 target (R-180) on the oracle's white page.
fn render_at_harness_width(scene: &Scene) -> RgbaImage {
    let opts = RasterOpts {
        target_width: Some(PARITY_WIDTH),
        background: Some(Background::Color(Color::WHITE)),
    };
    render_rgba(scene, mermaid::fontdb(), &opts).expect("raster")
}

/// Every `<case>.mmd` under `goldens/er/`, with or without a PNG, so the
/// snapshots cover a case before its golden exists.
fn sources() -> Vec<(String, String)> {
    let dir = golden_dir().join("er");
    let mut cases: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "mmd"))
        .filter_map(|p| {
            let name = p.file_stem()?.to_str()?.to_string();
            let source = std::fs::read_to_string(&p).ok()?;
            Some((name, source))
        })
        .collect();
    cases.sort();
    cases
}

#[test]
#[cfg_attr(not(feature = "phase2"), ignore)]
fn er_svg_snapshots() {
    let cases = sources();
    assert!(!cases.is_empty(), "no er cases under tests/goldens/er");
    for (name, source) in cases {
        let scene = render_scene(&source);
        insta::assert_snapshot!(name.as_str(), scene.to_svg());
    }
}

/// Every case is compared before the test fails, so one run reports the
/// SSIM of all of them; the failing ones leave their diff images behind.
#[test]
#[cfg_attr(not(feature = "phase2"), ignore)]
fn er_ssim_against_oracle() {
    let cases = skip_if_no_goldens!("er");
    let mut failures = Vec::new();
    for case in cases {
        let scene = render_scene(&case.source());
        let goldens = case.goldens();
        let actual = render_at_harness_width(&scene);
        let name = format!("er_{}", case.name);
        // `assert_ssim` prints the score of each case and, below the
        // threshold, where it wrote the diff images.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_ssim(&actual, &goldens, SSIM_THRESHOLD, &name);
        }));
        if result.is_err() {
            failures.push(name);
        }
    }
    assert!(
        failures.is_empty(),
        "{} case(s) below {SSIM_THRESHOLD}: {failures:?}",
        failures.len()
    );
}

fn parse(source: &str) -> er::db::ErDb {
    let resolved = config::resolve(source, &BaseConfig::new("default")).expect("resolves");
    er::parse(&resolved).expect("parses")
}

/// Attribute blocks keep their order, keys, and comments; an alias names
/// the entity box.
#[test]
fn attributes_keys_comments_and_aliases() {
    let db = parse(
        "erDiagram\n  p[\"Person\"] {\n    string id PK, FK \"the key\"\n    string? name\n    int age UK\n  }\n",
    );
    let p = &db.entities["p"];
    assert_eq!(p.alias, "Person");
    let rows: Vec<(&str, &str, Vec<String>, &str)> = p
        .attributes
        .iter()
        .map(|a| {
            (
                a.type_name.as_str(),
                a.name.as_str(),
                a.keys.clone(),
                a.comment.as_str(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            (
                "string",
                "id",
                vec!["PK".to_string(), "FK".to_string()],
                "the key"
            ),
            ("string?", "name", vec![], ""),
            ("int", "age", vec!["UK".to_string()], ""),
        ]
    );
}

/// Every cardinality spelling, both line types, and the word forms.
#[test]
fn relationships_read_every_cardinality() {
    let db = parse(
        "erDiagram\n  A |o--o| B : r\n  A }o..o{ B : r\n  A }|--|{ B : r\n  A ||.-|| B : r\n  A 1 to zero or more B : r\n  A many(0) optionally to 1+ B : r\n",
    );
    let specs: Vec<(&str, &str, &str)> = db
        .relationships
        .iter()
        .map(|r| {
            (
                r.rel_spec.card_b.as_str(),
                match r.rel_spec.rel_type {
                    er::db::Identification::Identifying => "--",
                    er::db::Identification::NonIdentifying => "..",
                },
                r.rel_spec.card_a.as_str(),
            )
        })
        .collect();
    assert_eq!(
        specs,
        vec![
            ("ZERO_OR_ONE", "--", "ZERO_OR_ONE"),
            ("ZERO_OR_MORE", "..", "ZERO_OR_MORE"),
            ("ONE_OR_MORE", "--", "ONE_OR_MORE"),
            ("ONLY_ONE", "..", "ONLY_ONE"),
            ("ONLY_ONE", "--", "ZERO_OR_MORE"),
            ("ZERO_OR_MORE", "..", "ONE_OR_MORE"),
        ]
    );
}

/// `classDef`, `class`, `:::`, and `style` reach the entities; `direction`
/// sets the layout direction.
#[test]
fn styles_classes_and_direction() {
    let db = parse(
        "erDiagram\n  direction LR\n  A:::hot ||--o{ B : r\n  classDef hot fill:#f96,stroke:#333\n  class B hot\n  style A stroke-width:4px\n",
    );
    assert_eq!(db.direction, "LR");
    assert_eq!(db.entities["A"].css_classes, "default hot");
    assert_eq!(db.entities["B"].css_classes, "default hot");
    assert_eq!(db.entities["A"].css_styles, vec!["stroke-width:4px"]);
    assert_eq!(db.classes["hot"].styles, vec!["fill:#f96", "stroke:#333"]);
}

/// R-102: `er.rankSpacing` from a directive replaces the 80 px default
/// between the ranks.
#[test]
fn a_directive_sets_the_rank_spacing() {
    let source = "erDiagram\n  A ||--o{ B : r\n";
    let tall = |s: &Scene| s.view_box.height;
    let base = tall(&render_scene(source));
    let spaced = tall(&render_scene(&format!(
        "%%{{init: {{\"er\": {{\"rankSpacing\": 180}}}}}}%%\n{source}"
    )));
    assert_eq!(spaced - base, 100.0);
}

/// R-111, R-177: a relationship without a role is a jison parse error with
/// the oracle's line.
#[test]
fn a_relationship_without_a_role_is_a_parse_error() {
    let err = render_with_theme("erDiagram\n  A ||--o{ B : r\n  A ||--o{ C\n", "default")
        .expect_err("no role");
    match err {
        HelperError::ParseError {
            line,
            diagram_type,
            message,
            ..
        } => {
            assert_eq!(diagram_type, "er");
            // The oracle's text, from mermaid-cli 12.0.0 with mermaid 12.2.0.
            assert_eq!(
                message,
                "Parse error on line 3:\n...{ B : r  A ||--o{ C\n----------------------^\nExpecting 'COLON', 'STYLE_SEPARATOR', got 'NEWLINE'"
            );
            assert_eq!(line, 3);
        }
        other => panic!("{other:?}"),
    }
}
