//! The compound graph of layout-base 2.0.1: `LGraphManager`, `LGraph`,
//! `LNode`, `LEdge`, `RectangleD`, and the `IGeometry` helpers the CoSE
//! forces use, as index-based arenas.
//!
//! The JS objects point at each other; here a node, graph, or edge is an
//! index into [`GraphManager`]. Graph indices are creation order, which is
//! the order of `LGraphManager.graphs`, so `getAllNodes` and `getAllEdges`
//! keep their JS order (the force sums depend on it).

/// `Integer.MAX_VALUE` of `util/Integer.js`, the sentinel the bounds and
/// size computations start from.
pub const INT_MAX: f64 = 2_147_483_647.0;
/// `Integer.MIN_VALUE`.
pub const INT_MIN: f64 = -2_147_483_648.0;

/// `LayoutConstants.DEFAULT_GRAPH_MARGIN`.
const DEFAULT_GRAPH_MARGIN: f64 = 15.0;
/// `LayoutConstants.EMPTY_COMPOUND_NODE_SIZE`.
const EMPTY_COMPOUND_NODE_SIZE: f64 = 40.0;

pub type NodeId = usize;
pub type GraphId = usize;
pub type EdgeId = usize;

/// `RectangleD`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RectD {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl RectD {
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    pub fn center_x(&self) -> f64 {
        self.x + self.width / 2.0
    }

    pub fn center_y(&self) -> f64 {
        self.y + self.height / 2.0
    }

    /// `RectangleD.prototype.intersects`: touching rectangles intersect.
    pub fn intersects(&self, a: &RectD) -> bool {
        !(self.right() < a.x || self.bottom() < a.y || a.right() < self.x || a.bottom() < self.y)
    }
}

/// `CoSENode` with the `FDLayoutNode` and `LNode` fields the layout reads.
#[derive(Debug, Clone)]
pub struct LNode {
    pub rect: RectD,
    pub owner: Option<GraphId>,
    pub child: Option<GraphId>,
    pub edges: Vec<EdgeId>,
    /// The cytoscape id; `None` for the root graph's dummy parent.
    pub id: Option<String>,
    /// Index of the caller's node this layout node stands for.
    pub input: Option<usize>,
    pub estimated_size: f64,
    pub inclusion_tree_depth: f64,
    pub no_of_children: f64,
    pub node_repulsion: f64,
    /// `paddingLeft`: `parseInt(ele.css('padding'))` for every cytoscape
    /// node, `undefined` for the root dummy.
    pub padding_left: Option<f64>,
    pub spring_force_x: f64,
    pub spring_force_y: f64,
    pub repulsion_force_x: f64,
    pub repulsion_force_y: f64,
    pub gravitation_force_x: f64,
    pub gravitation_force_y: f64,
    pub displacement_x: f64,
    pub displacement_y: f64,
    pub start_x: i64,
    pub finish_x: i64,
    pub start_y: i64,
    pub finish_y: i64,
    pub surrounding: Vec<NodeId>,
}

impl LNode {
    fn new(rect: RectD) -> LNode {
        LNode {
            rect,
            owner: None,
            child: None,
            edges: Vec::new(),
            id: None,
            input: None,
            estimated_size: INT_MIN,
            inclusion_tree_depth: INT_MAX,
            no_of_children: 0.0,
            // `FDLayoutConstants.DEFAULT_REPULSION_STRENGTH`.
            node_repulsion: 4500.0,
            padding_left: None,
            spring_force_x: 0.0,
            spring_force_y: 0.0,
            repulsion_force_x: 0.0,
            repulsion_force_y: 0.0,
            gravitation_force_x: 0.0,
            gravitation_force_y: 0.0,
            displacement_x: 0.0,
            displacement_y: 0.0,
            start_x: 0,
            finish_x: 0,
            start_y: 0,
            finish_y: 0,
            surrounding: Vec::new(),
        }
    }

    pub fn center_x(&self) -> f64 {
        self.rect.center_x()
    }

    pub fn center_y(&self) -> f64 {
        self.rect.center_y()
    }

    /// `LNode.prototype.setCenter`.
    pub fn set_center(&mut self, cx: f64, cy: f64) {
        self.rect.x = cx - self.rect.width / 2.0;
        self.rect.y = cy - self.rect.height / 2.0;
    }
}

