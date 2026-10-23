//! cytoscape-fcose 2.2.0 on cose-base 2.2.0 and layout-base 2.0.1, the
//! force-directed layout mermaid 12.2.0's architecture diagram runs.
//!
//! The port covers the path `layoutArchitecture` takes: `quality: "proof"`,
//! `randomize: false` (so no spectral step and an incremental CoSE run from
//! the current positions), no tiling and no tree reduction (both are off
//! whenever a constraint object is passed, and mermaid always passes an
//! `alignmentConstraint`), no component packing (mermaid registers no
//! `layoutUtilities`), and the alignment and relative placement
//! constraints without fixed nodes. Every statement on that path is ported
//! in its JS order, because the force sums are order-dependent floating
//! point.
//!
//! `Math.random` is a parameter: fcose calls it only to shuffle the
//! relative placement queues (`CoSELayout.prototype.initConstraintVariables`),
//! and mermaid swaps in a seeded generator around each run.
//!
//! What cytoscape contributes is the caller's job: node order
//! (`eles.nodes()`), compound padding (`ele.css('padding')`), leaf sizes
//! (`layoutDimensions`), current positions, and the centre of
//! `eles.boundingBox()` that `relocateComponent` moves the result to.

mod constraints;
pub mod graph;
mod layout;

use std::fmt;

/// One cytoscape node, in `eles.nodes()` order.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    /// Index of the parent compound in the node list.
    pub parent: Option<usize>,
    /// `position()`: the centre.
    pub x: f64,
    pub y: f64,
    /// `layoutDimensions()`: the outer size. Unused for a compound with
    /// children, whose rectangle CoSE derives from them.
    pub width: f64,
    pub height: f64,
    /// `parseInt(ele.css('padding'))`.
    pub padding: f64,
}

/// One cytoscape edge, in `eles.edges()` order.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub source: usize,
    pub target: usize,
    /// `options.idealEdgeLength(edge)`.
    pub ideal_length: f64,
    /// `options.edgeElasticity(edge)`.
    pub elasticity: f64,
}

/// `alignmentConstraint`: node ids sharing an x (`vertical`) or a y
/// (`horizontal`). An empty list is the `undefined` fcose turns it into.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AlignmentConstraint {
    pub vertical: Vec<Vec<String>>,
    pub horizontal: Vec<Vec<String>>,
}

/// One `relativePlacementConstraint` entry.
#[derive(Debug, Clone, PartialEq)]
pub enum RelativePlacement {
    /// `{ left, right, gap }`: `left` lies at least `gap` left of `right`.
    Horizontal {
        left: String,
        right: String,
        gap: f64,
    },
    /// `{ top, bottom, gap }`.
    Vertical {
        top: String,
        bottom: String,
        gap: f64,
    },
}

/// The fcose options mermaid sets or leaves at their defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// `nodeRepulsion` (default `node => 4500`).
    pub node_repulsion: f64,
    /// `nestingFactor`.
    pub nesting_factor: f64,
    /// `gravity`.
    pub gravity: f64,
    /// `numIter`.
    pub num_iter: f64,
    /// `gravityRange`.
    pub gravity_range: f64,
    /// `gravityCompound`.
    pub gravity_compound: f64,
    /// `gravityRangeCompound`.
    pub gravity_range_compound: f64,
    /// `initialEnergyOnIncremental`.
    pub initial_energy_on_incremental: f64,
    /// `alignmentConstraint`; `None` is `undefined`.
    pub alignment: Option<AlignmentConstraint>,
    /// `relativePlacementConstraint`; empty is `undefined`.
    pub relative: Vec<RelativePlacement>,
}

impl Default for Options {
    /// The fcose `defaults` object.
    fn default() -> Options {
        Options {
            node_repulsion: 4500.0,
            nesting_factor: 0.1,
            gravity: 0.25,
            num_iter: 2500.0,
            gravity_range: 3.8,
            gravity_compound: 1.0,
            gravity_range_compound: 1.5,
            initial_energy_on_incremental: 0.3,
            alignment: None,
            relative: Vec::new(),
        }
    }
}

/// A layout that cannot finish. In JS these surface as exceptions from
/// fcose; mermaid rethrows the `Invalid array length` one with its own
/// message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FcoseError {
    /// `new Array(n)` in `calcGrid` with a size that is not a valid array
    /// length (positions became infinite or NaN, which a contradictory
    /// relative placement cycle causes).
    InvalidArrayLength,
    /// An exception the JS would throw on a malformed input, such as an
    /// unknown node id in a constraint.
    Internal(String),
}

impl fmt::Display for FcoseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FcoseError::InvalidArrayLength => f.write_str("Invalid array length"),
            FcoseError::Internal(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for FcoseError {}

/// `Layout.prototype.run` of fcose for one connected run: builds the CoSE
/// graph (`coseLayout`), runs it, relocates the result to `center` when
/// given (`aux.relocateComponent`), and returns the new centre of every
/// input node. Compounds get their CoSE rectangle's centre; cytoscape
/// recomputes theirs from the children.
pub fn run(
    nodes: &[Node],
    edges: &[Edge],
    options: &Options,
    center: Option<(f64, f64)>,
    random: &mut dyn FnMut() -> f64,
) -> Result<Vec<(f64, f64)>, FcoseError> {
    let mut out: Vec<(f64, f64)> = nodes.iter().map(|n| (n.x, n.y)).collect();
    if nodes.is_empty() {
        return Ok(out);
    }
    let mut cose = layout::CoseLayout::build(nodes, edges, options)?;
    cose.run_layout(random)?;
    let gm = &mut cose.gm;
    if let Some((cx, cy)) = center {
        // `aux.relocateComponent` over every layout node, compounds included.
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for n in gm.nodes.iter().filter(|n| n.input.is_some()) {
            let r = n.rect;
            if r.x < min_x {
                min_x = r.x;
            }
            if r.right() > max_x {
                max_x = r.right();
            }
            if r.y < min_y {
                min_y = r.y;
            }
            if r.bottom() > max_y {
                max_y = r.bottom();
            }
        }
        let dx = cx - (max_x + min_x) / 2.0;
        let dy = cy - (max_y + min_y) / 2.0;
        for n in gm.nodes.iter_mut().filter(|n| n.input.is_some()) {
            let (x, y) = (n.center_x() + dx, n.center_y() + dy);
            n.set_center(x, y);
        }
    }
    for n in gm.nodes.iter() {
        if let Some(i) = n.input {
            out[i] = (n.center_x(), n.center_y());
        }
    }
    Ok(out)
}
