//! The golden harness in `tests/common`: discovery, the skip path, and the
//! best-of-goldens assertion with diff output.

mod common;

use image::{Rgba, RgbaImage};

use common::{SSIM_THRESHOLD, assert_ssim, golden_dir, goldens_for, parity_out_dir};

#[test]
fn golden_dir_is_under_tests() {
    let dir = golden_dir();
    assert!(dir.ends_with("tests/goldens"), "{}", dir.display());
}

#[test]
fn unknown_type_has_no_cases() {
    assert!(goldens_for("no-such-diagram-type").is_empty());
}

#[test]
fn skip_macro_returns_when_goldens_are_absent() {
    let cases = skip_if_no_goldens!("no-such-diagram-type");
    // Only reached when goldens exist; then the harness has to find a PNG
    // for every case.
    for case in cases {
        assert!(!case.pngs.is_empty(), "{} has no PNG", case.name);
    }
}

#[test]
fn best_of_goldens_passes_when_one_matches() {
    let actual = RgbaImage::from_pixel(40, 40, Rgba([200, 30, 30, 255]));
    let far = RgbaImage::from_pixel(40, 40, Rgba([30, 200, 30, 255]));
    assert_ssim(
        &actual,
        &[far, actual.clone()],
        SSIM_THRESHOLD,
        "harness-best-of",
    );
}

#[test]
fn failure_writes_actual_and_diff() {
    let mut actual = RgbaImage::from_pixel(40, 40, Rgba([255, 255, 255, 255]));
    for y in 10..30 {
        for x in 10..30 {
            actual.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    let golden = RgbaImage::from_pixel(40, 40, Rgba([255, 255, 255, 255]));
    let name = "harness-failure";
    let result = std::panic::catch_unwind(|| {
        assert_ssim(&actual, &[golden], SSIM_THRESHOLD, name);
    });
    assert!(result.is_err(), "a blank golden must fail");
    let dir = parity_out_dir();
    assert!(dir.join(format!("{name}.actual.png")).is_file());
    assert!(dir.join(format!("{name}.diff.png")).is_file());
}
