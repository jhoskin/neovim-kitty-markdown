//! Port of `rendering-util/layout-algorithms/dagre/index.js` and
//! `mermaid-graphlib.js`: the renderer flowchart, class, state, ER,
//! requirement, and usecase diagrams share.
//!
//! The pipeline per `render()`: build the compound graph from the layout
//! data (self loops become two `labelRect` dummies and three segments),
//! `adjustClustersAndEdges` (anchor edges that touch a cluster to a child,
//! extract clusters without external connections into sub-graphs), then
//! `recursiveRender`: draw the nodes to learn their sizes, lay the graph
//! out, place the nodes, draw the clusters, draw the edges and their
//! labels. A sub-graph renders the same way with `ranksep + 25` and
//! becomes a node of its parent's graph with the measured size.
//!
//! The layout engine enters through [`GraphLayout`]; [`FakeLayout`]
//! hand-places nodes for the tests until the dagre port is wired.

pub mod clusters;
pub mod graph;

use std::collections::HashMap;

use indexmap::IndexMap;

use crate::error::HelperError;
use crate::mermaid::edges::{
    Edge, EdgeCtx, EdgeLabels, EdgeOut, MarkerRef, insert_edge, insert_edge_label,
    position_edge_label,
};
use crate::mermaid::scene::markers::{MarkerType, insert_markers};
use crate::mermaid::scene::path::Command;
use crate::mermaid::scene::{Common, Element, Group, Marker, Paint, Point, Style, TransformOp};
use crate::mermaid::shapes::util::{BBox, path_bbox, points_bbox, translate};
use crate::mermaid::shapes::{Bounds, Dir, Node, ShapeCtx, insert_node};
pub use graph::{EdgeKey, GraphLabel, SelfLoop, UEdge, UGraph, UNode};

/// `LayoutData` of `rendering-util/types.ts`, the part the dagre renderer
/// reads.
#[derive(Debug, Clone, Default)]
pub struct LayoutData {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub direction: Dir,
    /// `data4Layout.nodeSpacing` / `rankSpacing`, the diagram's own values.
    pub node_spacing: Option<f64>,
    pub rank_spacing: Option<f64>,
    /// `data4Layout.type`: `flowchart-v2`, `stateDiagram`, `classDiagram`,
    /// `er`, `requirement`, `usecase`.
    pub diagram_type: String,
    pub diagram_id: String,
    /// `data4Layout.markers`.
    pub markers: Vec<MarkerType>,
}

/// The config the glue reads besides the shapes' context.
#[derive(Debug, Clone, Default)]
pub struct UnifiedConfig {
    /// `config.nodeSpacing` / `config.rankSpacing`: top-level overrides.
    pub node_spacing: Option<f64>,
    pub rank_spacing: Option<f64>,
    /// `flowchart.nodeSpacing` / `flowchart.rankSpacing` (default 50).
    pub flowchart_node_spacing: Option<f64>,
    pub flowchart_rank_spacing: Option<f64>,
    /// `flowchart.curve`.
    pub curve: Option<String>,
    /// `flowchart.subGraphTitleMargin.top` / `.bottom`.
    pub sub_graph_title_margin_top: f64,
    pub sub_graph_title_margin_bottom: f64,
}

/// The layout engine seam. The real implementation wraps
/// `crate::mermaid::layout::dagre::layout`; it reads
/// [`UGraph::label`], each node's `width`/`height`, the parents, and each
/// edge's `minlen`/`width`/`height`/`labelpos`, and writes node `x`/`y`
/// (plus `width`/`height` of compound nodes) and edge `points`/`x`/`y`.
pub trait GraphLayout {
    fn layout(&self, graph: &mut UGraph) -> Result<(), HelperError>;
}

/// Hand-placed layout for the tests: node centres from a table, compound
/// nodes sized around their children with a margin, edges routed straight
/// with a midpoint so `insertEdge` has inner points to clip against.
#[derive(Debug, Clone, Default)]
pub struct FakeLayout {
    pub positions: HashMap<String, (f64, f64)>,
    pub cluster_margin: f64,
}

impl FakeLayout {
    pub fn new(positions: &[(&str, f64, f64)]) -> FakeLayout {
        FakeLayout {
            positions: positions
                .iter()
                .map(|(id, x, y)| (id.to_string(), (*x, *y)))
                .collect(),
            cluster_margin: 20.0,
        }
    }
}

impl GraphLayout for FakeLayout {
    fn layout(&self, graph: &mut UGraph) -> Result<(), HelperError> {
        for id in graph.nodes() {
            if !graph.children(Some(&id)).is_empty() {
                continue;
            }
            let (x, y) = self.positions.get(&id).copied().unwrap_or((0.0, 0.0));
            if let Some(n) = graph.node_mut(&id) {
                n.node.x = x;
                n.node.y = y;
            }
        }
        // Compound nodes: deepest first so a parent sees sized children.
        let order = sort_nodes_by_hierarchy(graph);
        for id in order.iter().rev() {
            let children = graph.children(Some(id));
            if children.is_empty() {
                continue;
            }
            let mut bbox = BBox::EMPTY;
            for c in &children {
                if let Some(n) = graph.node(c) {
                    bbox = bbox.union(BBox {
                        x: n.node.x - n.node.width() / 2.0,
                        y: n.node.y - n.node.height() / 2.0,
                        width: n.node.width(),
                        height: n.node.height(),
                    });
                }
            }
            let m = self.cluster_margin;
            if let Some(n) = graph.node_mut(id) {
                n.node.x = bbox.x + bbox.width / 2.0;
                n.node.y = bbox.y + bbox.height / 2.0;
                n.node.width = Some(bbox.width + 2.0 * m);
                n.node.height = Some(bbox.height + 2.0 * m);
            }
        }
        for key in graph.edges() {
            let (Some(v), Some(w)) = (graph.node(&key.v), graph.node(&key.w)) else {
                continue;
            };
            let a = Point::new(v.node.x, v.node.y);
            let b = Point::new(w.node.x, w.node.y);
            let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            if let Some(e) = graph.edge_mut(&key) {
                e.edge.points = vec![a, mid, b];
                e.edge.x = Some(mid.x);
                e.edge.y = Some(mid.y);
            }
        }
        Ok(())
    }
}

