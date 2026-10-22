//! Port of the graphlib subset that dagre uses: dagre-d3-es 7.0.14
//! `src/graphlib/graph.js` plus `alg/dfs.js`, `alg/preorder.js`, and
//! `alg/postorder.js`.
//!
//! # Iteration order
//!
//! graphlib keeps every collection in a plain JS object, so dagre's results
//! depend on JS property order. The rule is: keys that are canonical array
//! indices (`"0"`, `"17"`, no leading zero, below 2^32 - 1) come first in
//! ascending numeric order, then every other key in insertion order. A key
//! that is deleted and inserted again moves to the end.
//!
//! The rule touches collections keyed by node id: `_nodes`, `_children[v]`,
//! `_preds[v]`, and `_sucs[v]`. `nodes()`, `children()`, `predecessors()`,
//! `successors()`, `neighbors()`, `sources()`, and `sinks()` apply it through
//! [`js_key_order`]. Collections keyed by edge id (`_edgeObjs`, `_in[v]`,
//! `_out[v]`) never hold an array index, because the id contains `\x01`, so
//! `edges()`, `in_edges()`, and `out_edges()` are plain insertion order.
//! `IndexMap` and `IndexSet` give insertion order; `shift_remove` keeps the
//! order of the remaining entries the way `delete` does.
//!
//! Undirected graphs (the tight tree in network simplex) store an edge with
//! `v <= w` after a string comparison. JS compares UTF-16 code units and Rust
//! compares bytes; the two agree on every id without astral characters.

use indexmap::{IndexMap, IndexSet};

/// `{v, w, name}` from `graph.js`. The default edge name is `None`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EdgeKey {
    pub v: String,
    pub w: String,
    pub name: Option<String>,
}

impl EdgeKey {
    pub fn new(v: &str, w: &str) -> Self {
        Self {
            v: v.to_string(),
            w: w.to_string(),
            name: None,
        }
    }

    pub fn named(v: &str, w: &str, name: Option<&str>) -> Self {
        Self {
            v: v.to_string(),
            w: w.to_string(),
            name: name.map(str::to_string),
        }
    }
}

/// Constructor options of `new Graph(opts)`.
#[derive(Clone, Copy, Debug)]
pub struct GraphOptions {
    pub directed: bool,
    pub multigraph: bool,
    pub compound: bool,
}

impl Default for GraphOptions {
    fn default() -> Self {
        Self {
            directed: true,
            multigraph: false,
            compound: false,
        }
    }
}

/// Returns true when `key` is a canonical array index, the keys JS orders
/// numerically before all other keys.
pub fn is_array_index(key: &str) -> bool {
    if key == "0" {
        return true;
    }
    let bytes = key.as_bytes();
    if bytes.is_empty() || bytes.len() > 10 || bytes[0] == b'0' {
        return false;
    }
    if !bytes.iter().all(u8::is_ascii_digit) {
        return false;
    }
    key.parse::<u64>().is_ok_and(|n| n < 4_294_967_295)
}

/// Orders keys the way `Object.keys` does: array indices ascending, then the
/// rest in insertion order.
pub fn js_key_order<'a, I>(keys: I) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut indices: Vec<(u64, &str)> = Vec::new();
    let mut rest: Vec<&str> = Vec::new();
    for key in keys {
        if is_array_index(key) {
            // The parse cannot fail after `is_array_index`; 0 is a safe fallback.
            indices.push((key.parse().unwrap_or(0), key));
        } else {
            rest.push(key);
        }
    }
    indices.sort_by_key(|(n, _)| *n);
    indices
        .into_iter()
        .map(|(_, k)| k.to_string())
        .chain(rest.into_iter().map(str::to_string))
        .collect()
}

