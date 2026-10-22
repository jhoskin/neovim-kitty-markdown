//! The curve ports against d3-shape 3.2.0 (mermaid's dependency) and
//! mermaid's `generateRoundedPath`.
//!
//! The expected strings come from `d3.line().curve(c)(points)` run in node
//! against `helper/oracle/node_modules/d3-shape`, and from the
//! `generateRoundedPath` source of `edges.js` with each number rounded to
//! three decimals like d3-path does. Regenerate them with a script that
//! prints `line()` output for the point sets below if d3 ever changes.

use kitty_markdown_helper::mermaid::scene::Point;
use kitty_markdown_helper::mermaid::scene::curves::{self, CurveKind, ROUNDED_RADIUS};

fn points(set: &str) -> Vec<Point> {
    let raw: &[(f64, f64)] = match set {
        "FIVE" => &[
            (10.0, 20.0),
            (40.0, 55.5),
            (70.25, 30.0),
            (100.0, 80.0),
            (130.0, 60.125),
        ],
        "TWO" => &[(0.0, 0.0), (50.0, 25.0)],
        "THREE" => &[(0.0, 0.0), (50.0, 25.0), (100.0, 0.0)],
        "ORTHO" => &[(0.0, 0.0), (0.0, 40.0), (60.0, 40.0), (60.0, 100.0)],
        "COLLINEAR" => &[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)],
        "SHORT" => &[(0.0, 0.0), (0.0, 4.0), (4.0, 4.0), (4.0, 100.0)],
        other => panic!("unknown point set {other}"),
    };
    raw.iter().map(|&(x, y)| Point::new(x, y)).collect()
}