/// What `render` produces: the `g.root`, the marker definitions for
/// `defs`, and the content bounds the diagram package turns into the
/// viewBox.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub root: Group,
    pub markers: Vec<Marker>,
    pub bounds: BBox,
}

// ---------------------------------------------------------------------------
// mermaid-graphlib.js

/// `clusterDb` entry.
#[derive(Debug, Clone, Default)]
struct ClusterEntry {
    /// The anchor node id edges to the cluster are redirected to.
    id: Option<String>,
    cluster_data: Option<Node>,
    external_connections: bool,
    /// The drawn cluster's bounds, for `cutPathAtIntersect`.
    node: Option<Bounds>,
}

#[derive(Debug, Clone, Default)]
struct ClusterDb {
    entries: IndexMap<String, ClusterEntry>,
    descendants: HashMap<String, Vec<String>>,
}

impl ClusterDb {
    fn is_descendant(&self, id: &str, ancestor: &str) -> bool {
        self.descendants
            .get(ancestor)
            .is_some_and(|d| d.iter().any(|x| x == id))
    }

    fn edge_in_cluster(&self, edge: &EdgeKey, cluster_id: &str) -> bool {
        if edge.v == cluster_id || edge.w == cluster_id {
            return false;
        }
        let Some(desc) = self.descendants.get(cluster_id) else {
            return false;
        };
        desc.contains(&edge.v)
            || self.is_descendant(&edge.v, cluster_id)
            || self.is_descendant(&edge.w, cluster_id)
            || desc.contains(&edge.w)
    }

    fn bounds(&self) -> HashMap<String, Bounds> {
        self.entries
            .iter()
            .filter_map(|(k, e)| e.node.map(|b| (k.clone(), b)))
            .collect()
    }
}

/// `extractDescendants(id, graph)`.
fn extract_descendants(id: &str, graph: &UGraph) -> Vec<String> {
    let children = graph.children(Some(id));
    let mut res = children.clone();
    for child in &children {
        res.extend(extract_descendants(child, graph));
    }
    res
}

/// `validate(graph)`: no edge may end at a node with children.
pub fn validate(graph: &UGraph) -> bool {
    graph
        .edges()
        .iter()
        .all(|e| graph.children(Some(&e.v)).is_empty() && graph.children(Some(&e.w)).is_empty())
}

/// `findCommonEdges(graph, id1, id2)`, with its `w` mapping kept as is.
fn find_common_edges(graph: &UGraph, id1: Option<&str>, id2: &str) -> Vec<(String, String)> {
    let edges = graph.edges();
    let Some(id1) = id1 else {
        return Vec::new();
    };
    let edges1: Vec<&EdgeKey> = edges.iter().filter(|e| e.v == id1 || e.w == id1).collect();
    let edges2: Vec<&EdgeKey> = edges.iter().filter(|e| e.v == id2 || e.w == id2).collect();
    let edges1_prim: Vec<(String, String)> = edges1
        .iter()
        .map(|e| {
            (
                if e.v == id1 {
                    id2.to_string()
                } else {
                    e.v.clone()
                },
                if e.w == id1 {
                    id1.to_string()
                } else {
                    e.w.clone()
                },
            )
        })
        .collect();
    edges1_prim
        .into_iter()
        .filter(|(v, w)| edges2.iter().any(|e| e.v == *v && e.w == *w))
        .collect()
}

/// `findNonClusterChild(id, graph, clusterId)`.
fn find_non_cluster_child(id: &str, graph: &UGraph, cluster_id: Option<&str>) -> Option<String> {
    let children = graph.children(Some(id));
    if children.is_empty() {
        return Some(id.to_string());
    }
    let mut reserve = None;
    for child in &children {
        let found = find_non_cluster_child(child, graph, cluster_id);
        if let Some(found) = found {
            let common = find_common_edges(graph, cluster_id, &found);
            if !common.is_empty() {
                reserve = Some(found);
            } else {
                return Some(found);
            }
        }
    }
    reserve
}

fn is_node_in_extractable_cluster(db: &ClusterDb, graph: &UGraph, node: &str, root: &str) -> bool {
    let mut parent = graph.parent(node);
    while let Some(p) = parent {
        if p == root {
            break;
        }
        if let Some(c) = db.entries.get(p)
            && !c.external_connections
        {
            return true;
        }
        parent = graph.parent(p);
    }
    false
}

fn find_safe_anchor_node(
    db: &ClusterDb,
    graph: &UGraph,
    cluster_id: &str,
    excluded: Option<&str>,
) -> Option<String> {
    for child in graph.children(Some(cluster_id)) {
        if Some(child.as_str()) == excluded
            || excluded.is_some_and(|ex| db.is_descendant(&child, ex))
        {
            continue;
        }
        let Some(candidate) = find_non_cluster_child(&child, graph, Some(cluster_id)) else {
            continue;
        };
        if !is_node_in_extractable_cluster(db, graph, &candidate, cluster_id) {
            return Some(candidate);
        }
    }
    None
}

