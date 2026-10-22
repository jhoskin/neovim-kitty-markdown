//! `image::prepare` and the `image.prepare` handler (R-086, R-087, R-089 to
//! R-092, R-145, R-147, R-172, R-200). Fixtures are encoded in the test with
//! the `image` crate so nothing binary is committed.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use image::codecs::bmp::BmpEncoder;
use image::codecs::gif::{GifEncoder, Repeat};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::webp::WebPEncoder;
use image::{Delay, ExtendedColorType, Frame, ImageEncoder, ImageFormat, Rgba, RgbaImage};
use kitty_markdown_helper::image as helper_image;
use kitty_markdown_helper::image::{Error, Limits, handler, prepare, target_size};
use serde_json::json;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A fresh directory per test under the system temp dir; `tempfile` is not a
/// dependency of the crate and this is all the isolation the tests need.
fn temp_dir(name: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "kitty-markdown-image-{}-{n}-{name}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn solid_rgba(w: u32, h: u32, color: Rgba<u8>) -> RgbaImage {
    RgbaImage::from_pixel(w, h, color)
}

const RED: Rgba<u8> = Rgba([255, 0, 0, 255]);
const BLUE: Rgba<u8> = Rgba([0, 0, 255, 255]);

fn encode(img: &RgbaImage, format: ImageFormat) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    match format {
        ImageFormat::Png => img.write_to(&mut out, ImageFormat::Png).expect("png"),
        ImageFormat::Jpeg => {
            let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
            JpegEncoder::new_with_quality(&mut out, 95)
                .write_image(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    ExtendedColorType::Rgb8,
                )
                .expect("jpeg");
        }
        ImageFormat::WebP => WebPEncoder::new_lossless(&mut out)
            .write_image(
                img.as_raw(),
                img.width(),
                img.height(),
                ExtendedColorType::Rgba8,
            )
            .expect("webp"),
        ImageFormat::Bmp => {
            let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
            BmpEncoder::new(&mut out)
                .write_image(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    ExtendedColorType::Rgb8,
                )
                .expect("bmp");
        }
        ImageFormat::Gif => {
            let mut enc = GifEncoder::new(&mut out);
            enc.encode_frame(Frame::new(img.clone())).expect("gif");
        }
        other => panic!("no encoder for {other:?}"),
    }
    out.into_inner()
}

fn two_frame_gif(w: u32, h: u32) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    {
        let mut enc = GifEncoder::new(&mut out);
        enc.set_repeat(Repeat::Infinite).expect("repeat");
        let delay = Delay::from_numer_denom_ms(100, 1);
        enc.encode_frames(vec![
            Frame::from_parts(solid_rgba(w, h, RED), 0, 0, delay),
            Frame::from_parts(solid_rgba(w, h, BLUE), 0, 0, delay),
        ])
        .expect("frames");
    }
    out.into_inner()
}

fn decode_png(png: &[u8]) -> image::DynamicImage {
    image::load_from_memory_with_format(png, ImageFormat::Png).expect("output is PNG")
}

fn assert_png_header(png: &[u8]) {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "output must be PNG");
}

#[test]
fn decodes_every_supported_format_to_png() {
    let src = solid_rgba(20, 10, RED);
    for format in [
        ImageFormat::Png,
        ImageFormat::Jpeg,
        ImageFormat::Gif,
        ImageFormat::WebP,
        ImageFormat::Bmp,
    ] {
        let bytes = encode(&src, format);
        let out = prepare(&bytes, 100, None, None).unwrap_or_else(|e| panic!("{format:?}: {e}"));
        assert_eq!((out.width, out.height), (20, 10), "{format:?} size");
        assert_png_header(&out.png);
        let decoded = decode_png(&out.png).to_rgba8();
        let px = decoded.get_pixel(5, 5);
        assert!(
            px[0] > 200 && px[1] < 60 && px[2] < 60,
            "{format:?} pixel {px:?}"
        );
    }
}