/// `CoSEGraph`.
#[derive(Debug, Clone)]
pub struct LGraph {
    pub parent: NodeId,
    pub nodes: Vec<NodeId>,
    pub edges: Vec<EdgeId>,
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
    pub estimated_size: f64,
    pub margin: f64,
    pub is_connected: bool,
}

/// `CoSEEdge` with the `FDLayoutEdge` and `LEdge` fields.
#[derive(Debug, Clone)]
pub struct LEdge {
    pub source: NodeId,
    pub target: NodeId,
    pub is_inter_graph: bool,
    pub ideal_length: f64,
    pub edge_elasticity: f64,
    pub length: f64,
    pub length_x: f64,
    pub length_y: f64,
    pub is_overlaping_source_and_target: bool,
    pub lca: Option<GraphId>,
    pub source_in_lca: NodeId,
    pub target_in_lca: NodeId,
}

/// `CoSEGraphManager`: owns every node, graph, and edge.
#[derive(Debug, Clone, Default)]
pub struct GraphManager {
    pub nodes: Vec<LNode>,
    pub graphs: Vec<LGraph>,
    pub edges: Vec<LEdge>,
    /// `LGraphManager.edges`: the inter-graph edges.
    pub inter_graph_edges: Vec<EdgeId>,
    pub root: GraphId,
    all_nodes: Option<Vec<NodeId>>,
    all_edges: Option<Vec<EdgeId>>,
    pub all_nodes_to_apply_gravitation: Option<Vec<NodeId>>,
}

impl GraphManager {
    /// `newGraphManager()` followed by `addRoot()`: the root graph and its
    /// dummy parent node (`layout.newNode(null)`).
    pub fn with_root() -> GraphManager {
        let mut gm = GraphManager::default();
        let dummy = gm.push_node(RectD::default());
        gm.root = gm.add_graph(dummy);
        gm
    }

    fn push_node(&mut self, rect: RectD) -> NodeId {
        self.nodes.push(LNode::new(rect));
        self.nodes.len() - 1
    }

    /// `gm.add(newGraph, parentNode)`.
    pub fn add_graph(&mut self, parent: NodeId) -> GraphId {
        self.graphs.push(LGraph {
            parent,
            nodes: Vec::new(),
            edges: Vec::new(),
            left: f64::NAN,
            right: f64::NAN,
            top: f64::NAN,
            bottom: f64::NAN,
            estimated_size: INT_MIN,
            margin: DEFAULT_GRAPH_MARGIN,
            is_connected: false,
        });
        let g = self.graphs.len() - 1;
        self.nodes[parent].child = Some(g);
        g
    }

    /// `graph.add(new CoSENode(gm, loc, size))`.
    pub fn add_node(&mut self, graph: GraphId, rect: RectD) -> NodeId {
        let n = self.push_node(rect);
        self.nodes[n].owner = Some(graph);
        self.graphs[graph].nodes.push(n);
        n
    }

    /// `gm.add(newEdge, source, target)`: an intra-graph edge when both ends
    /// share an owner, an inter-graph edge otherwise.
    pub fn add_edge(&mut self, source: NodeId, target: NodeId) -> EdgeId {
        let source_graph = self.nodes[source].owner;
        let target_graph = self.nodes[target].owner;
        let is_inter_graph = source_graph != target_graph;
        self.edges.push(LEdge {
            source,
            target,
            is_inter_graph,
            // `FDLayoutConstants.DEFAULT_EDGE_LENGTH` and
            // `DEFAULT_SPRING_STRENGTH`; the caller overwrites both.
            ideal_length: 50.0,
            edge_elasticity: 0.45,
            length: 0.0,
            length_x: 0.0,
            length_y: 0.0,
            is_overlaping_source_and_target: false,
            lca: None,
            source_in_lca: source,
            target_in_lca: target,
        });
        let e = self.edges.len() - 1;
        if is_inter_graph {
            self.inter_graph_edges.push(e);
            self.nodes[source].edges.push(e);
            self.nodes[target].edges.push(e);
        } else if let Some(g) = source_graph {
            self.graphs[g].edges.push(e);
            self.nodes[source].edges.push(e);
            if target != source {
                self.nodes[target].edges.push(e);
            }
        }
        e
    }

