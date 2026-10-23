//! Port of `diagrams/mindmap/mindmapDb.ts` (mermaid 12.2.0): the node tree
//! the grammar actions fill by indentation, `decorateNode`, and `getData`,
//! the unified layout data with section classes and depth classes.

use std::fmt;

use crate::mermaid::edges::{Edge, Pattern, Thickness};
use crate::mermaid::shapes::Node;

/// `MAX_SECTIONS` of `svgDraw.ts`: the sections cycle through
/// `MAX_SECTIONS - 1` colors.
pub const MAX_SECTIONS: i64 = 12;

/// The errors `addNode` and `decorateNode` throw. mermaid reports them as
/// the diagram's error, not as a parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MindmapError {
    /// `addNode` found no parent for a node that is not the first one.
    NoParent(String),
    /// `decorateNode` ran before any node existed: the JavaScript
    /// dereferences `nodes[-1]`.
    NoNodeToDecorate(String),
}

impl fmt::Display for MindmapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MindmapError::NoParent(descr) => write!(
                f,
                "There can be only one root. No parent could be found for (\"{descr}\")"
            ),
            MindmapError::NoNodeToDecorate(field) => write!(
                f,
                "TypeError: Cannot set properties of undefined (setting '{field}')"
            ),
        }
    }
}

impl std::error::Error for MindmapError {}

/// `nodeType` of `mindmapDb.ts`. `NO_BORDER` shares 0 with `DEFAULT`.
pub mod node_type {
    pub const DEFAULT: u8 = 0;
    pub const ROUNDED_RECT: u8 = 1;
    pub const RECT: u8 = 2;
    pub const CIRCLE: u8 = 3;
    pub const CLOUD: u8 = 4;
    pub const BANG: u8 = 5;
    pub const HEXAGON: u8 = 6;
}

/// `getType(startStr, endStr)`.
pub fn get_type(start: &str, end: &str) -> u8 {
    match start {
        "[" => node_type::RECT,
        "(" => {
            if end == ")" {
                node_type::ROUNDED_RECT
            } else {
                node_type::CLOUD
            }
        }
        "((" => node_type::CIRCLE,
        ")" => node_type::CLOUD,
        "))" => node_type::BANG,
        "{{" => node_type::HEXAGON,
        _ => node_type::DEFAULT,
    }
}

/// `MindmapNode` of `mindmapTypes.ts`.
#[derive(Debug, Clone, PartialEq)]
pub struct MindmapNode {
    /// `this.count++`: the insertion index.
    pub id: usize,
    pub node_id: String,
    /// The indentation relative to the root's; the root is 0. Negative
    /// values are possible and end in [`MindmapError::NoParent`].
    pub level: i64,
    pub descr: String,
    pub node_type: u8,
    /// Indices into the db's node list.
    pub children: Vec<usize>,
    /// `mindmap.maxNodeWidth`.
    pub width: f64,
    /// `mindmap.padding`, doubled for rect, rounded rect, and hexagon.
    pub padding: f64,
    pub section: Option<i64>,
    pub class: Option<String>,
    pub icon: Option<String>,
    pub is_root: bool,
}

/// The `mindmap` config values `addNode` reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeDefaults {
    pub padding: f64,
    pub max_node_width: f64,
}

impl Default for NodeDefaults {
    /// `defaultConfig.mindmap`.
    fn default() -> NodeDefaults {
        NodeDefaults {
            padding: 10.0,
            max_node_width: 200.0,
        }
    }
}

/// `getData()` without the config: the nodes and edges in tree order.
#[derive(Debug, Clone, Default)]
pub struct MindmapData {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Default)]
pub struct MindmapDb {
    nodes: Vec<MindmapNode>,
    base_level: Option<i64>,
    defaults: NodeDefaults,
}

impl MindmapDb {
    pub fn new(defaults: NodeDefaults) -> MindmapDb {
        MindmapDb {
            defaults,
            ..MindmapDb::default()
        }
    }

    pub fn nodes(&self) -> &[MindmapNode] {
        &self.nodes
    }

    /// `getParent(level)`: the last node shallower than `level`.
    fn get_parent(&self, level: i64) -> Option<usize> {
        self.nodes.iter().rposition(|n| n.level < level)
    }

    /// `getMindmap()`: the first node.
    pub fn root(&self) -> Option<&MindmapNode> {
        self.nodes.first()
    }

