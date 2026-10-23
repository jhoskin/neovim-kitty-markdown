//! `CoSELayout` of cose-base 2.2.0 over `FDLayout` and `Layout` of
//! layout-base 2.0.1, and `coseLayout` of fcose's `cose.js` that builds it.
//!
//! The constants fcose writes into `CoSEConstants`, `FDLayoutConstants`,
//! and `LayoutConstants` before each run are fields of [`Constants`]. They
//! are module globals in JS, but every run sets them from the options and
//! the edges, so nothing leaks from one run to the next.

use indexmap::IndexMap;

use super::graph::{GraphManager, NodeId, RectD, calc_separation_amount, get_intersection};
use super::{Edge, FcoseError, Node, Options, RelativePlacement, constraints};

/// `LayoutConstants.SIMPLE_NODE_SIZE`.
const SIMPLE_NODE_SIZE: f64 = 40.0;
/// `FDLayoutConstants.CONVERGENCE_CHECK_PERIOD`.
const CONVERGENCE_CHECK_PERIOD: i64 = 100;
/// `FDLayoutConstants.GRID_CALCULATION_CHECK_PERIOD`.
const GRID_CALCULATION_CHECK_PERIOD: i64 = 10;
/// `FDLayoutConstants.ADAPTATION_LOWER_NODE_LIMIT`.
const ADAPTATION_LOWER_NODE_LIMIT: f64 = 1000.0;
/// `FDLayoutConstants.ADAPTATION_UPPER_NODE_LIMIT`.
const ADAPTATION_UPPER_NODE_LIMIT: f64 = 5000.0;
/// `FDLayoutConstants.COOLING_ADAPTATION_FACTOR`.
const COOLING_ADAPTATION_FACTOR: f64 = 0.33;
/// `FDLayoutConstants.MAX_NODE_DISPLACEMENT_INCREMENTAL`.
const MAX_NODE_DISPLACEMENT_INCREMENTAL: f64 = 100.0;

/// The constants `coseLayout` and `processEdges` set for a run.
#[derive(Debug, Clone)]
pub struct Constants {
    /// `DEFAULT_EDGE_LENGTH`: the mean ideal edge length.
    pub default_edge_length: f64,
    /// `MIN_REPULSION_DIST`: a tenth of it.
    pub min_repulsion_dist: f64,
    pub per_level_ideal_edge_length_factor: f64,
    pub default_gravity_strength: f64,
    pub max_iterations: f64,
    pub default_gravity_range_factor: f64,
    pub default_compound_gravity_strength: f64,
    pub default_compound_gravity_range_factor: f64,
    pub default_cooling_factor_incremental: f64,
}

/// The `RELAX_MOVEMENT_ON_CONSTRAINTS` state `initConstraintVariables`
/// builds for the relative placement constraints.
#[derive(Debug, Clone, Default)]
struct RelativeState {
    dummy_to_node_vertical: IndexMap<String, Vec<String>>,
    dummy_to_node_horizontal: IndexMap<String, Vec<String>>,
    nodes_in_relative_horizontal: Vec<String>,
    nodes_in_relative_vertical: Vec<String>,
    /// Per node: `(other, gap, other_is_right_or_bottom)`.
    constraint_map_horizontal: IndexMap<String, Vec<(String, f64, bool)>>,
    constraint_map_vertical: IndexMap<String, Vec<(String, f64, bool)>>,
    temp_position_horizontal: IndexMap<String, f64>,
    temp_position_vertical: IndexMap<String, f64>,
}

pub struct CoseLayout {
    pub gm: GraphManager,
    pub constants: Constants,
    pub(super) alignment_vertical: Vec<Vec<String>>,
    pub(super) alignment_horizontal: Vec<Vec<String>>,
    has_alignment: bool,
    pub(super) relative: Vec<RelativePlacement>,
    id_to_node: IndexMap<String, NodeId>,
    relative_state: Option<RelativeState>,
    // `Layout`, `FDLayout`, and `CoSELayout` instance fields.
    ideal_edge_length: f64,
    gravity_constant: f64,
    compound_gravity_constant: f64,
    gravity_range_factor: f64,
    compound_gravity_range_factor: f64,
    cooling_factor: f64,
    initial_cooling_factor: f64,
    total_displacement: f64,
    old_total_displacement: f64,
    max_iterations: f64,
    total_iterations: i64,
    total_displacement_threshold: f64,
    max_node_displacement: f64,
    repulsion_range: f64,
    cooling_cycle: f64,
    max_cooling_cycle: f64,
    final_temperature: f64,
    cooling_adjuster: f64,
    grid: Vec<Vec<Vec<NodeId>>>,
}

