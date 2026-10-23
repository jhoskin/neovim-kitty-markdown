//! d3-sankey 0.12.3 (`src/sankey.js`, `src/align.js`) and the
//! `sankeyLinkHorizontal` path of `src/sankeyLinkHorizontal.js`.
//!
//! The port is statement by statement, with the float operations in the
//! order the JS runs them. Nodes and links live in two vectors and refer to
//! each other by index; the per-node link lists and the columns are vectors
//! of indices that the layout sorts in place, as d3 sorts its arrays.
//!
//! What mermaid does not use is left out: `nodeSort`, `linkSort`,
//! `fixedValue`, and `sankey.update`. With `sort` and `linkSort` undefined,
//! every `if (sort === undefined)` and `if (linkSort === undefined)` branch
//! is taken.

use std::collections::HashMap;
use std::fmt;

use crate::mermaid::scene::PathData;

/// `sankeyLeft`, `sankeyRight`, `sankeyCenter`, `sankeyJustify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
    Center,
    Justify,
}

impl Align {
    /// mermaid's `nodeAlignment` config value.
    pub fn from_config(name: &str) -> Option<Align> {
        match name {
            "left" => Some(Align::Left),
            "right" => Some(Align::Right),
            "center" => Some(Align::Center),
            "justify" => Some(Align::Justify),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SankeyNode {
    pub id: String,
    pub index: usize,
    pub value: f64,
    pub depth: usize,
    pub height: usize,
    pub layer: usize,
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
    /// Indices into [`SankeyGraph::links`], in d3's sorted order.
    pub source_links: Vec<usize>,
    pub target_links: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SankeyLink {
    pub index: usize,
    /// Indices into [`SankeyGraph::nodes`].
    pub source: usize,
    pub target: usize,
    pub value: f64,
    pub width: f64,
    pub y0: f64,
    pub y1: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SankeyGraph {
    pub nodes: Vec<SankeyNode>,
    pub links: Vec<SankeyLink>,
}

/// The errors d3-sankey throws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SankeyError {
    /// `find`: `throw new Error("missing: " + id)`.
    Missing(String),
    /// `computeNodeDepths` and `computeNodeHeights`.
    CircularLink,
}

impl fmt::Display for SankeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SankeyError::Missing(id) => write!(f, "missing: {id}"),
            SankeyError::CircularLink => write!(f, "circular link"),
        }
    }
}

impl std::error::Error for SankeyError {}

/// One input link: source id, target id, value.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkInput {
    pub source: String,
    pub target: String,
    pub value: f64,
}

/// The generator `Sankey()` returns, with its settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Sankey {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    /// `nodeWidth`.
    dx: f64,
    /// `nodePadding` as set.
    dy: f64,
    /// `nodePadding` as `computeNodeBreadths` limits it.
    py: f64,
    align: Align,
    iterations: usize,
}

impl Default for Sankey {
    fn default() -> Self {
        Sankey::new()
    }
}

/// d3-array `sum`: `if (value = +value) sum += value`, so NaN and zero are
/// skipped.
fn d3_sum(values: impl Iterator<Item = f64>) -> f64 {
    let mut sum = 0.0;
    for v in values {
        if v != 0.0 && !v.is_nan() {
            sum += v;
        }
    }
    sum
}

/// d3-array `min`: the least value that is not NaN, or `undefined`.
fn d3_min(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut min: Option<f64> = None;
    for v in values {
        if v.is_nan() {
            continue;
        }
        if min.is_none_or(|m| m > v) {
            min = Some(v);
        }
    }
    min
}