    /// `addNode(level, id, descr, type)`.
    pub fn add_node(
        &mut self,
        level: usize,
        id: &str,
        descr: &str,
        node_type: u8,
    ) -> Result<(), MindmapError> {
        let mut level = level as i64;
        let is_root = if self.nodes.is_empty() {
            self.base_level = Some(level);
            level = 0;
            true
        } else {
            if let Some(base) = self.base_level {
                level -= base;
            }
            false
        };
        let mut padding = self.defaults.padding;
        if matches!(
            node_type,
            node_type::ROUNDED_RECT | node_type::RECT | node_type::HEXAGON
        ) {
            padding *= 2.0;
        }
        let node = MindmapNode {
            id: self.nodes.len(),
            node_id: id.to_string(),
            level,
            descr: descr.to_string(),
            node_type,
            children: Vec::new(),
            width: self.defaults.max_node_width,
            padding,
            section: None,
            class: None,
            icon: None,
            is_root,
        };
        let index = self.nodes.len();
        match self.get_parent(level) {
            Some(parent) => {
                self.nodes[parent].children.push(index);
                self.nodes.push(node);
            }
            None if is_root => self.nodes.push(node),
            None => return Err(MindmapError::NoParent(node.descr)),
        }
        Ok(())
    }

    /// `decorateNode({ icon })`. An empty icon is falsy and ignored.
    pub fn decorate_icon(&mut self, icon: &str) -> Result<(), MindmapError> {
        if icon.is_empty() {
            return Ok(());
        }
        let node = self
            .nodes
            .last_mut()
            .ok_or_else(|| MindmapError::NoNodeToDecorate("icon".to_string()))?;
        node.icon = Some(icon.to_string());
        Ok(())
    }

    /// `decorateNode({ class })`.
    pub fn decorate_class(&mut self, class: &str) -> Result<(), MindmapError> {
        if class.is_empty() {
            return Ok(());
        }
        let node = self
            .nodes
            .last_mut()
            .ok_or_else(|| MindmapError::NoNodeToDecorate("class".to_string()))?;
        node.class = Some(class.to_string());
        Ok(())
    }

    /// `assignSections(node, sectionNumber, depth, colorByDepth)`.
    fn assign_sections(
        &mut self,
        index: usize,
        section: Option<i64>,
        depth: i64,
        color_by_depth: bool,
    ) {
        let level = self.nodes[index].level;
        self.nodes[index].section = if level == 0 {
            None
        } else if color_by_depth {
            // JS `%` keeps the sign; depth is at least 1 here.
            Some((depth - 1) % (MAX_SECTIONS - 1))
        } else {
            section
        };
        let children = self.nodes[index].children.clone();
        for (i, child) in children.into_iter().enumerate() {
            let child_section = if level == 0 {
                Some(i as i64 % (MAX_SECTIONS - 1))
            } else {
                section
            };
            self.assign_sections(child, child_section, depth + 1, color_by_depth);
        }
    }

