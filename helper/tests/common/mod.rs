//! Shared helpers for the parity tests: golden discovery, PNG loading, and
//! the best-of-goldens SSIM assertion with diff output (R-110, R-181,
//! R-182).
//!
//! Goldens live under `helper/tests/goldens/<type>/`: `<case>.mmd` next to
//! `<case>.png`, or `<case>.1.png` to `<case>.5.png` for the unseeded
//! layouts (mindmap, cynefin). `just goldens` writes them with the pinned
//! oracle. Failures write `<name>.actual.png` and `<name>.diff.png` under
//! `helper/target/parity/`.
//!
//! A golden can be larger than the helper output: mermaid-cli draws a
//! `useMaxWidth` diagram 8 px narrower than its screenshot clip, so
//! `helper/oracle/goldens.mjs` (see its header comment for the mechanism)
//! asks for a clip 8 px wider than the helper output, which puts the diagram
//! at the helper's scale in the top-left corner with page background on the
//! right and bottom. [`golden_frame`] crops that corner before the SSIM.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use image::RgbaImage;
use kitty_markdown_helper::ssim::{diff_image, ssim_with_tolerance};

/// The SSIM a delivered diagram type has to reach (R-110).
pub const SSIM_THRESHOLD: f64 = 0.95;
/// The size tolerance of the parity check (R-181).
pub const SIZE_TOLERANCE_PX: u32 = 1;

/// One golden case: the mermaid source and one or more oracle renders.
#[derive(Debug, Clone)]
pub struct Case {
    pub name: String,
    pub mmd: PathBuf,
    pub pngs: Vec<PathBuf>,
}

impl Case {
    pub fn source(&self) -> String {
        std::fs::read_to_string(&self.mmd)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", self.mmd.display()))
    }

    pub fn goldens(&self) -> Vec<RgbaImage> {
        self.pngs.iter().map(|p| load_png(p)).collect()
    }
}

pub fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("goldens")
}

pub fn parity_out_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("parity");
    std::fs::create_dir_all(&dir).expect("create helper/target/parity");
    dir
}

pub fn load_png(path: &Path) -> RgbaImage {
    image::open(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
        .to_rgba8()
}

/// Every `<case>.mmd` under `goldens/<diagram_type>/` that has at least one
/// PNG, sorted by name. A `.mmd` without a PNG is not a case yet.
pub fn goldens_for(diagram_type: &str) -> Vec<Case> {
    let dir = golden_dir().join(diagram_type);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut cases: Vec<Case> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "mmd"))
        .filter_map(|mmd| {
            let name = mmd.file_stem()?.to_str()?.to_string();
            let mut pngs = Vec::new();
            let single = dir.join(format!("{name}.png"));
            if single.is_file() {
                pngs.push(single);
            }
            for i in 1..=5 {
                let numbered = dir.join(format!("{name}.{i}.png"));
                if numbered.is_file() {
                    pngs.push(numbered);
                }
            }
            if pngs.is_empty() {
                return None;
            }
            Some(Case { name, mmd, pngs })
        })
        .collect();
    cases.sort_by(|a, b| a.name.cmp(&b.name));
    cases
}

/// The part of `golden` that corresponds to `actual`: the top-left corner
/// of the golden at the actual's size, where the oracle drew the diagram
/// (see the module docs). A golden no larger than the actual on an axis
/// keeps that axis; the R-181 tolerance then covers a 1 px difference.
pub fn golden_frame(actual: &RgbaImage, golden: &RgbaImage) -> RgbaImage {
    let w = golden.width().min(actual.width());
    let h = golden.height().min(actual.height());
    if (w, h) == golden.dimensions() {
        return golden.clone();
    }
    image::imageops::crop_imm(golden, 0, 0, w, h).to_image()
}