/// `Array.prototype.sort` with a comparator, as a stable insertion sort:
/// `cmp(a, b) > 0` puts `b` first. For a consistent comparator every stable
/// sort agrees with V8's TimSort; an inconsistent one (NaN keys) cannot
/// panic here, where `slice::sort_by` may.
fn js_sort<T: Copy>(items: &mut [T], cmp: impl Fn(T, T) -> f64) {
    for i in 1..items.len() {
        let mut j = i;
        while j > 0 && cmp(items[j - 1], items[j]) > 0.0 {
            items.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// `a || b` on comparator numbers: 0 and NaN are falsy.
fn or(a: f64, b: f64) -> f64 {
    if a == 0.0 || a.is_nan() { b } else { a }
}

impl Sankey {
    /// `Sankey()`: extent `[[0, 0], [1, 1]]`, `nodeWidth` 24, `nodePadding`
    /// 8, `justify`, 6 iterations.
    pub fn new() -> Sankey {
        Sankey {
            x0: 0.0,
            y0: 0.0,
            x1: 1.0,
            y1: 1.0,
            dx: 24.0,
            dy: 8.0,
            py: 8.0,
            align: Align::Justify,
            iterations: 6,
        }
    }

    pub fn node_width(mut self, dx: f64) -> Sankey {
        self.dx = dx;
        self
    }

    /// `dy = py = +_`.
    pub fn node_padding(mut self, padding: f64) -> Sankey {
        self.dy = padding;
        self.py = padding;
        self
    }

    pub fn node_align(mut self, align: Align) -> Sankey {
        self.align = align;
        self
    }

    /// `extent([[x0, y0], [x1, y1]])`.
    pub fn extent(mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> Sankey {
        self.x0 = x0;
        self.x1 = x1;
        self.y0 = y0;
        self.y1 = y1;
        self
    }

    pub fn iterations(mut self, iterations: usize) -> Sankey {
        self.iterations = iterations;
        self
    }

    /// `sankey(graph)` with `nodeId(d => d.id)`.
    pub fn layout(
        &self,
        node_ids: &[String],
        links: &[LinkInput],
    ) -> Result<SankeyGraph, SankeyError> {
        let mut s = self.clone();
        let mut graph = compute_node_links(node_ids, links)?;
        compute_node_values(&mut graph);
        compute_node_depths(&mut graph)?;
        compute_node_heights(&mut graph)?;
        s.compute_node_breadths(&mut graph);
        compute_link_breadths(&mut graph);
        Ok(graph)
    }

    /// `align.call(null, node, x)` for the configured alignment.
    fn align_value(&self, graph: &SankeyGraph, node: usize, n: usize) -> f64 {
        let node = &graph.nodes[node];
        match self.align {
            // `left(node)`: `node.depth`.
            Align::Left => node.depth as f64,
            // `right(node, n)`: `n - 1 - node.height`.
            Align::Right => n as f64 - 1.0 - node.height as f64,
            // `justify(node, n)`.
            Align::Justify => {
                if node.source_links.is_empty() {
                    n as f64 - 1.0
                } else {
                    node.depth as f64
                }
            }
            // `center(node)`.
            Align::Center => {
                if !node.target_links.is_empty() {
                    node.depth as f64
                } else if !node.source_links.is_empty() {
                    let depths = node
                        .source_links
                        .iter()
                        .map(|&l| graph.nodes[graph.links[l].target].depth as f64);
                    d3_min(depths).map_or(f64::NAN, |m| m - 1.0)
                } else {
                    0.0
                }
            }
        }
    }

    /// `computeNodeLayers`.
    fn compute_node_layers(&self, graph: &mut SankeyGraph) -> Vec<Vec<usize>> {
        let x = graph
            .nodes
            .iter()
            .map(|n| n.depth)
            .max()
            .map_or(0, |d| d + 1);
        let kx = (self.x1 - self.x0 - self.dx) / (x as f64 - 1.0);
        let mut columns: Vec<Vec<usize>> = vec![Vec::new(); x];
        for i in 0..graph.nodes.len() {
            // `Math.max(0, Math.min(x - 1, Math.floor(align(node, x))))`.
            let a = self.align_value(graph, i, x).floor();
            let layer = (x as f64 - 1.0).min(a).max(0.0) as usize;
            let node = &mut graph.nodes[i];
            node.layer = layer;
            node.x0 = self.x0 + layer as f64 * kx;
            node.x1 = node.x0 + self.dx;
            if let Some(column) = columns.get_mut(layer) {
                column.push(i);
            }
        }
        columns
    }

    /// `initializeNodeBreadths`.
    fn initialize_node_breadths(&self, graph: &mut SankeyGraph, columns: &[Vec<usize>]) {
        let py = self.py;
        let ky = d3_min(columns.iter().map(|c| {
            (self.y1 - self.y0 - (c.len() as f64 - 1.0) * py)
                / d3_sum(c.iter().map(|&n| graph.nodes[n].value))
        }))
        .unwrap_or(f64::NAN);
        for nodes in columns {
            let mut y = self.y0;
            for &n in nodes {
                let node = &mut graph.nodes[n];
                node.y0 = y;
                node.y1 = y + node.value * ky;
                y = node.y1 + py;
                for &l in &graph.nodes[n].source_links.clone() {
                    let link = &mut graph.links[l];
                    link.width = link.value * ky;
                }
            }
            y = (self.y1 - y + py) / (nodes.len() as f64 + 1.0);
            for (i, &n) in nodes.iter().enumerate() {
                let node = &mut graph.nodes[n];
                node.y0 += y * (i as f64 + 1.0);
                node.y1 += y * (i as f64 + 1.0);
            }
            reorder_links(graph, nodes);
        }
    }

    /// `computeNodeBreadths`.
    fn compute_node_breadths(&mut self, graph: &mut SankeyGraph) {
        let mut columns = self.compute_node_layers(graph);
        let longest = columns.iter().map(Vec::len).max().unwrap_or(0);
        self.py = self.dy.min((self.y1 - self.y0) / (longest as f64 - 1.0));
        self.initialize_node_breadths(graph, &columns);
        for i in 0..self.iterations {
            let alpha = 0.99f64.powf(i as f64);
            let beta = (1.0 - alpha).max((i as f64 + 1.0) / self.iterations as f64);
            self.relax_right_to_left(graph, &mut columns, alpha, beta);
            self.relax_left_to_right(graph, &mut columns, alpha, beta);
        }
    }

    /// `relaxLeftToRight`: reposition each node based on its incoming
    /// (target) links.
    fn relax_left_to_right(
        &self,
        graph: &mut SankeyGraph,
        columns: &mut [Vec<usize>],
        alpha: f64,
        beta: f64,
    ) {
        for column in columns.iter_mut().skip(1) {
            for &target in column.iter() {
                let mut y = 0.0;
                let mut w = 0.0;
                for &l in &graph.nodes[target].target_links {
                    let link = &graph.links[l];
                    let source = link.source;
                    let v = link.value
                        * (graph.nodes[target].layer as f64 - graph.nodes[source].layer as f64);
                    y += self.target_top(graph, source, target) * v;
                    w += v;
                }
                // `if (!(w > 0)) continue;` also skips a NaN weight.
                if w.is_nan() || w <= 0.0 {
                    continue;
                }
                let node = &mut graph.nodes[target];
                let dy = (y / w - node.y0) * alpha;
                node.y0 += dy;
                node.y1 += dy;
                reorder_node_links(graph, target);
            }
            sort_column(graph, column);
            self.resolve_collisions(graph, column, beta);
        }
    }

    /// `relaxRightToLeft`: reposition each node based on its outgoing
    /// (source) links.
    fn relax_right_to_left(
        &self,
        graph: &mut SankeyGraph,
        columns: &mut [Vec<usize>],
        alpha: f64,
        beta: f64,
    ) {
        let n = columns.len();
        for i in (0..n.saturating_sub(1)).rev() {
            for &source in &columns[i] {
                let mut y = 0.0;
                let mut w = 0.0;
                for &l in &graph.nodes[source].source_links {
                    let link = &graph.links[l];
                    let target = link.target;
                    let v = link.value
                        * (graph.nodes[target].layer as f64 - graph.nodes[source].layer as f64);
                    y += self.source_top(graph, source, target) * v;
                    w += v;
                }
                // `if (!(w > 0)) continue;` also skips a NaN weight.
                if w.is_nan() || w <= 0.0 {
                    continue;
                }
                let node = &mut graph.nodes[source];
                let dy = (y / w - node.y0) * alpha;
                node.y0 += dy;
                node.y1 += dy;
                reorder_node_links(graph, source);
            }
            sort_column(graph, &mut columns[i]);
            self.resolve_collisions(graph, &columns[i], beta);
        }
    }

    /// `resolveCollisions`.
    fn resolve_collisions(&self, graph: &mut SankeyGraph, nodes: &[usize], alpha: f64) {
        let i = nodes.len() >> 1;
        let Some(&subject) = nodes.get(i) else {
            return;
        };
        let (s0, s1) = (graph.nodes[subject].y0, graph.nodes[subject].y1);
        self.resolve_bottom_to_top(graph, nodes, s0 - self.py, i as isize - 1, alpha);
        self.resolve_top_to_bottom(graph, nodes, s1 + self.py, i + 1, alpha);
        self.resolve_bottom_to_top(graph, nodes, self.y1, nodes.len() as isize - 1, alpha);
        self.resolve_top_to_bottom(graph, nodes, self.y0, 0, alpha);
    }

    /// `resolveCollisionsTopToBottom`: push any overlapping nodes down.
    fn resolve_top_to_bottom(
        &self,
        graph: &mut SankeyGraph,
        nodes: &[usize],
        mut y: f64,
        start: usize,
        alpha: f64,
    ) {
        for &n in nodes.iter().skip(start) {
            let node = &mut graph.nodes[n];
            let dy = (y - node.y0) * alpha;
            if dy > 1e-6 {
                node.y0 += dy;
                node.y1 += dy;
            }
            y = node.y1 + self.py;
        }
    }

    /// `resolveCollisionsBottomToTop`: push any overlapping nodes up.
    fn resolve_bottom_to_top(
        &self,
        graph: &mut SankeyGraph,
        nodes: &[usize],
        mut y: f64,
        start: isize,
        alpha: f64,
    ) {
        let mut i = start;
        while i >= 0 {
            let node = &mut graph.nodes[nodes[i as usize]];
            let dy = (node.y1 - y) * alpha;
            if dy > 1e-6 {
                node.y0 -= dy;
                node.y1 -= dy;
            }
            y = node.y0 - self.py;
            i -= 1;
        }
    }

    /// `targetTop`: the `target.y0` that would produce an ideal link from
    /// source to target.
    fn target_top(&self, graph: &SankeyGraph, source: usize, target: usize) -> f64 {
        let s = &graph.nodes[source];
        let mut y = s.y0 - (s.source_links.len() as f64 - 1.0) * self.py / 2.0;
        for &l in &s.source_links {
            let link = &graph.links[l];
            if link.target == target {
                break;
            }
            y += link.width + self.py;
        }
        for &l in &graph.nodes[target].target_links {
            let link = &graph.links[l];
            if link.source == source {
                break;
            }
            y -= link.width;
        }
        y
    }

    /// `sourceTop`: the `source.y0` that would produce an ideal link from
    /// source to target.
    fn source_top(&self, graph: &SankeyGraph, source: usize, target: usize) -> f64 {
        let t = &graph.nodes[target];
        let mut y = t.y0 - (t.target_links.len() as f64 - 1.0) * self.py / 2.0;
        for &l in &t.target_links {
            let link = &graph.links[l];
            if link.source == source {
                break;
            }
            y += link.width + self.py;
        }
        for &l in &graph.nodes[source].source_links {
            let link = &graph.links[l];
            if link.target == target {
                break;
            }
            y -= link.width;
        }
        y
    }
}

/// `computeNodeLinks`: indices, the link lists, and `find` by id. A
/// repeated id maps to its last node, as `new Map(entries)` keeps the last.
fn compute_node_links(
    node_ids: &[String],
    links: &[LinkInput],
) -> Result<SankeyGraph, SankeyError> {
    let mut nodes: Vec<SankeyNode> = node_ids
        .iter()
        .enumerate()
        .map(|(i, id)| SankeyNode {
            id: id.clone(),
            index: i,
            value: 0.0,
            depth: 0,
            height: 0,
            layer: 0,
            x0: 0.0,
            x1: 0.0,
            y0: 0.0,
            y1: 0.0,
            source_links: Vec::new(),
            target_links: Vec::new(),
        })
        .collect();
    let by_id: HashMap<&str, usize> = node_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    let find = |id: &str| {
        by_id
            .get(id)
            .copied()
            .ok_or_else(|| SankeyError::Missing(id.to_string()))
    };
    let mut out = Vec::with_capacity(links.len());
    for (i, link) in links.iter().enumerate() {
        let source = find(&link.source)?;
        let target = find(&link.target)?;
        nodes[source].source_links.push(i);
        nodes[target].target_links.push(i);
        out.push(SankeyLink {
            index: i,
            source,
            target,
            value: link.value,
            width: 0.0,
            y0: 0.0,
            y1: 0.0,
        });
    }
    Ok(SankeyGraph { nodes, links: out })
}

/// `computeNodeValues` without `fixedValue`.
fn compute_node_values(graph: &mut SankeyGraph) {
    for i in 0..graph.nodes.len() {
        let node = &graph.nodes[i];
        let out = d3_sum(node.source_links.iter().map(|&l| graph.links[l].value));
        let inc = d3_sum(node.target_links.iter().map(|&l| graph.links[l].value));
        graph.nodes[i].value = js_max(out, inc);
    }
}

/// `Math.max` of two numbers: NaN wins.
fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// An insertion-ordered set of node indices, as a JS `Set`.
struct NodeSet {
    order: Vec<usize>,
    seen: Vec<bool>,
}

impl NodeSet {
    fn new(n: usize) -> NodeSet {
        NodeSet {
            order: Vec::new(),
            seen: vec![false; n],
        }
    }

    fn add(&mut self, i: usize) {
        if !self.seen[i] {
            self.seen[i] = true;
            self.order.push(i);
        }
    }
}

/// `computeNodeDepths` (`forward`) and `computeNodeHeights` (backward):
/// breadth-first layers over JS sets in insertion order.
fn layer_walk(graph: &mut SankeyGraph, forward: bool) -> Result<(), SankeyError> {
    let n = graph.nodes.len();
    let mut current: Vec<usize> = (0..n).collect();
    let mut x = 0usize;
    while !current.is_empty() {
        let mut next = NodeSet::new(n);
        for &i in &current {
            let node = &mut graph.nodes[i];
            if forward {
                node.depth = x;
                for &l in &node.source_links {
                    next.add(graph.links[l].target);
                }
            } else {
                node.height = x;
                for &l in &node.target_links {
                    next.add(graph.links[l].source);
                }
            }
        }
        x += 1;
        if x > n {
            return Err(SankeyError::CircularLink);
        }
        current = next.order;
    }
    Ok(())
}

fn compute_node_depths(graph: &mut SankeyGraph) -> Result<(), SankeyError> {
    layer_walk(graph, true)
}

fn compute_node_heights(graph: &mut SankeyGraph) -> Result<(), SankeyError> {
    layer_walk(graph, false)
}

/// `column.sort(ascendingBreadth)`.
fn sort_column(graph: &SankeyGraph, column: &mut [usize]) {
    js_sort(column, |a, b| graph.nodes[a].y0 - graph.nodes[b].y0);
}

/// `ascendingSourceBreadth`.
fn ascending_source_breadth(graph: &SankeyGraph, a: usize, b: usize) -> f64 {
    let (la, lb) = (&graph.links[a], &graph.links[b]);
    or(
        graph.nodes[la.source].y0 - graph.nodes[lb.source].y0,
        la.index as f64 - lb.index as f64,
    )
}

/// `ascendingTargetBreadth`.
fn ascending_target_breadth(graph: &SankeyGraph, a: usize, b: usize) -> f64 {
    let (la, lb) = (&graph.links[a], &graph.links[b]);
    or(
        graph.nodes[la.target].y0 - graph.nodes[lb.target].y0,
        la.index as f64 - lb.index as f64,
    )
}

fn sort_source_links(graph: &mut SankeyGraph, node: usize) {
    let mut list = std::mem::take(&mut graph.nodes[node].source_links);
    js_sort(&mut list, |a, b| ascending_target_breadth(graph, a, b));
    graph.nodes[node].source_links = list;
}

fn sort_target_links(graph: &mut SankeyGraph, node: usize) {
    let mut list = std::mem::take(&mut graph.nodes[node].target_links);
    js_sort(&mut list, |a, b| ascending_source_breadth(graph, a, b));
    graph.nodes[node].target_links = list;
}

/// `reorderNodeLinks`.
fn reorder_node_links(graph: &mut SankeyGraph, node: usize) {
    let target_links = graph.nodes[node].target_links.clone();
    for l in target_links {
        let source = graph.links[l].source;
        sort_source_links(graph, source);
    }
    let source_links = graph.nodes[node].source_links.clone();
    for l in source_links {
        let target = graph.links[l].target;
        sort_target_links(graph, target);
    }
}

/// `reorderLinks`.
fn reorder_links(graph: &mut SankeyGraph, nodes: &[usize]) {
    for &n in nodes {
        sort_source_links(graph, n);
        sort_target_links(graph, n);
    }
}

/// `computeLinkBreadths`.
fn compute_link_breadths(graph: &mut SankeyGraph) {
    for n in 0..graph.nodes.len() {
        let mut y0 = graph.nodes[n].y0;
        let mut y1 = y0;
        for &l in &graph.nodes[n].source_links {
            let link = &mut graph.links[l];
            link.y0 = y0 + link.width / 2.0;
            y0 += link.width;
        }
        for &l in &graph.nodes[n].target_links {
            let link = &mut graph.links[l];
            link.y1 = y1 + link.width / 2.0;
            y1 += link.width;
        }
    }
}

/// `sankeyLinkHorizontal()(link)`: d3-shape's `linkHorizontal` from
/// `[source.x1, link.y0]` to `[target.x0, link.y1]`, which draws
/// `moveTo(x0, y0)` and `bezierCurveTo(x0 = (x0 + x1) / 2, y0, x0, y1, x1,
/// y1)`.
pub fn link_horizontal(graph: &SankeyGraph, link: usize) -> PathData {
    let l = &graph.links[link];
    let (x0, y0) = (graph.nodes[l.source].x1, l.y0);
    let (x1, y1) = (graph.nodes[l.target].x0, l.y1);
    let mx = (x0 + x1) / 2.0;
    let mut d = PathData::new();
    d.move_to(x0, y0);
    d.cubic_to(mx, y0, mx, y1, x1, y1);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sum_and_min_skip_nan_like_d3_array() {
        assert_eq!(d3_sum([1.0, f64::NAN, 2.0].into_iter()), 3.0);
        assert_eq!(d3_min([3.0, f64::NAN, 2.0].into_iter()), Some(2.0));
        assert_eq!(d3_min([f64::NAN].into_iter()), None);
    }

    #[test]
    fn js_sort_is_stable() {
        let mut v = [(1, 'a'), (0, 'b'), (1, 'c'), (0, 'd')];
        js_sort(&mut v, |a, b| f64::from(a.0) - f64::from(b.0));
        assert_eq!(v, [(0, 'b'), (0, 'd'), (1, 'a'), (1, 'c')]);
    }

    #[test]
    fn a_cycle_is_a_circular_link() {
        let ids = vec!["a".to_string(), "b".to_string()];
        let links = vec![
            LinkInput {
                source: "a".into(),
                target: "b".into(),
                value: 1.0,
            },
            LinkInput {
                source: "b".into(),
                target: "a".into(),
                value: 1.0,
            },
        ];
        assert_eq!(
            Sankey::new().layout(&ids, &links),
            Err(SankeyError::CircularLink)
        );
    }
}