impl CoseLayout {
    /// `coseLayout(options, undefined)` up to `runLayout`: the constants,
    /// `processChildrenList`, `processEdges`, and `processConstraints`.
    pub fn build(
        nodes: &[Node],
        edges: &[Edge],
        options: &Options,
    ) -> Result<CoseLayout, FcoseError> {
        let mut gm = GraphManager::with_root();
        let root = gm.root;

        // `processChildrenList(gm.addRoot(), aux.getTopMostNodes(nodes), ...)`:
        // depth first, each compound's graph created right after its node.
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
        let mut top_most = Vec::new();
        for (i, n) in nodes.iter().enumerate() {
            match n.parent {
                Some(p) if p < nodes.len() => children[p].push(i),
                _ => top_most.push(i),
            }
        }
        let mut id_to_lnode: IndexMap<String, NodeId> = IndexMap::new();
        fn process(
            gm: &mut GraphManager,
            graph: usize,
            list: &[usize],
            nodes: &[Node],
            children: &[Vec<usize>],
            options: &Options,
            id_to_lnode: &mut IndexMap<String, NodeId>,
        ) {
            for &i in list {
                let n = &nodes[i];
                let rect = RectD {
                    x: n.x - n.width / 2.0,
                    y: n.y - n.height / 2.0,
                    width: n.width,
                    height: n.height,
                };
                let ln = gm.add_node(graph, rect);
                {
                    let node = &mut gm.nodes[ln];
                    node.id = Some(n.id.clone());
                    node.input = Some(i);
                    node.node_repulsion = options.node_repulsion;
                    node.padding_left = Some(n.padding);
                    if node.rect.x.is_nan() {
                        node.rect.x = 0.0;
                    }
                    if node.rect.y.is_nan() {
                        node.rect.y = 0.0;
                    }
                }
                id_to_lnode.insert(n.id.clone(), ln);
                if !children[i].is_empty() {
                    let child_graph = gm.add_graph(ln);
                    process(
                        gm,
                        child_graph,
                        &children[i],
                        nodes,
                        children,
                        options,
                        id_to_lnode,
                    );
                }
            }
        }
        process(
            &mut gm,
            root,
            &top_most,
            nodes,
            &children,
            options,
            &mut id_to_lnode,
        );

        // `processEdges`.
        let mut ideal_length_total = 0.0;
        let mut edge_count = 0.0;
        for e in edges {
            let (Some(source), Some(target)) = (
                nodes.get(e.source).and_then(|n| id_to_lnode.get(&n.id)),
                nodes.get(e.target).and_then(|n| id_to_lnode.get(&n.id)),
            ) else {
                continue;
            };
            let (source, target) = (*source, *target);
            if source != target && gm.edges_between(source, target) == 0 {
                let le = gm.add_edge(source, target);
                gm.edges[le].ideal_length = e.ideal_length;
                gm.edges[le].edge_elasticity = e.elasticity;
                ideal_length_total += e.ideal_length;
                edge_count += 1.0;
            }
        }
        // `idealEdgeLength` is a function in mermaid, so an edgeless graph
        // falls back on 50.
        let default_edge_length = if edge_count > 0.0 {
            ideal_length_total / edge_count
        } else {
            50.0
        };

        let constants = Constants {
            default_edge_length,
            min_repulsion_dist: default_edge_length / 10.0,
            per_level_ideal_edge_length_factor: options.nesting_factor,
            default_gravity_strength: options.gravity,
            max_iterations: options.num_iter,
            default_gravity_range_factor: options.gravity_range,
            default_compound_gravity_strength: options.gravity_compound,
            default_compound_gravity_range_factor: options.gravity_range_compound,
            default_cooling_factor_incremental: options.initial_energy_on_incremental,
        };

        // `processConstraints`, after fcose's `run` dropped empty lists.
        let has_alignment = options.alignment.is_some();
        let (alignment_vertical, alignment_horizontal) = options
            .alignment
            .as_ref()
            .map(|a| (a.vertical.clone(), a.horizontal.clone()))
            .unwrap_or_default();

        Ok(CoseLayout {
            gm,
            alignment_vertical,
            alignment_horizontal,
            has_alignment,
            relative: options.relative.clone(),
            id_to_node: IndexMap::new(),
            relative_state: None,
            // `FDLayout` constructor values.
            ideal_edge_length: constants.default_edge_length,
            gravity_constant: constants.default_gravity_strength,
            compound_gravity_constant: constants.default_compound_gravity_strength,
            gravity_range_factor: constants.default_gravity_range_factor,
            compound_gravity_range_factor: constants.default_compound_gravity_range_factor,
            cooling_factor: constants.default_cooling_factor_incremental,
            initial_cooling_factor: constants.default_cooling_factor_incremental,
            total_displacement: 0.0,
            old_total_displacement: 0.0,
            max_iterations: constants.max_iterations,
            total_iterations: 0,
            total_displacement_threshold: 0.0,
            max_node_displacement: 0.0,
            repulsion_range: 0.0,
            cooling_cycle: 0.0,
            max_cooling_cycle: 0.0,
            final_temperature: 0.0,
            cooling_adjuster: 1.0,
            grid: Vec::new(),
            constants,
        })
    }

    fn has_constraints(&self) -> bool {
        self.has_alignment || !self.relative.is_empty()
    }