/// `adjustClustersAndEdges(graph)`.
fn adjust_clusters_and_edges(db: &mut ClusterDb, graph: &mut UGraph) {
    for id in graph.nodes() {
        if !graph.children(Some(&id)).is_empty() {
            db.descendants
                .insert(id.clone(), extract_descendants(&id, graph));
            let anchor = find_non_cluster_child(&id, graph, Some(&id));
            let cluster_data = graph.node(&id).map(|n| n.node.clone());
            db.entries.insert(
                id.clone(),
                ClusterEntry {
                    id: anchor,
                    cluster_data,
                    external_connections: false,
                    node: None,
                },
            );
        }
    }
    let edges = graph.edges();
    for id in graph.nodes() {
        if graph.children(Some(&id)).is_empty() {
            continue;
        }
        for edge in &edges {
            let d1 = db.is_descendant(&edge.v, &id);
            let d2 = db.is_descendant(&edge.w, &id);
            if d1 ^ d2
                && let Some(entry) = db.entries.get_mut(&id)
            {
                entry.external_connections = true;
            }
        }
    }
    let keys: Vec<String> = db.entries.keys().cloned().collect();
    for id in keys {
        let non_cluster_child = db.entries[&id].id.clone();
        let parent = non_cluster_child
            .as_deref()
            .and_then(|c| graph.parent(c))
            .map(str::to_string);
        if let Some(p) = parent.as_deref()
            && p != id
            && db.entries.get(p).is_some_and(|e| !e.external_connections)
        {
            db.entries[&id].id = Some(p.to_string());
        }
        let has_direct_outgoing_edge = edges.iter().any(|e| e.v == id);
        if let Some(child) = non_cluster_child.as_deref()
            && db.entries[&id].external_connections
            && has_direct_outgoing_edge
            && is_node_in_extractable_cluster(db, graph, child, &id)
        {
            let excluded = graph.parent(child).map(str::to_string);
            if let Some(safe) = find_safe_anchor_node(db, graph, &id, excluded.as_deref()) {
                db.entries[&id].id = Some(safe);
            }
        }
    }
    for e in graph.edges() {
        if !(db.entries.contains_key(&e.v) || db.entries.contains_key(&e.w)) {
            continue;
        }
        let anchor = |db: &ClusterDb, id: &str| -> String {
            match db.entries.get(id) {
                Some(entry) if entry.external_connections => {
                    entry.id.clone().unwrap_or_else(|| id.to_string())
                }
                _ => id.to_string(),
            }
        };
        let v = anchor(db, &e.v);
        let w = anchor(db, &e.w);
        let Some(mut uedge) = graph.remove_edge(&e) else {
            continue;
        };
        if v != e.v {
            if let Some(p) = graph.parent(&v).map(str::to_string)
                && let Some(entry) = db.entries.get_mut(&p)
            {
                entry.external_connections = true;
            }
            uedge.edge.from_cluster = Some(e.v.clone());
        }
        if w != e.w {
            if let Some(p) = graph.parent(&w).map(str::to_string)
                && let Some(entry) = db.entries.get_mut(&p)
            {
                entry.external_connections = true;
            }
            uedge.edge.to_cluster = Some(e.w.clone());
        }
        graph.set_edge(&v, &w, uedge, e.name.as_deref());
    }
    extractor(db, graph, 0);
}

/// `copy(clusterId, graph, newGraph, rootId)`: moves a cluster's contents
/// into its own graph.
fn copy(
    db: &ClusterDb,
    cluster_id: &str,
    graph: &mut UGraph,
    new_graph: &mut UGraph,
    root_id: &str,
) {
    let mut nodes = graph.children(Some(cluster_id));
    if cluster_id != root_id {
        nodes.push(cluster_id.to_string());
    }
    for node in nodes {
        if !graph.children(Some(&node)).is_empty() {
            copy(db, &node, graph, new_graph, root_id);
        } else {
            let Some(data) = graph.node(&node).cloned() else {
                continue;
            };
            new_graph.set_node(&node, data);
            let parent = graph.parent(&node).map(str::to_string);
            if parent.as_deref() != Some(root_id) {
                new_graph.set_parent(&node, parent.as_deref());
            }
            if cluster_id != root_id && node != cluster_id {
                new_graph.set_parent(&node, Some(cluster_id));
            }
            for edge in graph.node_edges(&node) {
                let Some(data) = graph.edge(&edge).cloned() else {
                    continue;
                };
                if db.edge_in_cluster(&edge, root_id) {
                    new_graph.set_edge(&edge.v, &edge.w, data, edge.name.as_deref());
                }
            }
        }
        graph.remove_node(&node);
    }
}

/// `extractor(graph, depth)`.
fn extractor(db: &ClusterDb, graph: &mut UGraph, depth: u32) {
    if depth > 10 {
        return;
    }
    let nodes = graph.nodes();
    let has_children = nodes.iter().any(|n| !graph.children(Some(n)).is_empty());
    if !has_children {
        return;
    }
    for node in &nodes {
        let Some(entry) = db.entries.get(node) else {
            continue;
        };
        if entry.external_connections || graph.children(Some(node)).is_empty() {
            continue;
        }
        let mut dir = if graph.label.rankdir == "TB" {
            "LR"
        } else {
            "TB"
        }
        .to_string();
        if let Some(d) = entry.cluster_data.as_ref().and_then(|c| c.dir) {
            dir = d.as_str().to_string();
        }
        let mut cluster_graph = UGraph::new(GraphLabel {
            rankdir: dir,
            nodesep: 50.0,
            ranksep: 50.0,
            marginx: 8.0,
            marginy: 8.0,
        });
        copy(db, node, graph, &mut cluster_graph, node);
        let placeholder = Node {
            id: node.clone(),
            ..Node::default()
        };
        graph.set_node(
            node,
            UNode {
                node: placeholder,
                cluster_node: true,
                cluster_data: entry.cluster_data.clone(),
                graph: Some(Box::new(cluster_graph)),
                diff: 0.0,
                label_height: 0.0,
            },
        );
    }
    for node in graph.nodes() {
        if let Some(n) = graph.node_mut(&node)
            && n.cluster_node
            && let Some(sub) = n.graph.as_mut()
        {
            extractor(db, sub, depth + 1);
        }
    }
}

fn sorter(graph: &UGraph, nodes: &[String]) -> Vec<String> {
    let mut result = nodes.to_vec();
    for node in nodes {
        let children = graph.children(Some(node));
        result.extend(sorter(graph, &children));
    }
    result
}

/// `sortNodesByHierarchy(graph)`: roots first, then each root's subtree.
pub fn sort_nodes_by_hierarchy(graph: &UGraph) -> Vec<String> {
    sorter(graph, &graph.children(None))
}

// ---------------------------------------------------------------------------
// index.js

/// `shouldMergeSelfLoopSegments(diagramType)`.
fn should_merge_self_loops(diagram_type: &str) -> bool {
    matches!(
        diagram_type,
        "flowchart" | "flowchart-v2" | "stateDiagram" | "er" | "classDiagram"
    )
}

