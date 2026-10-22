//! The compound multigraph mermaid's dagre glue builds with graphlib
//! (`new graphlib.Graph({ multigraph: true, compound: true })`), carrying
//! the renderer's own node and edge records.
//!
//! The layout engine behind [`super::GraphLayout`] reads the sizes and
//! writes the positions. The lead's adapter copies this graph into
//! `dagre::LayoutGraph` field by field: `GraphLabel` from [`GraphLabel`],
//! `NodeLabel { width, height }` from [`UNode`], `EdgeLabel { minlen,
//! weight, width, height, labelpos, labeloffset }` from [`UEdge`], the
//! parents with `set_parent`, and `reset_unique_ids()` once per diagram
//! before the first `layout`.
//!
//! Iteration order follows graphlib's JS object order: `nodes()` and
//! `children()` list canonical array-index keys first in numeric order,
//! then the rest in insertion order; `edges()` is insertion order.

use indexmap::{IndexMap, IndexSet};
use std::collections::HashMap;

use crate::mermaid::edges::Edge;
use crate::mermaid::shapes::Node;

/// `graph.setGraph({...})`: the attributes dagre reads.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphLabel {
    /// Upper case, as mermaid passes it (`TB`, `LR`, ...).
    pub rankdir: String,
    pub nodesep: f64,
    pub ranksep: f64,
    pub marginx: f64,
    pub marginy: f64,
}

impl Default for GraphLabel {
    fn default() -> GraphLabel {
        GraphLabel {
            rankdir: "TB".to_string(),
            nodesep: 50.0,
            ranksep: 50.0,
            marginx: 8.0,
            marginy: 8.0,
        }
    }
}

/// A node record: the renderer's [`Node`] (whose `x`, `y`, `width`,
/// `height` are the layout fields) plus the cluster bookkeeping of
/// `mermaid-graphlib.js`.
#[derive(Debug, Clone, Default)]
pub struct UNode {
    pub node: Node,
    /// `clusterNode: true`: this node stands for an extracted cluster whose
    /// contents live in `graph`.
    pub cluster_node: bool,
    /// `clusterData` of an extracted cluster.
    pub cluster_data: Option<Node>,
    pub graph: Option<Box<UGraph>>,
    /// `diff` from the recursive render.
    pub diff: f64,
    /// `labelBBox`, `offsetY` written by `insertCluster`.
    pub label_height: f64,
}

impl UNode {
    pub fn new(node: Node) -> UNode {
        UNode {
            node,
            ..UNode::default()
        }
    }
}

/// Self-loop bookkeeping on the three layout segments of a self edge.
#[derive(Debug, Clone, PartialEq)]
pub struct SelfLoop {
    pub id: String,
    pub order: u8,
}

/// An edge record: the renderer's [`Edge`] plus dagre's inputs and the
/// self-loop fields `prepareLayoutForDagre` adds.
#[derive(Debug, Clone, Default)]
pub struct UEdge {
    pub edge: Edge,
    pub self_loop: Option<SelfLoop>,
    pub original_edge: Option<Box<Edge>>,
}

impl UEdge {
    pub fn new(edge: Edge) -> UEdge {
        UEdge {
            edge,
            ..UEdge::default()
        }
    }

    /// dagre's `minlen` input (`edge.minlen`, default 1).
    pub fn minlen(&self) -> f64 {
        self.edge.minlen.unwrap_or(1.0)
    }

    /// dagre's `labelpos` input (`edge.labelpos`, default `r`).
    pub fn labelpos(&self) -> &str {
        self.edge.labelpos.as_deref().unwrap_or("r")
    }
}

/// `{ v, w, name }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EdgeKey {
    pub v: String,
    pub w: String,
    pub name: Option<String>,
}

impl EdgeKey {
    pub fn new(v: &str, w: &str, name: Option<&str>) -> EdgeKey {
        EdgeKey {
            v: v.to_string(),
            w: w.to_string(),
            name: name.map(str::to_string),
        }
    }
}

/// graphlib's `Graph` with the operations the glue uses.
#[derive(Debug, Clone, Default)]
pub struct UGraph {
    pub label: GraphLabel,
    nodes: IndexMap<String, UNode>,
    parent: HashMap<String, String>,
    children: IndexMap<String, IndexSet<String>>,
    edges: IndexMap<EdgeKey, UEdge>,
}

/// Whether a key is a canonical array index, which JS objects list first.
fn is_array_index(key: &str) -> bool {
    if key.is_empty() || key.len() > 10 {
        return false;
    }
    if key != "0" && key.starts_with('0') {
        return false;
    }
    if !key.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    key.parse::<u64>().is_ok_and(|v| v < u32::MAX as u64)
}

/// JS object key order: array indices ascending, then insertion order.
pub fn js_key_order<'a>(keys: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    let mut indices: Vec<(u64, String)> = Vec::new();
    let mut rest: Vec<String> = Vec::new();
    for k in keys {
        if is_array_index(k) {
            indices.push((k.parse::<u64>().unwrap_or(0), k.clone()));
        } else {
            rest.push(k.clone());
        }
    }
    indices.sort_by_key(|(n, _)| *n);
    indices.into_iter().map(|(_, k)| k).chain(rest).collect()
}

impl UGraph {
    pub fn new(label: GraphLabel) -> UGraph {
        UGraph {
            label,
            ..UGraph::default()
        }
    }