    /// `getAllNodes`: the nodes of every graph in graph order.
    pub fn all_nodes(&mut self) -> Vec<NodeId> {
        if self.all_nodes.is_none() {
            let list = self
                .graphs
                .iter()
                .flat_map(|g| g.nodes.iter().copied())
                .collect();
            self.all_nodes = Some(list);
        }
        self.all_nodes.clone().unwrap_or_default()
    }

    /// `getAllEdges`: the intra-graph edges in graph order, then the
    /// inter-graph edges.
    pub fn all_edges(&mut self) -> Vec<EdgeId> {
        if self.all_edges.is_none() {
            let mut list: Vec<EdgeId> = self
                .graphs
                .iter()
                .flat_map(|g| g.edges.iter().copied())
                .collect();
            list.extend(self.inter_graph_edges.iter().copied());
            self.all_edges = Some(list);
        }
        self.all_edges.clone().unwrap_or_default()
    }

    /// `LNode.prototype.getEdgesBetween`, counted.
    pub fn edges_between(&self, a: NodeId, b: NodeId) -> usize {
        self.nodes[a]
            .edges
            .iter()
            .filter(|&&e| self.edges[e].target == b || self.edges[e].source == b)
            .count()
    }

    /// `LNode.prototype.getParent`: the node that owns this node's graph.
    pub fn parent_of(&self, n: NodeId) -> Option<NodeId> {
        self.nodes[n].owner.map(|g| self.graphs[g].parent)
    }

    /// `LEdge.prototype.getOtherEnd`.
    fn other_end(&self, e: EdgeId, n: NodeId) -> NodeId {
        let edge = &self.edges[e];
        if edge.source == n {
            edge.target
        } else {
            edge.source
        }
    }

    /// `LEdge.prototype.getOtherEndInGraph`.
    fn other_end_in_graph(&self, e: EdgeId, n: NodeId, graph: GraphId) -> Option<NodeId> {
        let mut other = self.other_end(e, n);
        loop {
            let owner = self.nodes[other].owner?;
            if owner == graph {
                return Some(other);
            }
            if owner == self.root {
                return None;
            }
            other = self.graphs[owner].parent;
        }
    }

    /// `LNode.prototype.withChildren`, in the set's insertion order.
    fn with_children(&self, n: NodeId, out: &mut Vec<NodeId>) {
        if !out.contains(&n) {
            out.push(n);
        }
        if let Some(child) = self.nodes[n].child {
            for &c in &self.graphs[child].nodes {
                self.with_children(c, out);
            }
        }
    }

    /// `LGraph.prototype.updateConnected`.
    pub fn update_connected(&mut self, graph: GraphId) {
        let nodes = self.graphs[graph].nodes.clone();
        if nodes.is_empty() {
            self.graphs[graph].is_connected = true;
            return;
        }
        let mut queue = std::collections::VecDeque::new();
        let mut visited: Vec<NodeId> = Vec::new();
        let mut seen = vec![false; self.nodes.len()];
        let mut start = Vec::new();
        self.with_children(nodes[0], &mut start);
        for n in start {
            queue.push_back(n);
            if !seen[n] {
                seen[n] = true;
                visited.push(n);
            }
        }
        while let Some(current) = queue.pop_front() {
            for &e in &self.nodes[current].edges {
                if let Some(neighbor) = self.other_end_in_graph(e, current, graph)
                    && !seen[neighbor]
                {
                    let mut children = Vec::new();
                    self.with_children(neighbor, &mut children);
                    for c in children {
                        queue.push_back(c);
                        if !seen[c] {
                            seen[c] = true;
                            visited.push(c);
                        }
                    }
                }
            }
        }
        let mut connected = false;
        if visited.len() >= nodes.len() {
            let in_graph = visited
                .iter()
                .filter(|&&v| self.nodes[v].owner == Some(graph))
                .count();
            connected = in_graph == nodes.len();
        }
        self.graphs[graph].is_connected = connected;
    }