#[test]
fn gif_uses_the_first_frame() {
    let bytes = two_frame_gif(8, 8);
    let out = prepare(&bytes, 100, None, None).expect("gif");
    let decoded = decode_png(&out.png).to_rgba8();
    assert_eq!(*decoded.get_pixel(3, 3), RED, "first frame is red");
}

#[test]
fn downscales_to_target_width_and_keeps_aspect() {
    let bytes = encode(&solid_rgba(400, 100, BLUE), ImageFormat::Png);
    let out = prepare(&bytes, 200, None, None).expect("png");
    assert_eq!((out.width, out.height), (200, 50));
    let decoded = decode_png(&out.png);
    assert_eq!((decoded.width(), decoded.height()), (200, 50));
}

#[test]
fn never_upscales() {
    let bytes = encode(&solid_rgba(40, 30, BLUE), ImageFormat::Png);
    let out = prepare(&bytes, 2000, None, None).expect("png");
    assert_eq!((out.width, out.height), (40, 30));
}

#[test]
fn img_tag_limits_shrink_but_never_grow() {
    let bytes = encode(&solid_rgba(400, 200, BLUE), ImageFormat::Png);
    let by_width = prepare(&bytes, 1000, Some(100), None).expect("png");
    assert_eq!((by_width.width, by_width.height), (100, 50));
    let by_height = prepare(&bytes, 1000, None, Some(20)).expect("png");
    assert_eq!((by_height.width, by_height.height), (40, 20));
    let both = prepare(&bytes, 300, Some(1000), Some(1000)).expect("png");
    assert_eq!(
        (both.width, both.height),
        (300, 150),
        "large limits are no-ops"
    );
    let larger = prepare(&bytes, 1000, Some(800), Some(800)).expect("png");
    assert_eq!(
        (larger.width, larger.height),
        (400, 200),
        "no scale up past native"
    );
    assert_eq!(
        target_size(
            400,
            200,
            1000,
            Limits {
                max_width: Some(100),
                max_height: Some(20)
            }
        ),
        (40, 20)
    );
}

#[test]
fn text_is_unsupported_format() {
    let err = prepare(b"hello, this is not an image\n", 100, None, None).unwrap_err();
    assert_eq!(err.code(), "unsupported_format");
    assert_eq!(err.data(), Some(json!({ "format": "unknown" })));
}

#[test]
fn tiff_is_unsupported_format_with_its_name() {
    // Little-endian TIFF magic; the crate recognizes it but the helper does
    // not enable the decoder.
    let err = prepare(b"II*\x00\x08\x00\x00\x00", 100, None, None).unwrap_err();
    assert_eq!(err.code(), "unsupported_format");
    assert_eq!(err.data(), Some(json!({ "format": "tiff" })));
}

#[test]
fn truncated_png_is_unsupported_format_png() {
    let bytes = encode(&solid_rgba(20, 10, RED), ImageFormat::Png);
    let err = prepare(&bytes[..40], 100, None, None).unwrap_err();
    assert_eq!(err.code(), "unsupported_format");
    assert_eq!(err.data(), Some(json!({ "format": "png" })));
}

#[test]
fn svg_bytes_route_to_the_rasterizer() {
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect width="200" height="100" fill="#00f"/></svg>"##;
    let out = prepare(svg, 50, None, None).expect("svg");
    assert_eq!((out.width, out.height), (50, 25));
    let decoded = decode_png(&out.png).to_rgba8();
    assert_eq!(*decoded.get_pixel(10, 10), BLUE);
    assert!(helper_image::looks_like_svg(svg));
}

#[test]
fn run_render_image_writes_output_and_reports_size() {
    let dir = temp_dir("render");
    let input = dir.join("in.jpg");
    std::fs::write(
        &input,
        encode(&solid_rgba(300, 150, RED), ImageFormat::Jpeg),
    )
    .expect("write");
    let output = dir.join("out.png");
    let size = helper_image::run_render_image(&input, 100, &output).expect("render");
    assert_eq!(size, (100, 50));
    let png = std::fs::read(&output).expect("output exists");
    assert_png_header(&png);
    assert_eq!(decode_png(&png).width(), 100);

    let err = helper_image::run_render_image(&dir.join("missing.png"), 100, &output).unwrap_err();
    assert_eq!(err.code(), "io");
    assert!(matches!(err, Error::Io { .. }));
}