    /// `Layout.prototype.runLayout` with tiling off and the post-layout
    /// transform skipped (the layout is incremental).
    pub fn run_layout(&mut self, random: &mut dyn FnMut() -> f64) -> Result<(), FcoseError> {
        self.init_parameters();
        let root = self.gm.root;
        if self.gm.graphs[root].nodes.is_empty() {
            return Ok(());
        }
        // `includesInvalidEdge` removes inter-graph edges between a node
        // and its own ancestor; mermaid connects leaves only, so there are
        // none.
        self.classic_layout(random)
    }

    /// `CoSELayout.prototype.initParameters` over `FDLayout`'s.
    fn init_parameters(&mut self) {
        self.total_iterations = 0;
        self.grid = Vec::new();
        self.ideal_edge_length = if self.constants.default_edge_length < 10.0 {
            10.0
        } else {
            self.constants.default_edge_length
        };
        self.gravity_constant = self.constants.default_gravity_strength;
        self.compound_gravity_constant = self.constants.default_compound_gravity_strength;
        self.gravity_range_factor = self.constants.default_gravity_range_factor;
        self.compound_gravity_range_factor = self.constants.default_compound_gravity_range_factor;
    }

    /// `CoSELayout.prototype.classicLayout` on the incremental path.
    fn classic_layout(&mut self, random: &mut dyn FnMut() -> f64) -> Result<(), FcoseError> {
        let nodes_with_gravity = self.calculate_nodes_to_apply_gravitation_to();
        self.gm.all_nodes_to_apply_gravitation = Some(nodes_with_gravity);
        for n in self.gm.all_nodes() {
            self.gm.nodes[n].no_of_children = self.gm.no_of_children(n);
        }
        self.gm.calc_lowest_common_ancestors();
        let root = self.gm.root;
        self.gm.calc_inclusion_tree_depths(root, 1.0);
        self.gm.graph_calc_estimated_size(root);
        self.calc_ideal_edge_lengths();

        if self.has_constraints() {
            constraints::handle_constraints(self)?;
            self.init_constraint_variables()?;
        }

        self.init_spring_embedder();
        self.run_spring_embedder(random)
    }

    /// `CoSELayout.prototype.calculateNodesToApplyGravitationTo`.
    fn calculate_nodes_to_apply_gravitation_to(&mut self) -> Vec<NodeId> {
        let mut list = Vec::new();
        for g in 0..self.gm.graphs.len() {
            self.gm.update_connected(g);
            if !self.gm.graphs[g].is_connected {
                list.extend(self.gm.graphs[g].nodes.iter().copied());
            }
        }
        list
    }

    /// `FDLayout.prototype.calcIdealEdgeLengths`.
    fn calc_ideal_edge_lengths(&mut self) {
        for e in self.gm.all_edges() {
            let edge = self.gm.edges[e].clone();
            if !edge.is_inter_graph {
                continue;
            }
            let original = edge.ideal_length;
            let mut ideal = edge.ideal_length;
            let size_source = self.gm.nodes[edge.source_in_lca].estimated_size;
            let size_target = self.gm.nodes[edge.target_in_lca].estimated_size;
            // `useSmartIdealEdgeLengthCalculation` is on.
            ideal += size_source + size_target - 2.0 * SIMPLE_NODE_SIZE;
            let lca_depth = edge
                .lca
                .map_or(1.0, |g| self.gm.graph_inclusion_tree_depth(g));
            ideal += original
                * self.constants.per_level_ideal_edge_length_factor
                * (self.gm.nodes[edge.source].inclusion_tree_depth
                    + self.gm.nodes[edge.target].inclusion_tree_depth
                    - 2.0 * lca_depth);
            self.gm.edges[e].ideal_length = ideal;
        }
    }

    pub(super) fn node_by_id(&self, id: &str) -> Result<NodeId, FcoseError> {
        self.id_to_node
            .get(id)
            .copied()
            .ok_or_else(|| FcoseError::Internal(format!("unknown node id {id}")))
    }