/// `prepareLayoutForDagre(data4Layout)`: the graph with self-loop dummies.
fn prepare_layout(data: &LayoutData, config: &UnifiedConfig) -> UGraph {
    let nodesep = config
        .node_spacing
        .or(data.node_spacing)
        .or(config.flowchart_node_spacing)
        .unwrap_or(50.0);
    let ranksep = config
        .rank_spacing
        .or(data.rank_spacing)
        .or(config.flowchart_rank_spacing)
        .unwrap_or(50.0);
    let mut graph = UGraph::new(GraphLabel {
        rankdir: data.direction.as_str().to_string(),
        nodesep,
        ranksep,
        marginx: 8.0,
        marginy: 8.0,
    });
    for node in &data.nodes {
        graph.set_node(&node.id, UNode::new(node.clone()));
        if let Some(parent) = node.parent_id.as_deref() {
            graph.set_parent(&node.id, Some(parent));
        }
    }
    for edge in &data.edges {
        if edge.start == edge.end {
            let node_id = edge.start.clone();
            let special1 = format!("{node_id}---{node_id}---1");
            let special2 = format!("{node_id}---{node_id}---2");
            let (parent_id, is_group) = graph
                .node(&node_id)
                .map(|n| (n.node.parent_id.clone(), n.node.is_group))
                .unwrap_or((None, false));
            for special in [&special1, &special2] {
                let dummy = Node {
                    id: special.clone(),
                    dom_id: Some(special.clone()),
                    parent_id: parent_id.clone(),
                    label_style: Some(String::new()),
                    label: Some(String::new()),
                    padding: Some(0.0),
                    shape: "labelRect".to_string(),
                    width: Some(10.0),
                    height: Some(10.0),
                    ..Node::default()
                };
                graph.set_node(special, UNode::new(dummy));
                graph.set_parent(special, parent_id.as_deref());
            }
            let original = edge.clone();
            let mut edge1 = edge.clone();
            let mut edge_mid = edge.clone();
            let mut edge2 = edge.clone();
            edge1.label = Some(String::new());
            edge1.arrow_type_end = Some("none".to_string());
            edge1.end_label_left = Some(String::new());
            edge1.end_label_right = Some(String::new());
            edge1.start_label_left = Some(String::new());
            edge1.id = format!("{node_id}-cyclic-special-1");
            edge_mid.start_label_right = Some(String::new());
            edge_mid.start_label_left = Some(String::new());
            edge_mid.end_label_left = Some(String::new());
            edge_mid.end_label_right = Some(String::new());
            edge_mid.arrow_type_start = Some("none".to_string());
            edge_mid.arrow_type_end = Some("none".to_string());
            edge_mid.id = format!("{node_id}-cyclic-special-mid");
            edge2.label = Some(String::new());
            edge2.start_label_right = Some(String::new());
            edge2.start_label_left = Some(String::new());
            edge2.arrow_type_start = Some("none".to_string());
            if is_group {
                edge1.from_cluster = Some(node_id.clone());
                edge2.to_cluster = Some(node_id.clone());
            }
            edge2.id = format!("{node_id}-cyclic-special-2");
            let wrap = |e: Edge, order: u8| UEdge {
                edge: e,
                self_loop: Some(SelfLoop {
                    id: original.id.clone(),
                    order,
                }),
                original_edge: Some(Box::new(original.clone())),
            };
            graph.set_edge(
                &node_id,
                &special1,
                wrap(edge1, 0),
                Some(&format!("{node_id}-cyclic-special-0")),
            );
            graph.set_edge(
                &special1,
                &special2,
                wrap(edge_mid, 1),
                Some(&format!("{node_id}-cyclic-special-1")),
            );
            graph.set_edge(
                &special2,
                &node_id,
                wrap(edge2, 2),
                Some(&format!("{node_id}-cyclic-special-2")),
            );
        } else {
            graph.set_edge(
                &edge.start,
                &edge.end,
                UEdge::new(edge.clone()),
                Some(&edge.id),
            );
        }
    }
    graph
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    max.min(value).max(min)
}

fn default_self_loop_side(rankdir: &str) -> &'static str {
    match rankdir {
        "BT" => "bottom",
        "LR" => "right",
        "RL" => "left",
        _ => "top",
    }
}

/// `getSelfLoopSide(graph, node, segments, originalNodeId, rankdir)`.
fn self_loop_side(
    graph: &UGraph,
    node: &Node,
    segments: &[(EdgeKey, Edge)],
    original_node_id: &str,
    rankdir: &str,
) -> &'static str {
    let mut hints: Vec<Point> = Vec::new();
    let mut dummy_ids: Vec<String> = Vec::new();
    for (key, _) in segments {
        if key.v != original_node_id && !dummy_ids.contains(&key.v) {
            dummy_ids.push(key.v.clone());
        }
        if key.w != original_node_id && !dummy_ids.contains(&key.w) {
            dummy_ids.push(key.w.clone());
        }
    }
    for id in &dummy_ids {
        if let Some(n) = graph.node(id) {
            hints.push(Point::new(n.node.x, n.node.y));
        }
    }
    if hints.is_empty() {
        for (_, edge) in segments {
            hints.extend(edge.points.iter().copied());
        }
    }
    if hints.is_empty() {
        return default_self_loop_side(rankdir);
    }
    let n = hints.len() as f64;
    let center = hints.iter().fold(Point::default(), |acc, p| {
        Point::new(acc.x + p.x / n, acc.y + p.y / n)
    });
    let dx = center.x - node.x;
    let dy = center.y - node.y;
    if dx.abs() > dy.abs() {
        return if dx > 0.0 { "right" } else { "left" };
    }
    if dy.abs() > 0.0 {
        return if dy > 0.0 { "bottom" } else { "top" };
    }
    default_self_loop_side(rankdir)
}

/// `getSelfLoopPoints(node, side, yOffset, labelWidth)`.
fn self_loop_points(node: &Node, side: &str, y_offset: f64, label_width: f64) -> Vec<Point> {
    let x = node.x;
    let y = node.y - y_offset;
    let half_width = node.width() / 2.0;
    let half_height = node.height() / 2.0;
    let max_span = 36.0_f64.max(100.0_f64.min(node.width() * 0.8));
    let span = clamp(label_width.max(node.width() * 0.35), 36.0, max_span);
    let depth = clamp(node.width().min(node.height()) * 0.45, 24.0, 48.0);
    match side {
        "bottom" => {
            let bottom = y + half_height;
            vec![
                Point::new(x - span / 2.0, bottom),
                Point::new(x - span / 2.0, bottom + depth),
                Point::new(x + span / 2.0, bottom + depth),
                Point::new(x + span / 2.0, bottom),
            ]
        }
        "right" => {
            let right = x + half_width;
            vec![
                Point::new(right, y - span / 2.0),
                Point::new(right + depth, y - span / 2.0),
                Point::new(right + depth, y + span / 2.0),
                Point::new(right, y + span / 2.0),
            ]
        }
        "left" => {
            let left = x - half_width;
            vec![
                Point::new(left, y - span / 2.0),
                Point::new(left - depth, y - span / 2.0),
                Point::new(left - depth, y + span / 2.0),
                Point::new(left, y + span / 2.0),
            ]
        }
        _ => {
            let top = y - half_height;
            vec![
                Point::new(x - span / 2.0, top),
                Point::new(x - span / 2.0, top - depth),
                Point::new(x + span / 2.0, top - depth),
                Point::new(x + span / 2.0, top),
            ]
        }
    }
}