/// A graph with node labels `N`, edge labels `E`, and a graph label `G`.
///
/// Missing-node and missing-edge accessors panic with the node or edge in the
/// message; the JS code would throw a `TypeError` on `undefined.prop` at the
/// same points, so a panic marks a port bug, not a user input problem.
#[derive(Clone, Debug)]
pub struct Graph<N, E, G> {
    directed: bool,
    multigraph: bool,
    compound: bool,
    label: G,
    nodes: IndexMap<String, N>,
    parent: IndexMap<String, Option<String>>,
    children: IndexMap<Option<String>, IndexSet<String>>,
    in_edges: IndexMap<String, IndexSet<EdgeKey>>,
    out_edges: IndexMap<String, IndexSet<EdgeKey>>,
    preds: IndexMap<String, IndexMap<String, usize>>,
    sucs: IndexMap<String, IndexMap<String, usize>>,
    edges: IndexMap<EdgeKey, E>,
}

impl<N, E, G: Default> Default for Graph<N, E, G> {
    fn default() -> Self {
        Self::new(GraphOptions::default())
    }
}

impl<N, E, G: Default> Graph<N, E, G> {
    pub fn new(opts: GraphOptions) -> Self {
        let mut children = IndexMap::new();
        if opts.compound {
            children.insert(None, IndexSet::new());
        }
        Self {
            directed: opts.directed,
            multigraph: opts.multigraph,
            compound: opts.compound,
            label: G::default(),
            nodes: IndexMap::new(),
            parent: IndexMap::new(),
            children,
            in_edges: IndexMap::new(),
            out_edges: IndexMap::new(),
            preds: IndexMap::new(),
            sucs: IndexMap::new(),
            edges: IndexMap::new(),
        }
    }
}

impl<N, E, G> Graph<N, E, G> {
    pub fn is_directed(&self) -> bool {
        self.directed
    }

    pub fn is_multigraph(&self) -> bool {
        self.multigraph
    }

    pub fn is_compound(&self) -> bool {
        self.compound
    }

    pub fn set_graph(&mut self, label: G) -> &mut Self {
        self.label = label;
        self
    }

    pub fn graph(&self) -> &G {
        &self.label
    }

