//! Scene to PNG through resvg: output size, background, and text with a
//! system font.

use std::sync::Arc;

use image::Rgba;
use kitty_markdown_helper::mermaid::raster::{
    Background, OutputSize, RasterOpts, render_png, render_rgba,
};
use kitty_markdown_helper::mermaid::scene::{
    Baseline, Color, Common, Element, FontSpec, Paint, Rect, Scene, Style, Text, TextAnchor,
    TextRun,
};
use usvg::fontdb;

fn system_fonts() -> Arc<fontdb::Database> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    Arc::new(db)
}

fn scene_with_text() -> Scene {
    let mut scene = Scene::new(200.0, 100.0);
    scene.font_family = "sans-serif".to_string();
    scene.root.children.push(Element::Rect(Rect {
        common: Common::with_style(Style {
            fill: Some(Paint::Color(Color::from_hex("#ECECFF").unwrap())),
            stroke: Some(Paint::Color(Color::from_hex("#9370DB").unwrap())),
            stroke_width: Some(1.0),
            ..Style::default()
        }),
        x: 20.0,
        y: 20.0,
        width: 160.0,
        height: 40.0,
        rx: Some(5.0),
        ry: Some(5.0),
    }));
    scene.root.children.push(Element::Text(Text {
        common: Common::with_style(Style::fill(Color::BLACK)),
        x: 100.0,
        y: 40.0,
        anchor: Some(TextAnchor::Middle),
        baseline: Some(Baseline::Middle),
        font: FontSpec {
            size: Some(16.0),
            ..FontSpec::default()
        },
        runs: vec![TextRun::new("Hello kitty")],
    }));
    scene
}

#[test]
fn natural_size_png_has_the_scene_dimensions() {
    let out = render_png(&scene_with_text(), system_fonts(), &RasterOpts::default()).unwrap();
    assert_eq!((out.width, out.height), (200, 100));
    assert_eq!(&out.png[..8], b"\x89PNG\r\n\x1a\n");
    let decoded = image::load_from_memory(&out.png).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (200, 100));
}

#[test]
fn target_width_scales_the_canvas() {
    let opts = RasterOpts {
        target_width: Some(100),
        background: None,
    };
    let out = render_png(&scene_with_text(), system_fonts(), &opts).unwrap();
    assert_eq!((out.width, out.height), (100, 50));
    assert_eq!(
        OutputSize::for_target(200.0, 100.0, 100),
        OutputSize {
            width: 100,
            height: 50,
            scale: 0.5
        }
    );
}

#[test]
fn background_is_white_where_nothing_is_drawn_and_text_leaves_ink() {
    let img = render_rgba(&scene_with_text(), system_fonts(), &RasterOpts::default()).unwrap();
    assert_eq!(img.get_pixel(2, 2), &Rgba([255, 255, 255, 255]));
    assert_eq!(img.get_pixel(197, 97), &Rgba([255, 255, 255, 255]));
    // The rect interior is the fill color.
    assert_eq!(img.get_pixel(30, 30), &Rgba([0xEC, 0xEC, 0xFF, 255]));
    // Somewhere on the text baseline band a pixel is darker than the fill.
    let dark = (60..140).any(|x| (32..48).any(|y| img.get_pixel(x, y).0[0] < 0x80));
    assert!(dark, "text in a system font left no dark pixels");
}

#[test]
fn transparent_background_keeps_alpha_zero() {
    let opts = RasterOpts {
        target_width: None,
        background: Some(Background::Transparent),
    };
    let img = render_rgba(&scene_with_text(), system_fonts(), &opts).unwrap();
    assert_eq!(img.get_pixel(2, 2).0[3], 0);
    assert_eq!(img.get_pixel(30, 30).0[3], 255);
}

#[test]
fn scene_background_is_used_without_an_override() {
    let mut scene = scene_with_text();
    scene.background = Some(Color::rgb(10, 20, 30));
    let img = render_rgba(&scene, system_fonts(), &RasterOpts::default()).unwrap();
    assert_eq!(img.get_pixel(2, 2), &Rgba([10, 20, 30, 255]));
}

fn solid_rect(x: f64, width: f64, paint: Paint) -> Element {
    Element::Rect(Rect {
        common: Common::with_style(Style {
            fill: Some(paint),
            ..Style::default()
        }),
        x,
        y: 0.0,
        width,
        height: 10.0,
        rx: None,
        ry: None,
    })
}

