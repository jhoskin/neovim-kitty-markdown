//! Calibration of the text measurer against Chrome (R-115, R-179, R-214,
//! R-301, R-302).
//!
//! `helper/oracle/measure.mjs` records `getBBox` and `getBoundingClientRect`
//! for a fixed corpus in the page mermaid-cli renders into, and writes
//! `helper/tests/goldens/metrics.json`. This test replays the corpus
//! through `TextMetrics` and checks every width and height within 0.5 px
//! for ASCII text and within 3 px for fallback text (any character outside
//! ASCII). The corpus covers Latin accents, precomposed and combining
//! marks, Greek, Cyrillic, Han, kana, Hangul, Arabic, Hebrew, Devanagari,
//! Thai, emoji (ZWJ sequences, skin tones, flags, variation selectors),
//! symbols, mixed runs, and `<br>` lines in three families, three sizes,
//! bold, and italic. The oracle also records the face Chrome draws each
//! fallback grapheme in, and `fallback_faces_match_chrome` checks the
//! helper picks the same one. The log lists the largest deviation per
//! string; on the oracle machine every fallback string is within 0.5 px.

use std::path::PathBuf;

use kitty_markdown_helper::text::font_system;
use kitty_markdown_helper::text::markdown;
use kitty_markdown_helper::text::measure::{FontSpec, FontStyle, TextMetrics};
use serde::Deserialize;

const TOLERANCE: f32 = 0.5;
/// Fallback-font strings: the face differs per character in Chrome.
const FALLBACK_TOLERANCE: f32 = 3.0;

#[derive(Deserialize)]
struct Metrics {
    platform: String,
    fonts: std::collections::BTreeMap<String, bool>,
    generics: std::collections::BTreeMap<String, Option<String>>,
    cases: Vec<Case>,
    wrapped: Vec<Wrapped>,
    faces: Vec<Faces>,
}

/// The faces Chrome drew each grapheme of one fallback string in.
#[derive(Deserialize)]
struct Faces {
    id: String,
    family: String,
    weight: u16,
    style: String,
    graphemes: Vec<GraphemeFaces>,
}

#[derive(Deserialize)]
struct GraphemeFaces {
    grapheme: String,
    fonts: Vec<String>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    text: String,
    family: String,
    size: f32,
    weight: u16,
    style: String,
    svg: SvgCase,
    html: Box2,
}

#[derive(Deserialize)]
struct SvgCase {
    width: f32,
    height: f32,
    text_length: f32,
}

#[derive(Deserialize)]
struct Box2 {
    width: f32,
    height: f32,
}

#[derive(Deserialize)]
struct Wrapped {
    id: String,
    text: String,
    family: String,
    size: f32,
    weight: u16,
    style: String,
    max_width: f32,
    width: f32,
    height: f32,
    lines: Vec<String>,
}

fn goldens_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("goldens")
        .join("metrics.json")
}

fn load() -> Option<Metrics> {
    let path = goldens_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!(
            "skipping text metrics calibration: {} is absent; run `node helper/oracle/measure.mjs`",
            path.display()
        );
        return None;
    };
    Some(serde_json::from_str(&text).expect("metrics.json parses"))
}

fn spec_for(family: &str, size: f32, weight: u16, style: &str) -> FontSpec {
    let style = if style == "italic" {
        FontStyle::Italic
    } else {
        FontStyle::Normal
    };
    FontSpec::new(family, size).weight(weight).style(style)
}

fn needs_fallback(text: &str) -> bool {
    !text.is_ascii()
}

struct Deviation {
    id: String,
    what: &'static str,
    expected: f32,
    got: f32,
    bound: f32,
}

impl Deviation {
    fn over(&self) -> bool {
        (self.expected - self.got).abs() > self.bound
    }
}

fn report(deviations: &[Deviation]) -> String {
    let mut lines = Vec::new();
    for d in deviations.iter().filter(|d| d.over()) {
        lines.push(format!(
            "  {:<48} {:<12} expected {:>10.4} got {:>10.4} diff {:>8.4} (bound {})",
            d.id,
            d.what,
            d.expected,
            d.got,
            d.got - d.expected,
            d.bound
        ));
    }
    lines.join("\n")
}