fn handler_params(path: &Path, width: u32) -> serde_json::Value {
    json!({ "path": path, "width_px": width })
}

#[test]
fn handler_writes_to_cache_then_hits() {
    let dir = temp_dir("handler");
    let cache = dir.join("cache");
    let input = dir.join("pic.png");
    let bytes = encode(&solid_rgba(400, 100, BLUE), ImageFormat::Png);
    std::fs::write(&input, &bytes).expect("write");

    let first = handler::handle(handler_params(&input, 200), &cache).expect("miss");
    assert_eq!(first["cached"], json!(false));
    assert_eq!(first["width_px"], json!(200));
    assert_eq!(first["height_px"], json!(50));
    let path = PathBuf::from(first["path"].as_str().expect("path"));
    assert!(path.starts_with(cache.join("images")), "{path:?}");
    assert_eq!(path.extension().and_then(|e| e.to_str()), Some("png"));
    let expected_key = handler::key_image(&handler::sha256_hex(&bytes), 200, None, None);
    assert_eq!(
        path.file_stem().and_then(|s| s.to_str()),
        Some(expected_key.as_str())
    );
    let on_disk = std::fs::read(&path).expect("entry exists");
    assert_eq!(first["bytes"], json!(on_disk.len()));
    assert_png_header(&on_disk);

    // Age the entry so the hit's touch is observable.
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::open(&path)
        .expect("open")
        .set_modified(old)
        .expect("set mtime");

    let second = handler::handle(handler_params(&input, 200), &cache).expect("hit");
    assert_eq!(second["cached"], json!(true));
    assert_eq!(second["path"], first["path"]);
    assert_eq!(second["width_px"], json!(200));
    assert_eq!(second["height_px"], json!(50));
    assert_eq!(second["bytes"], first["bytes"]);
    let mtime = std::fs::metadata(&path)
        .expect("meta")
        .modified()
        .expect("mtime");
    assert!(
        mtime > old + std::time::Duration::from_secs(60),
        "hit touched the entry"
    );

    // Different limits mean a different key.
    let limited = handler::handle(
        json!({ "path": input, "width_px": 200, "max_height_px": 25 }),
        &cache,
    )
    .expect("limited");
    assert_eq!(limited["cached"], json!(false));
    assert_ne!(limited["path"], first["path"]);
    assert_eq!(limited["height_px"], json!(25));
}

#[test]
fn handler_reports_io_for_missing_file() {
    let dir = temp_dir("io");
    let missing = dir.join("nope.png");
    let err = handler::handle(handler_params(&missing, 100), &dir.join("cache")).unwrap_err();
    assert_eq!(err.code(), "io");
    assert_eq!(err.data(), Some(json!({ "path": missing })));
    assert!(err.to_string().contains("cannot read file"));
}

#[test]
fn handler_names_the_declared_format_when_the_sniff_fails() {
    let dir = temp_dir("declared");
    let input = dir.join("notes.TXT");
    std::fs::write(&input, b"plain text").expect("write");
    let err = handler::handle(handler_params(&input, 100), &dir.join("cache")).unwrap_err();
    assert_eq!(err.code(), "unsupported_format");
    assert_eq!(err.data(), Some(json!({ "format": "txt" })));
}

#[test]
fn handler_rejects_bad_params() {
    let err = handler::handle(json!({ "width_px": 10 }), Path::new("/nonexistent")).unwrap_err();
    assert_eq!(err.code(), "invalid_params");
}

#[test]
fn key_image_follows_the_convention() {
    let sha = handler::sha256_hex(b"abc");
    assert_eq!(
        sha,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let expected = handler::sha256_hex(format!("{sha}|300|0|120").as_bytes());
    assert_eq!(handler::key_image(&sha, 300, None, Some(120)), expected);
}