    /// `LNode.prototype.getNoOfChildren`.
    pub fn no_of_children(&self, n: NodeId) -> f64 {
        let mut count = 0.0;
        match self.nodes[n].child {
            None => count = 1.0,
            Some(child) => {
                for &c in &self.graphs[child].nodes {
                    count += self.no_of_children(c);
                }
            }
        }
        if count == 0.0 { 1.0 } else { count }
    }

    /// `LGraphManager.prototype.calcLowestCommonAncestors`.
    pub fn calc_lowest_common_ancestors(&mut self) {
        let root = self.root;
        for e in self.all_edges() {
            let source = self.edges[e].source;
            let target = self.edges[e].target;
            let mut lca = None;
            let mut source_in_lca = source;
            let mut target_in_lca = target;
            if source == target {
                lca = self.nodes[source].owner;
            } else {
                let mut source_ancestor = self.nodes[source].owner;
                while lca.is_none() {
                    target_in_lca = target;
                    let mut target_ancestor = self.nodes[target].owner;
                    while lca.is_none() {
                        if target_ancestor == source_ancestor {
                            lca = target_ancestor;
                            break;
                        }
                        if target_ancestor == Some(root) {
                            break;
                        }
                        let Some(ta) = target_ancestor else { break };
                        target_in_lca = self.graphs[ta].parent;
                        target_ancestor = self.nodes[target_in_lca].owner;
                    }
                    if source_ancestor == Some(root) {
                        break;
                    }
                    if lca.is_none() {
                        let Some(sa) = source_ancestor else { break };
                        source_in_lca = self.graphs[sa].parent;
                        source_ancestor = self.nodes[source_in_lca].owner;
                    }
                }
            }
            let edge = &mut self.edges[e];
            edge.lca = lca;
            edge.source_in_lca = source_in_lca;
            edge.target_in_lca = target_in_lca;
        }
    }

    /// `LGraphManager.prototype.calcInclusionTreeDepths`.
    pub fn calc_inclusion_tree_depths(&mut self, graph: GraphId, depth: f64) {
        for n in self.graphs[graph].nodes.clone() {
            self.nodes[n].inclusion_tree_depth = depth;
            if let Some(child) = self.nodes[n].child {
                self.calc_inclusion_tree_depths(child, depth + 1.0);
            }
        }
    }

    /// `LGraph.prototype.getInclusionTreeDepth`.
    pub fn graph_inclusion_tree_depth(&self, graph: GraphId) -> f64 {
        if graph == self.root {
            1.0
        } else {
            self.nodes[self.graphs[graph].parent].inclusion_tree_depth
        }
    }

    /// `LGraph.prototype.calcEstimatedSize`.
    pub fn graph_calc_estimated_size(&mut self, graph: GraphId) -> f64 {
        let nodes = self.graphs[graph].nodes.clone();
        let mut size = 0.0;
        for &n in &nodes {
            size += self.node_calc_estimated_size(n);
        }
        let estimated = if size == 0.0 {
            EMPTY_COMPOUND_NODE_SIZE
        } else {
            size / (nodes.len() as f64).sqrt()
        };
        self.graphs[graph].estimated_size = estimated;
        estimated
    }

    /// `LNode.prototype.calcEstimatedSize`.
    fn node_calc_estimated_size(&mut self, n: NodeId) -> f64 {
        match self.nodes[n].child {
            None => {
                let rect = self.nodes[n].rect;
                let size = (rect.width + rect.height) / 2.0;
                self.nodes[n].estimated_size = size;
                size
            }
            Some(child) => {
                let size = self.graph_calc_estimated_size(child);
                let node = &mut self.nodes[n];
                node.estimated_size = size;
                node.rect.width = size;
                node.rect.height = size;
                size
            }
        }
    }

    /// `LGraphManager.prototype.updateBounds`.
    pub fn update_bounds(&mut self) {
        let root = self.root;
        self.graph_update_bounds(root, true);
    }