fn max_abs(deviations: &[Deviation], bound: f32) -> f32 {
    deviations
        .iter()
        .filter(|d| d.bound == bound)
        .map(|d| (d.expected - d.got).abs())
        .fold(0.0, f32::max)
}

/// The largest deviation of each corpus string (the first part of the
/// case id) and the measurement it was in, for the test log.
fn per_string(deviations: &[Deviation]) -> String {
    let mut worst: std::collections::BTreeMap<&str, &Deviation> = std::collections::BTreeMap::new();
    for d in deviations {
        let key = d.id.split('/').next().unwrap_or(&d.id);
        let diff = (d.expected - d.got).abs();
        let entry = worst.entry(key).or_insert(d);
        if diff > (entry.expected - entry.got).abs() {
            *entry = d;
        }
    }
    worst
        .iter()
        .map(|(key, d)| {
            format!(
                "    {key:<16} {:>+8.4} {:<12} {}",
                d.got - d.expected,
                d.what,
                d.id
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The five largest deviations at a bound, for the test log.
fn worst(deviations: &[Deviation], bound: f32) -> String {
    let mut at_bound: Vec<&Deviation> = deviations.iter().filter(|d| d.bound == bound).collect();
    at_bound.sort_by(|a, b| {
        (b.expected - b.got)
            .abs()
            .total_cmp(&(a.expected - a.got).abs())
    });
    at_bound
        .iter()
        .take(5)
        .map(|d| {
            format!(
                "    {:<44} {:<12} {:>+8.4}",
                d.id,
                d.what,
                d.got - d.expected
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn widths_and_heights_match_chrome() {
    let Some(metrics) = load() else {
        return;
    };
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    assert_eq!(
        metrics.platform, platform,
        "metrics.json was generated on another platform; regenerate it here"
    );
    let fonts = font_system();
    let report_fonts = fonts.report();
    for (family, present) in &metrics.fonts {
        let key = family.to_ascii_lowercase();
        assert_eq!(
            report_fonts.get(&key),
            Some(present),
            "font report for {family} differs from the oracle machine"
        );
    }
    let text = TextMetrics::new(fonts);
    let mut deviations: Vec<Deviation> = Vec::new();
    for case in &metrics.cases {
        let spec = spec_for(&case.family, case.size, case.weight, &case.style);
        let bound = if needs_fallback(&case.text) {
            FALLBACK_TOLERANCE
        } else {
            TOLERANCE
        };
        let svg = text.svg_bbox(&case.text, &spec);
        deviations.push(Deviation {
            id: case.id.clone(),
            what: "svg.width",
            expected: case.svg.width,
            got: svg.width,
            bound,
        });
        deviations.push(Deviation {
            id: case.id.clone(),
            what: "svg.height",
            expected: case.svg.height,
            got: svg.height,
            bound,
        });
        deviations.push(Deviation {
            id: case.id.clone(),
            what: "text_length",
            expected: case.svg.text_length,
            got: text
                .shape(&markdown::split_breaks(&case.text).join(" "), &spec)
                .width,
            bound,
        });
        let html_lines = if case.text.is_empty() {
            markdown::html_to_lines("")
        } else {
            markdown::html_to_lines(&format!("<p>{}</p>", case.text))
        };
        let html = text.html_label(&html_lines, &spec, None);
        deviations.push(Deviation {
            id: case.id.clone(),
            what: "html.width",
            expected: case.html.width,
            got: html.width,
            bound,
        });
        deviations.push(Deviation {
            id: case.id.clone(),
            what: "html.height",
            expected: case.html.height,
            got: html.height,
            bound,
        });
    }
    eprintln!(
        "text metrics: largest deviation per string\n{}",
        per_string(&deviations)
    );
    let over: Vec<&Deviation> = deviations.iter().filter(|d| d.over()).collect();
    eprintln!(
        "text metrics: {} measurements, max deviation {:.4} px at bound {}, {:.4} px at bound {} (fallback strings)\n  worst at bound {}:\n{}\n  worst at bound {}:\n{}",
        deviations.len(),
        max_abs(&deviations, TOLERANCE),
        TOLERANCE,
        max_abs(&deviations, FALLBACK_TOLERANCE),
        FALLBACK_TOLERANCE,
        TOLERANCE,
        worst(&deviations, TOLERANCE),
        FALLBACK_TOLERANCE,
        worst(&deviations, FALLBACK_TOLERANCE)
    );
    assert!(
        over.is_empty(),
        "{} measurements outside tolerance:\n{}",
        over.len(),
        report(&deviations)
    );
}

/// Every grapheme of a fallback string goes to the face Chrome used. A
/// wrong face is the main way a fallback width or height misses, so this
/// names the cause before the width test reports the effect.
#[test]
fn fallback_faces_match_chrome() {
    let Some(metrics) = load() else {
        return;
    };
    let fonts = font_system();
    let text = TextMetrics::new(fonts);
    let mut mismatches = Vec::new();
    for case in &metrics.faces {
        let spec = spec_for(&case.family, 16.0, case.weight, &case.style);
        for g in &case.graphemes {
            // Chrome lists one face for a grapheme it draws in one face.
            let [chrome] = g.fonts.as_slice() else {
                mismatches.push(format!(
                    "  {} {:?}: Chrome used {:?}",
                    case.id, g.grapheme, g.fonts
                ));
                continue;
            };
            // The oracle puts each grapheme in its own span, so the helper
            // looks each one up alone too.
            let names = text
                .grapheme_faces(&g.grapheme, &spec)
                .first()
                .map(|&(_, face)| fonts.family_names(face))
                .unwrap_or_default();
            if !names.iter().any(|name| name.eq_ignore_ascii_case(chrome)) {
                mismatches.push(format!(
                    "  {} {:?}: Chrome {chrome}, helper {:?}",
                    case.id, g.grapheme, names
                ));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} graphemes in another face than Chrome's:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[test]
fn generic_families_resolve_like_chrome() {
    let Some(metrics) = load() else {
        return;
    };
    let fonts = font_system();
    for (generic, expected) in &metrics.generics {
        let Some(expected) = expected else {
            continue;
        };
        let resolved = fonts
            .resolve(
                generic,
                kitty_markdown_helper::text::fonts::Weight::NORMAL,
                kitty_markdown_helper::text::fonts::Style::Normal,
            )
            .and_then(|id| fonts.family_name(id));
        assert_eq!(
            resolved.as_deref(),
            Some(expected.as_str()),
            "{generic} resolves to a different family than Chrome used"
        );
    }
}

#[test]
fn wrapped_labels_match_chrome() {
    let Some(metrics) = load() else {
        return;
    };
    let fonts = font_system();
    let text = TextMetrics::new(fonts);
    let mut deviations: Vec<Deviation> = Vec::new();
    let mut line_mismatches: Vec<String> = Vec::new();
    for case in &metrics.wrapped {
        let spec = spec_for(&case.family, case.size, case.weight, &case.style);
        let lines = markdown::html_to_lines(&markdown::markdown_to_html(&case.text));
        let label = text.html_label(&lines, &spec, Some(case.max_width));
        deviations.push(Deviation {
            id: case.id.clone(),
            what: "wrapped.width",
            expected: case.width,
            got: label.width,
            bound: TOLERANCE,
        });
        deviations.push(Deviation {
            id: case.id.clone(),
            what: "wrapped.height",
            expected: case.height,
            got: label.height,
            bound: TOLERANCE,
        });
        let got_lines: Vec<String> = label
            .lines
            .iter()
            .map(|line| {
                line.iter()
                    .map(|w| w.content.as_str())
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .collect();
        if got_lines != case.lines {
            let widths = |lines: &[String]| -> Vec<f32> {
                lines
                    .iter()
                    .map(|line| text.words_width(&[markdown::Word::normal(line.as_str())], &spec))
                    .collect()
            };
            line_mismatches.push(format!(
                "  {} (avail {}): expected {:?} widths {:?}\n{:>4}got      {:?} widths {:?}",
                case.id,
                label.width,
                case.lines,
                widths(&case.lines),
                "",
                got_lines,
                widths(&got_lines)
            ));
        }
    }
    let over: Vec<&Deviation> = deviations.iter().filter(|d| d.over()).collect();
    assert!(
        over.is_empty() && line_mismatches.is_empty(),
        "wrapped labels differ:\n{}\n{}",
        report(&deviations),
        line_mismatches.join("\n")
    );
}