/// `(curve, point set, d3 output)`.
const D3_EXPECTED: &[(CurveKind, &str, &str)] = &[
    (
        CurveKind::Basis,
        "FIVE",
        "M10,20L15,25.917C20,31.833,30,43.667,40.042,45.333C50.083,47,60.167,38.5,70.167,42.583C80.167,46.667,90.083,63.333,100.042,68.354C110,73.375,120,66.75,125,63.438L130,60.125",
    ),
    (
        CurveKind::BasisClosed,
        "FIVE",
        "M40.042,45.333C50.083,47,60.167,38.5,70.167,42.583C80.167,46.667,90.083,63.333,100.042,68.354C110,73.375,120,66.75,105,56.75C90,46.75,50,33.375,35,32.604C20,31.833,30,43.667,40.042,45.333",
    ),
    (
        CurveKind::BasisOpen,
        "FIVE",
        "M40.042,45.333C50.083,47,60.167,38.5,70.167,42.583C80.167,46.667,90.083,63.333,100.042,68.354",
    ),
    (
        CurveKind::BumpX,
        "FIVE",
        "M10,20C25,20,25,55.5,40,55.5C55.125,55.5,55.125,30,70.25,30C85.125,30,85.125,80,100,80C115,80,115,60.125,130,60.125",
    ),
    (
        CurveKind::BumpY,
        "FIVE",
        "M10,20C10,37.75,40,37.75,40,55.5C40,42.75,70.25,42.75,70.25,30C70.25,55,100,55,100,80C100,70.063,130,70.063,130,60.125",
    ),
    (
        CurveKind::Bundle,
        "FIVE",
        "M10,20L15,25.28C20,30.56,30,41.12,40.035,43.038C50.071,44.956,60.142,38.233,70.142,42.205C80.142,46.178,90.071,60.846,100.035,65.615C110,70.384,120,65.255,125,62.69L130,60.125",
    ),
    (
        CurveKind::Cardinal,
        "FIVE",
        "M10,20C10,20,29.958,53.833,40,55.5C50.042,57.167,60.25,25.917,70.25,30C80.25,34.083,90.042,74.979,100,80C109.958,85.021,130,60.125,130,60.125",
    ),
    (
        CurveKind::CardinalClosed,
        "FIVE",
        "M40,55.5C50.042,57.167,60.25,25.917,70.25,30C80.25,34.083,90.042,74.979,100,80C109.958,85.021,145,70.125,130,60.125C115,50.125,25,20.771,10,20C-5,19.229,29.958,53.833,40,55.5",
    ),
    (
        CurveKind::CardinalOpen,
        "FIVE",
        "M40,55.5C50.042,57.167,60.25,25.917,70.25,30C80.25,34.083,90.042,74.979,100,80",
    ),
    (
        CurveKind::CatmullRom,
        "FIVE",
        "M10,20C10,20,29.517,54.613,40,55.5C49.672,56.318,61.028,28.447,70.25,30C81.433,31.883,88.517,77.378,100,80C109.031,82.062,130,60.125,130,60.125",
    ),
    (
        CurveKind::CatmullRomClosed,
        "FIVE",
        "M40,55.5C49.672,56.318,61.028,28.447,70.25,30C81.433,31.883,88.517,77.378,100,80C109.031,82.062,130.898,66.927,130,60.125C128.317,47.371,14.821,12.891,10,20C7.078,24.309,29.517,54.613,40,55.5",
    ),
    (
        CurveKind::CatmullRomOpen,
        "FIVE",
        "M40,55.5C49.672,56.318,61.028,28.447,70.25,30C81.433,31.883,88.517,77.378,100,80",
    ),
    (
        CurveKind::Linear,
        "FIVE",
        "M10,20L40,55.5L70.25,30L100,80L130,60.125",
    ),
    (
        CurveKind::LinearClosed,
        "FIVE",
        "M10,20L40,55.5L70.25,30L100,80L130,60.125Z",
    ),
    (
        CurveKind::MonotoneX,
        "FIVE",
        "M10,20C20,37.75,30,55.5,40,55.5C50.083,55.5,60.167,30,70.25,30C80.167,30,90.083,80,100,80C110,80,120,70.063,130,60.125",
    ),
    (
        CurveKind::MonotoneY,
        "FIVE",
        "M10,20C25,31.833,40,43.667,40,55.5C40,47,70.25,38.5,70.25,30C70.25,46.667,100,63.333,100,80C100,73.375,115,66.75,130,60.125",
    ),
    (
        CurveKind::Natural,
        "FIVE",
        "M10,20C19.964,39.493,29.929,58.987,40,55.5C50.071,52.013,60.25,25.547,70.25,30C80.25,34.453,90.071,69.826,100,80C109.929,90.174,119.964,75.15,130,60.125",
    ),
    (
        CurveKind::Step,
        "FIVE",
        "M10,20L25,20L25,55.5L55.125,55.5L55.125,30L85.125,30L85.125,80L115,80L115,60.125L130,60.125",
    ),
    (
        CurveKind::StepAfter,
        "FIVE",
        "M10,20L40,20L40,55.5L70.25,55.5L70.25,30L100,30L100,80L130,80L130,60.125",
    ),
    (
        CurveKind::StepBefore,
        "FIVE",
        "M10,20L10,55.5L40,55.5L40,30L70.25,30L70.25,80L100,80L100,60.125L130,60.125",
    ),
    (CurveKind::Basis, "TWO", "M0,0L50,25"),
    (
        CurveKind::BasisClosed,
        "TWO",
        "M33.333,16.667L16.667,8.333Z",
    ),
    (CurveKind::BasisOpen, "TWO", ""),
    (CurveKind::BumpX, "TWO", "M0,0C25,0,25,25,50,25"),
    (CurveKind::BumpY, "TWO", "M0,0C0,12.5,50,12.5,50,25"),
    (CurveKind::Bundle, "TWO", "M0,0L50,25"),
    (CurveKind::Cardinal, "TWO", "M0,0L50,25"),
    (CurveKind::CardinalClosed, "TWO", "M50,25L0,0Z"),
    (CurveKind::CardinalOpen, "TWO", ""),
    (CurveKind::CatmullRom, "TWO", "M0,0L50,25"),
    (CurveKind::CatmullRomClosed, "TWO", "M50,25L0,0Z"),
    (CurveKind::CatmullRomOpen, "TWO", ""),
    (CurveKind::Linear, "TWO", "M0,0L50,25"),
    (CurveKind::LinearClosed, "TWO", "M0,0L50,25Z"),
    (CurveKind::MonotoneX, "TWO", "M0,0L50,25"),
    (CurveKind::MonotoneY, "TWO", "M0,0L50,25"),
    (CurveKind::Natural, "TWO", "M0,0L50,25"),
    (CurveKind::Step, "TWO", "M0,0L25,0L25,25L50,25"),
    (CurveKind::StepAfter, "TWO", "M0,0L50,0L50,25"),
    (CurveKind::StepBefore, "TWO", "M0,0L0,25L50,25"),
    (
        CurveKind::Basis,
        "THREE",
        "M0,0L8.333,4.167C16.667,8.333,33.333,16.667,50,16.667C66.667,16.667,83.333,8.333,91.667,4.167L100,0",
    ),
    (
        CurveKind::BasisClosed,
        "THREE",
        "M50,16.667C66.667,16.667,83.333,8.333,75,4.167C66.667,0,33.333,0,25,4.167C16.667,8.333,33.333,16.667,50,16.667",
    ),
    (CurveKind::BasisOpen, "THREE", "M50,16.667Z"),
    (
        CurveKind::BumpX,
        "THREE",
        "M0,0C25,0,25,25,50,25C75,25,75,0,100,0",
    ),
    (
        CurveKind::BumpY,
        "THREE",
        "M0,0C0,12.5,50,12.5,50,25C50,12.5,100,12.5,100,0",
    ),
    (
        CurveKind::Bundle,
        "THREE",
        "M0,0L8.333,3.542C16.667,7.083,33.333,14.167,50,14.167C66.667,14.167,83.333,7.083,91.667,3.542L100,0",
    ),
    (
        CurveKind::Cardinal,
        "THREE",
        "M0,0C0,0,33.333,25,50,25C66.667,25,100,0,100,0",
    ),
    (
        CurveKind::CardinalClosed,
        "THREE",
        "M50,25C66.667,25,108.333,4.167,100,0C91.667,-4.167,8.333,-4.167,0,0C-8.333,4.167,33.333,25,50,25",
    ),
    (CurveKind::CardinalOpen, "THREE", "M50,25Z"),
    (
        CurveKind::CatmullRom,
        "THREE",
        "M0,0C0,0,33.333,25,50,25C66.667,25,100,0,100,0",
    ),
    (
        CurveKind::CatmullRomClosed,
        "THREE",
        "M50,25C66.667,25,101.126,4.768,100,0C98.494,-6.377,1.506,-6.377,0,0C-1.126,4.768,33.333,25,50,25",
    ),
    (CurveKind::CatmullRomOpen, "THREE", "M50,25Z"),
    (CurveKind::Linear, "THREE", "M0,0L50,25L100,0"),
    (CurveKind::LinearClosed, "THREE", "M0,0L50,25L100,0Z"),
    (
        CurveKind::MonotoneX,
        "THREE",
        "M0,0C16.667,12.5,33.333,25,50,25C66.667,25,83.333,12.5,100,0",
    ),
    (
        CurveKind::MonotoneY,
        "THREE",
        "M0,0C25,8.333,50,16.667,50,25C50,16.667,75,8.333,100,0",
    ),
    (
        CurveKind::Natural,
        "THREE",
        "M0,0C16.667,12.5,33.333,25,50,25C66.667,25,83.333,12.5,100,0",
    ),
    (CurveKind::Step, "THREE", "M0,0L25,0L25,25L75,25L75,0L100,0"),
    (
        CurveKind::StepAfter,
        "THREE",
        "M0,0L50,0L50,25L100,25L100,0",
    ),
    (CurveKind::StepBefore, "THREE", "M0,0L0,25L50,25L50,0L100,0"),
    (
        CurveKind::Basis,
        "ORTHO",
        "M0,0L0,6.667C0,13.333,0,26.667,10,33.333C20,40,40,40,50,50C60,60,60,80,60,90L60,100",
    ),
    (
        CurveKind::BasisClosed,
        "ORTHO",
        "M10,33.333C20,40,40,40,50,50C60,60,60,80,50,73.333C40,66.667,20,33.333,10,23.333C0,13.333,0,26.667,10,33.333",
    ),
    (
        CurveKind::BasisOpen,
        "ORTHO",
        "M10,33.333C20,40,40,40,50,50",
    ),
    (
        CurveKind::BumpX,
        "ORTHO",
        "M0,0C0,0,0,40,0,40C30,40,30,40,60,40C60,40,60,100,60,100",
    ),
    (
        CurveKind::BumpY,
        "ORTHO",
        "M0,0C0,20,0,20,0,40C0,40,60,40,60,40C60,70,60,70,60,100",
    ),
    (
        CurveKind::Bundle,
        "ORTHO",
        "M0,0L0.5,6.5C1,13,2,26,11.5,33.333C21,40.667,39,42.333,48.5,52.5C58,62.667,59,81.333,59.5,90.667L60,100",
    ),
    (
        CurveKind::Cardinal,
        "ORTHO",
        "M0,0C0,0,-10,33.333,0,40C10,46.667,50,30,60,40C70,50,60,100,60,100",
    ),
    (
        CurveKind::CardinalClosed,
        "ORTHO",
        "M0,40C10,46.667,50,30,60,40C70,50,70,106.667,60,100C50,93.333,10,10,0,0C-10,-10,-10,33.333,0,40",
    ),
    (
        CurveKind::CardinalOpen,
        "ORTHO",
        "M0,40C10,46.667,50,30,60,40",
    ),
    (
        CurveKind::CatmullRom,
        "ORTHO",
        "M0,0C0,0,-7.34,32.66,0,40C8.99,48.99,50,30,60,40C70,50,60,100,60,100",
    ),
    (
        CurveKind::CatmullRomClosed,
        "ORTHO",
        "M0,40C8.99,48.99,50,30,60,40C70,50,65.992,98.34,60,100C51.646,102.314,7.387,-2.046,0,0C-4.326,1.198,-7.34,32.66,0,40",
    ),
    (
        CurveKind::CatmullRomOpen,
        "ORTHO",
        "M0,40C8.99,48.99,50,30,60,40",
    ),
    (CurveKind::Linear, "ORTHO", "M0,0L0,40L60,40L60,100"),
    (CurveKind::LinearClosed, "ORTHO", "M0,0L0,40L60,40L60,100Z"),
    (
        CurveKind::MonotoneX,
        "ORTHO",
        "M0,0C0,0,0,40,0,40C20,40,40,40,60,40C60,40,60,100,60,100",
    ),
    (
        CurveKind::MonotoneY,
        "ORTHO",
        "M0,0C0,13.333,0,26.667,0,40C0,40,60,40,60,40C60,60,60,80,60,100",
    ),
    (
        CurveKind::Natural,
        "ORTHO",
        "M0,0C-6.667,18.222,-13.333,36.444,0,40C13.333,43.556,46.667,32.444,60,40C73.333,47.556,66.667,73.778,60,100",
    ),
    (
        CurveKind::Step,
        "ORTHO",
        "M0,0L0,0L0,40L30,40L30,40L60,40L60,100L60,100",
    ),
    (
        CurveKind::StepAfter,
        "ORTHO",
        "M0,0L0,0L0,40L60,40L60,40L60,40L60,100",
    ),
    (
        CurveKind::StepBefore,
        "ORTHO",
        "M0,0L0,40L0,40L0,40L60,40L60,100L60,100",
    ),
];

