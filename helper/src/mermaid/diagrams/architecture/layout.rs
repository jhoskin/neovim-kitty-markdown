//! `layoutArchitecture` of `architectureRenderer.ts` (mermaid 12.2.0) and
//! the parts of cytoscape 3.34.4 it reads back: compound bounds, element
//! bounding boxes, manual edge endpoints, and segment points.
//!
//! mermaid adds the groups, services, junctions, and edges to a cytoscape
//! instance whose nodes all sit at the origin, derives alignment and
//! relative placement constraints from the BFS grid of
//! `getDataStructures`, and runs fcose inside `withSeededRandom(seed)`. The
//! first run's `layoutstop` sets `segment-weights` and `segment-distances`
//! on every edge whose ends differ on both axes and runs the layout again
//! from the first result, so fcose runs twice. Each run relocates its
//! result to the centre of `eles.boundingBox()`, which includes the
//! service labels cytoscape measures on a canvas in its default font.
//!
//! Positions here are cytoscape positions. The renderer draws each node's
//! 80 px icon with its top-left corner there, which is why the group boxes
//! and edge endpoints come out shifted by half an icon.

use indexmap::IndexMap;
use serde_json::Value;

use super::db::{self, ArchitectureDb, Dir, NodeType};
use crate::error::HelperError;
use crate::mermaid::layout::fcose::{
    self, AlignmentConstraint, FcoseError, Options, RelativePlacement,
};

/// `defaultConfig.architecture` merged with the diagram's config.
#[derive(Debug, Clone, PartialEq)]
pub struct ArchConfig {
    pub padding: f64,
    pub icon_size: f64,
    pub font_size: f64,
    pub randomize: bool,
    pub ideal_edge_length_multiplier: f64,
    pub edge_elasticity: f64,
    pub num_iter: f64,
    pub seed: f64,
    pub use_max_width: bool,
}

impl ArchConfig {
    pub fn from_raw(raw: &Value) -> ArchConfig {
        let section = &raw["architecture"];
        let number =
            |key: &str, default: f64| section.get(key).and_then(Value::as_f64).unwrap_or(default);
        let flag =
            |key: &str, default: bool| section.get(key).and_then(Value::as_bool).unwrap_or(default);
        ArchConfig {
            padding: number("padding", 40.0),
            icon_size: number("iconSize", 80.0),
            font_size: number("fontSize", 16.0),
            randomize: flag("randomize", false),
            ideal_edge_length_multiplier: number("idealEdgeLengthMultiplier", 1.5),
            edge_elasticity: number("edgeElasticity", 0.45),
            num_iter: number("numIter", 2500.0),
            seed: number("seed", 1.0),
            use_max_width: flag("useMaxWidth", true),
        }
    }
}

/// cytoscape's default `font-family`, which the label measurement uses
/// (mermaid's style sets only the size).
pub const CYTOSCAPE_FONT_FAMILY: &str = "Helvetica Neue, Helvetica, sans-serif";
/// cytoscape's default node `width` and `height`.
const CYTOSCAPE_NODE_SIZE: f64 = 30.0;
/// `border-width: 1` of the default stylesheet's `:parent` rule, centred
/// on the outline, so a compound is one pixel wider and taller.
const PARENT_BORDER: f64 = 1.0;
/// The default stylesheet's edge `width`.
const EDGE_WIDTH: f64 = 3.0;
/// `marginOfError` of `updateBoundsFromLabel`.
const LABEL_MARGIN_OF_ERROR: f64 = 2.0;

