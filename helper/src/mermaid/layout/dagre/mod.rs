//! 1:1 port of dagre-d3-es 7.0.14 `src/dagre`, the layout engine mermaid
//! 12.2.0 runs for flowchart, class, state, ER, requirement, and usecase.
//!
//! Every function names the JS file and function it ports. The port keeps the
//! JS operation order on `f64` so coordinates match to 1e-6, keeps JS object
//! iteration order through [`graphlib`], and keeps lodash semantics where
//! they leak into results (`_.min`/`_.max` skip `undefined` and `NaN`,
//! `_.minBy` keeps the first minimum, `_.sortBy` is stable, `Math.max`
//! propagates `NaN`).
//!
//! Dummy node ids come from a counter that mirrors lodash's module-global
//! `_.uniqueId`. mermaid's bundle calls `uniqueId` only from dagre and lays
//! out cluster sub-graphs with the same counter inside one render, so the glue
//! calls [`reset_unique_ids`] once per diagram, before the first layout.

pub mod acyclic;
pub mod add_border_segments;
pub mod coordinate_system;
pub mod graphlib;
pub mod greedy_fas;
pub mod layout;
pub mod nesting_graph;
pub mod normalize;
pub mod order;
pub mod parent_dummy_chains;
pub mod position;
pub mod rank;
pub mod util;

use std::collections::BTreeMap;

pub use graphlib::{EdgeKey, Graph, GraphOptions};
pub use layout::{LayoutError, layout};
pub use util::{reset_unique_ids, set_unique_id_counter, unique_id_counter};

/// The graph type `dagre.layout` takes and returns.
pub type LayoutGraph = Graph<NodeLabel, EdgeLabel, GraphLabel>;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// The `dummy` attribute dagre puts on helper nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dummy {
    Root,
    EdgeProxy,
    Edge,
    EdgeLabel,
    Border,
    SelfEdge,
}

/// The `borderType` attribute of a border segment node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderType {
    BorderLeft,
    BorderRight,
}

/// One entry of `node.selfEdges` from `layout.js` `removeSelfEdges`.
#[derive(Clone, Debug)]
pub struct SelfEdge {
    pub e: EdgeKey,
    pub label: EdgeLabel,
}

/// Graph label: `graphDefaults` and the attribute lists in `layout.js`
/// `buildLayoutGraph`, plus the attributes dagre adds while it runs.
///
/// `rankdir` is kept verbatim because `makeSpaceForEdgeLabels` compares it
/// case-sensitively (`'TB'`, `'BT'`) while `coordinate-system.js` lowercases
/// it; mermaid passes upper case.
#[derive(Clone, Debug)]
pub struct GraphLabel {
    pub rankdir: String,
    pub align: Option<String>,
    pub acyclicer: Option<String>,
    pub ranker: Option<String>,
    pub nodesep: f64,
    pub edgesep: f64,
    pub ranksep: f64,
    pub marginx: f64,
    pub marginy: f64,
    /// Output.
    pub width: f64,
    /// Output.
    pub height: f64,
    /// Set by `nesting-graph.js run`, removed by `cleanup`.
    pub nesting_root: Option<String>,
    /// Set by `nesting-graph.js run`, read by `util.js removeEmptyRanks`.
    pub node_rank_factor: Option<f64>,
    /// Set by `normalize.js run`, read by `parent-dummy-chains.js` and
    /// `normalize.js undo`.
    pub dummy_chains: Vec<String>,
}

impl Default for GraphLabel {
    fn default() -> Self {
        Self {
            rankdir: "tb".to_string(),
            align: None,
            acyclicer: None,
            ranker: None,
            nodesep: 50.0,
            edgesep: 20.0,
            ranksep: 50.0,
            marginx: 0.0,
            marginy: 0.0,
            width: 0.0,
            height: 0.0,
            nesting_root: None,
            node_rank_factor: None,
            dummy_chains: Vec::new(),
        }
    }
}

/// Node label: `nodeDefaults` from `layout.js` plus every attribute dagre
/// writes on a node while it runs. Input: `width`, `height`. Output: `x`,
/// `y`, and for a compound node `width` and `height`.
///
/// `rank` is `f64` because an edge label proxy sits at `(w.rank - v.rank) / 2
/// + v.rank`, which can be fractional; `min_rank` and `max_rank` follow.
#[derive(Clone, Debug, Default)]
pub struct NodeLabel {
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
    pub rank: Option<f64>,
    pub order: usize,
    pub dummy: Option<Dummy>,
    pub border_type: Option<BorderType>,
    pub min_rank: Option<f64>,
    pub max_rank: Option<f64>,
    pub border_top: Option<String>,
    pub border_bottom: Option<String>,
    /// `node.borderLeft[rank]` from `add-border-segments.js`.
    pub border_left: BTreeMap<i64, String>,
    pub border_right: BTreeMap<i64, String>,
    pub self_edges: Vec<SelfEdge>,
    /// `e` of an edge proxy or self-edge dummy, `edgeObj` of an edge dummy.
    pub edge_obj: Option<EdgeKey>,
    /// `edgeLabel` of the first dummy in a chain, `label` of a self-edge
    /// dummy. JS shares one label object along the chain; the port keeps it
    /// on the chain head only, the node `normalize.js undo` reads it from.
    pub edge_label: Option<EdgeLabel>,
    /// `labelpos` copied onto an `edge-label` dummy; `position/bk.js sep`
    /// tests `hasOwnProperty`, so absence matters.
    pub labelpos: Option<String>,
}

/// Edge label: `edgeDefaults` from `layout.js` plus the attributes dagre
/// writes. Input: `minlen`, `weight`, `width`, `height`, `labelpos`,
/// `labeloffset`. Output: `points`, and `x`, `y` when the edge has a label.
///
/// `points` empty stands for the JS `undefined`; `assignNodeIntersects`
/// tests `!edge.points`, and every path that sets `points` leaves at least
/// one point. `x`/`y` are `Option` because `hasOwnProperty(edge, 'x')`
/// decides whether an edge takes part in `translateGraph`.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeLabel {
    pub minlen: f64,
    pub weight: f64,
    pub width: f64,
    pub height: f64,
    pub labeloffset: f64,
    pub labelpos: String,
    pub points: Vec<Point>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// Set by `layout.js removeEdgeLabelProxies`.
    pub label_rank: Option<f64>,
    /// Set by `acyclic.js run`.
    pub reversed: bool,
    pub forward_name: Option<String>,
    /// Set by `nesting-graph.js run`.
    pub nesting_edge: bool,
}

impl Default for EdgeLabel {
    fn default() -> Self {
        Self {
            minlen: 1.0,
            weight: 1.0,
            width: 0.0,
            height: 0.0,
            labeloffset: 10.0,
            labelpos: "r".to_string(),
            points: Vec::new(),
            x: None,
            y: None,
            label_rank: None,
            reversed: false,
            forward_name: None,
            nesting_edge: false,
        }
    }
}

impl EdgeLabel {
    /// `{ weight }`: the label dagre gives a dummy chain segment.
    pub fn with_weight(weight: f64) -> Self {
        Self {
            weight,
            ..Self::default()
        }
    }

    /// `{ weight, minlen }`: the label of a nesting or simplified edge.
    pub fn weight_minlen(weight: f64, minlen: f64) -> Self {
        Self {
            weight,
            minlen,
            ..Self::default()
        }
    }
}