/// `getSelfLoopLabelPosition(node, points, side, yOffset, label)`.
fn self_loop_label_position(
    node: &Node,
    points: &[Point],
    side: &str,
    y_offset: f64,
    label_width: f64,
    label_height: f64,
) -> Point {
    let gap = 4.0;
    let x = node.x;
    let y = node.y - y_offset;
    let max_y = points.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    let min_y = points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let max_x = points.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
    let min_x = points.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    match side {
        "bottom" => Point::new(x, max_y + label_height / 2.0 + gap),
        "right" => Point::new(max_x + label_width / 2.0 + gap, y),
        "left" => Point::new(min_x - label_width / 2.0 - gap, y),
        _ => Point::new(x, min_y - label_height / 2.0 - gap),
    }
}

/// `getEdgesToRender(graph, yOffset, { mergeSelfLoops })`: the edges to
/// draw, with the three self-loop segments merged into one loop.
fn edges_to_render(
    graph: &UGraph,
    y_offset: f64,
    merge_self_loops: bool,
) -> Vec<(Edge, String, String)> {
    let mut groups: IndexMap<String, Vec<(EdgeKey, UEdge)>> = IndexMap::new();
    let mut out: Vec<(Edge, String, String)> = Vec::new();
    for key in graph.edges() {
        let Some(uedge) = graph.edge(&key) else {
            continue;
        };
        if merge_self_loops && let Some(sl) = &uedge.self_loop {
            groups
                .entry(sl.id.clone())
                .or_default()
                .push((key.clone(), uedge.clone()));
        } else {
            out.push((uedge.edge.clone(), key.v.clone(), key.w.clone()));
        }
    }
    for (_, mut segments) in groups {
        if segments.len() != 3 {
            for (key, uedge) in segments {
                out.push((uedge.edge, key.v, key.w));
            }
            continue;
        }
        segments.sort_by_key(|(_, e)| e.self_loop.as_ref().map_or(0, |s| s.order));
        let original = segments
            .iter()
            .find_map(|(_, e)| e.original_edge.as_deref().cloned())
            .unwrap_or_else(|| segments[1].1.edge.clone());
        let Some(node) = graph.node(&original.start) else {
            for (key, uedge) in segments {
                out.push((uedge.edge, key.v, key.w));
            }
            continue;
        };
        let middle = &segments[1].1.edge;
        let label_width = middle.width;
        let label_height = middle.height;
        let seg_edges: Vec<(EdgeKey, Edge)> = segments
            .iter()
            .map(|(k, e)| (k.clone(), e.edge.clone()))
            .collect();
        let side = self_loop_side(
            graph,
            &node.node,
            &seg_edges,
            &original.start,
            &graph.label.rankdir,
        );
        let points = self_loop_points(&node.node, side, y_offset, label_width);
        let label_pos = self_loop_label_position(
            &node.node,
            &points,
            side,
            y_offset,
            label_width,
            label_height,
        );
        let mut merged = original.clone();
        merged.points = points;
        merged.x = Some(label_pos.x);
        merged.y = Some(label_pos.y);
        merged.width = label_width;
        merged.height = label_height;
        merged.label_style = middle.label_style.clone();
        merged.from_cluster = segments
            .iter()
            .find_map(|(_, e)| e.edge.from_cluster.clone());
        merged.to_cluster = segments.iter().find_map(|(_, e)| e.edge.to_cluster.clone());
        out.push((merged, original.start.clone(), original.end.clone()));
    }
    out
}

/// A rendered sub-graph: `g.root` and the `diff` of its last group.
struct SubRender {
    elem: Group,
    diff: f64,
}

struct RenderCtx<'a> {
    shapes: &'a ShapeCtx<'a>,
    config: &'a UnifiedConfig,
    layout: &'a dyn GraphLayout,
    diagram_type: &'a str,
    diagram_id: &'a str,
    db: ClusterDb,
    /// Colored marker clones the edges asked for.
    marker_refs: Vec<MarkerRef>,
}

/// `positionNode(node)`: the node group's transform.
fn position_node(group: &mut Group, node: &Node, cluster_node: bool, diff: f64) {
    group.common.transform = if cluster_node {
        translate(
            node.x + diff - node.width() / 2.0,
            node.y - node.height() / 2.0 - 8.0,
        )
    } else {
        translate(node.x, node.y)
    };
}