    /// `CoSELayout.prototype.initConstraintVariables` with no fixed nodes
    /// and `RELAX_MOVEMENT_ON_CONSTRAINTS`.
    fn init_constraint_variables(&mut self) -> Result<(), FcoseError> {
        self.id_to_node = IndexMap::new();
        for n in self.gm.all_nodes() {
            if let Some(id) = self.gm.nodes[n].id.clone() {
                self.id_to_node.insert(id, n);
            }
        }
        if self.relative.is_empty() {
            return Ok(());
        }
        let mut state = RelativeState::default();
        let mut node_to_dummy_vertical: IndexMap<String, String> = IndexMap::new();
        let mut node_to_dummy_horizontal: IndexMap<String, String> = IndexMap::new();
        if self.has_alignment {
            for (i, group) in self.alignment_vertical.iter().enumerate() {
                let dummy = format!("dummy{i}");
                state
                    .dummy_to_node_vertical
                    .insert(dummy.clone(), Vec::new());
                for id in group {
                    node_to_dummy_vertical.insert(id.clone(), dummy.clone());
                    if let Some(list) = state.dummy_to_node_vertical.get_mut(&dummy) {
                        list.push(id.clone());
                    }
                }
            }
            for (i, group) in self.alignment_horizontal.iter().enumerate() {
                let dummy = format!("dummy{i}");
                state
                    .dummy_to_node_horizontal
                    .insert(dummy.clone(), Vec::new());
                for id in group {
                    node_to_dummy_horizontal.insert(id.clone(), dummy.clone());
                    if let Some(list) = state.dummy_to_node_horizontal.get_mut(&dummy) {
                        list.push(id.clone());
                    }
                }
            }
        }

        let relative = self.relative.clone();
        for constraint in &relative {
            match constraint {
                RelativePlacement::Horizontal { left, right, gap } => {
                    let left_id = node_to_dummy_vertical.get(left).unwrap_or(left).clone();
                    let right_id = node_to_dummy_vertical.get(right).unwrap_or(right).clone();
                    for id in [&left_id, &right_id] {
                        if !state.nodes_in_relative_horizontal.contains(id) {
                            state.nodes_in_relative_horizontal.push(id.clone());
                            state
                                .constraint_map_horizontal
                                .insert(id.clone(), Vec::new());
                            let anchor = match state.dummy_to_node_vertical.get(id) {
                                Some(members) => members.first().cloned().unwrap_or_default(),
                                None => id.clone(),
                            };
                            let x = self.gm.nodes[self.node_by_id(&anchor)?].center_x();
                            state.temp_position_horizontal.insert(id.clone(), x);
                        }
                    }
                    if let Some(list) = state.constraint_map_horizontal.get_mut(&left_id) {
                        list.push((right_id.clone(), *gap, true));
                    }
                    if let Some(list) = state.constraint_map_horizontal.get_mut(&right_id) {
                        list.push((left_id.clone(), *gap, false));
                    }
                }
                RelativePlacement::Vertical { top, bottom, gap } => {
                    let top_id = node_to_dummy_horizontal.get(top).unwrap_or(top).clone();
                    let bottom_id = node_to_dummy_horizontal
                        .get(bottom)
                        .unwrap_or(bottom)
                        .clone();
                    for id in [&top_id, &bottom_id] {
                        if !state.nodes_in_relative_vertical.contains(id) {
                            state.nodes_in_relative_vertical.push(id.clone());
                            state.constraint_map_vertical.insert(id.clone(), Vec::new());
                            let anchor = match state.dummy_to_node_horizontal.get(id) {
                                Some(members) => members.first().cloned().unwrap_or_default(),
                                None => id.clone(),
                            };
                            let y = self.gm.nodes[self.node_by_id(&anchor)?].center_y();
                            state.temp_position_vertical.insert(id.clone(), y);
                        }
                    }
                    if let Some(list) = state.constraint_map_vertical.get_mut(&top_id) {
                        list.push((bottom_id.clone(), *gap, true));
                    }
                    if let Some(list) = state.constraint_map_vertical.get_mut(&bottom_id) {
                        list.push((top_id.clone(), *gap, false));
                    }
                }
            }
        }
        self.relative_state = Some(state);
        Ok(())
    }

    /// `CoSELayout.prototype.initSpringEmbedder` over `FDLayout`'s, for an
    /// incremental layout.
    fn init_spring_embedder(&mut self) {
        let s = self.gm.all_nodes().len() as f64;
        if s > ADAPTATION_LOWER_NODE_LIMIT {
            self.cooling_factor = (self.cooling_factor * COOLING_ADAPTATION_FACTOR).max(
                self.cooling_factor
                    - (s - ADAPTATION_LOWER_NODE_LIMIT)
                        / (ADAPTATION_UPPER_NODE_LIMIT - ADAPTATION_LOWER_NODE_LIMIT)
                        * self.cooling_factor
                        * (1.0 - COOLING_ADAPTATION_FACTOR),
            );
        }
        self.max_node_displacement = MAX_NODE_DISPLACEMENT_INCREMENTAL;
        self.max_iterations = (s * 5.0).max(self.max_iterations);
        let displacement_threshold_per_node = (3.0 * self.constants.default_edge_length) / 100.0;
        self.total_displacement_threshold = displacement_threshold_per_node * s;
        // `CoSELayout.prototype.calcRepulsionRange` at level 0.
        self.repulsion_range = 2.0 * self.ideal_edge_length;

        self.cooling_cycle = 0.0;
        self.max_cooling_cycle = self.max_iterations / CONVERGENCE_CHECK_PERIOD as f64;
        self.final_temperature = 0.04;
        self.cooling_adjuster = 1.0;
    }

    /// `CoSELayout.prototype.runSpringEmbedder` without animation.
    fn run_spring_embedder(&mut self, random: &mut dyn FnMut() -> f64) -> Result<(), FcoseError> {
        while !self.tick(random)? {}
        self.gm.update_bounds();
        Ok(())
    }

