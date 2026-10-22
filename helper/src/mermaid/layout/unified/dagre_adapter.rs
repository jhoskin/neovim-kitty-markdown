//! The dagre seam: [`GraphLayout`] over the dagre port, copying the
//! unified graph into a `dagre::LayoutGraph` field by field and the result
//! back.
//!
//! In mermaid the two graphs are one object: `prepareLayoutForDagre` in
//! `rendering-util/layout-algorithms/dagre/index.js` builds a graphlib
//! graph with `setGraph({ rankdir, nodesep, ranksep, marginx: 8, marginy:
//! 8 })`, `setNode(node.id, { ...node })`, `setParent(node.id,
//! node.parentId)`, and `setEdge(edge.start, edge.end, { ...edge },
//! edge.id)`, and `dagreLayout(graph)` reads the node `width`/`height` and
//! the edge `minlen`/`weight`/`width`/`height`/`labelpos`/`labeloffset`
//! from those records (`buildLayoutGraph` in dagre's `layout.js`, with
//! `nodeDefaults` and `edgeDefaults` for the absent ones). The port keeps
//! the renderer's records in [`UGraph`] and dagre's in its own typed graph,
//! so this module is the copy in both directions.

use crate::error::HelperError;
use crate::mermaid::layout::dagre::{
    self, EdgeKey as DagreEdgeKey, EdgeLabel, GraphLabel as DagreGraphLabel, GraphOptions,
    LayoutGraph, NodeLabel,
};
use crate::mermaid::scene::Point;
use crate::mermaid::shapes::ShapeCtx;

use super::{EdgeKey, GraphLayout, LayoutData, Rendered, UGraph, UnifiedConfig};

/// `dagreLayout(graph)` for one graph. `render` calls it once per
/// sub-graph while it recurses, exactly as `recursiveRender` does.
#[derive(Debug, Clone, Copy, Default)]
pub struct DagreLayout;

impl GraphLayout for DagreLayout {
    fn layout(&self, graph: &mut UGraph) -> Result<(), HelperError> {
        let mut g = to_dagre(graph);
        dagre::layout(&mut g).map_err(|e| HelperError::internal(e.to_string()))?;
        copy_back(&g, graph);
        Ok(())
    }
}

/// `buildLayoutGraph`'s view of the mermaid graph: the graph label, each
/// node's size and parent, each edge's layout inputs. Nodes and edges go
/// in `graph.nodes()` and `graph.edges()` order, which is the JS object
/// order dagre iterates.
fn to_dagre(graph: &UGraph) -> LayoutGraph {
    let mut g = LayoutGraph::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });
    // `setGraph`: `edgesep`, `align`, `acyclicer`, and `ranker` stay at
    // dagre's `graphDefaults`; mermaid does not set them.
    g.set_graph(DagreGraphLabel {
        rankdir: graph.label.rankdir.clone(),
        nodesep: graph.label.nodesep,
        ranksep: graph.label.ranksep,
        marginx: graph.label.marginx,
        marginy: graph.label.marginy,
        ..DagreGraphLabel::default()
    });
    for id in graph.nodes() {
        let Some(unode) = graph.node(&id) else {
            continue;
        };
        // `nodeDefaults` are `{ width: 0, height: 0 }`: a cluster that was
        // not drawn has no size yet and gets zero, as in JS.
        g.set_node(
            &id,
            NodeLabel {
                width: unode.node.width(),
                height: unode.node.height(),
                ..NodeLabel::default()
            },
        );
        if let Some(parent) = graph.parent(&id) {
            g.set_parent(&id, Some(parent));
        }
    }
    for key in graph.edges() {
        let Some(uedge) = graph.edge(&key) else {
            continue;
        };
        // `edgeDefaults`: `weight` 1 and `labeloffset` 10, which no mermaid
        // edge record carries; `minlen` and `labelpos` have the record's
        // value or the default through `UEdge`.
        g.set_edge(
            &DagreEdgeKey::named(&key.v, &key.w, key.name.as_deref()),
            EdgeLabel {
                minlen: uedge.minlen(),
                weight: 1.0,
                width: uedge.edge.width,
                height: uedge.edge.height,
                labelpos: uedge.labelpos().to_string(),
                labeloffset: 10.0,
                ..EdgeLabel::default()
            },
        );
    }
    g
}