/// resvg honors the `style="mix-blend-mode: multiply;"` form the scene
/// writes: blue multiplied over red is black, blue over white stays blue.
#[test]
fn multiply_blend_mode_reaches_the_pixels() {
    use kitty_markdown_helper::mermaid::scene::{BlendMode, Group};
    let mut scene = Scene::new(40.0, 10.0);
    scene
        .root
        .children
        .push(solid_rect(0.0, 20.0, Paint::Color(Color::rgb(255, 0, 0))));
    scene.root.children.push(Element::Group(Group {
        common: Common::with_style(Style {
            mix_blend_mode: Some(BlendMode::Multiply),
            ..Style::default()
        }),
        children: vec![solid_rect(10.0, 30.0, Paint::Color(Color::rgb(0, 0, 255)))],
    }));
    let img = render_rgba(&scene, system_fonts(), &RasterOpts::default()).unwrap();
    assert_eq!(*img.get_pixel(5, 5), Rgba([255, 0, 0, 255]));
    assert_eq!(*img.get_pixel(15, 5), Rgba([0, 0, 0, 255]));
    assert_eq!(*img.get_pixel(35, 5), Rgba([0, 0, 255, 255]));
}

/// A user-space linear gradient paints its first stop at `x1` and its last
/// at `x2`.
#[test]
fn linear_gradient_reaches_the_pixels() {
    use kitty_markdown_helper::mermaid::scene::{GradientStop, GradientUnits, LinearGradient};
    let mut scene = Scene::new(100.0, 10.0);
    scene.linear_gradients.push(LinearGradient {
        id: "g".to_string(),
        units: GradientUnits::UserSpaceOnUse,
        x1: Some(0.0),
        y1: None,
        x2: Some(100.0),
        y2: None,
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: Color::rgb(255, 0, 0),
            },
            GradientStop {
                offset: 100.0,
                color: Color::rgb(0, 0, 255),
            },
        ],
    });
    scene
        .root
        .children
        .push(solid_rect(0.0, 100.0, Paint::Reference("g".to_string())));
    let img = render_rgba(&scene, system_fonts(), &RasterOpts::default()).unwrap();
    let left = img.get_pixel(0, 5);
    let right = img.get_pixel(99, 5);
    assert!(left[0] > 245 && left[2] < 10, "left {left:?}");
    assert!(right[2] > 245 && right[0] < 10, "right {right:?}");
}

/// Open Sans ships no bold face, so a bold Open Sans text draws with the
/// synthetic bold stroke and leaves more ink than the regular weight.
#[test]
fn bold_open_sans_draws_heavier_than_regular() {
    use kitty_markdown_helper::mermaid::scene::FontWeight;
    let ink = |weight: FontWeight| {
        let mut scene = Scene::new(120.0, 30.0);
        scene.root.children.push(Element::Text(Text {
            common: Common::with_style(Style::fill(Color::BLACK)),
            x: 4.0,
            y: 20.0,
            anchor: None,
            baseline: None,
            font: FontSpec {
                family: Some("\"Open Sans\", sans-serif".to_string()),
                size: Some(16.0),
                weight: Some(weight),
                style: None,
            },
            runs: vec![TextRun::new("Hello kitty")],
        }));
        let img = render_rgba(
            &scene,
            kitty_markdown_helper::mermaid::fontdb(),
            &RasterOpts::default(),
        )
        .unwrap();
        img.pixels().map(|p| 255 - u64::from(p[0])).sum::<u64>()
    };
    let (regular, bold) = (ink(FontWeight::Normal), ink(FontWeight::Bold));
    assert!(
        bold as f64 > regular as f64 * 1.1,
        "bold {bold} regular {regular}"
    );
}

/// A clip is in the user space of the element that uses it, so a clip rect
/// at the origin follows the group's translation like mermaid's treemap
/// cells.
#[test]
fn clip_path_cuts_in_the_user_space_of_its_user() {
    use kitty_markdown_helper::mermaid::scene::{ClipPath, Group, TransformOp};
    let mut scene = Scene::new(40.0, 10.0);
    let mut clipped = solid_rect(0.0, 30.0, Paint::Color(Color::rgb(255, 0, 0)));
    if let Element::Rect(r) = &mut clipped {
        r.common.style.clip_path = Some(ClipPath::rect("cell", 0.0, 0.0, 10.0, 10.0));
    }
    scene.root.children.push(Element::Group(Group {
        common: Common {
            transform: vec![TransformOp::Translate(10.0, 0.0)],
            ..Common::default()
        },
        children: vec![clipped],
    }));
    let img = render_rgba(&scene, system_fonts(), &RasterOpts::default()).unwrap();
    assert_eq!(*img.get_pixel(5, 5), Rgba([255, 255, 255, 255]));
    assert_eq!(*img.get_pixel(15, 5), Rgba([255, 0, 0, 255]));
    assert_eq!(*img.get_pixel(25, 5), Rgba([255, 255, 255, 255]));
}
