//! Shared helpers for the parity tests: golden discovery, PNG loading, and
//! the best-of-goldens SSIM assertion with diff output (R-110, R-181,
//! R-182).
//!
//! Goldens live under `helper/tests/goldens/<type>/`: `<case>.mmd` next to
//! `<case>.png`, or `<case>.1.png` to `<case>.5.png` for the unseeded
//! layouts (mindmap, cynefin). `just goldens` writes them with the pinned
//! oracle. Failures write `<name>.actual.png` and `<name>.diff.png` under
//! `helper/target/parity/`.

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

/// Asserts that `actual` reaches `threshold` against the best of `goldens`
/// (R-182). On failure the actual image and a diff against the best golden
/// land in `helper/target/parity/<name>.*.png` and the panic message says
/// where.
pub fn assert_ssim(actual: &RgbaImage, goldens: &[RgbaImage], threshold: f64, name: &str) {
    assert!(!goldens.is_empty(), "{name}: no goldens to compare against");
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