/// `recursiveRender(_elem, graph, diagramType, id, parentCluster,
/// siteConfig)`.
fn recursive_render(
    rc: &mut RenderCtx,
    graph: &mut UGraph,
    parent_cluster: Option<&UNode>,
) -> Result<SubRender, HelperError> {
    let merge_self_loops = should_merge_self_loops(rc.diagram_type);
    let total_margin =
        rc.config.sub_graph_title_margin_top + rc.config.sub_graph_title_margin_bottom;
    let dir = Dir::parse(&graph.label.rankdir);

    // Measure: draw the nodes, recurse into extracted clusters.
    let mut node_groups: IndexMap<String, Group> = IndexMap::new();
    for v in graph.nodes() {
        if let Some(parent) = parent_cluster {
            let data = parent.cluster_data.clone().unwrap_or_default();
            let parent_id = data.id.clone();
            graph.set_node(&parent_id, UNode::new(data));
            if graph.parent(&v).is_none() && v != parent_id {
                graph.set_parent(&v, Some(&parent_id));
            }
        }
        let Some(unode) = graph.node(&v).cloned() else {
            continue;
        };
        if unode.cluster_node {
            let Some(mut sub) = unode.graph.clone() else {
                continue;
            };
            sub.label.ranksep = graph.label.ranksep + 25.0;
            sub.label.nodesep = graph.label.nodesep;
            let o = recursive_render(rc, &mut sub, Some(&unode))?;
            let bbox = group_bbox(&o.elem);
            if let Some(n) = graph.node_mut(&v) {
                n.node.width = Some(bbox.width);
                n.node.height = Some(bbox.height);
                n.diff = o.diff;
                n.graph = Some(sub);
            }
            node_groups.insert(v.clone(), o.elem);
        } else if !graph.children(Some(&v)).is_empty() {
            let anchor = find_non_cluster_child(&unode.node.id, graph, None);
            rc.db.entries.entry(unode.node.id.clone()).or_default().id = anchor;
        } else {
            let mut node = unode.node.clone();
            let mut ctx = ShapeCtx {
                measurer: rc.shapes.measurer,
                vars: rc.shapes.vars.clone(),
                dir: dir.or(rc.shapes.dir),
                html_labels: rc.shapes.html_labels,
                wrapping_width: rc.shapes.wrapping_width,
                class_padding: rc.shapes.class_padding,
                hide_empty_members_box: rc.shapes.hide_empty_members_box,
                er_diagram_padding: rc.shapes.er_diagram_padding,
                er_entity_padding: rc.shapes.er_entity_padding,
                er_min_entity_width: rc.shapes.er_min_entity_width,
                state_padding: rc.shapes.state_padding,
                kanban_ticket_base_url: rc.shapes.kanban_ticket_base_url.clone(),
                elk: rc.shapes.elk,
            };
            ctx.dir = dir.or(rc.shapes.dir);
            let out = insert_node(&mut node, &ctx)?;
            // `insertMeasuredNode`: the group's bbox is the node size.
            let bbox = group_bbox(&out.group);
            node.width = Some(bbox.width);
            node.height = Some(bbox.height);
            if let Some(n) = graph.node_mut(&v) {
                n.node = node;
            }
            node_groups.insert(v.clone(), out.group);
        }
    }

    // Edge labels.
    let mut edge_labels: IndexMap<String, EdgeLabels> = IndexMap::new();
    for key in graph.edges() {
        let Some(uedge) = graph.edge(&key).cloned() else {
            continue;
        };
        if merge_self_loops && let Some(sl) = &uedge.self_loop {
            if sl.order != 1 {
                continue;
            }
            let mut label_edge = uedge.original_edge.as_deref().cloned().unwrap_or_default();
            let overlay = uedge.edge.clone();
            label_edge.id = sl.id.clone();
            label_edge.label = overlay.label.clone();
            label_edge.label_type = overlay.label_type.clone();
            label_edge.label_style = overlay.label_style.clone();
            let orig = uedge.original_edge.as_deref();
            label_edge.start_label_left = orig
                .and_then(|o| o.start_label_left.clone())
                .or(overlay.start_label_left);
            label_edge.start_label_right = orig
                .and_then(|o| o.start_label_right.clone())
                .or(overlay.start_label_right);
            label_edge.end_label_left = orig
                .and_then(|o| o.end_label_left.clone())
                .or(overlay.end_label_left);
            label_edge.end_label_right = orig
                .and_then(|o| o.end_label_right.clone())
                .or(overlay.end_label_right);
            let labels = insert_edge_label(&mut label_edge, rc.shapes);
            if let Some(e) = graph.edge_mut(&key) {
                e.edge.width = label_edge.width;
                e.edge.height = label_edge.height;
                e.edge.label_style = label_edge.label_style.clone();
            }
            edge_labels.insert(sl.id.clone(), labels);
            continue;
        }
        let mut edge = uedge.edge.clone();
        let labels = insert_edge_label(&mut edge, rc.shapes);
        if let Some(e) = graph.edge_mut(&key) {
            e.edge.width = edge.width;
            e.edge.height = edge.height;
        }
        edge_labels.insert(edge.id.clone(), labels);
    }

    rc.layout.layout(graph)?;

    // Paint: position nodes, draw clusters.
    let mut cluster_groups: Vec<Element> = Vec::new();
    let mut diff = 0.0;
    for v in sort_nodes_by_hierarchy(graph) {
        let Some(unode) = graph.node(&v).cloned() else {
            continue;
        };
        if unode.cluster_node {
            let mut node = unode.node.clone();
            node.y += total_margin;
            rc.db.entries.entry(v.clone()).or_default().node = Some(node.bounds());
            if let Some(g) = node_groups.get_mut(&v) {
                position_node(g, &node, true, unode.diff);
            }
            if let Some(n) = graph.node_mut(&v) {
                n.node = node;
            }
        } else if !graph.children(Some(&v)).is_empty() {
            let mut node = unode.node.clone();
            node.height = Some(node.height() + total_margin);
            let (out, d) = clusters::insert_cluster(
                &mut node,
                rc.shapes,
                rc.config.sub_graph_title_margin_top,
            );
            cluster_groups.push(Element::Group(out.group));
            let entry = rc.db.entries.entry(v.clone()).or_default();
            entry.node = Some(node.bounds());
            if let Some(n) = graph.node_mut(&v) {
                n.node = node;
                n.diff = d;
                n.label_height = out.label_bbox.height;
            }
        } else {
            let mut node = unode.node.clone();
            node.y += total_margin / 2.0;
            if let Some(g) = node_groups.get_mut(&v) {
                position_node(g, &node, false, 0.0);
            }
            if let Some(n) = graph.node_mut(&v) {
                n.node = node;
            }
        }
    }

    // Edges.
    let edge_offset_y = total_margin / 2.0;
    let edge_ctx = EdgeCtx {
        shapes: rc.shapes,
        curve: rc.config.curve.clone(),
        sub_graph_title_total_margin: total_margin,
    };
    let cluster_bounds = rc.db.bounds();
    let mut edge_paths: Vec<Element> = Vec::new();
    for (mut edge, start, end) in edges_to_render(graph, edge_offset_y, merge_self_loops) {
        for p in &mut edge.points {
            p.y += edge_offset_y;
        }
        let (Some(start_node), Some(end_node)) = (graph.node(&start), graph.node(&end)) else {
            continue;
        };
        let EdgeOut {
            path,
            paths,
            markers,
        } = insert_edge(
            &edge,
            &cluster_bounds,
            rc.diagram_type,
            &start_node.node,
            &end_node.node,
            rc.diagram_id,
            &edge_ctx,
        );
        rc.marker_refs.extend(markers);
        edge_paths.push(path);
        if let Some(labels) = edge_labels.get_mut(&edge.id) {
            position_edge_label(labels, &edge, &paths, total_margin);
        }
    }

    for v in graph.nodes() {
        if let Some(n) = graph.node(&v)
            && n.node.is_group
        {
            diff = n.diff;
        }
    }

    // Assemble `g.root` in `createLayoutElementGroups` order.
    let mut edge_label_children: Vec<Element> = Vec::new();
    for (_, labels) in edge_labels {
        if let Some(g) = labels.label {
            edge_label_children.push(Element::Group(g));
        }
        for g in [
            labels.start_left,
            labels.start_right,
            labels.end_left,
            labels.end_right,
        ]
        .into_iter()
        .flatten()
        {
            edge_label_children.push(Element::Group(g));
        }
    }
    let root = Group {
        common: Common {
            class: Some("root".to_string()),
            ..Common::default()
        },
        children: vec![
            Element::Group(Group {
                common: Common {
                    class: Some("clusters".to_string()),
                    ..Common::default()
                },
                children: cluster_groups,
            }),
            Element::Group(Group {
                common: Common {
                    class: Some("edgePaths".to_string()),
                    ..Common::default()
                },
                children: edge_paths,
            }),
            Element::Group(Group {
                common: Common {
                    class: Some("edgeLabels".to_string()),
                    ..Common::default()
                },
                children: edge_label_children,
            }),
            Element::Group(Group {
                common: Common {
                    class: Some("nodes".to_string()),
                    ..Common::default()
                },
                children: node_groups.into_values().map(Element::Group).collect(),
            }),
        ],
    };
    Ok(SubRender { elem: root, diff })
}

