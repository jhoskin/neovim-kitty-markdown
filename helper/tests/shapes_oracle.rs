//! Geometry pinned against mermaid 12.2.0 through the oracle
//! (`helper/oracle`, `mmdc -e svg -c helper/oracle/mermaid-config.json`).
//! The oracle measures an HTML label at 24 px per line, so these tests
//! use a 24 px measurer; the label width is the 120 px `minNodeWidth`.
//!
//! Sources:
//!
//! ```text
//! flowchart LR
//!     A[Hello] --> B{Choice}
//! ```
//!
//! ```text
//! flowchart LR
//!     A([Hello]) --> B[World]
//! ```

use std::collections::HashMap;

use kitty_markdown_helper::mermaid::edges::{Edge, EdgeCtx, insert_edge};
use kitty_markdown_helper::mermaid::scene::{Element, Point, TransformOp};
use kitty_markdown_helper::mermaid::shapes::{FixedMeasurer, Node, ShapeCtx, insert_node};

const ORACLE: FixedMeasurer = FixedMeasurer {
    char_width: 8.0,
    line_height: 24.0,
};

fn fmt3(v: f64) -> String {
    format!("{:.3}", v)
}

#[test]
fn rect_matches_oracle() {
    // <rect class="basic label-container" x="-90" y="-27" width="180" height="54"/>
    // <g class="label" transform="translate(-60, -12)">
    let ctx = ShapeCtx::new(&ORACLE);
    let mut node = Node::flowchart("A", "Hello", "rect");
    let out = insert_node(&mut node, &ctx).expect("draws");
    let Element::Rect(rect) = &out.group.children[0] else {
        panic!("first child is the rect");
    };
    assert_eq!(
        (
            fmt3(rect.x),
            fmt3(rect.y),
            fmt3(rect.width),
            fmt3(rect.height)
        ),
        (
            "-90.000".into(),
            "-27.000".into(),
            "180.000".into(),
            "54.000".into()
        )
    );
    let Element::Group(label) = &out.group.children[1] else {
        panic!("second child is the label");
    };
    assert_eq!(
        label.common.transform,
        vec![TransformOp::Translate(-60.0, -12.0)]
    );
}

#[test]
fn diamond_matches_oracle() {
    // <polygon points="87,0 174,-87 87,-174 0,-87" class="label-container"
    //          transform="translate(-86.5, 87)"/>
    let ctx = ShapeCtx::new(&ORACLE);
    let mut node = Node::flowchart("B", "Choice", "diamond");
    let out = insert_node(&mut node, &ctx).expect("draws");
    let Element::Polygon(poly) = &out.group.children[0] else {
        panic!("first child is the polygon");
    };
    let points: Vec<String> = poly
        .points
        .iter()
        .map(|p| format!("{},{}", fmt3(p.x), fmt3(p.y)))
        .collect();
    assert_eq!(
        points,
        vec![
            "87.000,0.000",
            "174.000,-87.000",
            "87.000,-174.000",
            "0.000,-87.000"
        ]
    );
    assert_eq!(
        poly.common.transform,
        vec![TransformOp::Translate(-86.5, 87.0)]
    );
    assert_eq!(fmt3(out.width), "174.000");
}

#[test]
fn stadium_matches_oracle() {
    // <path d="M-52.875 -19.5 C-25.08 -19.5, 2.71 -19.5, 52.875 -19.5 ..."
    // rough.js writes the straight segments as degenerate cubics; the
    // corner points and the node size are what the port must reproduce.
    let ctx = ShapeCtx::new(&ORACLE);
    let mut node = Node::flowchart("A", "Hello", "stadium");
    let out = insert_node(&mut node, &ctx).expect("draws");
    let Element::Path(path) = &out.group.children[0] else {
        panic!("first child is the path");
    };
    let d = path.d.to_svg();
    assert!(d.starts_with("M-52.875,-19.5L52.875,-19.5"), "{d}");
    // `stadiumDimensions` says 144.75, but `updateNodeBounds` reads the
    // path's bbox: the 50-point caps stop 1.84 degrees short of the pole,
    // so the node is 144.73 wide. The oracle agrees: A sits at x = 80.365
    // with its left edge on the 8 px margin.
    assert_eq!(fmt3(out.width), "144.730");
    assert_eq!(fmt3(out.height), "39.000");
    // The edge leaves the right cap at 72.365 from the centre: the oracle
    // path starts at x = 152.73 for a node centred at 80.365.
    node.x = 80.364_982_604_980_47;
    node.y = 35.0;
    let hit = node.intersect(Point::new(177.73, 35.0));
    assert_eq!(fmt3(hit.x), "152.730");
}

#[test]
fn edge_path_matches_oracle() {
    // <path d="M188,95L192.167,95C196.333,95,204.667,95,212.333,95
    //          C220,95,227,95,230.5,95L234,95" .../>
    // dagre placed A at (98, 95), the dummy at (213, 95), B at (325, 95).
    let ctx = ShapeCtx::new(&ORACLE);
    let mut a = Node::flowchart("A", "Hello", "rect");
    insert_node(&mut a, &ctx).expect("draws");
    a.x = 98.0;
    a.y = 95.0;
    let mut b = Node::flowchart("B", "Choice", "diamond");
    insert_node(&mut b, &ctx).expect("draws");
    b.x = 325.0;
    b.y = 95.0;
    let mut edge = Edge::flowchart("L_A_B_0", "A", "B");
    edge.points = vec![
        Point::new(188.0, 95.0),
        Point::new(213.0, 95.0),
        Point::new(238.0, 95.0),
    ];
    let ectx = EdgeCtx {
        shapes: &ctx,
        curve: Some("basis".to_string()),
        sub_graph_title_total_margin: 0.0,
    };
    let out = insert_edge(
        &edge,
        &HashMap::new(),
        "flowchart-v2",
        &a,
        &b,
        "my-svg",
        &ectx,
    );
    let Element::Path(path) = &out.path else {
        panic!("edge is a path");
    };
    assert_eq!(
        path.d.to_svg(),
        "M188,95L192.167,95C196.333,95,204.667,95,212.333,95C220,95,227,95,230.5,95L234,95"
    );
    assert_eq!(
        path.common.style.marker_end.as_deref(),
        Some("my-svg_flowchart-v2-pointEnd")
    );
    assert_eq!(path.common.style.marker_start, None);
}
