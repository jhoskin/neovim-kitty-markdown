//! `ConstraintHandler.handleConstraints` of cose-base 2.2.0 on the path a
//! non-randomized fcose run takes: `TRANSFORM_ON_CONSTRAINT_HANDLING` is off
//! (it follows `randomize`), `ENFORCE_CONSTRAINTS` is on, and there are no
//! fixed nodes. The draft positions are the current leaf centres; the
//! alignment constraints move each group to its mean, then the relative
//! placement constraints are enforced on the horizontal and vertical DAGs
//! with every aligned group collapsed into one dummy node.
//!
//! Every JS `Map` and `Set` here is an [`IndexMap`] or an insertion-ordered
//! list, since iteration order decides which component a node lands in.

use std::collections::VecDeque;

use indexmap::IndexMap;

use super::layout::CoseLayout;
use super::{FcoseError, RelativePlacement};

/// One DAG edge: `{ id, gap, direction }` (the direction is implied by the
/// DAG it sits in).
#[derive(Debug, Clone)]
struct Adjacent {
    id: String,
    gap: f64,
}

type Dag = IndexMap<String, Vec<Adjacent>>;

/// The leaf draft coordinates, indexed like `nodeIndexes`.
struct Draft {
    node_indexes: IndexMap<String, usize>,
    x: Vec<f64>,
    y: Vec<f64>,
}

impl Draft {
    fn coord(&self, horizontal: bool, id: &str, dummy_positions: &IndexMap<String, f64>) -> f64 {
        match self.node_indexes.get(id) {
            Some(&i) => {
                if horizontal {
                    self.x[i]
                } else {
                    self.y[i]
                }
            }
            None => dummy_positions.get(id).copied().unwrap_or(f64::NAN),
        }
    }
}

/// `findAppropriatePositionForRelativePlacement` with `fixedNodes` a set
/// (always passed, so the enforcement branches run) that holds no node.
fn position_for_relative_placement(
    graph: &Dag,
    horizontal: bool,
    draft: &Draft,
    dummy_positions: &IndexMap<String, f64>,
    component_sources: &[Vec<String>],
) -> IndexMap<String, f64> {
    let mut in_degrees: IndexMap<String, i64> = graph.keys().map(|k| (k.clone(), 0)).collect();
    for adjacent in graph.values() {
        for a in adjacent {
            if let Some(d) = in_degrees.get_mut(&a.id) {
                *d += 1;
            }
        }
    }

    let mut position_map: IndexMap<String, f64> = IndexMap::new();
    let mut past_map: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    for (key, &value) in &in_degrees {
        if value == 0 {
            queue.push_back(key.clone());
        } else {
            position_map.insert(key.clone(), f64::NEG_INFINITY);
        }
        past_map.insert(key.clone(), vec![key.clone()]);
    }

    // Align the sources of each component (none is fixed).
    for component in component_sources {
        let mut position = 0.0;
        for id in component {
            position += draft.coord(horizontal, id, dummy_positions);
        }
        position /= component.len() as f64;
        for id in component {
            position_map.insert(id.clone(), position);
        }
    }

    let get = |map: &IndexMap<String, f64>, id: &str| map.get(id).copied().unwrap_or(f64::NAN);
    while let Some(current) = queue.pop_front() {
        let neighbors = graph.get(&current).cloned().unwrap_or_default();
        for neighbor in neighbors {
            let candidate = get(&position_map, &current) + neighbor.gap;
            if get(&position_map, &neighbor.id) < candidate {
                position_map.insert(neighbor.id.clone(), candidate);
            }
            if let Some(d) = in_degrees.get_mut(&neighbor.id) {
                *d -= 1;
                if *d == 0 {
                    queue.push_back(neighbor.id.clone());
                }
            }
            // `pastMap.set(neighbor, setUnion(past(current), past(neighbor)))`.
            let mut union = past_map.get(&current).cloned().unwrap_or_default();
            for id in past_map.get(&neighbor.id).cloned().unwrap_or_default() {
                if !union.contains(&id) {
                    union.push(id);
                }
            }
            past_map.insert(neighbor.id.clone(), union);
        }
    }

    // Readjust each sink's component back to its draft centre.
    let sink_nodes: Vec<&String> = graph
        .iter()
        .filter(|(_, v)| v.is_empty())
        .map(|(k, _)| k)
        .collect();
    let mut components: Vec<Vec<String>> = Vec::new();
    for (key, value) in &past_map {
        if !sink_nodes.contains(&key) {
            continue;
        }
        let first = value.first();
        // The last component holding the first node wins.
        let exist_at = components
            .iter()
            .rposition(|c| first.is_some_and(|f| c.contains(f)));
        match exist_at {
            None => {
                let mut set = Vec::new();
                for id in value {
                    if !set.contains(id) {
                        set.push(id.clone());
                    }
                }
                components.push(set);
            }
            Some(i) => {
                for id in value {
                    if !components[i].contains(id) {
                        components[i].push(id.clone());
                    }
                }
            }
        }
    }
    for component in &components {
        let mut min_before = f64::INFINITY;
        let mut min_after = f64::INFINITY;
        let mut max_before = f64::NEG_INFINITY;
        let mut max_after = f64::NEG_INFINITY;
        for id in component {
            let before = draft.coord(horizontal, id, dummy_positions);
            let after = get(&position_map, id);
            if before < min_before {
                min_before = before;
            }
            if before > max_before {
                max_before = before;
            }
            if after < min_after {
                min_after = after;
            }
            if after > max_after {
                max_after = after;
            }
        }
        let diff = (min_before + max_before) / 2.0 - (min_after + max_after) / 2.0;
        for id in component {
            let v = get(&position_map, id) + diff;
            position_map.insert(id.clone(), v);
        }
    }
    position_map
}

