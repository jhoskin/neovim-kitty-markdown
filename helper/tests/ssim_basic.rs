//! The SSIM implementation against the SPEC.md definition.

use image::{Rgba, RgbaImage};
use kitty_markdown_helper::ssim::{
    SIGMA, WINDOW, diff_image, gaussian_kernel, ssim, ssim_with_tolerance, to_gray,
};

/// A smooth test image: a radial gradient with a soft diagonal stripe, so
/// a 1 px shift changes little but not nothing.
fn smooth(w: u32, h: u32, dx: i32) -> RgbaImage {
    let mut img = RgbaImage::new(w, h);
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    for (x, y, px) in img.enumerate_pixels_mut() {
        let fx = x as f64 - dx as f64;
        let fy = y as f64;
        let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
        let base = 255.0 * (1.0 - (d / (w.max(h) as f64)).clamp(0.0, 1.0));
        let stripe = 60.0 * (((fx + fy) / 12.0).sin() * 0.5 + 0.5);
        let v = (base * 0.75 + stripe).clamp(0.0, 255.0) as u8;
        *px = Rgba([v, v, v, 255]);
    }
    img
}

fn inverted(img: &RgbaImage) -> RgbaImage {
    let mut out = img.clone();
    for px in out.pixels_mut() {
        px.0 = [255 - px.0[0], 255 - px.0[1], 255 - px.0[2], px.0[3]];
    }
    out
}

#[test]
fn kernel_sums_to_one() {
    let k = gaussian_kernel();
    assert_eq!(k.len(), WINDOW);
    assert!((k.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    // The center tap of a normalized 11-tap sigma 1.5 Gaussian.
    let expected_center = 1.0
        / (0..WINDOW)
            .map(|i| {
                let x = i as f64 - 5.0;
                (-(x * x) / (2.0 * SIGMA * SIGMA)).exp()
            })
            .sum::<f64>();
    assert!((k[5] - expected_center).abs() < 1e-12);
}

#[test]
fn identical_images_score_one() {
    let a = smooth(64, 48, 0);
    let value = ssim(&a, &a).unwrap();
    assert!((value - 1.0).abs() < 1e-9, "got {value}");
}

/// A mostly black image with gentle texture. Against its inverse the
/// luminance term is near 0 (one mean near 0, the other near 255), which is
/// what pulls SSIM toward 0; a high-contrast texture would go to -1 instead.
fn dark(w: u32, h: u32) -> RgbaImage {
    let mut img = smooth(w, h, 0);
    for px in img.pixels_mut() {
        let v = px.0[0] / 12;
        *px = Rgba([v, v, v, 255]);
    }
    img
}

#[test]
fn inverse_scores_near_zero() {
    let a = dark(64, 48);
    let b = inverted(&a);
    let value = ssim(&a, &b).unwrap();
    assert!(value.abs() < 0.2, "got {value}");
}

#[test]
fn one_pixel_shift_is_high_but_below_one() {
    let a = smooth(96, 64, 0);
    let b = smooth(96, 64, 1);
    let value = ssim(&a, &b).unwrap();
    assert!(value < 1.0, "got {value}");
    assert!(value > 0.9, "got {value}");
}

#[test]
fn size_mismatch_beyond_tolerance_errors() {
    let a = smooth(64, 48, 0);
    let b = smooth(66, 48, 0);
    assert!(ssim(&a, &b).is_err());
    let err = ssim_with_tolerance(&a, &b, 1).unwrap_err();
    assert_eq!(err.a, (64, 48));
    assert_eq!(err.b, (66, 48));
    assert!(ssim_with_tolerance(&a, &b, 2).is_ok());
}

#[test]
fn center_crop_handles_one_pixel_difference() {
    let a = smooth(64, 48, 0);
    let mut b = RgbaImage::from_pixel(65, 49, Rgba([255, 255, 255, 255]));
    // `a` sits at the center-crop offset (0, 0) of a 65x49 canvas; the extra
    // row and column are what the oracle's `ceil` adds.
    image::imageops::replace(&mut b, &a, 0, 0);
    let value = ssim_with_tolerance(&a, &b, 1).unwrap();
    assert!((value - 1.0).abs() < 1e-9, "got {value}");
}

#[test]
fn alpha_composites_over_white() {
    let mut a = RgbaImage::from_pixel(32, 32, Rgba([0, 0, 0, 0]));
    let b = RgbaImage::from_pixel(32, 32, Rgba([255, 255, 255, 255]));
    assert!((ssim(&a, &b).unwrap() - 1.0).abs() < 1e-9);
    for px in a.pixels_mut() {
        *px = Rgba([0, 0, 0, 128]);
    }
    let gray = to_gray(&a)[0];
    assert!((gray - (255.0 - 255.0 * 128.0 / 255.0)).abs() < 1e-9);
}

#[test]
fn diff_image_marks_changed_pixels() {
    let a = RgbaImage::from_pixel(20, 20, Rgba([255, 255, 255, 255]));
    let mut b = a.clone();
    b.put_pixel(5, 5, Rgba([0, 0, 0, 255]));
    let d = diff_image(&a, &b);
    assert_eq!(d.dimensions(), (20, 20));
    assert_eq!(d.get_pixel(5, 5).0[0], 255);
    assert!(d.get_pixel(5, 5).0[1] < 100, "changed pixel is red");
    assert_eq!(d.get_pixel(0, 0).0, [255, 255, 255, 255]);
}