    /// `FDLayout.prototype.isConverged`.
    fn is_converged(&mut self) -> bool {
        let mut oscilating = false;
        if self.total_iterations as f64 > self.max_iterations / 3.0 {
            oscilating = (self.total_displacement - self.old_total_displacement).abs() < 2.0;
        }
        let converged = self.total_displacement < self.total_displacement_threshold;
        self.old_total_displacement = self.total_displacement;
        converged || oscilating
    }

    /// `CoSELayout.prototype.tick` with no pruned trees (tree reduction is
    /// off), so the growth phases never start. Returns whether the layout
    /// ended.
    fn tick(&mut self, random: &mut dyn FnMut() -> f64) -> Result<bool, FcoseError> {
        self.total_iterations += 1;
        if self.total_iterations as f64 == self.max_iterations {
            return Ok(true);
        }
        if self.total_iterations % CONVERGENCE_CHECK_PERIOD == 0 {
            if self.is_converged() {
                return Ok(true);
            }
            self.cooling_cycle += 1.0;
            // `layoutQuality` is 2 ("proof"), so `coolingAdjuster` stays 1.
            self.cooling_factor = (self.initial_cooling_factor
                - self.cooling_cycle.powf(
                    (100.0 * (self.initial_cooling_factor - self.final_temperature)).ln()
                        / self.max_cooling_cycle.ln(),
                ) / 100.0
                    * self.cooling_adjuster)
                .max(self.final_temperature);
        }
        self.total_displacement = 0.0;
        self.gm.update_bounds();
        self.calc_spring_forces();
        self.calc_repulsion_forces()?;
        self.calc_gravitational_forces();
        self.move_nodes(random)?;
        Ok(false)
    }

    /// `FDLayout.prototype.calcSpringForces` and `calcSpringForce`.
    fn calc_spring_forces(&mut self) {
        for e in self.gm.all_edges() {
            let (source, target) = (self.gm.edges[e].source, self.gm.edges[e].target);
            // `edge.updateLength()`: clipping points from target to source.
            let (overlap, clip) =
                get_intersection(&self.gm.nodes[target].rect, &self.gm.nodes[source].rect);
            let edge = &mut self.gm.edges[e];
            edge.is_overlaping_source_and_target = overlap;
            if !overlap {
                edge.length_x = clip[0] - clip[2];
                edge.length_y = clip[1] - clip[3];
                if edge.length_x.abs() < 1.0 {
                    edge.length_x = js_sign(edge.length_x);
                }
                if edge.length_y.abs() < 1.0 {
                    edge.length_y = js_sign(edge.length_y);
                }
                edge.length =
                    (edge.length_x * edge.length_x + edge.length_y * edge.length_y).sqrt();
            }
            if edge.is_overlaping_source_and_target {
                continue;
            }
            let length = edge.length;
            if length == 0.0 {
                continue;
            }
            let spring_force = edge.edge_elasticity * (length - edge.ideal_length);
            let fx = spring_force * (edge.length_x / length);
            let fy = spring_force * (edge.length_y / length);
            self.gm.nodes[source].spring_force_x += fx;
            self.gm.nodes[source].spring_force_y += fy;
            self.gm.nodes[target].spring_force_x -= fx;
            self.gm.nodes[target].spring_force_y -= fy;
        }
    }

    /// `FDLayout.prototype.calcRepulsionForces` with the FR grid variant.
    fn calc_repulsion_forces(&mut self) -> Result<(), FcoseError> {
        let nodes = self.gm.all_nodes();
        let update = self.total_iterations % GRID_CALCULATION_CHECK_PERIOD == 1;
        if update {
            self.update_grid()?;
        }
        let mut processed = vec![false; self.gm.nodes.len()];
        for &a in &nodes {
            self.calculate_repulsion_force_of_a_node(a, &processed, update);
            processed[a] = true;
        }
        Ok(())
    }

    /// `FDLayout.prototype.calcGrid` and `updateGrid`.
    fn update_grid(&mut self) -> Result<(), FcoseError> {
        let root = &self.gm.graphs[self.gm.root];
        let (left, right, top, bottom) = (root.left, root.right, root.top, root.bottom);
        let size_x = array_length(((right - left) / self.repulsion_range).ceil())?;
        let size_y = array_length(((bottom - top) / self.repulsion_range).ceil())?;
        self.grid = vec![vec![Vec::new(); size_y]; size_x];
        for n in self.gm.all_nodes() {
            // `addNodeToGrid(v, left, top)`.
            let rect = self.gm.nodes[n].rect;
            let start_x = js_parse_int(((rect.x - left) / self.repulsion_range).floor());
            let finish_x =
                js_parse_int(((rect.width + rect.x - left) / self.repulsion_range).floor());
            let start_y = js_parse_int(((rect.y - top) / self.repulsion_range).floor());
            let finish_y =
                js_parse_int(((rect.height + rect.y - top) / self.repulsion_range).floor());
            for i in start_x..=finish_x {
                for j in start_y..=finish_y {
                    let cell = usize::try_from(i)
                        .ok()
                        .and_then(|i| self.grid.get_mut(i))
                        .and_then(|col| usize::try_from(j).ok().and_then(|j| col.get_mut(j)))
                        .ok_or_else(|| {
                            FcoseError::Internal(
                                "Cannot read properties of undefined (reading 'push')".to_string(),
                            )
                        })?;
                    cell.push(n);
                    let node = &mut self.gm.nodes[n];
                    node.start_x = start_x;
                    node.finish_x = finish_x;
                    node.start_y = start_y;
                    node.finish_y = finish_y;
                }
            }
        }
        Ok(())
    }