/// `findComponents`: weakly connected components of an undirected graph.
fn find_components(graph: &Dag) -> Vec<Vec<String>> {
    let mut components = Vec::new();
    let mut visited: Vec<String> = Vec::new();
    for key in graph.keys() {
        if visited.contains(key) {
            continue;
        }
        let mut component = vec![key.clone()];
        let mut queue = VecDeque::from([key.clone()]);
        visited.push(key.clone());
        while let Some(current) = queue.pop_front() {
            for neighbor in graph.get(&current).map(Vec::as_slice).unwrap_or(&[]) {
                if !visited.contains(&neighbor.id) {
                    queue.push_back(neighbor.id.clone());
                    visited.push(neighbor.id.clone());
                    component.push(neighbor.id.clone());
                }
            }
        }
        components.push(component);
    }
    components
}

/// `dagToUndirected`.
fn dag_to_undirected(dag: &Dag) -> Dag {
    let mut undirected: Dag = dag.keys().map(|k| (k.clone(), Vec::new())).collect();
    for (key, value) in dag {
        for adjacent in value {
            if let Some(list) = undirected.get_mut(key) {
                list.push(adjacent.clone());
            }
            if let Some(list) = undirected.get_mut(&adjacent.id) {
                list.push(Adjacent {
                    id: key.clone(),
                    gap: adjacent.gap,
                });
            }
        }
    }
    undirected
}

/// `dagToReversed`.
fn dag_to_reversed(dag: &Dag) -> Dag {
    let mut reversed: Dag = dag.keys().map(|k| (k.clone(), Vec::new())).collect();
    for (key, value) in dag {
        for adjacent in value {
            if let Some(list) = reversed.get_mut(&adjacent.id) {
                list.push(Adjacent {
                    id: key.clone(),
                    gap: adjacent.gap,
                });
            }
        }
    }
    reversed
}

fn push_edge(dag: &mut Dag, source: &str, target: Adjacent) {
    let target_id = target.id.clone();
    dag.entry(source.to_string()).or_default().push(target);
    dag.entry(target_id).or_default();
}

