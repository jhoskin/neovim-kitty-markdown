//! Mean SSIM as defined in SPEC.md, shared by `compare` and the parity tests.
//!
//! Definition (SPEC.md, "SSIM"): both images are composited over white and
//! converted to BT.601 grayscale (`0.299 R + 0.587 G + 0.114 B`). The index
//! uses an 11 by 11 Gaussian window with sigma 1.5, `K1 = 0.01`,
//! `K2 = 0.03`, `L = 255`, and reports the mean of the SSIM map. The
//! convolution is "valid": the map is `(w - 10) x (h - 10)`, so no border
//! padding invents structure. This is the Wang et al. 2004 reference form.

use std::fmt;
use std::path::Path;

use image::{Rgba, RgbaImage};

pub const WINDOW: usize = 11;
pub const SIGMA: f64 = 1.5;
pub const K1: f64 = 0.01;
pub const K2: f64 = 0.03;
pub const L: f64 = 255.0;

/// The two images have different pixel sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeMismatch {
    pub a: (u32, u32),
    pub b: (u32, u32),
}

impl fmt::Display for SizeMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "image sizes differ: {}x{} vs {}x{}",
            self.a.0, self.a.1, self.b.0, self.b.1
        )
    }
}

impl std::error::Error for SizeMismatch {}

/// The 1-D Gaussian with `WINDOW` taps and `SIGMA`, normalized to sum 1. The
/// 2-D window is its outer product, so a separable convolution is exact.
pub fn gaussian_kernel() -> [f64; WINDOW] {
    let mut k = [0.0; WINDOW];
    let half = (WINDOW / 2) as f64;
    let mut sum = 0.0;
    for (i, v) in k.iter_mut().enumerate() {
        let x = i as f64 - half;
        *v = (-(x * x) / (2.0 * SIGMA * SIGMA)).exp();
        sum += *v;
    }
    for v in &mut k {
        *v /= sum;
    }
    k
}

/// Composites over white and converts to BT.601 luma in `0..=255`.
pub fn to_gray(img: &RgbaImage) -> Vec<f64> {
    img.pixels()
        .map(|&Rgba([r, g, b, a])| {
            let alpha = f64::from(a) / 255.0;
            let over = |c: u8| f64::from(c) * alpha + 255.0 * (1.0 - alpha);
            0.299 * over(r) + 0.587 * over(g) + 0.114 * over(b)
        })
        .collect()
}

/// Valid-region separable Gaussian blur of a `w x h` plane. Returns the
/// `(w - 10) x (h - 10)` result.
fn blur(plane: &[f64], w: usize, h: usize, k: &[f64; WINDOW]) -> Vec<f64> {
    let ow = w - (WINDOW - 1);
    let oh = h - (WINDOW - 1);
    // Horizontal pass keeps every row, narrows the width.
    let mut tmp = vec![0.0; ow * h];
    for y in 0..h {
        let row = &plane[y * w..(y + 1) * w];
        let out = &mut tmp[y * ow..(y + 1) * ow];
        for (x, o) in out.iter_mut().enumerate() {
            let mut acc = 0.0;
            for (i, kv) in k.iter().enumerate() {
                acc += row[x + i] * kv;
            }
            *o = acc;
        }
    }
    // Vertical pass narrows the height.
    let mut out = vec![0.0; ow * oh];
    for y in 0..oh {
        for x in 0..ow {
            let mut acc = 0.0;
            for (i, kv) in k.iter().enumerate() {
                acc += tmp[(y + i) * ow + x] * kv;
            }
            out[y * ow + x] = acc;
        }
    }
    out
}

/// Mean SSIM of two images of the same size.
pub fn ssim(a: &RgbaImage, b: &RgbaImage) -> Result<f64, SizeMismatch> {
    if a.dimensions() != b.dimensions() {
        return Err(SizeMismatch {
            a: a.dimensions(),
            b: b.dimensions(),
        });
    }
    let (w, h) = (a.width() as usize, a.height() as usize);
    let ga = to_gray(a);
    let gb = to_gray(b);
    Ok(ssim_gray(&ga, &gb, w, h))
}