    pub fn graph_mut(&mut self) -> &mut G {
        &mut self.label
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// `nodes()`: `_.keys(this._nodes)`.
    pub fn nodes(&self) -> Vec<String> {
        js_key_order(self.nodes.keys().map(String::as_str))
    }

    /// `sources()`: nodes with no in-edges.
    pub fn sources(&self) -> Vec<String> {
        self.nodes()
            .into_iter()
            .filter(|v| self.in_edges.get(v).is_none_or(IndexSet::is_empty))
            .collect()
    }

    /// `sinks()`: nodes with no out-edges.
    pub fn sinks(&self) -> Vec<String> {
        self.nodes()
            .into_iter()
            .filter(|v| self.out_edges.get(v).is_none_or(IndexSet::is_empty))
            .collect()
    }

    /// `setNode(v, value)`: inserts or replaces the label and keeps position.
    pub fn set_node(&mut self, v: &str, label: N) -> &mut Self {
        if let Some(slot) = self.nodes.get_mut(v) {
            *slot = label;
            return self;
        }
        self.insert_node(v, label);
        self
    }

    fn insert_node(&mut self, v: &str, label: N) {
        self.nodes.insert(v.to_string(), label);
        if self.compound {
            self.parent.insert(v.to_string(), None);
            self.children.insert(Some(v.to_string()), IndexSet::new());
            self.children.entry(None).or_default().insert(v.to_string());
        }
        self.in_edges.insert(v.to_string(), IndexSet::new());
        self.preds.insert(v.to_string(), IndexMap::new());
        self.out_edges.insert(v.to_string(), IndexSet::new());
        self.sucs.insert(v.to_string(), IndexMap::new());
    }

    pub fn has_node(&self, v: &str) -> bool {
        self.nodes.contains_key(v)
    }

    pub fn try_node(&self, v: &str) -> Option<&N> {
        self.nodes.get(v)
    }

    pub fn try_node_mut(&mut self, v: &str) -> Option<&mut N> {
        self.nodes.get_mut(v)
    }

    /// `node(v)` for a node that must exist.
    pub fn node(&self, v: &str) -> &N {
        match self.nodes.get(v) {
            Some(label) => label,
            None => panic!("graphlib: node {v:?} is not in the graph"),
        }
    }

    pub fn node_mut(&mut self, v: &str) -> &mut N {
        match self.nodes.get_mut(v) {
            Some(label) => label,
            None => panic!("graphlib: node {v:?} is not in the graph"),
        }
    }

    /// `removeNode(v)`: drops the node and its edges, and moves its children
    /// to the root. Returns the label.
    pub fn remove_node(&mut self, v: &str) -> Option<N> {
        let label = self.nodes.shift_remove(v)?;
        if self.compound {
            self.remove_from_parents_child_list(v);
            self.parent.shift_remove(v);
            for child in self.children(Some(v)) {
                self.set_parent_inner(&child, None);
            }
            self.children.shift_remove(&Some(v.to_string()));
        }
        let incoming: Vec<EdgeKey> = self
            .in_edges
            .get(v)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        for e in incoming {
            self.remove_edge(&e);
        }
        self.in_edges.shift_remove(v);
        self.preds.shift_remove(v);
        let outgoing: Vec<EdgeKey> = self
            .out_edges
            .get(v)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        for e in outgoing {
            self.remove_edge(&e);
        }
        self.out_edges.shift_remove(v);
        self.sucs.shift_remove(v);
        Some(label)
    }

    fn remove_from_parents_child_list(&mut self, v: &str) {
        let parent = self.parent.get(v).cloned().unwrap_or_default();
        if let Some(set) = self.children.get_mut(&parent) {
            set.shift_remove(v);
        }
    }

    /// The part of `setParent` that runs after both nodes exist.
    fn set_parent_inner(&mut self, v: &str, parent: Option<&str>) {
        self.remove_from_parents_child_list(v);
        self.parent
            .insert(v.to_string(), parent.map(str::to_string));
        self.children
            .entry(parent.map(str::to_string))
            .or_default()
            .insert(v.to_string());
    }

    /// `parent(v)`: `None` for a root node, a non-compound graph, or an
    /// unknown node.
    pub fn parent(&self, v: &str) -> Option<String> {
        if !self.compound {
            return None;
        }
        self.parent.get(v).cloned().flatten()
    }

    /// `children(v)`: `None` asks for the root children. An unknown node
    /// yields an empty list where JS yields `undefined`.
    pub fn children(&self, v: Option<&str>) -> Vec<String> {
        if self.compound {
            return self
                .children
                .get(&v.map(str::to_string))
                .map(|set| js_key_order(set.iter().map(String::as_str)))
                .unwrap_or_default();
        }
        match v {
            None => self.nodes(),
            Some(_) => Vec::new(),
        }
    }

    pub fn predecessors(&self, v: &str) -> Vec<String> {
        self.preds
            .get(v)
            .map(|m| js_key_order(m.keys().map(String::as_str)))
            .unwrap_or_default()
    }

    pub fn successors(&self, v: &str) -> Vec<String> {
        self.sucs
            .get(v)
            .map(|m| js_key_order(m.keys().map(String::as_str)))
            .unwrap_or_default()
    }

    /// `neighbors(v)`: `_.union(predecessors, successors)`.
    pub fn neighbors(&self, v: &str) -> Vec<String> {
        let mut seen: IndexSet<String> = IndexSet::new();
        for u in self.predecessors(v) {
            seen.insert(u);
        }
        for w in self.successors(v) {
            seen.insert(w);
        }
        seen.into_iter().collect()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// `edges()`: `_.values(this._edgeObjs)`, insertion order.
    pub fn edges(&self) -> Vec<EdgeKey> {
        self.edges.keys().cloned().collect()
    }

    /// `edgeArgsToId` and `edgeArgsToObj`: an undirected graph stores the
    /// smaller endpoint first.
    fn normalize_key(&self, key: &EdgeKey) -> EdgeKey {
        if !self.directed && key.v > key.w {
            EdgeKey {
                v: key.w.clone(),
                w: key.v.clone(),
                name: key.name.clone(),
            }
        } else {
            key.clone()
        }
    }

    pub fn has_edge(&self, key: &EdgeKey) -> bool {
        self.edges.contains_key(&self.normalize_key(key))
    }

    pub fn try_edge(&self, key: &EdgeKey) -> Option<&E> {
        self.edges.get(&self.normalize_key(key))
    }

    pub fn try_edge_mut(&mut self, key: &EdgeKey) -> Option<&mut E> {
        let key = self.normalize_key(key);
        self.edges.get_mut(&key)
    }

    /// `edge(e)` for an edge that must exist.
    pub fn edge(&self, key: &EdgeKey) -> &E {
        match self.edges.get(&self.normalize_key(key)) {
            Some(label) => label,
            None => panic!("graphlib: edge {key:?} is not in the graph"),
        }
    }

    pub fn edge_mut(&mut self, key: &EdgeKey) -> &mut E {
        let key = self.normalize_key(key);
        match self.edges.get_mut(&key) {
            Some(label) => label,
            None => panic!("graphlib: edge {key:?} is not in the graph"),
        }
    }

    /// `edge(v, w)` on a graph without edge names.
    pub fn try_edge_vw(&self, v: &str, w: &str) -> Option<&E> {
        self.try_edge(&EdgeKey::new(v, w))
    }

    pub fn has_edge_vw(&self, v: &str, w: &str) -> bool {
        self.has_edge(&EdgeKey::new(v, w))
    }

    /// `removeEdge(e)`: returns the label so callers can keep it.
    pub fn remove_edge(&mut self, key: &EdgeKey) -> Option<E> {
        let key = self.normalize_key(key);
        let label = self.edges.shift_remove(&key)?;
        if let Some(m) = self.preds.get_mut(&key.w) {
            decrement_or_remove(m, &key.v);
        }
        if let Some(m) = self.sucs.get_mut(&key.v) {
            decrement_or_remove(m, &key.w);
        }
        if let Some(s) = self.in_edges.get_mut(&key.w) {
            s.shift_remove(&key);
        }
        if let Some(s) = self.out_edges.get_mut(&key.v) {
            s.shift_remove(&key);
        }
        Some(label)
    }

    /// `inEdges(v, u)`: edges into `v`, optionally only those from `u`.
    pub fn in_edges(&self, v: &str, u: Option<&str>) -> Vec<EdgeKey> {
        self.in_edges
            .get(v)
            .map(|s| {
                s.iter()
                    .filter(|e| u.is_none_or(|u| e.v == u))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `outEdges(v, w)`: edges out of `v`, optionally only those into `w`.
    pub fn out_edges(&self, v: &str, w: Option<&str>) -> Vec<EdgeKey> {
        self.out_edges
            .get(v)
            .map(|s| {
                s.iter()
                    .filter(|e| w.is_none_or(|w| e.w == w))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `nodeEdges(v, w)`: in-edges then out-edges.
    pub fn node_edges(&self, v: &str, w: Option<&str>) -> Vec<EdgeKey> {
        let mut edges = self.in_edges(v, w);
        edges.extend(self.out_edges(v, w));
        edges
    }

    /// Runs `alg/dfs.js` from each start node with one shared visited set.
    fn dfs(&self, starts: &[String], postorder: bool) -> Vec<String> {
        let mut visited: IndexSet<String> = IndexSet::new();
        let mut acc = Vec::new();
        for v in starts {
            if !self.has_node(v) {
                panic!("graphlib: dfs start {v:?} is not in the graph");
            }
            self.do_dfs(v, postorder, &mut visited, &mut acc);
        }
        acc
    }

    fn do_dfs(
        &self,
        v: &str,
        postorder: bool,
        visited: &mut IndexSet<String>,
        acc: &mut Vec<String>,
    ) {
        if visited.contains(v) {
            return;
        }
        visited.insert(v.to_string());
        if !postorder {
            acc.push(v.to_string());
        }
        let next = if self.directed {
            self.successors(v)
        } else {
            self.neighbors(v)
        };
        for w in next {
            self.do_dfs(&w, postorder, visited, acc);
        }
        if postorder {
            acc.push(v.to_string());
        }
    }

    /// `alg.preorder(g, vs)`.
    pub fn preorder(&self, starts: &[String]) -> Vec<String> {
        self.dfs(starts, false)
    }

    /// `alg.postorder(g, vs)`.
    pub fn postorder(&self, starts: &[String]) -> Vec<String> {
        self.dfs(starts, true)
    }
}

impl<N: Default, E, G> Graph<N, E, G> {
    /// `setNode(v)` with no value: creates the node with a default label and
    /// keeps an existing label.
    pub fn ensure_node(&mut self, v: &str) -> &mut Self {
        if !self.nodes.contains_key(v) {
            self.insert_node(v, N::default());
        }
        self
    }

    /// `setParent(v, parent)`: `None` moves `v` to the root. Both nodes are
    /// created when missing. A cycle panics where JS throws.
    pub fn set_parent(&mut self, v: &str, parent: Option<&str>) -> &mut Self {
        if !self.compound {
            panic!("graphlib: cannot set parent in a non-compound graph");
        }
        if let Some(parent) = parent {
            let mut ancestor = Some(parent.to_string());
            while let Some(a) = ancestor {
                if a == v {
                    panic!("graphlib: setting {parent:?} as parent of {v:?} would create a cycle");
                }
                ancestor = self.parent(&a);
            }
            self.ensure_node(parent);
        }
        self.ensure_node(v);
        self.set_parent_inner(v, parent);
        self
    }

    /// `setEdge(v, w, value, name)`: inserts or replaces the label and
    /// creates missing endpoints.
    pub fn set_edge(&mut self, key: &EdgeKey, label: E) -> &mut Self {
        let key = self.normalize_key(key);
        if let Some(slot) = self.edges.get_mut(&key) {
            *slot = label;
            return self;
        }
        self.insert_edge(key, label);
        self
    }

    fn insert_edge(&mut self, key: EdgeKey, label: E) {
        if key.name.is_some() && !self.multigraph {
            panic!("graphlib: cannot set a named edge when multigraph is false");
        }
        self.ensure_node(&key.v);
        self.ensure_node(&key.w);
        increment_or_init(self.preds.entry(key.w.clone()).or_default(), &key.v);
        increment_or_init(self.sucs.entry(key.v.clone()).or_default(), &key.w);
        self.in_edges
            .entry(key.w.clone())
            .or_default()
            .insert(key.clone());
        self.out_edges
            .entry(key.v.clone())
            .or_default()
            .insert(key.clone());
        self.edges.insert(key, label);
    }

    pub fn set_edge_vw(&mut self, v: &str, w: &str, label: E) -> &mut Self {
        self.set_edge(&EdgeKey::new(v, w), label)
    }
}

impl<N: Default, E: Default, G> Graph<N, E, G> {
    /// `setEdge(v, w)` with no value: creates the edge with a default label
    /// and keeps an existing label.
    pub fn ensure_edge(&mut self, key: &EdgeKey) -> &mut Self {
        let key = self.normalize_key(key);
        if !self.edges.contains_key(&key) {
            self.insert_edge(key, E::default());
        }
        self
    }
}

fn increment_or_init(map: &mut IndexMap<String, usize>, k: &str) {
    if let Some(n) = map.get_mut(k) {
        *n += 1;
    } else {
        map.insert(k.to_string(), 1);
    }
}

fn decrement_or_remove(map: &mut IndexMap<String, usize>, k: &str) {
    let remove = match map.get_mut(k) {
        Some(n) => {
            *n -= 1;
            *n == 0
        }
        None => false,
    };
    if remove {
        map.shift_remove(k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_index_keys_come_first_in_numeric_order() {
        let keys = ["b", "10", "a", "2", "-1", "01", "4294967295", "4294967294"];
        assert_eq!(
            js_key_order(keys),
            vec!["2", "10", "4294967294", "b", "a", "-1", "01", "4294967295"]
        );
    }

    #[test]
    fn nodes_follow_js_object_order() {
        let mut g: Graph<(), (), ()> = Graph::default();
        g.set_node("b", ());
        g.set_node("3", ());
        g.set_node("a", ());
        g.set_node("1", ());
        assert_eq!(g.nodes(), vec!["1", "3", "b", "a"]);
    }

    #[test]
    fn set_parent_moves_a_child_to_the_end() {
        let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
            compound: true,
            ..GraphOptions::default()
        });
        g.set_node("a", ());
        g.set_node("b", ());
        g.set_node("p", ());
        g.set_parent("a", Some("p"));
        g.set_parent("b", Some("p"));
        g.set_parent("a", Some("p"));
        assert_eq!(g.children(Some("p")), vec!["b", "a"]);
        assert_eq!(g.children(None), vec!["p"]);
        assert_eq!(g.parent("a"), Some("p".to_string()));
        g.remove_node("p");
        // `removeNode` re-parents the children in their own order.
        assert_eq!(g.children(None), vec!["b", "a"]);
        assert_eq!(g.parent("a"), None);
    }

    #[test]
    fn undirected_edges_normalize_their_endpoints() {
        let mut g: Graph<(), i32, ()> = Graph::new(GraphOptions {
            directed: false,
            ..GraphOptions::default()
        });
        g.set_edge_vw("b", "a", 1);
        assert!(g.has_edge_vw("a", "b"));
        assert_eq!(g.edges(), vec![EdgeKey::new("a", "b")]);
        assert_eq!(g.neighbors("b"), vec!["a"]);
        assert_eq!(g.neighbors("a"), vec!["b"]);
        assert_eq!(g.predecessors("b"), vec!["a"]);
        assert!(g.successors("b").is_empty());
    }

    #[test]
    fn multi_edges_and_counts() {
        let mut g: Graph<(), i32, ()> = Graph::new(GraphOptions {
            multigraph: true,
            ..GraphOptions::default()
        });
        g.set_edge(&EdgeKey::new("a", "b"), 1);
        g.set_edge(&EdgeKey::named("a", "b", Some("x")), 2);
        assert_eq!(g.edge_count(), 2);
        assert_eq!(g.successors("a"), vec!["b"]);
        assert_eq!(g.out_edges("a", None).len(), 2);
        g.remove_edge(&EdgeKey::new("a", "b"));
        assert_eq!(g.successors("a"), vec!["b"]);
        g.remove_edge(&EdgeKey::named("a", "b", Some("x")));
        assert!(g.successors("a").is_empty());
        assert_eq!(g.sources(), vec!["a", "b"]);
        assert_eq!(g.sinks(), vec!["a", "b"]);
    }

    #[test]
    fn remove_node_drops_its_edges() {
        let mut g: Graph<(), (), ()> = Graph::default();
        g.set_edge_vw("a", "b", ());
        g.set_edge_vw("b", "c", ());
        g.remove_node("b");
        assert_eq!(g.nodes(), vec!["a", "c"]);
        assert!(g.edges().is_empty());
        assert!(g.out_edges("a", None).is_empty());
        assert!(g.in_edges("c", None).is_empty());
    }

    #[test]
    fn pre_and_postorder_follow_successor_order() {
        let mut g: Graph<(), (), ()> = Graph::default();
        g.set_edge_vw("a", "b", ());
        g.set_edge_vw("a", "c", ());
        g.set_edge_vw("c", "d", ());
        assert_eq!(g.preorder(&["a".into()]), vec!["a", "b", "c", "d"]);
        assert_eq!(g.postorder(&["a".into()]), vec!["b", "d", "c", "a"]);
    }
}