    /// `LGraph.prototype.updateBounds(recursive)`.
    pub fn graph_update_bounds(&mut self, graph: GraphId, recursive: bool) {
        let mut left = INT_MAX;
        let mut right = -INT_MAX;
        let mut top = INT_MAX;
        let mut bottom = -INT_MAX;
        let nodes = self.graphs[graph].nodes.clone();
        for &n in &nodes {
            if recursive && self.nodes[n].child.is_some() {
                self.node_update_bounds(n);
            }
            let rect = self.nodes[n].rect;
            if left > rect.x {
                left = rect.x;
            }
            if right < rect.right() {
                right = rect.right();
            }
            if top > rect.y {
                top = rect.y;
            }
            if bottom < rect.bottom() {
                bottom = rect.bottom();
            }
        }
        let bounding = RectD {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        };
        let margin = nodes
            .first()
            .and_then(|&n| self.parent_of(n))
            .and_then(|p| self.nodes[p].padding_left)
            .unwrap_or(self.graphs[graph].margin);
        let g = &mut self.graphs[graph];
        g.left = bounding.x - margin;
        g.right = bounding.x + bounding.width + margin;
        g.top = bounding.y - margin;
        g.bottom = bounding.y + bounding.height + margin;
    }

    /// `LNode.prototype.updateBounds` without the label terms
    /// (`NODE_DIMENSIONS_INCLUDE_LABELS` is false).
    fn node_update_bounds(&mut self, n: NodeId) {
        let Some(child) = self.nodes[n].child else {
            return;
        };
        if self.graphs[child].nodes.is_empty() {
            return;
        }
        self.graph_update_bounds(child, true);
        let g = &self.graphs[child];
        let rect = RectD {
            x: g.left,
            y: g.top,
            width: g.right - g.left,
            height: g.bottom - g.top,
        };
        self.nodes[n].rect = rect;
    }
}

/// `IGeometry.calcSeparationAmount`: half the amounts that separate two
/// overlapping rectangles, plus the buffer.
pub fn calc_separation_amount(a: &RectD, b: &RectD, separation_buffer: f64) -> [f64; 2] {
    let directions = [
        if a.center_x() < b.center_x() {
            -1.0
        } else {
            1.0
        },
        if a.center_y() < b.center_y() {
            -1.0
        } else {
            1.0
        },
    ];
    let mut overlap = [
        a.right().min(b.right()) - a.x.max(b.x),
        a.bottom().min(b.bottom()) - a.y.max(b.y),
    ];
    if a.x <= b.x && a.right() >= b.right() {
        overlap[0] += (b.x - a.x).min(a.right() - b.right());
    } else if b.x <= a.x && b.right() >= a.right() {
        overlap[0] += (a.x - b.x).min(b.right() - a.right());
    }
    if a.y <= b.y && a.bottom() >= b.bottom() {
        overlap[1] += (b.y - a.y).min(a.bottom() - b.bottom());
    } else if b.y <= a.y && b.bottom() >= a.bottom() {
        overlap[1] += (a.y - b.y).min(b.bottom() - a.bottom());
    }
    let mut slope = ((b.center_y() - a.center_y()) / (b.center_x() - a.center_x())).abs();
    if b.center_y() == a.center_y() && b.center_x() == a.center_x() {
        slope = 1.0;
    }
    let mut move_by_y = slope * overlap[0];
    let mut move_by_x = overlap[1] / slope;
    if overlap[0] < move_by_x {
        move_by_x = overlap[0];
    } else {
        move_by_y = overlap[1];
    }
    [
        -directions[0] * ((move_by_x / 2.0) + separation_buffer),
        -directions[1] * ((move_by_y / 2.0) + separation_buffer),
    ]
}

/// `IGeometry.getCardinalDirection`.
fn cardinal_direction(slope: f64, slope_prime: f64, line: i32) -> i32 {
    if slope > slope_prime {
        line
    } else {
        1 + line % 4
    }
}

