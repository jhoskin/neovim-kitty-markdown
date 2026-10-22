//! Port of `src/dagre/order/build-layer-graph.js`: one rank of the layout
//! graph with its subgraph hierarchy, plus the fixed neighbours on one side.

use super::super::LayoutGraph;
use super::super::graphlib::{Graph, GraphOptions};
use super::super::util::unique_id;

/// Which side's edges the layer graph copies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relationship {
    InEdges,
    OutEdges,
}

/// Node label of a layer graph. JS gives every node the layout graph's own
/// label object through `setDefaultNodeLabel`, and replaces it with
/// `{borderLeft, borderRight}` for a subgraph node. The port keeps only the
/// subgraph case here and reads everything else from the layout graph.
#[derive(Clone, Debug, Default)]
pub struct LayerNode {
    pub border_left: Option<String>,
    pub border_right: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct LayerEdge {
    pub weight: f64,
}

#[derive(Clone, Debug, Default)]
pub struct LayerGraphLabel {
    pub root: String,
}

pub type LayerGraph = Graph<LayerNode, LayerEdge, LayerGraphLabel>;

/// `buildLayerGraph(g, rank, relationship)`.
pub fn build_layer_graph(g: &LayoutGraph, rank: f64, relationship: Relationship) -> LayerGraph {
    let root = create_root_node(g);
    let mut result: LayerGraph = Graph::new(GraphOptions {
        compound: true,
        ..GraphOptions::default()
    });
    result.set_graph(LayerGraphLabel { root: root.clone() });

    for v in g.nodes() {
        let node = g.node(&v);
        let parent = g.parent(&v);

        let spans = node.min_rank.is_some_and(|min| min <= rank)
            && node.max_rank.is_some_and(|max| rank <= max);
        if node.rank != Some(rank) && !spans {
            continue;
        }

        result.ensure_node(&v);
        result.set_parent(&v, Some(parent.as_deref().unwrap_or(&root)));

        let edges = match relationship {
            Relationship::InEdges => g.in_edges(&v, None),
            Relationship::OutEdges => g.out_edges(&v, None),
        };
        for e in edges {
            let u = if e.v == v { &e.w } else { &e.v };
            let weight = result.try_edge_vw(u, &v).map_or(0.0, |edge| edge.weight);
            result.set_edge_vw(
                u,
                &v,
                LayerEdge {
                    weight: g.edge(&e).weight + weight,
                },
            );
        }

        if node.min_rank.is_some() {
            result.set_node(
                &v,
                LayerNode {
                    border_left: node.border_left.get(&(rank as i64)).cloned(),
                    border_right: node.border_right.get(&(rank as i64)).cloned(),
                },
            );
        }
    }

    result
}

/// `createRootNode(g)`: a `_root<n>` id that is not in the layout graph.
fn create_root_node(g: &LayoutGraph) -> String {
    let mut v = unique_id("_root");
    while g.has_node(&v) {
        v = unique_id("_root");
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::layout::dagre::graphlib::EdgeKey;
    use crate::mermaid::layout::dagre::util::new_layout_graph;
    use crate::mermaid::layout::dagre::{EdgeLabel, NodeLabel};

    fn node(g: &mut LayoutGraph, v: &str, rank: f64) {
        g.set_node(
            v,
            NodeLabel {
                rank: Some(rank),
                ..NodeLabel::default()
            },
        );
    }

    fn edge(g: &mut LayoutGraph, v: &str, w: &str, weight: f64) {
        g.set_edge(
            &EdgeKey::new(v, w),
            EdgeLabel {
                weight,
                ..EdgeLabel::default()
            },
        );
    }

    #[test]
    fn copies_the_rank_with_in_edges_and_sums_parallel_weights() {
        let mut g = new_layout_graph();
        node(&mut g, "a", 1.0);
        node(&mut g, "b", 1.0);
        node(&mut g, "c", 2.0);
        node(&mut g, "d", 2.0);
        edge(&mut g, "a", "c", 2.0);
        g.set_edge(
            &EdgeKey::named("a", "c", Some("x")),
            EdgeLabel {
                weight: 3.0,
                ..EdgeLabel::default()
            },
        );
        edge(&mut g, "b", "d", 1.0);
        let lg = build_layer_graph(&g, 2.0, Relationship::InEdges);
        let root = lg.graph().root.clone();
        assert_eq!(lg.children(Some(&root)), vec!["c", "d"]);
        assert_eq!(lg.edge(&EdgeKey::new("a", "c")).weight, 5.0);
        assert_eq!(lg.edge(&EdgeKey::new("b", "d")).weight, 1.0);
        assert!(lg.has_node("a"));
        assert_eq!(lg.parent("a"), None);
    }

    #[test]
    fn keeps_the_hierarchy_and_border_nodes_of_a_subgraph() {
        let mut g = new_layout_graph();
        node(&mut g, "a", 1.0);
        node(&mut g, "bl", 1.0);
        node(&mut g, "br", 1.0);
        let mut sg = NodeLabel {
            min_rank: Some(1.0),
            max_rank: Some(2.0),
            ..NodeLabel::default()
        };
        sg.border_left.insert(1, "bl".to_string());
        sg.border_right.insert(1, "br".to_string());
        g.set_node("sg", sg);
        for v in ["a", "bl", "br"] {
            g.set_parent(v, Some("sg"));
        }
        let lg = build_layer_graph(&g, 1.0, Relationship::OutEdges);
        let root = lg.graph().root.clone();
        assert_eq!(lg.children(Some(&root)), vec!["sg"]);
        assert_eq!(lg.children(Some("sg")), vec!["a", "bl", "br"]);
        assert_eq!(lg.node("sg").border_left.as_deref(), Some("bl"));
        assert_eq!(lg.node("sg").border_right.as_deref(), Some("br"));
        assert!(lg.node("a").border_left.is_none());
    }
}