    /// `FDLayout.prototype.calculateRepulsionForceOfANode`.
    fn calculate_repulsion_force_of_a_node(&mut self, a: NodeId, processed: &[bool], update: bool) {
        if update {
            let mut surrounding: Vec<NodeId> = Vec::new();
            let mut in_surrounding = vec![false; self.gm.nodes.len()];
            let (sx, fx, sy, fy) = {
                let n = &self.gm.nodes[a];
                (n.start_x, n.finish_x, n.start_y, n.finish_y)
            };
            let width = self.grid.len() as i64;
            let height = self.grid.first().map_or(0, Vec::len) as i64;
            for i in (sx - 1)..(fx + 2) {
                for j in (sy - 1)..(fy + 2) {
                    if i < 0 || j < 0 || i >= width || j >= height {
                        continue;
                    }
                    for &b in &self.grid[i as usize][j as usize] {
                        let (na, nb) = (&self.gm.nodes[a], &self.gm.nodes[b]);
                        if na.owner != nb.owner || a == b {
                            continue;
                        }
                        if !processed[b] && !in_surrounding[b] {
                            let distance_x = (na.center_x() - nb.center_x()).abs()
                                - ((na.rect.width / 2.0) + (nb.rect.width / 2.0));
                            let distance_y = (na.center_y() - nb.center_y()).abs()
                                - ((na.rect.height / 2.0) + (nb.rect.height / 2.0));
                            if distance_x <= self.repulsion_range
                                && distance_y <= self.repulsion_range
                            {
                                in_surrounding[b] = true;
                                surrounding.push(b);
                            }
                        }
                    }
                }
            }
            self.gm.nodes[a].surrounding = surrounding;
        }
        let surrounding = self.gm.nodes[a].surrounding.clone();
        for b in surrounding {
            self.calc_repulsion_force(a, b);
        }
    }

    /// `FDLayout.prototype.calcRepulsionForce` with clipping points (leaf
    /// sizes are not uniform).
    fn calc_repulsion_force(&mut self, a: NodeId, b: NodeId) {
        let rect_a = self.gm.nodes[a].rect;
        let rect_b = self.gm.nodes[b].rect;
        let children_a = self.gm.nodes[a].no_of_children;
        let children_b = self.gm.nodes[b].no_of_children;
        if rect_a.intersects(&rect_b) {
            let overlap =
                calc_separation_amount(&rect_a, &rect_b, self.constants.default_edge_length / 2.0);
            let repulsion_x = 2.0 * overlap[0];
            let repulsion_y = 2.0 * overlap[1];
            let children_constant = children_a * children_b / (children_a + children_b);
            self.gm.nodes[a].repulsion_force_x -= children_constant * repulsion_x;
            self.gm.nodes[a].repulsion_force_y -= children_constant * repulsion_y;
            self.gm.nodes[b].repulsion_force_x += children_constant * repulsion_x;
            self.gm.nodes[b].repulsion_force_y += children_constant * repulsion_y;
        } else {
            let (_, clip) = get_intersection(&rect_a, &rect_b);
            let mut distance_x = clip[2] - clip[0];
            let mut distance_y = clip[3] - clip[1];
            let min = self.constants.min_repulsion_dist;
            if distance_x.abs() < min {
                distance_x = js_sign(distance_x) * min;
            }
            if distance_y.abs() < min {
                distance_y = js_sign(distance_y) * min;
            }
            let distance_squared = distance_x * distance_x + distance_y * distance_y;
            let distance = distance_squared.sqrt();
            let repulsion = (self.gm.nodes[a].node_repulsion / 2.0
                + self.gm.nodes[b].node_repulsion / 2.0)
                * children_a
                * children_b
                / distance_squared;
            let rx = repulsion * distance_x / distance;
            let ry = repulsion * distance_y / distance;
            self.gm.nodes[a].repulsion_force_x -= rx;
            self.gm.nodes[a].repulsion_force_y -= ry;
            self.gm.nodes[b].repulsion_force_x += rx;
            self.gm.nodes[b].repulsion_force_y += ry;
        }
    }

