//! Snapshot of every ported shape at a fixed label size (8 px per
//! character, 16 px lines, `FixedMeasurer::SNAPSHOT`) with the flowchart
//! default padding, plus an intersection probe from each side.

use kitty_markdown_helper::mermaid::scene::svg::element_to_svg;
use kitty_markdown_helper::mermaid::scene::{Element, Point};
use kitty_markdown_helper::mermaid::shapes::{
    ClassMember, Classifier, ErAttribute, FixedMeasurer, Intersector, KanbanData, Node, NodeExtra,
    RequirementData, ShapeCtx, insert_node,
};

/// The shapes that draw (one representative alias per handler).
const DRAWN: &[&str] = &[
    "squareRect",
    "roundedRect",
    "stadium",
    "subroutine",
    "cylinder",
    "circle",
    "doublecircle",
    "question",
    "hexagon",
    "lean_right",
    "lean_left",
    "trapezoid",
    "inv_trapezoid",
    "odd",
    "text",
    "card",
    "shaded-process",
    "stateStart",
    "stateEnd",
    "forkJoin",
    "choice",
    "note",
    "hourglass",
    "brace-l",
    "brace-r",
    "braces",
    "bolt",
    "doc",
    "delay",
    "h-cyl",
    "lin-cyl",
    "curv-trap",
    "div-rect",
    "tri",
    "win-pane",
    "f-circ",
    "notch-pent",
    "flip-tri",
    "sl-rect",
    "docs",
    "st-rect",
    "bow-rect",
    "cross-circ",
    "tag-doc",
    "tag-rect",
    "flag",
    "lin-doc",
    "state",
    "composite",
    "rectWithTitle",
    "labelRect",
    "block_arrow",
    "collapsedGroup",
    "anchor",
    "kanbanItem",
    "mindmapCircle",
    "defaultMindmapNode",
    "classBox",
    "erBox",
    "requirementBox",
    "usecaseActor",
    "usecaseActorHollow",
    "usecaseActorAwesome",
    "usecaseBusiness",
    "usecaseEllipse",
    "usecaseJsonTable",
    "person",
    "folder",
    "bucket",
    "console",
    "browser",
    "datastore",
    "cloud",
    "bang",
];

fn node_for(shape: &str) -> Node {
    let mut node = Node::flowchart("n1", "Label", shape);
    node.description = vec!["line one".to_string(), "line two".to_string()];
    if shape == "kanbanItem" {
        // The kanban renderer sizes its cards; the shape reads the width.
        node.width = Some(200.0);
    }
    node.extra = match shape {
        "classBox" => NodeExtra::Class {
            annotations: vec!["interface".to_string()],
            members: vec![ClassMember {
                text: "+id: int".to_string(),
                classifier: Classifier::None,
            }],
            methods: vec![ClassMember {
                text: "run()".to_string(),
                classifier: Classifier::Abstract,
            }],
        },
        "erBox" => NodeExtra::Entity {
            alias: None,
            attributes: vec![
                ErAttribute {
                    type_name: "int".to_string(),
                    name: "id".to_string(),
                    keys: vec!["PK".to_string()],
                    comment: String::new(),
                },
                ErAttribute {
                    type_name: "string".to_string(),
                    name: "name".to_string(),
                    keys: vec![],
                    comment: "the name".to_string(),
                },
            ],
        },
        "requirementBox" => NodeExtra::Requirement(RequirementData::Requirement {
            type_name: "requirement".to_string(),
            name: "Label".to_string(),
            requirement_id: "1".to_string(),
            text: "the text".to_string(),
            risk: "High".to_string(),
            verify_method: "Test".to_string(),
        }),
        "kanbanItem" => NodeExtra::Kanban(KanbanData {
            ticket: Some("T-1".to_string()),
            assigned: Some("joh".to_string()),
            priority: Some("High".to_string()),
        }),
        "usecaseBusiness" | "usecaseActor" | "usecaseActorHollow" | "usecaseActorAwesome" => {
            NodeExtra::Usecase {
                stereotype: Some("actor".to_string()),
                business: true,
                rows: Vec::new(),
            }
        }
        "usecaseJsonTable" => NodeExtra::Usecase {
            stereotype: None,
            business: false,
            rows: vec![("key".to_string(), "value".to_string())],
        },
        _ => NodeExtra::None,
    };
    node
}