/// Asserts that `actual` reaches `threshold` against the best of `goldens`
/// (R-182), each cropped with [`golden_frame`]. On failure the actual image
/// and a diff against the best golden land in
/// `helper/target/parity/<name>.*.png` and the panic message says where.
pub fn assert_ssim(actual: &RgbaImage, goldens: &[RgbaImage], threshold: f64, name: &str) {
    assert!(!goldens.is_empty(), "{name}: no goldens to compare against");
    let goldens: Vec<RgbaImage> = goldens.iter().map(|g| golden_frame(actual, g)).collect();
    let mut best: Option<(f64, usize)> = None;
    let mut size_errors = Vec::new();
    for (i, golden) in goldens.iter().enumerate() {
        match ssim_with_tolerance(actual, golden, SIZE_TOLERANCE_PX) {
            Ok(value) => {
                if best.is_none_or(|(b, _)| value > b) {
                    best = Some((value, i));
                }
            }
            Err(e) => size_errors.push(format!("golden {}: {e}", i + 1)),
        }
    }
    match best {
        Some((value, i)) if value >= threshold => {
            eprintln!("{name}: SSIM {value:.4} (golden {}) >= {threshold}", i + 1);
        }
        Some((value, i)) => {
            let (actual_path, diff_path) = write_failure(actual, &goldens[i], name);
            panic!(
                "{name}: SSIM {value:.4} < {threshold} against the best golden ({})\n  actual: {}\n  diff:   {}",
                i + 1,
                actual_path.display(),
                diff_path.display()
            );
        }
        None => {
            let (actual_path, _) = write_failure(actual, &goldens[0], name);
            panic!(
                "{name}: no golden has a comparable size\n  {}\n  actual: {}",
                size_errors.join("\n  "),
                actual_path.display()
            );
        }
    }
}

fn write_failure(actual: &RgbaImage, golden: &RgbaImage, name: &str) -> (PathBuf, PathBuf) {
    let dir = parity_out_dir();
    let actual_path = dir.join(format!("{name}.actual.png"));
    let diff_path = dir.join(format!("{name}.diff.png"));
    actual
        .save(&actual_path)
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", actual_path.display()));
    diff_image(actual, golden)
        .save(&diff_path)
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", diff_path.display()));
    (actual_path, diff_path)
}

/// Returns from the test with a clear message when `goldens_for(type)` is
/// empty, so `cargo test` stays green on a checkout without goldens and the
/// log says why nothing was compared.
#[macro_export]
macro_rules! skip_if_no_goldens {
    ($diagram_type:expr) => {{
        let cases = $crate::common::goldens_for($diagram_type);
        if cases.is_empty() {
            eprintln!(
                "skipping: no goldens under {} (run `just goldens`)",
                $crate::common::golden_dir().join($diagram_type).display()
            );
            return;
        }
        cases
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    use kitty_markdown_helper::ssim::ssim;

    /// A 40 by 30 picture with a dark box and a diagonal, so SSIM has
    /// structure to measure.
    fn diagram() -> RgbaImage {
        let mut img = RgbaImage::from_pixel(40, 30, Rgba([255, 255, 255, 255]));
        for y in 0..30 {
            for x in 0..40 {
                let boxed = (5..20).contains(&x) && (5..15).contains(&y);
                if boxed || x == y {
                    img.put_pixel(x, y, Rgba([40, 40, 120, 255]));
                }
            }
        }
        img
    }

    /// The oracle framing: the diagram in the top-left corner of a canvas
    /// `extra` px larger on each axis, white elsewhere.
    fn framed(diagram: &RgbaImage, extra: u32) -> RgbaImage {
        let mut canvas = RgbaImage::from_pixel(
            diagram.width() + extra,
            diagram.height() + extra,
            Rgba([255, 255, 255, 255]),
        );
        image::imageops::overlay(&mut canvas, diagram, 0, 0);
        canvas
    }

    #[test]
    fn golden_frame_crops_the_top_left_corner_to_the_actual_size() {
        let actual = diagram();
        let golden = framed(&actual, 8);
        let frame = golden_frame(&actual, &golden);
        assert_eq!(frame.dimensions(), actual.dimensions());
        assert_eq!(frame, actual);
        assert_eq!(ssim(&actual, &frame).unwrap(), 1.0);
        // The uncropped golden is too large for the R-181 tolerance.
        assert!(ssim_with_tolerance(&actual, &golden, SIZE_TOLERANCE_PX).is_err());
    }

    #[test]
    fn golden_frame_keeps_a_golden_that_is_not_larger() {
        let actual = diagram();
        let same = golden_frame(&actual, &actual);
        assert_eq!(same, actual);
        let smaller = image::imageops::crop_imm(&actual, 0, 0, 39, 30).to_image();
        assert_eq!(golden_frame(&actual, &smaller).dimensions(), (39, 30));
    }

    #[test]
    fn assert_ssim_accepts_the_framed_golden_and_rejects_a_shifted_one() {
        let actual = diagram();
        assert_ssim(&actual, &[framed(&actual, 8)], 0.999, "framing_synthetic");
        // The same diagram centered on the larger canvas is a different
        // picture after the crop.
        let mut shifted = RgbaImage::from_pixel(48, 38, Rgba([255, 255, 255, 255]));
        image::imageops::overlay(&mut shifted, &actual, 4, 4);
        let value = ssim(&actual, &golden_frame(&actual, &shifted)).unwrap();
        assert!(value < 0.9, "shifted golden still scored {value}");
    }
}