/// `render(data4Layout, svg)`: the whole unified renderer for one diagram.
/// The caller resets the dagre id counter once before calling this.
pub fn render(
    data: &LayoutData,
    config: &UnifiedConfig,
    shapes: &ShapeCtx,
    layout: &dyn GraphLayout,
) -> Result<Rendered, HelperError> {
    let mut markers = insert_markers(&data.markers, &data.diagram_type, &data.diagram_id);
    let marker_paint = Style {
        fill: Some(Paint::Color(shapes.vars.line_color)),
        stroke: Some(Paint::Color(shapes.vars.line_color)),
        ..Style::default()
    };
    for m in &mut markers {
        m.apply_paint(&marker_paint);
    }

    let mut graph = prepare_layout(data, config);
    let mut rc = RenderCtx {
        shapes,
        config,
        layout,
        diagram_type: &data.diagram_type,
        diagram_id: &data.diagram_id,
        db: ClusterDb::default(),
        marker_refs: Vec::new(),
    };
    adjust_clusters_and_edges(&mut rc.db, &mut graph);
    let sub = recursive_render(&mut rc, &mut graph, None)?;

    // Colored marker variants (`addEdgeMarker` clones the marker in defs).
    for mref in &rc.marker_refs {
        let Some(color) = mref.color else { continue };
        if markers.iter().any(|m| m.id == mref.id) {
            continue;
        }
        if let Some(original) = markers.iter().find(|m| m.id == mref.original_id) {
            let mut clone = original.clone();
            clone.id = mref.id.clone();
            let mut paint = Style {
                stroke: Some(Paint::Color(color)),
                ..Style::default()
            };
            if mref.fill {
                paint.fill = Some(Paint::Color(color));
            }
            override_paint(&mut clone, &paint);
            markers.push(clone);
        }
    }

    let bounds = group_bbox(&sub.elem);
    Ok(Rendered {
        root: sub.elem,
        markers,
        bounds,
    })
}

/// Sets stroke (and fill when given) on every child of a marker clone.
fn override_paint(marker: &mut Marker, paint: &Style) {
    fn apply(el: &mut Element, paint: &Style) {
        let common = match el {
            Element::Group(g) => {
                for c in &mut g.children {
                    apply(c, paint);
                }
                &mut g.common
            }
            Element::Rect(e) => &mut e.common,
            Element::Circle(e) => &mut e.common,
            Element::Ellipse(e) => &mut e.common,
            Element::Line(e) => &mut e.common,
            Element::Polygon(e) => &mut e.common,
            Element::Polyline(e) => &mut e.common,
            Element::Path(e) => &mut e.common,
            Element::Text(e) => &mut e.common,
            Element::Image(e) => &mut e.common,
            Element::Use(e) => &mut e.common,
            Element::Marker(_) => return,
        };
        if paint.stroke.is_some() {
            common.style.stroke = paint.stroke.clone();
        }
        if paint.fill.is_some() {
            common.style.fill = paint.fill.clone();
        }
    }
    for child in &mut marker.children {
        apply(child, paint);
    }
}

// ---------------------------------------------------------------------------
// getBBox without a DOM

fn translation(common: &Common) -> (f64, f64) {
    common
        .transform
        .iter()
        .fold((0.0, 0.0), |(x, y), op| match op {
            TransformOp::Translate(dx, dy) => (x + dx, y + dy),
            _ => (x, y),
        })
}