    /// `FDLayout.prototype.calcGravitationalForces` and
    /// `calcGravitationalForce`.
    fn calc_gravitational_forces(&mut self) {
        let list = self
            .gm
            .all_nodes_to_apply_gravitation
            .clone()
            .unwrap_or_default();
        let root = self.gm.root;
        for n in list {
            let Some(owner) = self.gm.nodes[n].owner else {
                continue;
            };
            let g = &self.gm.graphs[owner];
            let owner_center_x = (g.right + g.left) / 2.0;
            let owner_center_y = (g.top + g.bottom) / 2.0;
            let estimated = g.estimated_size;
            let node = &self.gm.nodes[n];
            let distance_x = node.center_x() - owner_center_x;
            let distance_y = node.center_y() - owner_center_y;
            let abs_x = distance_x.abs() + node.rect.width / 2.0;
            let abs_y = distance_y.abs() + node.rect.height / 2.0;
            if owner == root {
                let size = estimated * self.gravity_range_factor;
                if abs_x > size || abs_y > size {
                    let node = &mut self.gm.nodes[n];
                    node.gravitation_force_x = -self.gravity_constant * distance_x;
                    node.gravitation_force_y = -self.gravity_constant * distance_y;
                }
            } else {
                let size = estimated * self.compound_gravity_range_factor;
                if abs_x > size || abs_y > size {
                    let node = &mut self.gm.nodes[n];
                    node.gravitation_force_x =
                        -self.gravity_constant * distance_x * self.compound_gravity_constant;
                    node.gravitation_force_y =
                        -self.gravity_constant * distance_y * self.compound_gravity_constant;
                }
            }
        }
    }

    /// `CoSELayout.prototype.moveNodes`.
    fn move_nodes(&mut self, random: &mut dyn FnMut() -> f64) -> Result<(), FcoseError> {
        let nodes = self.gm.all_nodes();
        for &n in &nodes {
            self.calculate_displacement(n);
        }
        if self.has_constraints() {
            self.update_displacements(random)?;
        }
        for &n in &nodes {
            self.move_node(n);
        }
        Ok(())
    }

    /// `CoSENode.prototype.calculateDisplacement` (no fixed node weights).
    fn calculate_displacement(&mut self, n: NodeId) {
        let cooling = self.cooling_factor;
        let limit = cooling * self.max_node_displacement;
        let node = &mut self.gm.nodes[n];
        node.displacement_x += cooling
            * (node.spring_force_x + node.repulsion_force_x + node.gravitation_force_x)
            / node.no_of_children;
        node.displacement_y += cooling
            * (node.spring_force_y + node.repulsion_force_y + node.gravitation_force_y)
            / node.no_of_children;
        if node.displacement_x.abs() > limit {
            node.displacement_x = limit * js_sign(node.displacement_x);
        }
        if node.displacement_y.abs() > limit {
            node.displacement_y = limit * js_sign(node.displacement_y);
        }
        let (dx, dy) = (node.displacement_x, node.displacement_y);
        if let Some(child) = node.child
            && !self.gm.graphs[child].nodes.is_empty()
        {
            self.propogate_displacement_to_children(child, dx, dy);
        }
    }

    /// `CoSENode.prototype.propogateDisplacementToChildren`.
    fn propogate_displacement_to_children(&mut self, graph: usize, dx: f64, dy: f64) {
        for c in self.gm.graphs[graph].nodes.clone() {
            match self.gm.nodes[c].child {
                None => {
                    self.gm.nodes[c].displacement_x += dx;
                    self.gm.nodes[c].displacement_y += dy;
                }
                Some(child) => self.propogate_displacement_to_children(child, dx, dy),
            }
        }
    }

    /// `CoSENode.prototype.move`.
    fn move_node(&mut self, n: NodeId) {
        let is_leaf = match self.gm.nodes[n].child {
            None => true,
            Some(child) => self.gm.graphs[child].nodes.is_empty(),
        };
        let node = &mut self.gm.nodes[n];
        if is_leaf {
            node.rect.x += node.displacement_x;
            node.rect.y += node.displacement_y;
            self.total_displacement += node.displacement_x.abs() + node.displacement_y.abs();
        }
        node.spring_force_x = 0.0;
        node.spring_force_y = 0.0;
        node.repulsion_force_x = 0.0;
        node.repulsion_force_y = 0.0;
        node.gravitation_force_x = 0.0;
        node.gravitation_force_y = 0.0;
        node.displacement_x = 0.0;
        node.displacement_y = 0.0;
    }