    /// `getShapeFromType(type)` for the built-in themes (no `redux` theme
    /// renders, so the default node keeps `defaultMindmapNode`).
    fn shape(node_type: u8) -> &'static str {
        match node_type {
            node_type::CIRCLE => "mindmapCircle",
            node_type::RECT => "rect",
            node_type::ROUNDED_RECT => "rounded",
            node_type::CLOUD => "cloud",
            node_type::BANG => "bang",
            node_type::HEXAGON => "hexagon",
            node_type::DEFAULT => "defaultMindmapNode",
            _ => "rect",
        }
    }

    /// `flattenNodes(node, processedNodes)`: pre-order.
    fn flatten(&self, index: usize, out: &mut Vec<Node>) {
        let node = &self.nodes[index];
        let mut classes = vec!["mindmap-node".to_string()];
        if node.is_root {
            classes.push("section-root".to_string());
            classes.push("section--1".to_string());
        } else if let Some(section) = node.section {
            classes.push(format!("section-{section}"));
        }
        if let Some(class) = &node.class {
            classes.push(class.clone());
        }
        out.push(Node {
            id: node.id.to_string(),
            dom_id: Some(format!("node_{}", node.id)),
            label: Some(node.descr.clone()),
            label_type: Some("markdown".to_string()),
            shape: Self::shape(node.node_type).to_string(),
            width: Some(node.width),
            // `height: node.height ?? 0`: falsy, as if unset.
            height: None,
            padding: Some(node.padding),
            css_classes: classes.join(" "),
            center_label: true,
            ..Node::default()
        });
        for &child in &node.children {
            self.flatten(child, out);
        }
    }

    /// `generateEdges(node, edges)`: one edge per child, pre-order.
    fn generate_edges(&self, index: usize, out: &mut Vec<Edge>) {
        let node = &self.nodes[index];
        for &child_index in &node.children {
            let child = &self.nodes[child_index];
            let mut classes = "edge".to_string();
            if let Some(section) = child.section {
                classes.push_str(&format!(" section-edge-{section}"));
            }
            classes.push_str(&format!(" edge-depth-{}", node.level + 1));
            out.push(Edge {
                curve: Some("basis".to_string()),
                thickness: Some(Thickness::Normal),
                pattern: Some(Pattern::Solid),
                classes: Some(classes),
                ..Edge::new(
                    &format!("edge_{}_{}", node.id, child.id),
                    &node.id.to_string(),
                    &child.id.to_string(),
                )
            });
            self.generate_edges(child_index, out);
        }
    }

    /// `getData()`: assigns the sections, then flattens the tree into the
    /// layout nodes and edges. Without a root both lists are empty.
    pub fn get_data(&mut self, color_by_depth: bool) -> MindmapData {
        if self.nodes.is_empty() {
            return MindmapData::default();
        }
        self.assign_sections(0, None, 0, color_by_depth);
        let mut data = MindmapData::default();
        self.flatten(0, &mut data.nodes);
        self.generate_edges(0, &mut data.edges);
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> MindmapDb {
        MindmapDb::new(NodeDefaults::default())
    }

    #[test]
    fn indentation_builds_the_tree_relative_to_the_root() {
        let mut db = db();
        db.add_node(2, "root", "root", node_type::CIRCLE).unwrap();
        db.add_node(4, "a", "A", node_type::DEFAULT).unwrap();
        db.add_node(6, "a1", "A1", node_type::RECT).unwrap();
        db.add_node(4, "b", "B", node_type::DEFAULT).unwrap();
        let nodes = db.nodes();
        assert_eq!(nodes[0].level, 0);
        assert_eq!(nodes[1].level, 2);
        assert_eq!(nodes[0].children, vec![1, 3]);
        assert_eq!(nodes[1].children, vec![2]);
        assert_eq!(nodes[2].padding, 20.0);
        assert_eq!(nodes[0].padding, 10.0);
    }

    #[test]
    fn a_second_root_throws() {
        let mut db = db();
        db.add_node(2, "r", "R", 0).unwrap();
        let err = db.add_node(2, "s", "S", 0).unwrap_err();
        assert_eq!(
            err.to_string(),
            "There can be only one root. No parent could be found for (\"S\")"
        );
    }

    #[test]
    fn sections_follow_the_root_children_and_cycle() {
        let mut db = db();
        db.add_node(0, "r", "R", 0).unwrap();
        for i in 0..12 {
            db.add_node(2, &format!("c{i}"), "C", 0).unwrap();
        }
        db.add_node(4, "g", "G", 0).unwrap();
        let data = db.get_data(false);
        assert_eq!(
            data.nodes[0].css_classes,
            "mindmap-node section-root section--1"
        );
        assert_eq!(data.nodes[1].css_classes, "mindmap-node section-0");
        assert_eq!(data.nodes[11].css_classes, "mindmap-node section-10");
        assert_eq!(data.nodes[12].css_classes, "mindmap-node section-0");
        // The grandchild inherits its branch.
        assert_eq!(data.nodes[13].css_classes, "mindmap-node section-0");
        assert_eq!(
            data.edges[12].classes.as_deref(),
            Some("edge section-edge-0 edge-depth-3")
        );
        assert_eq!(data.edges[12].id, "edge_12_13");
    }

    #[test]
    fn color_by_depth_uses_the_distance_from_the_root() {
        let mut db = db();
        db.add_node(0, "r", "R", 0).unwrap();
        db.add_node(2, "a", "A", 0).unwrap();
        db.add_node(6, "a1", "A1", 0).unwrap();
        db.add_node(2, "b", "B", 0).unwrap();
        let data = db.get_data(true);
        assert_eq!(data.nodes[2].css_classes, "mindmap-node section-1");
        assert_eq!(data.nodes[3].css_classes, "mindmap-node section-0");
    }

    #[test]
    fn decorations_hit_the_last_node() {
        let mut db = db();
        assert!(db.decorate_icon("fa fa-book").is_err());
        db.add_node(0, "r", "R", 0).unwrap();
        db.decorate_icon("fa fa-book").unwrap();
        db.decorate_class("urgent large").unwrap();
        let data = db.get_data(false);
        assert_eq!(
            data.nodes[0].css_classes,
            "mindmap-node section-root section--1 urgent large"
        );
        assert_eq!(db.nodes()[0].icon.as_deref(), Some("fa fa-book"));
    }
}