    /// `graph.setNode(v, label)`: inserts or replaces the label, keeping
    /// the node's place and its parent/children relations.
    pub fn set_node(&mut self, v: &str, node: UNode) {
        match self.nodes.get_mut(v) {
            Some(existing) => *existing = node,
            None => {
                self.nodes.insert(v.to_string(), node);
            }
        }
    }

    /// `graph.setNode(v)` with no label: creates the node if missing.
    fn ensure_node(&mut self, v: &str) {
        if !self.nodes.contains_key(v) {
            let placeholder = Node {
                id: v.to_string(),
                ..Node::default()
            };
            self.nodes.insert(v.to_string(), UNode::new(placeholder));
        }
    }

    pub fn has_node(&self, v: &str) -> bool {
        self.nodes.contains_key(v)
    }

    pub fn node(&self, v: &str) -> Option<&UNode> {
        self.nodes.get(v)
    }

    pub fn node_mut(&mut self, v: &str) -> Option<&mut UNode> {
        self.nodes.get_mut(v)
    }

    /// `graph.nodes()` in JS object order.
    pub fn nodes(&self) -> Vec<String> {
        js_key_order(self.nodes.keys())
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// `graph.removeNode(v)`: drops the node, its edges, and lifts its
    /// children to its own parent's level.
    pub fn remove_node(&mut self, v: &str) {
        if self.nodes.shift_remove(v).is_none() {
            return;
        }
        if let Some(parent) = self.parent.remove(v)
            && let Some(set) = self.children.get_mut(&parent)
        {
            set.shift_remove(v);
        }
        if let Some(children) = self.children.shift_remove(v) {
            for child in children {
                self.parent.remove(&child);
            }
        }
        let keys: Vec<EdgeKey> = self
            .edges
            .keys()
            .filter(|k| k.v == v || k.w == v)
            .cloned()
            .collect();
        for k in keys {
            self.edges.shift_remove(&k);
        }
    }

    /// `graph.setParent(v, parent)`; `None` makes `v` a root.
    pub fn set_parent(&mut self, v: &str, parent: Option<&str>) {
        self.ensure_node(v);
        if let Some(old) = self.parent.remove(v)
            && let Some(set) = self.children.get_mut(&old)
        {
            set.shift_remove(v);
        }
        if let Some(p) = parent {
            self.ensure_node(p);
            self.parent.insert(v.to_string(), p.to_string());
            self.children
                .entry(p.to_string())
                .or_default()
                .insert(v.to_string());
        }
    }

    pub fn parent(&self, v: &str) -> Option<&str> {
        self.parent.get(v).map(String::as_str)
    }

    /// `graph.children(v)`; `None` lists the roots.
    pub fn children(&self, v: Option<&str>) -> Vec<String> {
        match v {
            Some(v) => self
                .children
                .get(v)
                .map(|set| js_key_order(set.iter()))
                .unwrap_or_default(),
            None => js_key_order(self.nodes.keys().filter(|k| !self.parent.contains_key(*k))),
        }
    }

    /// `graph.setEdge(v, w, label, name)`.
    pub fn set_edge(&mut self, v: &str, w: &str, edge: UEdge, name: Option<&str>) {
        self.ensure_node(v);
        self.ensure_node(w);
        let key = EdgeKey::new(v, w, name);
        match self.edges.get_mut(&key) {
            Some(existing) => *existing = edge,
            None => {
                self.edges.insert(key, edge);
            }
        }
    }

    pub fn edge(&self, key: &EdgeKey) -> Option<&UEdge> {
        self.edges.get(key)
    }

    pub fn edge_mut(&mut self, key: &EdgeKey) -> Option<&mut UEdge> {
        self.edges.get_mut(key)
    }

    pub fn remove_edge(&mut self, key: &EdgeKey) -> Option<UEdge> {
        self.edges.shift_remove(key)
    }

    /// `graph.edges()` in insertion order.
    pub fn edges(&self) -> Vec<EdgeKey> {
        self.edges.keys().cloned().collect()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// `graph.nodeEdges(v)`: in and out edges of `v`, in edge order.
    pub fn node_edges(&self, v: &str) -> Vec<EdgeKey> {
        self.edges
            .keys()
            .filter(|k| k.v == v || k.w == v)
            .cloned()
            .collect()
    }

    /// Every node record, in JS order, with its id.
    pub fn iter_nodes(&self) -> impl Iterator<Item = (String, &UNode)> {
        self.nodes().into_iter().map(move |id| {
            let n = &self.nodes[&id];
            (id, n)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_ids_come_first() {
        let mut g = UGraph::default();
        g.set_node("b", UNode::default());
        g.set_node("10", UNode::default());
        g.set_node("2", UNode::default());
        g.set_node("a", UNode::default());
        assert_eq!(g.nodes(), vec!["2", "10", "b", "a"]);
    }

    #[test]
    fn remove_node_drops_edges_and_lifts_children() {
        let mut g = UGraph::default();
        g.set_node("p", UNode::default());
        g.set_parent("c", Some("p"));
        g.set_edge("c", "x", UEdge::default(), None);
        assert_eq!(g.children(None), vec!["p", "x"]);
        g.remove_node("p");
        assert_eq!(g.parent("c"), None);
        assert_eq!(g.edge_count(), 1);
        g.remove_node("c");
        assert_eq!(g.edge_count(), 0);
    }
}