/// `IGeometry.getIntersection2`: the clipping points of the line between
/// the two centres with each rectangle, `[ax, ay, bx, by]`, and whether
/// the rectangles overlap. Entries JS leaves `undefined` are NaN.
pub fn get_intersection(a: &RectD, b: &RectD) -> (bool, [f64; 4]) {
    let mut result = [f64::NAN; 4];
    let p1x = a.center_x();
    let p1y = a.center_y();
    let p2x = b.center_x();
    let p2y = b.center_y();
    if a.intersects(b) {
        return (true, [p1x, p1y, p2x, p2y]);
    }
    let top_left_ax = a.x;
    let top_left_ay = a.y;
    let top_right_ax = a.right();
    let bottom_left_ax = a.x;
    let bottom_left_ay = a.bottom();
    let bottom_right_ax = a.right();
    let half_width_a = a.width / 2.0;
    let half_height_a = a.height / 2.0;
    let top_left_bx = b.x;
    let top_left_by = b.y;
    let top_right_bx = b.right();
    let bottom_left_bx = b.x;
    let bottom_left_by = b.bottom();
    let bottom_right_bx = b.right();
    let half_width_b = b.width / 2.0;
    let half_height_b = b.height / 2.0;
    let mut clip_a_found = false;
    let mut clip_b_found = false;

    if p1x == p2x {
        if p1y > p2y {
            return (false, [p1x, top_left_ay, p2x, bottom_left_by]);
        } else if p1y < p2y {
            return (false, [p1x, bottom_left_ay, p2x, top_left_by]);
        }
        return (false, result);
    }
    if p1y == p2y {
        if p1x > p2x {
            return (false, [top_left_ax, p1y, top_right_bx, p2y]);
        } else if p1x < p2x {
            return (false, [top_right_ax, p1y, top_left_bx, p2y]);
        }
        return (false, result);
    }

    let slope_a = a.height / a.width;
    let slope_b = b.height / b.width;
    let slope_prime = (p2y - p1y) / (p2x - p1x);

    if -slope_a == slope_prime {
        if p1x > p2x {
            result[0] = bottom_left_ax;
            result[1] = bottom_left_ay;
        } else {
            result[0] = top_right_ax;
            result[1] = top_left_ay;
        }
        clip_a_found = true;
    } else if slope_a == slope_prime {
        if p1x > p2x {
            result[0] = top_left_ax;
            result[1] = top_left_ay;
        } else {
            result[0] = bottom_right_ax;
            result[1] = bottom_left_ay;
        }
        clip_a_found = true;
    }

    if -slope_b == slope_prime {
        if p2x > p1x {
            result[2] = bottom_left_bx;
            result[3] = bottom_left_by;
        } else {
            result[2] = top_right_bx;
            result[3] = top_left_by;
        }
        clip_b_found = true;
    } else if slope_b == slope_prime {
        if p2x > p1x {
            result[2] = top_left_bx;
            result[3] = top_left_by;
        } else {
            result[2] = bottom_right_bx;
            result[3] = bottom_left_by;
        }
        clip_b_found = true;
    }

    if clip_a_found && clip_b_found {
        return (false, result);
    }

    let (dir_a, dir_b) = if p1x > p2x {
        if p1y > p2y {
            (
                cardinal_direction(slope_a, slope_prime, 4),
                cardinal_direction(slope_b, slope_prime, 2),
            )
        } else {
            (
                cardinal_direction(-slope_a, slope_prime, 3),
                cardinal_direction(-slope_b, slope_prime, 1),
            )
        }
    } else if p1y > p2y {
        (
            cardinal_direction(-slope_a, slope_prime, 1),
            cardinal_direction(-slope_b, slope_prime, 3),
        )
    } else {
        (
            cardinal_direction(slope_a, slope_prime, 2),
            cardinal_direction(slope_b, slope_prime, 4),
        )
    };

    if !clip_a_found {
        match dir_a {
            1 => {
                result[0] = p1x + (-half_height_a) / slope_prime;
                result[1] = top_left_ay;
            }
            2 => {
                result[0] = bottom_right_ax;
                result[1] = p1y + half_width_a * slope_prime;
            }
            3 => {
                result[0] = p1x + half_height_a / slope_prime;
                result[1] = bottom_left_ay;
            }
            4 => {
                result[0] = bottom_left_ax;
                result[1] = p1y + (-half_width_a) * slope_prime;
            }
            _ => {}
        }
    }
    if !clip_b_found {
        match dir_b {
            1 => {
                result[2] = p2x + (-half_height_b) / slope_prime;
                result[3] = top_left_by;
            }
            2 => {
                result[2] = bottom_right_bx;
                result[3] = p2y + half_width_b * slope_prime;
            }
            3 => {
                result[2] = p2x + half_height_b / slope_prime;
                result[3] = bottom_left_by;
            }
            4 => {
                result[2] = bottom_left_bx;
                result[3] = p2y + (-half_width_b) * slope_prime;
            }
            _ => {}
        }
    }
    (false, result)
}