/// An axis-aligned box as cytoscape keeps it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BBox {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl BBox {
    const EMPTY: BBox = BBox {
        x1: f64::INFINITY,
        y1: f64::INFINITY,
        x2: f64::NEG_INFINITY,
        y2: f64::NEG_INFINITY,
    };

    pub fn w(&self) -> f64 {
        self.x2 - self.x1
    }

    pub fn h(&self) -> f64 {
        self.y2 - self.y1
    }

    /// `updateBounds`: zero-area boxes are skipped.
    fn update(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        if x2 - x1 == 0.0 || y2 - y1 == 0.0 {
            return;
        }
        if x1.is_nan() || y1.is_nan() || x2.is_nan() || y2.is_nan() {
            return;
        }
        self.x1 = if x1 < self.x1 { x1 } else { self.x1 };
        self.x2 = if x2 > self.x2 { x2 } else { self.x2 };
        self.y1 = if y1 < self.y1 { y1 } else { self.y1 };
        self.y2 = if y2 > self.y2 { y2 } else { self.y2 };
    }

    fn update_box(&mut self, b: &BBox) {
        self.update(b.x1, b.y1, b.x2, b.y2);
    }

    /// `expandBoundingBox(bb, padding)`.
    fn expanded(&self, by: f64) -> BBox {
        BBox {
            x1: self.x1 - by,
            y1: self.y1 - by,
            x2: self.x2 + by,
            y2: self.y2 + by,
        }
    }

    /// `noninf` on every side.
    fn noninf(&self) -> BBox {
        let f = |v: f64| if v.is_infinite() { 0.0 } else { v };
        BBox {
            x1: f(self.x1),
            y1: f(self.y1),
            x2: f(self.x2),
            y2: f(self.y2),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CyKind {
    Group,
    Service,
    Junction,
}

/// One cytoscape node.
#[derive(Debug, Clone)]
pub struct CyNode {
    pub id: String,
    pub kind: CyKind,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// `position()`.
    pub pos: (f64, f64),
    /// The style `width` and `height` of a leaf.
    pub width: f64,
    pub height: f64,
    /// The style `padding` of a group, in px.
    pub padding: f64,
    /// The measured label, `None` without one.
    pub label: Option<(f64, f64)>,
}

/// One cytoscape edge with the style mermaid gives it.
#[derive(Debug, Clone)]
pub struct CyEdge {
    pub source: usize,
    pub target: usize,
    pub source_dir: Dir,
    pub target_dir: Dir,
    /// `curve-style: segments` (an edge that bends).
    pub segments: bool,
    /// `segment-weights` and `segment-distances`.
    pub weight: f64,
    pub distance: f64,
    pub label: Option<(f64, f64)>,
}

/// An edge's geometry as `sourceEndpoint()`, `midpoint()`, and
/// `targetEndpoint()` report it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgePoints {
    pub start: (f64, f64),
    pub mid: (f64, f64),
    pub end: (f64, f64),
}

/// The laid out cytoscape graph.
#[derive(Debug, Clone)]
pub struct Cy {
    pub nodes: Vec<CyNode>,
    pub edges: Vec<CyEdge>,
    pub index: IndexMap<String, usize>,
}

/// The `source-endpoint` / `target-endpoint` mermaid sets for a side, as
/// fractions of the node's outer size from its position.
fn endpoint_fraction(d: Dir) -> (f64, f64) {
    match d {
        'L' => (0.0, 0.5),
        'R' => (1.0, 0.5),
        'T' => (0.5, 0.0),
        _ => (0.5, 1.0),
    }
}

/// The `spacing` and `gap` of the arrow shape `none`, the shape every
/// mermaid architecture edge has in cytoscape.
const NO_ARROW_SPACING: f64 = 0.0;

/// `math.shortenIntersection(intersection, offset, amount)`.
fn shorten(intersection: (f64, f64), offset: (f64, f64)) -> (f64, f64) {
    let disp = (intersection.0 - offset.0, intersection.1 - offset.1);
    let length = (disp.0 * disp.0 + disp.1 * disp.1).sqrt();
    let mut ratio = (length - NO_ARROW_SPACING) / length;
    if ratio < 0.0 {
        ratio = 0.00001;
    }
    (offset.0 + ratio * disp.0, offset.1 + ratio * disp.1)
}

impl Cy {
    /// `outerWidth()` and `outerHeight()`.
    pub fn outer_size(&self, n: usize) -> (f64, f64) {
        let node = &self.nodes[n];
        if node.children.is_empty() {
            (
                node.width + 2.0 * node.padding,
                node.height + 2.0 * node.padding,
            )
        } else {
            let (_, w, h) = self.compound(n);
            (
                w + PARENT_BORDER + 2.0 * node.padding,
                h + PARENT_BORDER + 2.0 * node.padding,
            )
        }
    }

    /// `updateCompoundBounds` for one parent: the position and the auto
    /// size from the children's boxes with their labels.
    fn compound(&self, n: usize) -> ((f64, f64), f64, f64) {
        let mut bb = BBox::EMPTY;
        for &c in &self.nodes[n].children {
            bb.update_box(&self.body_bounds(c).expanded(1.0));
            if let Some(label) = self.label_bounds(c) {
                bb.update_box(&label);
            }
        }
        let bb = bb.noninf();
        if bb.w() == 0.0 || bb.h() == 0.0 {
            return (self.nodes[n].pos, CYTOSCAPE_NODE_SIZE, CYTOSCAPE_NODE_SIZE);
        }
        (
            ((bb.x1 + bb.x2) / 2.0, (bb.y1 + bb.y2) / 2.0),
            bb.w(),
            bb.h(),
        )
    }

    /// The node's position: its own for a leaf, the children's centre for
    /// a compound.
    pub fn position(&self, n: usize) -> (f64, f64) {
        if self.nodes[n].children.is_empty() {
            self.nodes[n].pos
        } else {
            self.compound(n).0
        }
    }

    /// The node body: `position() ± outer size / 2`.
    fn body_bounds(&self, n: usize) -> BBox {
        let (x, y) = self.position(n);
        let (w, h) = self.outer_size(n);
        BBox {
            x1: x - w / 2.0,
            y1: y - h / 2.0,
            x2: x + w / 2.0,
            y2: y + h / 2.0,
        }
    }

    /// `updateBoundsFromLabel` for a node label at `text-valign: bottom`,
    /// `text-halign: center`.
    fn label_bounds(&self, n: usize) -> Option<BBox> {
        let (lw, lh) = self.nodes[n].label?;
        let (x, y) = self.position(n);
        let label_y = y + self.nodes[n].height / 2.0 + self.nodes[n].padding;
        Some(BBox {
            x1: x - lw / 2.0 - LABEL_MARGIN_OF_ERROR,
            y1: label_y - LABEL_MARGIN_OF_ERROR,
            x2: x + lw / 2.0 + LABEL_MARGIN_OF_ERROR,
            y2: label_y + lh + LABEL_MARGIN_OF_ERROR,
        })
    }

    /// `boundingBox()` of a node with the default options: body, label,
    /// and the one pixel antialiasing margin.
    pub fn node_bounding_box(&self, n: usize) -> BBox {
        let mut bb = BBox::EMPTY;
        bb.update_box(&self.body_bounds(n));
        if let Some(label) = self.label_bounds(n) {
            bb.update_box(&label);
        }
        let bb = bb.noninf();
        if bb.w() > 0.0 && bb.h() > 0.0 {
            bb.expanded(1.0)
        } else {
            bb
        }
    }

    /// `manualEndptToPx`: position plus the fractions of the outer size.
    fn manual_endpoint(&self, n: usize, d: Dir) -> (f64, f64) {
        let (x, y) = self.position(n);
        let (w, h) = self.outer_size(n);
        let (fx, fy) = endpoint_fraction(d);
        (fx * w + x, fy * h + y)
    }

    /// `findSegmentsPoints` with `edge-distances: endpoints`: the one
    /// segment point.
    fn segment_point(&self, e: &CyEdge) -> (f64, f64) {
        let (x1, y1) = self.manual_endpoint(e.source, e.source_dir);
        let (x2, y2) = self.manual_endpoint(e.target, e.target_dir);
        let (dx, dy) = (x2 - x1, y2 - y1);
        let l = (dx * dx + dy * dy).sqrt();
        let normal = (-dy / l, dx / l);
        let (w1, w2) = (1.0 - e.weight, e.weight);
        let mid = (x1 * w1 + x2 * w2, y1 * w1 + y2 * w2);
        (mid.0 + normal.0 * e.distance, mid.1 + normal.1 * e.distance)
    }

    /// `findEndpoints` and `storeAllpts` for a straight or segments edge
    /// with manual endpoints and no arrows.
    pub fn edge_points(&self, e: &CyEdge) -> EdgePoints {
        let src_intersect = self.manual_endpoint(e.source, e.source_dir);
        let tgt_intersect = self.manual_endpoint(e.target, e.target_dir);
        if e.segments {
            let seg = self.segment_point(e);
            let start = shorten(src_intersect, seg);
            let end = shorten(tgt_intersect, seg);
            EdgePoints {
                start,
                mid: seg,
                end,
            }
        } else {
            let (sfx, sfy) = endpoint_fraction(e.source_dir);
            let (tfx, tfy) = endpoint_fraction(e.target_dir);
            let src_pos = self.position(e.source);
            let tgt_pos = self.position(e.target);
            // The direction points are the other end's position plus the
            // raw `pfValue` of its endpoint (a fraction, not pixels).
            let from_target = (tgt_pos.0 + tfx, tgt_pos.1 + tfy);
            let from_source = (src_pos.0 + sfx, src_pos.1 + sfy);
            let start = shorten(src_intersect, from_target);
            let end = shorten(tgt_intersect, from_source);
            EdgePoints {
                start,
                mid: (
                    (start.0 + end.0 + start.0 + end.0) / 4.0,
                    (start.1 + end.1 + start.1 + end.1) / 4.0,
                ),
                end,
            }
        }
    }

    /// `boundingBox()` of an edge: the rough box of its points widened by
    /// half the line width, the label, and the antialiasing margin.
    fn edge_bounding_box(&self, e: &CyEdge) -> BBox {
        let p = self.edge_points(e);
        let half = EDGE_WIDTH / 2.0;
        let mut bb = BBox::EMPTY;
        let xs = [p.start.0, p.mid.0, p.end.0];
        let ys = [p.start.1, p.mid.1, p.end.1];
        let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
        let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        bb.update(
            min(&xs) - half,
            min(&ys) - half,
            max(&xs) + half,
            max(&ys) + half,
        );
        if e.segments {
            for pt in [p.start, p.mid, p.end] {
                bb.update(pt.0 - half, pt.1 - half, pt.0 + half, pt.1 + half);
            }
        }
        if let Some((lw, lh)) = e.label {
            bb.update(
                p.mid.0 - lw / 2.0 - LABEL_MARGIN_OF_ERROR,
                p.mid.1 - lh / 2.0 - LABEL_MARGIN_OF_ERROR,
                p.mid.0 + lw / 2.0 + LABEL_MARGIN_OF_ERROR,
                p.mid.1 + lh / 2.0 + LABEL_MARGIN_OF_ERROR,
            );
        }
        let bb = bb.noninf();
        if bb.w() > 0.0 && bb.h() > 0.0 {
            bb.expanded(1.0)
        } else {
            bb
        }
    }

    /// The centre of `cy.elements().boundingBox()`.
    fn elements_center(&self) -> (f64, f64) {
        let mut bb = BBox::EMPTY;
        for n in 0..self.nodes.len() {
            bb.update_box(&self.node_bounding_box(n));
        }
        for e in &self.edges {
            bb.update_box(&self.edge_bounding_box(e));
        }
        let bb = bb.noninf();
        (bb.x1 + bb.w() / 2.0, bb.y1 + bb.h() / 2.0)
    }
}

/// The `withSeededRandom` generator of `architectureSeed.ts`, a mulberry32
/// with the state advanced before each draw.
pub fn seeded_random(seed: u32) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state = state.wrapping_add(1_831_565_813);
        let mut t = state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

/// `seed >>> 0`.
fn js_to_uint32(v: f64) -> u32 {
    if !v.is_finite() {
        return 0;
    }
    let t = v.trunc().rem_euclid(4_294_967_296.0);
    t as u32
}

/// `getAlignments(db, spatialMaps, groupAlignments, layoutHints)`.
pub fn alignments(db: &ArchitectureDb, ds: &db::DataStructures) -> AlignmentConstraint {
    let flatten = |map: &IndexMap<i64, IndexMap<String, Vec<String>>>, dir: db::Alignment| {
        let mut flattened: IndexMap<String, Vec<String>> = IndexMap::new();
        for (numeric, groups) in map {
            let key = numeric.to_string();
            let mut cnt = 0;
            let arr: Vec<(&String, &Vec<String>)> = groups.iter().collect();
            if arr.len() == 1 {
                flattened.insert(key, arr[0].1.clone());
                continue;
            }
            for i in 0..arr.len().saturating_sub(1) {
                for j in (i + 1)..arr.len() {
                    let (a_group, a_ids) = arr[i];
                    let (b_group, b_ids) = arr[j];
                    let alignment = ds.group_alignments.get(&db::group_key(a_group, b_group));
                    if alignment == Some(&dir) || a_group == "default" || b_group == "default" {
                        let entry = flattened.entry(key.clone()).or_default();
                        entry.extend(a_ids.iter().cloned());
                        entry.extend(b_ids.iter().cloned());
                    } else {
                        flattened.insert(format!("{key}-{cnt}"), a_ids.clone());
                        cnt += 1;
                        flattened.insert(format!("{key}-{cnt}"), b_ids.clone());
                        cnt += 1;
                    }
                }
            }
        }
        flattened
    };
    let mut horizontal_raw = Vec::new();
    let mut vertical_raw = Vec::new();
    for spatial in &ds.spatial_maps {
        let mut horizontal: IndexMap<i64, IndexMap<String, Vec<String>>> = IndexMap::new();
        let mut vertical: IndexMap<i64, IndexMap<String, Vec<String>>> = IndexMap::new();
        for (id, &(x, y)) in spatial {
            let group = db
                .nodes
                .get(id)
                .and_then(|n| n.parent.clone())
                .unwrap_or_else(|| "default".to_string());
            horizontal
                .entry(y)
                .or_default()
                .entry(group.clone())
                .or_default()
                .push(id.clone());
            vertical
                .entry(x)
                .or_default()
                .entry(group)
                .or_default()
                .push(id.clone());
        }
        horizontal_raw.extend(
            flatten(&horizontal, db::Alignment::Horizontal)
                .into_values()
                .filter(|a| a.len() > 1),
        );
        vertical_raw.extend(
            flatten(&vertical, db::Alignment::Vertical)
                .into_values()
                .filter(|a| a.len() > 1),
        );
    }
    let declared: Vec<&String> = db
        .layout_hints
        .iter()
        .flat_map(|h| h.members.iter())
        .collect();
    let keep = |group: &Vec<String>| !group.iter().any(|id| declared.contains(&id));
    let mut horizontal: Vec<Vec<String>> = horizontal_raw.into_iter().filter(keep).collect();
    let mut vertical: Vec<Vec<String>> = vertical_raw.into_iter().filter(keep).collect();
    for hint in &db.layout_hints {
        if hint.members.len() < 2 {
            continue;
        }
        if hint.row {
            horizontal.push(hint.members.clone());
        } else {
            vertical.push(hint.members.clone());
        }
    }
    AlignmentConstraint {
        vertical,
        horizontal,
    }
}

/// `getRelativeConstraints(spatialMaps, db, layoutHints)`.
pub fn relative_constraints(
    db: &ArchitectureDb,
    ds: &db::DataStructures,
    cfg: &ArchConfig,
) -> Vec<RelativePlacement> {
    let gap = cfg.ideal_edge_length_multiplier * cfg.icon_size;
    let mut constraints = Vec::new();
    let mut declared_pairs: Vec<(String, String)> = Vec::new();
    for hint in &db.layout_hints {
        for pair in hint.members.windows(2) {
            let (a, b) = (pair[0].clone(), pair[1].clone());
            declared_pairs.push((a.clone(), b.clone()));
            declared_pairs.push((b.clone(), a.clone()));
            constraints.push(if hint.row {
                RelativePlacement::Horizontal {
                    left: a,
                    right: b,
                    gap,
                }
            } else {
                RelativePlacement::Vertical {
                    top: a,
                    bottom: b,
                    gap,
                }
            });
        }
    }
    let directions: [(char, (i64, i64)); 4] =
        [('L', (-1, 0)), ('R', (1, 0)), ('T', (0, 1)), ('B', (0, -1))];
    for spatial in &ds.spatial_maps {
        let mut inverse: IndexMap<(i64, i64), String> = IndexMap::new();
        for (id, &pos) in spatial {
            inverse.insert(pos, id.clone());
        }
        let mut queue = std::collections::VecDeque::from([(0i64, 0i64)]);
        let mut visited: Vec<(i64, i64)> = Vec::new();
        while let Some(curr) = queue.pop_front() {
            if !visited.contains(&curr) {
                visited.push(curr);
            }
            let Some(curr_id) = inverse.get(&curr).cloned() else {
                continue;
            };
            for (dir, (dx, dy)) in directions {
                let new_pos = (curr.0 + dx, curr.1 + dy);
                let Some(new_id) = inverse.get(&new_pos).cloned() else {
                    continue;
                };
                if visited.contains(&new_pos) {
                    continue;
                }
                queue.push_back(new_pos);
                if declared_pairs.contains(&(curr_id.clone(), new_id.clone())) {
                    continue;
                }
                constraints.push(match dir {
                    'L' => RelativePlacement::Horizontal {
                        left: new_id,
                        right: curr_id.clone(),
                        gap,
                    },
                    'R' => RelativePlacement::Horizontal {
                        left: curr_id.clone(),
                        right: new_id,
                        gap,
                    },
                    'T' => RelativePlacement::Vertical {
                        top: new_id,
                        bottom: curr_id.clone(),
                        gap,
                    },
                    _ => RelativePlacement::Vertical {
                        top: curr_id.clone(),
                        bottom: new_id,
                        gap,
                    },
                });
            }
        }
    }
    constraints
}

/// The label size cytoscape measures: `ceil(measureText(text).width)`
/// and one line of `font-size`.
pub type MeasureLabel<'a> = &'a dyn Fn(&str) -> f64;

/// Builds the cytoscape graph mermaid builds: groups, then services, then
/// junctions, then edges, every node at the origin. An edge's id is
/// `${lhsId}-${rhsId}`, and cytoscape throws on an id it already has.
pub fn build(
    db: &ArchitectureDb,
    cfg: &ArchConfig,
    measure: MeasureLabel<'_>,
) -> Result<Cy, HelperError> {
    let mut nodes: Vec<CyNode> = Vec::new();
    let mut index: IndexMap<String, usize> = IndexMap::new();
    let mut parents: Vec<Option<String>> = Vec::new();
    for g in db.groups.values() {
        index.insert(g.id.clone(), nodes.len());
        parents.push(g.parent.clone());
        nodes.push(CyNode {
            id: g.id.clone(),
            kind: CyKind::Group,
            parent: None,
            children: Vec::new(),
            pos: (0.0, 0.0),
            width: CYTOSCAPE_NODE_SIZE,
            height: CYTOSCAPE_NODE_SIZE,
            padding: cfg.padding,
            label: None,
        });
    }
    for kind in [NodeType::Service, NodeType::Junction] {
        for n in db.nodes.values().filter(|n| n.kind == kind) {
            index.insert(n.id.clone(), nodes.len());
            parents.push(n.parent.clone());
            let label = n.title.as_ref().map(|t| (measure(t), cfg.font_size));
            nodes.push(CyNode {
                id: n.id.clone(),
                kind: if kind == NodeType::Service {
                    CyKind::Service
                } else {
                    CyKind::Junction
                },
                parent: None,
                children: Vec::new(),
                pos: (0.0, 0.0),
                width: cfg.icon_size,
                height: cfg.icon_size,
                padding: 0.0,
                label,
            });
        }
    }
    for (i, parent) in parents.iter().enumerate() {
        if let Some(p) = parent.as_ref().and_then(|p| index.get(p)).copied() {
            nodes[i].parent = Some(p);
            nodes[p].children.push(i);
        }
    }
    let mut edge_ids: Vec<String> = Vec::new();
    let mut edges = Vec::new();
    for e in &db.edges {
        let id = format!("{}-{}", e.lhs_id, e.rhs_id);
        if index.contains_key(&id) || edge_ids.contains(&id) {
            return Err(HelperError::Internal(format!(
                "Can not create second element with ID `{id}`"
            )));
        }
        edge_ids.push(id);
        let (Some(&source), Some(&target)) = (index.get(&e.lhs_id), index.get(&e.rhs_id)) else {
            continue;
        };
        edges.push(CyEdge {
            source,
            target,
            source_dir: e.lhs_dir,
            target_dir: e.rhs_dir,
            segments: db::is_xy(e.lhs_dir, e.rhs_dir),
            weight: 0.0,
            distance: 0.5,
            // `edge[label]`: the default 16 px font.
            label: e.title.as_ref().map(|t| (measure(t), 16.0)),
        });
    }
    Ok(Cy {
        nodes,
        edges,
        index,
    })
}

/// What `layoutArchitecture` hands fcose besides the node positions.
#[derive(Debug, Clone)]
pub struct FcoseInput {
    pub edges: Vec<fcose::Edge>,
    pub options: Options,
    /// `seed >>> 0`.
    pub seed: u32,
}

/// The fcose options of `layoutArchitecture`: the constraints from the
/// BFS grid and the `align` hints, and per edge an ideal length and
/// elasticity that depend on whether its ends share a group.
pub fn fcose_input(db: &ArchitectureDb, cfg: &ArchConfig, cy: &Cy) -> FcoseInput {
    let ds = db.data_structures();
    let alignment = alignments(db, &ds);
    let relative = relative_constraints(db, &ds, cfg);
    let options = Options {
        num_iter: cfg.num_iter,
        alignment: Some(alignment),
        relative,
        ..Options::default()
    };
    let same_group_length = cfg.ideal_edge_length_multiplier * cfg.icon_size;
    let cross_group_length = 0.5 * cfg.icon_size;
    let edges = cy
        .edges
        .iter()
        .map(|e| {
            let same = cy.nodes[e.source].parent == cy.nodes[e.target].parent;
            fcose::Edge {
                source: e.source,
                target: e.target,
                ideal_length: if same {
                    same_group_length
                } else {
                    cross_group_length
                },
                elasticity: if same { cfg.edge_elasticity } else { 1e-3 },
            }
        })
        .collect();
    FcoseInput {
        edges,
        options,
        seed: js_to_uint32(cfg.seed),
    }
}

/// The cytoscape nodes as fcose reads them: position, `layoutDimensions`,
/// and `parseInt(ele.css('padding'))`.
pub fn fcose_nodes(cy: &Cy) -> Vec<fcose::Node> {
    (0..cy.nodes.len())
        .map(|n| {
            let node = &cy.nodes[n];
            let (x, y) = cy.position(n);
            let (width, height) = cy.outer_size(n);
            fcose::Node {
                id: node.id.clone(),
                parent: node.parent,
                x,
                y,
                width,
                height,
                padding: node.padding.trunc(),
            }
        })
        .collect()
}

/// The `getSegmentWeights` helper of the `layoutstop` handler.
fn segment_weights(s: (f64, f64), t: (f64, f64), point: (f64, f64)) -> (f64, f64) {
    let (sx, sy) = s;
    let (tx, ty) = t;
    let (px, py) = point;
    let slope = (sy - ty) / (sx - tx);
    let mut d = (py - sy + (sx - px) * (sy - ty) / (sx - tx)) / (1.0 + slope * slope).sqrt();
    let mut w = ((py - sy) * (py - sy) + (px - sx) * (px - sx) - d * d).sqrt();
    let dist_ab = ((tx - sx) * (tx - sx) + (ty - sy) * (ty - sy)).sqrt();
    w /= dist_ab;
    let delta1 = if (tx - sx) * (py - sy) - (ty - sy) * (px - sx) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let delta2 = if (tx - sx) * (px - sx) + (ty - sy) * (py - sy) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    d = d.abs() * delta1;
    w *= delta2;
    (d, w)
}

/// `layoutArchitecture`: both fcose runs and the segment styles between
/// them. Returns the laid out graph.
pub fn layout(
    db: &ArchitectureDb,
    cfg: &ArchConfig,
    measure: MeasureLabel<'_>,
) -> Result<Cy, HelperError> {
    let mut cy = build(db, cfg, measure)?;
    let input = fcose_input(db, cfg, &cy);
    let run = |cy: &mut Cy| -> Result<(), HelperError> {
        let center = cy.elements_center();
        let nodes = fcose_nodes(cy);
        let mut random = seeded_random(input.seed);
        let positions = fcose::run(
            &nodes,
            &input.edges,
            &input.options,
            Some(center),
            &mut random,
        )
        .map_err(layout_error)?;
        for (n, p) in positions.into_iter().enumerate() {
            if cy.nodes[n].children.is_empty() {
                cy.nodes[n].pos = p;
            }
        }
        Ok(())
    };

    run(&mut cy)?;
    // The first run's `layoutstop`: bend points for edges whose ends differ
    // on both axes.
    let mut styles = Vec::new();
    for (i, e) in cy.edges.iter().enumerate() {
        let (sx, sy) = cy.position(e.source);
        let (tx, ty) = cy.position(e.target);
        if sx != tx && sy != ty {
            let p = cy.edge_points(e);
            let point = if db::is_y(e.source_dir) {
                (p.start.0, p.end.1)
            } else {
                (p.end.0, p.start.1)
            };
            let (d, w) = segment_weights(p.start, p.end, point);
            if d.is_finite() && w.is_finite() {
                styles.push((i, w, d));
            }
        }
    }
    for (i, w, d) in styles {
        cy.edges[i].weight = w;
        cy.edges[i].distance = d;
    }
    run(&mut cy)?;
    Ok(cy)
}

/// The layout errors `layoutArchitecture` throws during `draw`; mermaid
/// rewrites the `Invalid array length` one.
fn layout_error(e: FcoseError) -> HelperError {
    match e {
        FcoseError::InvalidArrayLength => HelperError::Internal(
            "Architecture layout failed: a declared `align row|column` directive likely contradicts the edge directions, or two declared alignments overlap on a shared node. Check that the order of members in each `align` chain is consistent with the edges between them, and that no node appears in two `align` directives along the same axis.".to_string(),
        ),
        FcoseError::Internal(message) => HelperError::Internal(message),
    }
}
