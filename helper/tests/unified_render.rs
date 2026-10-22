//! The unified renderer end to end with the hand-placing `FakeLayout`:
//! insta SVG snapshots of a three-node chain, a self loop, a cluster with
//! a title, and an edge label, plus the group order mermaid emits.

use kitty_markdown_helper::mermaid::edges::Edge;
use kitty_markdown_helper::mermaid::layout::unified::{
    FakeLayout, LayoutData, Rendered, UnifiedConfig, render,
};
use kitty_markdown_helper::mermaid::scene::markers::MarkerType;
use kitty_markdown_helper::mermaid::scene::{Element, Scene, ViewBox};
use kitty_markdown_helper::mermaid::shapes::{Dir, FixedMeasurer, Node, ShapeCtx};

fn flowchart_dir(nodes: Vec<Node>, edges: Vec<Edge>, direction: Dir) -> LayoutData {
    LayoutData {
        nodes,
        edges,
        direction,
        diagram_type: "flowchart-v2".to_string(),
        diagram_id: "svg".to_string(),
        markers: vec![MarkerType::Point],
        ..LayoutData::default()
    }
}

/// Wraps the render like a diagram package would: markers in `defs`, the
/// viewBox from the content bounds.
fn scene_svg(rendered: Rendered) -> String {
    let b = rendered.bounds;
    let mut scene = Scene::new(b.width, b.height);
    scene.view_box = ViewBox::new(b.x, b.y, b.width, b.height);
    scene.defs = rendered.markers.into_iter().map(Element::Marker).collect();
    scene.root.children.push(Element::Group(rendered.root));
    scene.to_svg()
}

fn render_svg(data: &LayoutData, layout: &FakeLayout) -> String {
    let measurer = FixedMeasurer::SNAPSHOT;
    let ctx = ShapeCtx::new(&measurer);
    let config = UnifiedConfig {
        curve: Some("basis".to_string()),
        ..UnifiedConfig::default()
    };
    let rendered = render(data, &config, &ctx, layout).expect("renders");
    scene_svg(rendered)
}

#[test]
fn three_node_chain() {
    let data = flowchart(
        vec![
            Node::flowchart("A", "Start", "rect"),
            Node::flowchart("B", "Decide", "diamond"),
            Node::flowchart("C", "End", "stadium"),
        ],
        vec![
            Edge::flowchart("L_A_B_0", "A", "B"),
            Edge::flowchart("L_B_C_0", "B", "C"),
        ],
    );
    let layout = FakeLayout::new(&[("A", 150.0, 50.0), ("B", 150.0, 250.0), ("C", 150.0, 450.0)]);
    insta::assert_snapshot!(render_svg(&data, &layout));
}

#[test]
fn self_loop() {
    let data = flowchart(
        vec![Node::flowchart("A", "Loop", "rect")],
        vec![Edge::flowchart("L_A_A_0", "A", "A")],
    );
    // The dummies below the node pick the `bottom` side.
    let layout = FakeLayout::new(&[
        ("A", 100.0, 50.0),
        ("A---A---1", 90.0, 130.0),
        ("A---A---2", 110.0, 180.0),
    ]);
    insta::assert_snapshot!(render_svg(&data, &layout));
}

fn flowchart(nodes: Vec<Node>, edges: Vec<Edge>) -> LayoutData {
    flowchart_dir(nodes, edges, Dir::TB)
}

#[test]
fn cluster_with_title() {
    let mut s = Node::flowchart("S", "Group", "rect");
    s.is_group = true;
    let mut a = Node::flowchart("A", "One", "rect");
    a.parent_id = Some("S".to_string());
    let mut b = Node::flowchart("B", "Two", "rect");
    b.parent_id = Some("S".to_string());
    let c = Node::flowchart("C", "Three", "rect");
    let data = flowchart_dir(
        vec![s, a, b, c],
        vec![
            Edge::flowchart("L_A_B_0", "A", "B"),
            Edge::flowchart("L_B_C_0", "B", "C"),
        ],
        Dir::TB,
    );
    let layout = FakeLayout::new(&[("A", 150.0, 60.0), ("B", 150.0, 180.0), ("C", 150.0, 360.0)]);
    insta::assert_snapshot!(render_svg(&data, &layout));
}

#[test]
fn edge_label() {
    let mut e = Edge::flowchart("L_A_B_0", "A", "B");
    e.label = Some("label".to_string());
    let data = flowchart_dir(
        vec![
            Node::flowchart("A", "One", "rect"),
            Node::flowchart("B", "Two", "rect"),
        ],
        vec![e],
        Dir::LR,
    );
    let layout = FakeLayout::new(&[("A", 100.0, 50.0), ("B", 400.0, 50.0)]);
    insta::assert_snapshot!(render_svg(&data, &layout));
}

#[test]
fn group_order_is_clusters_edge_paths_edge_labels_nodes() {
    let data = flowchart_dir(
        vec![
            Node::flowchart("A", "One", "rect"),
            Node::flowchart("B", "Two", "rect"),
        ],
        vec![Edge::flowchart("L_A_B_0", "A", "B")],
        Dir::TB,
    );
    let layout = FakeLayout::new(&[("A", 100.0, 50.0), ("B", 100.0, 200.0)]);
    let svg = render_svg(&data, &layout);
    let find = |class: &str| {
        svg.find(&format!("class=\"{class}\""))
            .unwrap_or_else(|| panic!("missing group {class}"))
    };
    let order = [
        find("clusters"),
        find("edgePaths"),
        find("edgeLabels"),
        find("nodes"),
    ];
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
}
