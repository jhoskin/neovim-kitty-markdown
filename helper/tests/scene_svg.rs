//! Snapshot tests of the deterministic SVG serializer. A snapshot change is
//! a byte change in every rendered diagram, so review it as such.

use kitty_markdown_helper::mermaid::scene::curves::{self, CurveKind};
use kitty_markdown_helper::mermaid::scene::markers::{MarkerType, markers_for};
use kitty_markdown_helper::mermaid::scene::{
    Baseline, Color, Common, Element, FontSpec, FontWeight, Group, Length, LineCap, Paint, Path,
    Point, Rect, Scene, Style, Text, TextAnchor, TextRun, TransformOp,
};

fn five_points() -> Vec<Point> {
    vec![
        Point::new(10.0, 20.0),
        Point::new(40.0, 55.5),
        Point::new(70.25, 30.0),
        Point::new(100.0, 80.0),
        Point::new(130.0, 60.125),
    ]
}

#[test]
fn rect_scene() {
    let mut scene = Scene::new(120.0, 60.5);
    scene.background = Some(Color::WHITE);
    scene.root.children.push(Element::Rect(Rect {
        common: Common {
            id: Some("node-A".to_string()),
            class: Some("node default".to_string()),
            transform: vec![TransformOp::Translate(10.0, 5.25)],
            style: Style {
                fill: Some(Paint::Color(Color::from_hex("#ECECFF").unwrap())),
                stroke: Some(Paint::Color(Color::from_hex("#9370DB").unwrap())),
                stroke_width: Some(1.0),
                ..Style::default()
            },
        },
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 50.0,
        rx: Some(5.0),
        ry: Some(5.0),
    }));
    insta::assert_snapshot!(scene.to_svg());
}

#[test]
fn basis_path_scene() {
    let mut scene = Scene::new(140.0, 100.0);
    scene.root.children.push(Element::Path(Path {
        common: Common::with_style(Style {
            fill: Some(Paint::None),
            stroke: Some(Paint::Color(Color::from_hex("#333333").unwrap())),
            stroke_width: Some(2.0),
            stroke_dasharray: Some(vec![3.0, 2.0]),
            stroke_linecap: Some(LineCap::Round),
            ..Style::default()
        }),
        d: curves::line(&five_points(), CurveKind::Basis),
    }));
    insta::assert_snapshot!(scene.to_svg());
}

#[test]
fn two_line_text() {
    let mut scene = Scene::new(200.0, 60.0);
    scene.root.children.push(Element::Text(Text {
        common: Common {
            class: Some("nodeLabel".to_string()),
            style: Style {
                fill: Some(Paint::Color(Color::from_hex("#333").unwrap())),
                ..Style::default()
            },
            ..Common::default()
        },
        x: 100.0,
        y: 20.0,
        anchor: Some(TextAnchor::Middle),
        baseline: Some(Baseline::Middle),
        font: FontSpec {
            size: Some(16.0),
            weight: Some(FontWeight::Bold),
            ..FontSpec::default()
        },
        runs: vec![
            TextRun::new("first <line> & co"),
            TextRun::line("second line", 100.0, Length::Em(1.1)),
        ],
    }));
    insta::assert_snapshot!(scene.to_svg());
}

#[test]
fn group_with_marker_reference() {
    let mut scene = Scene::new(200.0, 100.0);
    let line_paint = Style {
        fill: Some(Paint::Color(Color::from_hex("#333333").unwrap())),
        stroke: Some(Paint::Color(Color::from_hex("#333333").unwrap())),
        ..Style::default()
    };
    for mut marker in markers_for(MarkerType::Point, "flowchart-v2", "my-svg") {
        marker.apply_paint(&line_paint);
        scene.defs.push(Element::Marker(marker));
    }
    let mut edge = Path {
        common: Common {
            id: Some("my-svg_L_A_B_0".to_string()),
            class: Some("edge-thickness-normal edge-pattern-solid".to_string()),
            style: Style {
                fill: Some(Paint::None),
                stroke: Some(Paint::Color(Color::from_hex("#333333").unwrap())),
                stroke_width: Some(2.0),
                marker_end: Some("my-svg_flowchart-v2-pointEnd".to_string()),
                ..Style::default()
            },
            ..Common::default()
        },
        d: curves::line(
            &[
                Point::new(20.0, 50.0),
                Point::new(100.0, 50.0),
                Point::new(180.0, 50.0),
            ],
            CurveKind::Basis,
        ),
    };
    edge.common.transform.push(TransformOp::Translate(0.0, 0.0));
    scene.root.children.push(Element::Group(Group {
        common: Common {
            class: Some("edgePaths".to_string()),
            ..Common::default()
        },
        children: vec![Element::Path(edge)],
    }));
    insta::assert_snapshot!(scene.to_svg());
}

#[test]
fn serialization_is_deterministic() {
    let mut scene = Scene::new(140.0, 100.0);
    scene.root.children.push(Element::Path(Path {
        common: Common::with_style(Style::stroke(Color::BLACK, 1.0)),
        d: curves::line(&five_points(), CurveKind::CatmullRom),
    }));
    let a = scene.to_svg();
    let b = scene.clone().to_svg();
    assert_eq!(a, b);
    assert!(!a.contains("-0,"), "no negative zero in {a}");
}

/// A linear gradient goes into `defs` after the other definitions, with
/// mermaid's attribute order and percent offsets.
#[test]
fn linear_gradient_is_written_in_defs() {
    use kitty_markdown_helper::mermaid::scene::{GradientStop, GradientUnits, LinearGradient};
    let mut scene = Scene::new(100.0, 20.0);
    scene.linear_gradients.push(LinearGradient {
        id: "linearGradient-3".to_string(),
        units: GradientUnits::UserSpaceOnUse,
        x1: Some(10.0),
        y1: None,
        x2: Some(90.5),
        y2: None,
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: Color::from_hex("#4e79a7").unwrap(),
            },
            GradientStop {
                offset: 100.0,
                color: Color::rgba(255, 0, 0, 0.5),
            },
        ],
    });
    let svg = scene.to_svg();
    assert!(
        svg.contains(
        "  <defs>\n    <linearGradient id=\"linearGradient-3\" gradientUnits=\"userSpaceOnUse\" x1=\"10\" x2=\"90.5\">\n      <stop offset=\"0%\" stop-color=\"#4e79a7\"/>\n      <stop offset=\"100%\" stop-color=\"#ff0000\" stop-opacity=\"0.5\"/>\n    </linearGradient>\n  </defs>\n"
        ),
        "{svg}"
    );
}

/// A blend mode is a `style` declaration, as d3's `.style()` writes it,
/// and comes after every presentation attribute.
#[test]
fn mix_blend_mode_is_a_style_declaration() {
    use kitty_markdown_helper::mermaid::scene::BlendMode;
    let group = Element::Group(Group {
        common: Common {
            class: Some("link".to_string()),
            style: Style {
                stroke_opacity: Some(0.5),
                mix_blend_mode: Some(BlendMode::Multiply),
                ..Style::default()
            },
            ..Common::default()
        },
        children: Vec::new(),
    });
    assert_eq!(
        kitty_markdown_helper::mermaid::scene::svg::element_to_svg(&group),
        "<g class=\"link\" stroke-opacity=\"0.5\" style=\"mix-blend-mode: multiply;\"/>\n"
    );
}