    /// `CoSELayout.prototype.updateDisplacements` with no fixed nodes and
    /// `RELAX_MOVEMENT_ON_CONSTRAINTS`.
    fn update_displacements(&mut self, random: &mut dyn FnMut() -> f64) -> Result<(), FcoseError> {
        if self.has_alignment {
            for group in self.alignment_vertical.clone() {
                let mut total = 0.0;
                for id in &group {
                    total += self.gm.nodes[self.node_by_id(id)?].displacement_x;
                }
                let average = total / group.len() as f64;
                for id in &group {
                    let n = self.node_by_id(id)?;
                    self.gm.nodes[n].displacement_x = average;
                }
            }
            for group in self.alignment_horizontal.clone() {
                let mut total = 0.0;
                for id in &group {
                    total += self.gm.nodes[self.node_by_id(id)?].displacement_y;
                }
                let average = total / group.len() as f64;
                for id in &group {
                    let n = self.node_by_id(id)?;
                    self.gm.nodes[n].displacement_y = average;
                }
            }
        }

        let Some(mut state) = self.relative_state.take() else {
            return Ok(());
        };
        if self.total_iterations % 10 == 0 {
            shuffle(&mut state.nodes_in_relative_horizontal, random);
            shuffle(&mut state.nodes_in_relative_vertical, random);
        }
        let result = self.relax_relative(&mut state);
        self.relative_state = Some(state);
        result
    }

    fn relax_relative(&mut self, state: &mut RelativeState) -> Result<(), FcoseError> {
        for id in state.nodes_in_relative_horizontal.clone() {
            let members = state.dummy_to_node_vertical.get(&id).cloned();
            let anchor = members
                .as_ref()
                .and_then(|m| m.first().cloned())
                .unwrap_or_else(|| id.clone());
            let mut displacement = self.gm.nodes[self.node_by_id(&anchor)?].displacement_x;
            let own = temp(&state.temp_position_horizontal, &id);
            for (other, gap, other_is_right) in state
                .constraint_map_horizontal
                .get(&id)
                .cloned()
                .unwrap_or_default()
            {
                let other_pos = temp(&state.temp_position_horizontal, &other);
                if other_is_right {
                    let diff = (other_pos - own) - displacement;
                    if diff < gap {
                        displacement -= gap - diff;
                    }
                } else {
                    let diff = (own - other_pos) + displacement;
                    if diff < gap {
                        displacement += gap - diff;
                    }
                }
            }
            state
                .temp_position_horizontal
                .insert(id.clone(), own + displacement);
            match members {
                Some(members) => {
                    for m in members {
                        let n = self.node_by_id(&m)?;
                        self.gm.nodes[n].displacement_x = displacement;
                    }
                }
                None => {
                    let n = self.node_by_id(&id)?;
                    self.gm.nodes[n].displacement_x = displacement;
                }
            }
        }
        for id in state.nodes_in_relative_vertical.clone() {
            let members = state.dummy_to_node_horizontal.get(&id).cloned();
            let anchor = members
                .as_ref()
                .and_then(|m| m.first().cloned())
                .unwrap_or_else(|| id.clone());
            let mut displacement = self.gm.nodes[self.node_by_id(&anchor)?].displacement_y;
            let own = temp(&state.temp_position_vertical, &id);
            for (other, gap, other_is_bottom) in state
                .constraint_map_vertical
                .get(&id)
                .cloned()
                .unwrap_or_default()
            {
                let other_pos = temp(&state.temp_position_vertical, &other);
                if other_is_bottom {
                    let diff = (other_pos - own) - displacement;
                    if diff < gap {
                        displacement -= gap - diff;
                    }
                } else {
                    let diff = (own - other_pos) + displacement;
                    if diff < gap {
                        displacement += gap - diff;
                    }
                }
            }
            state
                .temp_position_vertical
                .insert(id.clone(), own + displacement);
            match members {
                Some(members) => {
                    for m in members {
                        let n = self.node_by_id(&m)?;
                        self.gm.nodes[n].displacement_y = displacement;
                    }
                }
                None => {
                    let n = self.node_by_id(&id)?;
                    self.gm.nodes[n].displacement_y = displacement;
                }
            }
        }
        Ok(())
    }
}

/// A temporary position; JS reads `undefined` (NaN in arithmetic) for a
/// missing one.
fn temp(map: &IndexMap<String, f64>, id: &str) -> f64 {
    map.get(id).copied().unwrap_or(f64::NAN)
}

/// `this.shuffle` of `initConstraintVariables`: a partial Fisher-Yates over
/// the last third of the array.
fn shuffle(array: &mut [String], random: &mut dyn FnMut() -> f64) {
    let len = array.len();
    if len == 0 {
        return;
    }
    let mut i = len as i64 - 1;
    while i as f64 >= (2.0 * len as f64 / 3.0) {
        let j = (random() * (i + 1) as f64).floor() as usize;
        array.swap(i as usize, j.min(len - 1));
        i -= 1;
    }
}

/// `IMath.sign`.
pub(super) fn js_sign(value: f64) -> f64 {
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// `parseInt(n)` of an integral number: its decimal string parsed back.
fn js_parse_int(value: f64) -> i64 {
    if value.is_finite() { value as i64 } else { 0 }
}

/// `new Array(parseInt(n))`: a valid length, or the `RangeError` V8 throws.
fn array_length(value: f64) -> Result<usize, FcoseError> {
    if !value.is_finite() || !(0.0..=4_294_967_295.0).contains(&value) {
        return Err(FcoseError::InvalidArrayLength);
    }
    Ok(value as usize)
}