/// `updateInputGraph`: `x` and `y` for every node, `width` and `height`
/// for a compound node, `points` for every edge, and the label `x`/`y`
/// when dagre placed one.
fn copy_back(g: &LayoutGraph, graph: &mut UGraph) {
    for id in g.nodes() {
        let Some(label) = g.try_node(&id) else {
            continue;
        };
        let compound = !g.children(Some(&id)).is_empty();
        if let Some(unode) = graph.node_mut(&id) {
            unode.node.x = label.x;
            unode.node.y = label.y;
            if compound {
                unode.node.width = Some(label.width);
                unode.node.height = Some(label.height);
            }
        }
    }
    for key in g.edges() {
        let Some(label) = g.try_edge(&key) else {
            continue;
        };
        let ukey = EdgeKey::new(&key.v, &key.w, key.name.as_deref());
        if let Some(uedge) = graph.edge_mut(&ukey) {
            uedge.edge.points = label.points.iter().map(|p| Point::new(p.x, p.y)).collect();
            if label.x.is_some() {
                uedge.edge.x = label.x;
                uedge.edge.y = label.y;
            }
        }
    }
}

/// The unified renderer with the real dagre: resets the dummy-id counter
/// that mermaid's bundle shares across one render (every sub-graph layout
/// continues the same `_.uniqueId` sequence), then renders.
pub fn render_with_dagre(
    data: &LayoutData,
    config: &UnifiedConfig,
    ctx: &ShapeCtx,
) -> Result<Rendered, HelperError> {
    dagre::reset_unique_ids();
    super::render(data, config, ctx, &DagreLayout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::edges::Edge;
    use crate::mermaid::layout::unified::{GraphLabel, UEdge, UNode};
    use crate::mermaid::shapes::Node;

    fn sized(id: &str, w: f64, h: f64) -> UNode {
        UNode::new(Node {
            width: Some(w),
            height: Some(h),
            ..Node::new(id, id, "rect")
        })
    }

    #[test]
    fn copies_sizes_parents_and_edge_inputs() {
        let mut graph = UGraph::new(GraphLabel {
            rankdir: "LR".to_string(),
            nodesep: 30.0,
            ranksep: 70.0,
            marginx: 8.0,
            marginy: 8.0,
        });
        graph.set_node("A", sized("A", 100.0, 40.0));
        graph.set_node("B", sized("B", 60.0, 20.0));
        graph.set_parent("B", Some("S"));
        let mut e = Edge::flowchart("e", "A", "B");
        e.minlen = Some(2.0);
        e.width = 30.0;
        e.height = 10.0;
        graph.set_edge("A", "B", UEdge::new(e), Some("e"));

        let g = to_dagre(&graph);
        assert_eq!(g.graph().rankdir, "LR");
        assert_eq!(g.graph().nodesep, 30.0);
        assert_eq!(g.graph().ranksep, 70.0);
        assert_eq!((g.graph().marginx, g.graph().marginy), (8.0, 8.0));
        assert_eq!(g.graph().edgesep, 20.0);
        // `setParent` created `S` after `B`, in both graphs.
        assert_eq!(g.nodes(), vec!["A", "B", "S"]);
        assert_eq!(g.node("A").width, 100.0);
        assert_eq!(g.node("S").width, 0.0);
        assert_eq!(g.parent("B").as_deref(), Some("S"));
        let key = DagreEdgeKey::named("A", "B", Some("e"));
        let label = g.edge(&key);
        assert_eq!(label.minlen, 2.0);
        assert_eq!(label.weight, 1.0);
        assert_eq!((label.width, label.height), (30.0, 10.0));
        assert_eq!(label.labelpos, "c");
        assert_eq!(label.labeloffset, 10.0);
    }

    #[test]
    fn lays_out_a_chain_and_writes_positions_back() {
        let mut graph = UGraph::new(GraphLabel {
            rankdir: "LR".to_string(),
            ..GraphLabel::default()
        });
        graph.set_node("A", sized("A", 100.0, 40.0));
        graph.set_node("B", sized("B", 100.0, 40.0));
        graph.set_edge(
            "A",
            "B",
            UEdge::new(Edge::flowchart("e", "A", "B")),
            Some("e"),
        );
        dagre::reset_unique_ids();
        DagreLayout.layout(&mut graph).expect("lays out");
        let a = graph.node("A").expect("A").node.clone();
        let b = graph.node("B").expect("B").node.clone();
        // marginx 8 + half width; B one rank (50 + 100) to the right.
        assert_eq!((a.x, a.y), (58.0, 28.0));
        assert_eq!((b.x, b.y), (208.0, 28.0));
        let e = graph
            .edge(&EdgeKey::new("A", "B", Some("e")))
            .expect("edge")
            .edge
            .clone();
        assert_eq!(e.points.len(), 3);
        assert_eq!(e.points[0], Point::new(108.0, 28.0));
        assert_eq!(e.points[2], Point::new(158.0, 28.0));
        assert_eq!(e.x, None);
    }
}