/// `generateRoundedPath(points, 5)` for the sets above.
const ROUNDED_EXPECTED: &[(&str, &str)] = &[
    (
        "FIVE",
        "M10,20L35.433,50.096Q40,55.5,45.41,50.94L65.237,34.226Q70.25,30,73.603,35.635L96.469,74.065Q100,80,105.757,76.186L130,60.125",
    ),
    (
        "ORTHO",
        "M0,0L0,32.929Q0,40,7.071,40L52.929,40Q60,40,60,47.071L60,100",
    ),
    ("COLLINEAR", "M0,0L10,0L20,0"),
    ("SHORT", "M0,0L0,2Q0,4,2,4L2,4Q4,4,4,6L4,100"),
];

#[test]
fn curves_match_d3() {
    let mut failures = Vec::new();
    for (kind, set, expected) in D3_EXPECTED {
        let actual = curves::line(&points(set), *kind).to_svg();
        if actual != *expected {
            failures.push(format!(
                "{:?} on {set}:\n  expected {expected}\n  actual   {actual}",
                kind
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn every_d3_curve_has_an_expectation_on_each_set() {
    for kind in CurveKind::ALL {
        if kind == CurveKind::Rounded {
            continue;
        }
        for set in ["FIVE", "TWO", "THREE", "ORTHO"] {
            assert!(
                D3_EXPECTED.iter().any(|(k, s, _)| *k == kind && *s == set),
                "{kind:?} has no expectation for {set}"
            );
        }
    }
}

#[test]
fn rounded_matches_generate_rounded_path() {
    for (set, expected) in ROUNDED_EXPECTED {
        let actual = curves::rounded(&points(set), ROUNDED_RADIUS).to_svg();
        assert_eq!(&actual, expected, "{set}");
        assert_eq!(
            curves::line(&points(set), CurveKind::Rounded).to_svg(),
            actual
        );
    }
}

#[test]
fn single_and_empty_inputs() {
    let one = [Point::new(3.0, 4.0)];
    assert_eq!(curves::line(&one, CurveKind::Linear).to_svg(), "M3,4Z");
    assert_eq!(
        curves::line(&one, CurveKind::LinearClosed).to_svg(),
        "M3,4Z"
    );
    assert_eq!(curves::line(&one, CurveKind::BasisClosed).to_svg(), "M3,4Z");
    assert_eq!(
        curves::line(&one, CurveKind::CardinalClosed).to_svg(),
        "M3,4Z"
    );
    assert_eq!(curves::line(&one, CurveKind::MonotoneX).to_svg(), "M3,4Z");
    assert_eq!(curves::line(&one, CurveKind::Bundle).to_svg(), "");
    assert_eq!(curves::line(&one, CurveKind::Rounded).to_svg(), "");
    for kind in CurveKind::ALL {
        assert_eq!(curves::line(&[], kind).to_svg(), "", "{kind:?}");
    }
}

#[test]
fn monotone_ignores_coincident_points() {
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(0.0, 0.0),
        Point::new(50.0, 25.0),
        Point::new(100.0, 0.0),
    ];
    assert_eq!(
        curves::line(&pts, CurveKind::MonotoneX).to_svg(),
        curves::line(&points("THREE"), CurveKind::MonotoneX).to_svg()
    );
}