#[test]
fn every_shape_snapshot() {
    let measurer = FixedMeasurer::SNAPSHOT;
    let ctx = ShapeCtx::new(&measurer);
    for shape in DRAWN {
        let mut node = node_for(shape);
        let out = insert_node(&mut node, &ctx).unwrap_or_else(|e| panic!("{shape}: {e}"));
        let svg = element_to_svg(&Element::Group(out.group));
        let text = format!("width={} height={}\n{svg}", out.width, out.height);
        insta::assert_snapshot!(format!("shape_{}", shape.replace('-', "_")), text);
    }
}

#[test]
fn every_shape_intersects_from_each_side() {
    let measurer = FixedMeasurer::SNAPSHOT;
    let ctx = ShapeCtx::new(&measurer);
    // `block_arrow` without `directions` is mermaid's `point` outline, a
    // single point with no size; the block diagram supplies the directions.
    for shape in DRAWN.iter().filter(|s| **s != "block_arrow") {
        let mut node = node_for(shape);
        insert_node(&mut node, &ctx).unwrap_or_else(|e| panic!("{shape}: {e}"));
        node.x = 500.0;
        node.y = 400.0;
        let w = node.width();
        let h = node.height();
        assert!(w > 0.0 && h > 0.0, "{shape} has no size");
        assert!(
            node.intersector != Intersector::None,
            "{shape} sets no intersector"
        );
        for (dx, dy) in [(400.0, 0.0), (-400.0, 0.0), (0.0, 300.0), (0.0, -300.0)] {
            let target = Point::new(node.x + dx, node.y + dy);
            let p = node.intersect(target);
            assert!(
                p.x.is_finite() && p.y.is_finite(),
                "{shape} ({dx},{dy}): {p:?}"
            );
            let eps = 1e-6;
            assert!(
                (p.x - node.x).abs() <= w / 2.0 + eps && (p.y - node.y).abs() <= h / 2.0 + eps,
                "{shape} ({dx},{dy}): {p:?} outside {w}x{h}"
            );
            // The hit lies on the ray toward the target.
            if dx != 0.0 {
                assert!((p.x - node.x) * dx >= -eps, "{shape} ({dx},{dy}): {p:?}");
            } else {
                assert!((p.y - node.y) * dy >= -eps, "{shape} ({dx},{dy}): {p:?}");
            }
        }
    }
}

#[test]
fn rect_intersect_hits_the_sides() {
    let measurer = FixedMeasurer::SNAPSHOT;
    let ctx = ShapeCtx::new(&measurer);
    let mut node = Node::flowchart("A", "Hello", "rect");
    insert_node(&mut node, &ctx).expect("draws");
    node.x = 100.0;
    node.y = 100.0;
    let right = node.intersect(Point::new(300.0, 100.0));
    assert_eq!(right, Point::new(100.0 + node.width() / 2.0, 100.0));
    let top = node.intersect(Point::new(100.0, -100.0));
    assert_eq!(top, Point::new(100.0, 100.0 - node.height() / 2.0));
}

#[test]
fn unsupported_shapes_report_the_feature() {
    let measurer = FixedMeasurer::SNAPSHOT;
    let ctx = ShapeCtx::new(&measurer);
    for (shape, feature) in [
        ("icon", "icon pack"),
        ("iconSquare", "icon pack"),
        ("imageSquare", "image node"),
        ("usecaseActorIcon", "icon pack"),
    ] {
        let mut node = Node::flowchart("n", "x", shape);
        let err = insert_node(&mut node, &ctx).expect_err("refused");
        assert_eq!(err.code(), "unsupported_feature");
        assert!(err.to_string().contains(feature), "{shape}: {err}");
    }
}