pub(super) fn handle_constraints(layout: &mut CoseLayout) -> Result<(), FcoseError> {
    let all_nodes = layout.gm.all_nodes();
    let mut draft = Draft {
        node_indexes: IndexMap::new(),
        x: Vec::new(),
        y: Vec::new(),
    };
    let mut leaf_by_id: IndexMap<String, usize> = IndexMap::new();
    for &n in &all_nodes {
        let node = &layout.gm.nodes[n];
        if node.child.is_none() {
            let id = node.id.clone().unwrap_or_default();
            draft.node_indexes.insert(id.clone(), draft.x.len());
            draft.x.push(node.center_x());
            draft.y.push(node.center_y());
            leaf_by_id.insert(id, n);
        }
    }
    let index_of = |draft: &Draft, id: &str| -> Result<usize, FcoseError> {
        draft
            .node_indexes
            .get(id)
            .copied()
            .ok_or_else(|| FcoseError::Internal(format!("unknown node id {id}")))
    };

    // Relative placement constraints without a gap get the default one.
    let default_edge_length = layout.constants.default_edge_length;
    let mut relative = layout.relative.clone();
    for constraint in &mut relative {
        match constraint {
            RelativePlacement::Horizontal { left, right, gap } => {
                if gap.is_nan() {
                    let l = leaf_by_id.get(left.as_str()).copied();
                    let r = leaf_by_id.get(right.as_str()).copied();
                    if let (Some(l), Some(r)) = (l, r) {
                        *gap = default_edge_length
                            + layout.gm.nodes[l].rect.width / 2.0
                            + layout.gm.nodes[r].rect.width / 2.0;
                    }
                }
            }
            RelativePlacement::Vertical { top, bottom, gap } => {
                if gap.is_nan() {
                    let t = leaf_by_id.get(top.as_str()).copied();
                    let b = leaf_by_id.get(bottom.as_str()).copied();
                    if let (Some(t), Some(b)) = (t, b) {
                        *gap = default_edge_length
                            + layout.gm.nodes[t].rect.height / 2.0
                            + layout.gm.nodes[b].rect.height / 2.0;
                    }
                }
            }
        }
    }
    layout.relative = relative.clone();

    // The constraint DAG, both directions in one map.
    let mut dag: IndexMap<String, Vec<(Adjacent, bool)>> = IndexMap::new();
    for constraint in &relative {
        let (source, target, gap, horizontal) = match constraint {
            RelativePlacement::Horizontal { left, right, gap } => (left, right, *gap, true),
            RelativePlacement::Vertical { top, bottom, gap } => (top, bottom, *gap, false),
        };
        dag.entry(source.clone()).or_default().push((
            Adjacent {
                id: target.clone(),
                gap,
            },
            horizontal,
        ));
        dag.entry(target.clone()).or_default();
    }

    // Enforce the alignment constraints.
    let vertical = layout.alignment_vertical.clone();
    let horizontal = layout.alignment_horizontal.clone();
    for group in &vertical {
        let set = dedup(group);
        let mut sum = 0.0;
        for id in &set {
            sum += draft.x[index_of(&draft, id)?];
        }
        let x = sum / set.len() as f64;
        for id in &set {
            let i = index_of(&draft, id)?;
            draft.x[i] = x;
        }
    }
    for group in &horizontal {
        let set = dedup(group);
        let mut sum = 0.0;
        for id in &set {
            sum += draft.y[index_of(&draft, id)?];
        }
        let y = sum / set.len() as f64;
        for id in &set {
            let i = index_of(&draft, id)?;
            draft.y[i] = y;
        }
    }

    // Enforce the relative placement constraints.
    if !relative.is_empty() {
        let mut node_to_dummy_vertical: IndexMap<String, String> = IndexMap::new();
        let mut node_to_dummy_horizontal: IndexMap<String, String> = IndexMap::new();
        let mut dummy_to_node_vertical: IndexMap<String, Vec<String>> = IndexMap::new();
        let mut dummy_to_node_horizontal: IndexMap<String, Vec<String>> = IndexMap::new();
        let mut dummy_positions_vertical: IndexMap<String, f64> = IndexMap::new();
        let mut dummy_positions_horizontal: IndexMap<String, f64> = IndexMap::new();
        for (i, group) in vertical.iter().enumerate() {
            let dummy = format!("dummy{i}");
            dummy_to_node_vertical.insert(dummy.clone(), Vec::new());
            for id in group {
                node_to_dummy_vertical.insert(id.clone(), dummy.clone());
                if let Some(list) = dummy_to_node_vertical.get_mut(&dummy) {
                    list.push(id.clone());
                }
            }
            if let Some(first) = group.first() {
                let x = draft.x[index_of(&draft, first)?];
                dummy_positions_vertical.insert(dummy, x);
            }
        }
        for (i, group) in horizontal.iter().enumerate() {
            let dummy = format!("dummy{i}");
            dummy_to_node_horizontal.insert(dummy.clone(), Vec::new());
            for id in group {
                node_to_dummy_horizontal.insert(id.clone(), dummy.clone());
                if let Some(list) = dummy_to_node_horizontal.get_mut(&dummy) {
                    list.push(id.clone());
                }
            }
            if let Some(first) = group.first() {
                let y = draft.y[index_of(&draft, first)?];
                dummy_positions_horizontal.insert(dummy, y);
            }
        }

        let mut dag_horizontal: Dag = IndexMap::new();
        let mut dag_vertical: Dag = IndexMap::new();
        for (node_id, adjacent) in &dag {
            for (a, is_horizontal) in adjacent {
                let (to_dummy, target_dag) = if *is_horizontal {
                    (&node_to_dummy_vertical, &mut dag_horizontal)
                } else {
                    (&node_to_dummy_horizontal, &mut dag_vertical)
                };
                let source_id = to_dummy.get(node_id).unwrap_or(node_id).clone();
                let target = match to_dummy.get(&a.id) {
                    Some(dummy) => Adjacent {
                        id: dummy.clone(),
                        gap: a.gap,
                    },
                    None => a.clone(),
                };
                push_edge(target_dag, &source_id, target);
            }
        }

        let sources = |dag: &Dag| -> Vec<Vec<String>> {
            let undirected = dag_to_undirected(dag);
            let reversed = dag_to_reversed(dag);
            find_components(&undirected)
                .into_iter()
                .map(|component| {
                    component
                        .into_iter()
                        .filter(|id| reversed.get(id).is_some_and(Vec::is_empty))
                        .collect()
                })
                .collect()
        };
        let sources_horizontal = sources(&dag_horizontal);
        let sources_vertical = sources(&dag_vertical);

        let position_horizontal = position_for_relative_placement(
            &dag_horizontal,
            true,
            &draft,
            &dummy_positions_vertical,
            &sources_horizontal,
        );
        let position_vertical = position_for_relative_placement(
            &dag_vertical,
            false,
            &draft,
            &dummy_positions_horizontal,
            &sources_vertical,
        );

        for (key, &value) in &position_horizontal {
            match dummy_to_node_vertical.get(key) {
                Some(members) => {
                    for id in members {
                        let i = index_of(&draft, id)?;
                        draft.x[i] = value;
                    }
                }
                None => {
                    let i = index_of(&draft, key)?;
                    draft.x[i] = value;
                }
            }
        }
        for (key, &value) in &position_vertical {
            match dummy_to_node_horizontal.get(key) {
                Some(members) => {
                    for id in members {
                        let i = index_of(&draft, id)?;
                        draft.y[i] = value;
                    }
                }
                None => {
                    let i = index_of(&draft, key)?;
                    draft.y[i] = value;
                }
            }
        }
    }

    for (id, &n) in &leaf_by_id {
        let i = index_of(&draft, id)?;
        let (x, y) = (draft.x[i], draft.y[i]);
        layout.gm.nodes[n].set_center(x, y);
    }
    Ok(())
}

/// A JS `Set` built from a list: first occurrences in order.
fn dedup(list: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in list {
        if !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}
