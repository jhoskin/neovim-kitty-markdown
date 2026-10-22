//! `svg::rasterize` and the `svg.rasterize` handler (R-088, R-089, R-091,
//! R-145, R-147, R-200).

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use image::{ImageFormat, Rgba};
use kitty_markdown_helper::image::handler::{key_image, sha256_hex};
use kitty_markdown_helper::svg::{self, handler, rasterize};
use serde_json::json;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(name: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "kitty-markdown-svg-{}-{n}-{name}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

const WIDE: &[u8] =
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="100" viewBox="0 0 400 100">
  <rect x="0" y="0" width="200" height="100" fill="#ff0000"/>
  <rect x="200" y="0" width="200" height="100" fill="#0000ff"/>
</svg>"##;

fn decode(png: &[u8]) -> image::RgbaImage {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "output must be PNG");
    image::load_from_memory_with_format(png, ImageFormat::Png)
        .expect("png")
        .to_rgba8()
}

#[test]
fn rasterizes_at_the_target_width_with_aspect() {
    let out = rasterize(WIDE, 100).expect("svg");
    assert_eq!((out.width, out.height), (100, 25));
    let img = decode(&out.png);
    assert_eq!((img.width(), img.height()), (100, 25));
    assert_eq!(*img.get_pixel(10, 12), Rgba([255, 0, 0, 255]));
    assert_eq!(*img.get_pixel(90, 12), Rgba([0, 0, 255, 255]));
}

#[test]
fn caps_at_the_intrinsic_width() {
    let out = rasterize(WIDE, 5000).expect("svg");
    assert_eq!((out.width, out.height), (400, 100));
}

#[test]
fn viewbox_only_svg_uses_the_viewbox_size() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30 60"><circle cx="15" cy="30" r="10"/></svg>"#;
    let out = rasterize(svg, 1000).expect("svg");
    assert_eq!((out.width, out.height), (30, 60));
}

#[test]
fn transparent_background_outside_the_shapes() {
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect x="5" y="5" width="5" height="5" fill="#000"/></svg>"##;
    let out = rasterize(svg, 10).expect("svg");
    let img = decode(&out.png);
    assert_eq!(img.get_pixel(1, 1)[3], 0, "empty area is transparent");
    assert_eq!(img.get_pixel(8, 8)[3], 255);
}

#[test]
fn broken_svg_is_unsupported_format_svg() {
    let err = rasterize(b"<svg xmlns='http://www.w3.org/2000/svg'><rect", 100).unwrap_err();
    assert_eq!(err.code(), "unsupported_format");
    assert_eq!(err.data(), Some(json!({ "format": "svg" })));
    assert!(err.to_string().starts_with("cannot parse SVG"));

    let err = rasterize(b"<html/>", 100).unwrap_err();
    assert_eq!(err.code(), "unsupported_format");
}

#[test]
fn run_render_svg_writes_output() {
    let dir = temp_dir("render");
    let input = dir.join("in.svg");
    std::fs::write(&input, WIDE).expect("write");
    let output = dir.join("out.png");
    assert_eq!(
        svg::run_render_svg(&input, 200, &output).expect("render"),
        (200, 50)
    );
    let img = decode(&std::fs::read(&output).expect("read"));
    assert_eq!((img.width(), img.height()), (200, 50));

    let err = svg::run_render_svg(&dir.join("missing.svg"), 200, &output).unwrap_err();
    assert_eq!(err.code(), "io");
    assert_eq!(err.data(), Some(json!({ "path": dir.join("missing.svg") })));
}

#[test]
fn handler_caches_under_images_with_the_image_key() {
    let dir = temp_dir("handler");
    let cache = dir.join("cache");
    let input = dir.join("d.svg");
    std::fs::write(&input, WIDE).expect("write");

    let params = json!({ "path": input, "width_px": 100 });
    let first = handler::handle(params.clone(), &cache).expect("miss");
    assert_eq!(first["cached"], json!(false));
    assert_eq!(first["width_px"], json!(100));
    assert_eq!(first["height_px"], json!(25));
    let path = PathBuf::from(first["path"].as_str().expect("path"));
    let key = key_image(&sha256_hex(WIDE), 100, None, None);
    assert_eq!(path, cache.join("images").join(format!("{key}.png")));
    assert_eq!(
        first["bytes"],
        json!(std::fs::metadata(&path).expect("meta").len())
    );

    let second = handler::handle(params, &cache).expect("hit");
    assert_eq!(second["cached"], json!(true));
    assert_eq!(second["path"], first["path"]);
    assert_eq!(second["height_px"], json!(25));

    let bad = dir.join("bad.svg");
    std::fs::write(&bad, b"<svg").expect("write");
    let err = handler::handle(json!({ "path": bad, "width_px": 100 }), &cache).unwrap_err();
    assert_eq!(err.code(), "unsupported_format");

    let err = handler::handle(
        json!({ "path": dir.join("none.svg"), "width_px": 1 }),
        &cache,
    )
    .unwrap_err();
    assert_eq!(err.code(), "io");
}