/// `element.getBBox()` for the scene: geometry only, text excluded, with
/// `translate` transforms applied (the only transform the renderer emits).
pub fn element_bbox(el: &Element) -> BBox {
    let (common, raw) = match el {
        Element::Group(g) => (&g.common, group_bbox_untransformed(g)),
        Element::Rect(r) => (
            &r.common,
            BBox {
                x: r.x,
                y: r.y,
                width: r.width,
                height: r.height,
            },
        ),
        Element::Circle(c) => (
            &c.common,
            BBox {
                x: c.cx - c.r,
                y: c.cy - c.r,
                width: 2.0 * c.r,
                height: 2.0 * c.r,
            },
        ),
        Element::Ellipse(e) => (
            &e.common,
            BBox {
                x: e.cx - e.rx,
                y: e.cy - e.ry,
                width: 2.0 * e.rx,
                height: 2.0 * e.ry,
            },
        ),
        Element::Line(l) => (
            &l.common,
            points_bbox(&[Point::new(l.x1, l.y1), Point::new(l.x2, l.y2)]),
        ),
        Element::Polygon(p) => (&p.common, points_bbox(&p.points)),
        Element::Polyline(p) => (&p.common, points_bbox(&p.points)),
        Element::Path(p) => {
            let has_geometry = p.d.commands().iter().any(|c| !matches!(c, Command::Close));
            (
                &p.common,
                if has_geometry {
                    path_bbox(&p.d)
                } else {
                    BBox::EMPTY
                },
            )
        }
        Element::Text(t) => (&t.common, BBox::EMPTY),
        Element::Image(i) => (
            &i.common,
            BBox {
                x: i.x,
                y: i.y,
                width: i.width,
                height: i.height,
            },
        ),
        Element::Use(_) | Element::Marker(_) => return BBox::EMPTY,
    };
    if raw == BBox::EMPTY {
        return raw;
    }
    let (dx, dy) = translation(common);
    raw.translated(dx, dy)
}

fn group_bbox_untransformed(g: &Group) -> BBox {
    g.children
        .iter()
        .fold(BBox::EMPTY, |acc, el| acc.union(element_bbox(el)))
}

/// `group.getBBox()` with the group's own transform applied.
pub fn group_bbox(g: &Group) -> BBox {
    let raw = group_bbox_untransformed(g);
    if raw == BBox::EMPTY {
        return raw;
    }
    let (dx, dy) = translation(&g.common);
    raw.translated(dx, dy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::shapes::FixedMeasurer;

    fn data(nodes: Vec<Node>, edges: Vec<Edge>) -> LayoutData {
        LayoutData {
            nodes,
            edges,
            direction: Dir::TB,
            diagram_type: "flowchart-v2".to_string(),
            diagram_id: "svg".to_string(),
            markers: vec![MarkerType::Point],
            ..LayoutData::default()
        }
    }

    #[test]
    fn self_loop_becomes_two_dummies_and_three_segments() {
        let d = data(
            vec![Node::flowchart("A", "A", "rect")],
            vec![Edge::flowchart("L_A_A_0", "A", "A")],
        );
        let g = prepare_layout(&d, &UnifiedConfig::default());
        assert_eq!(g.node_count(), 3);
        assert_eq!(g.edge_count(), 3);
        assert!(g.has_node("A---A---1"));
        assert_eq!(
            g.node("A---A---1").map(|n| n.node.shape.as_str()),
            Some("labelRect")
        );
    }

    #[test]
    fn cluster_without_external_edges_is_extracted() {
        let mut s = Node::flowchart("S", "Group", "rect");
        s.is_group = true;
        let mut a = Node::flowchart("A", "A", "rect");
        a.parent_id = Some("S".into());
        let mut b = Node::flowchart("B", "B", "rect");
        b.parent_id = Some("S".into());
        let d = data(vec![s, a, b], vec![Edge::flowchart("e", "A", "B")]);
        let mut g = prepare_layout(&d, &UnifiedConfig::default());
        let mut db = ClusterDb::default();
        adjust_clusters_and_edges(&mut db, &mut g);
        let s = g.node("S").expect("S stays");
        assert!(s.cluster_node);
        let sub = s.graph.as_ref().expect("sub graph");
        assert_eq!(sub.node_count(), 2);
        assert_eq!(sub.edge_count(), 1);
        assert_eq!(sub.label.rankdir, "LR");
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn edge_to_cluster_is_anchored_on_a_child() {
        let mut s = Node::flowchart("S", "Group", "rect");
        s.is_group = true;
        let mut a = Node::flowchart("A", "A", "rect");
        a.parent_id = Some("S".into());
        let c = Node::flowchart("C", "C", "rect");
        // A -> C makes S externally connected, so C -> S is redirected to
        // the anchor child A instead of collapsing S into one node.
        let d = data(
            vec![s, a, c],
            vec![
                Edge::flowchart("e0", "A", "C"),
                Edge::flowchart("e1", "C", "S"),
            ],
        );
        let mut g = prepare_layout(&d, &UnifiedConfig::default());
        let mut db = ClusterDb::default();
        adjust_clusters_and_edges(&mut db, &mut g);
        let edges = g.edges();
        assert_eq!(edges.len(), 2);
        let to_s = edges.iter().find(|e| e.v == "C").expect("C edge");
        assert_eq!(to_s.w, "A");
        assert_eq!(
            g.edge(to_s).and_then(|e| e.edge.to_cluster.clone()),
            Some("S".to_string())
        );
        assert!(!g.node("S").expect("S").cluster_node);
        assert!(validate(&g));
    }

    #[test]
    fn edge_to_unconnected_cluster_targets_the_collapsed_node() {
        let mut s = Node::flowchart("S", "Group", "rect");
        s.is_group = true;
        let mut a = Node::flowchart("A", "A", "rect");
        a.parent_id = Some("S".into());
        let c = Node::flowchart("C", "C", "rect");
        let d = data(vec![s, a, c], vec![Edge::flowchart("e", "C", "S")]);
        let mut g = prepare_layout(&d, &UnifiedConfig::default());
        let mut db = ClusterDb::default();
        adjust_clusters_and_edges(&mut db, &mut g);
        let edges = g.edges();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].w, "S");
        assert!(g.node("S").expect("S").cluster_node);
    }

    #[test]
    fn render_orders_groups_like_mermaid() {
        let d = data(
            vec![
                Node::flowchart("A", "A", "rect"),
                Node::flowchart("B", "B", "rect"),
            ],
            vec![Edge::flowchart("e", "A", "B")],
        );
        let m = FixedMeasurer::SNAPSHOT;
        let ctx = ShapeCtx::new(&m);
        let layout = FakeLayout::new(&[("A", 100.0, 50.0), ("B", 100.0, 200.0)]);
        let out = render(&d, &UnifiedConfig::default(), &ctx, &layout).expect("renders");
        let classes: Vec<String> = out
            .root
            .children
            .iter()
            .filter_map(|c| match c {
                Element::Group(g) => g.common.class.clone(),
                _ => None,
            })
            .collect();
        assert_eq!(
            classes,
            vec!["clusters", "edgePaths", "edgeLabels", "nodes"]
        );
    }
}