/// The SSIM map's mean over two gray planes of `w x h`. Images smaller than
/// the window have no valid region; they compare as the plain SSIM formula
/// over their global statistics, so tiny test images still get a value.
pub fn ssim_gray(ga: &[f64], gb: &[f64], w: usize, h: usize) -> f64 {
    let c1 = (K1 * L) * (K1 * L);
    let c2 = (K2 * L) * (K2 * L);
    if w < WINDOW || h < WINDOW {
        let n = (w * h) as f64;
        let mu_a = ga.iter().sum::<f64>() / n;
        let mu_b = gb.iter().sum::<f64>() / n;
        let var_a = ga.iter().map(|v| v * v).sum::<f64>() / n - mu_a * mu_a;
        let var_b = gb.iter().map(|v| v * v).sum::<f64>() / n - mu_b * mu_b;
        let cov = ga.iter().zip(gb).map(|(x, y)| x * y).sum::<f64>() / n - mu_a * mu_b;
        return ((2.0 * mu_a * mu_b + c1) * (2.0 * cov + c2))
            / ((mu_a * mu_a + mu_b * mu_b + c1) * (var_a + var_b + c2));
    }
    let k = gaussian_kernel();
    let mu_a = blur(ga, w, h, &k);
    let mu_b = blur(gb, w, h, &k);
    let aa: Vec<f64> = ga.iter().map(|v| v * v).collect();
    let bb: Vec<f64> = gb.iter().map(|v| v * v).collect();
    let ab: Vec<f64> = ga.iter().zip(gb).map(|(x, y)| x * y).collect();
    let e_aa = blur(&aa, w, h, &k);
    let e_bb = blur(&bb, w, h, &k);
    let e_ab = blur(&ab, w, h, &k);

    let mut sum = 0.0;
    for i in 0..mu_a.len() {
        let ma = mu_a[i];
        let mb = mu_b[i];
        let var_a = e_aa[i] - ma * ma;
        let var_b = e_bb[i] - mb * mb;
        let cov = e_ab[i] - ma * mb;
        sum += ((2.0 * ma * mb + c1) * (2.0 * cov + c2))
            / ((ma * ma + mb * mb + c1) * (var_a + var_b + c2));
    }
    sum / mu_a.len() as f64
}

/// Center-crops `img` to `w x h`.
fn center_crop(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    let x0 = (img.width() - w) / 2;
    let y0 = (img.height() - h) / 2;
    image::imageops::crop_imm(img, x0, y0, w, h).to_image()
}

/// SSIM with the R-181 tolerance: when the sizes differ by at most `tol_px`
/// on each axis, both images are center-cropped to the common size first.
/// A larger difference is a [`SizeMismatch`].
pub fn ssim_with_tolerance(a: &RgbaImage, b: &RgbaImage, tol_px: u32) -> Result<f64, SizeMismatch> {
    let (aw, ah) = a.dimensions();
    let (bw, bh) = b.dimensions();
    if aw.abs_diff(bw) > tol_px || ah.abs_diff(bh) > tol_px {
        return Err(SizeMismatch {
            a: (aw, ah),
            b: (bw, bh),
        });
    }
    if (aw, ah) == (bw, bh) {
        return ssim(a, b);
    }
    let w = aw.min(bw);
    let h = ah.min(bh);
    ssim(&center_crop(a, w, h), &center_crop(b, w, h))
}

/// A picture of where two images differ, on the common center-cropped size:
/// matching pixels fade to a light gray of `a`, differing pixels are red
/// with intensity proportional to the luma gap. Written next to a failing
/// golden so the difference can be looked at.
pub fn diff_image(a: &RgbaImage, b: &RgbaImage) -> RgbaImage {
    let w = a.width().min(b.width());
    let h = a.height().min(b.height());
    let a = center_crop(a, w, h);
    let b = center_crop(b, w, h);
    let ga = to_gray(&a);
    let gb = to_gray(&b);
    let mut out = RgbaImage::new(w, h);
    for (i, px) in out.pixels_mut().enumerate() {
        let delta = (ga[i] - gb[i]).abs();
        if delta < 8.0 {
            let faded = (255.0 - (255.0 - ga[i]) * 0.25) as u8;
            *px = Rgba([faded, faded, faded, 255]);
        } else {
            let keep = (255.0 - delta).clamp(0.0, 200.0) as u8;
            *px = Rgba([255, keep, keep, 255]);
        }
    }
    out
}

#[derive(Debug)]
pub enum CompareError {
    Read {
        path: std::path::PathBuf,
        source: image::ImageError,
    },
    Size(SizeMismatch),
}

impl fmt::Display for CompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompareError::Read { path, source } => {
                write!(f, "cannot read {}: {source}", path.display())
            }
            CompareError::Size(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CompareError {}

pub fn load_rgba(path: &Path) -> Result<RgbaImage, CompareError> {
    image::open(path)
        .map(|img| img.to_rgba8())
        .map_err(|source| CompareError::Read {
            path: path.to_path_buf(),
            source,
        })
}

/// `kitty-markdown-helper compare <a.png> <b.png>` (R-199): prints the SSIM
/// with four decimals on stdout and returns it. `main.rs` maps an error to
/// a message on stderr and a non-zero exit.
pub fn run_compare(a: &Path, b: &Path) -> Result<f64, CompareError> {
    let img_a = load_rgba(a)?;
    let img_b = load_rgba(b)?;
    let value = ssim_with_tolerance(&img_a, &img_b, 1).map_err(CompareError::Size)?;
    println!("{value:.4}");
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_is_normalized_and_symmetric() {
        let k = gaussian_kernel();
        assert!((k.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        for i in 0..WINDOW / 2 {
            assert!((k[i] - k[WINDOW - 1 - i]).abs() < 1e-15);
        }
        assert!(k[WINDOW / 2] > k[0]);
    }

    #[test]
    fn gray_composites_over_white() {
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 0]));
        assert!((to_gray(&img)[0] - 255.0).abs() < 1e-9);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        assert!(to_gray(&img)[0].abs() < 1e-9);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        assert!((to_gray(&img)[0] - 0.299 * 255.0).abs() < 1e-9);
    }
}
