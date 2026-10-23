//! `sankeyDB.ts`: nodes in first-seen order, unique by sanitized id, and
//! the links in input order.

use std::collections::HashMap;

use crate::mermaid::diagrams::xychart::db::sanitize_text;
use crate::mermaid::layout::d3::sankey::LinkInput;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SankeyDb {
    /// `nodes`: the `ID` of each `SankeyNode`, in creation order.
    nodes: Vec<String>,
    /// `nodesMap`.
    by_id: HashMap<String, usize>,
    /// `links`: source and target node indices and the value.
    links: Vec<(usize, usize, f64)>,
}

impl SankeyDb {
    pub fn new() -> SankeyDb {
        SankeyDb::default()
    }

    /// `findOrCreateNode`: `common.sanitizeText(ID, getConfig())`, then the
    /// existing node or a new one at the end.
    pub fn find_or_create_node(&mut self, id: &str) -> usize {
        let id = sanitize_text(id);
        if let Some(&i) = self.by_id.get(&id) {
            return i;
        }
        let i = self.nodes.len();
        self.by_id.insert(id.clone(), i);
        self.nodes.push(id);
        i
    }

    /// `addLink`.
    pub fn add_link(&mut self, source: usize, target: usize, value: f64) {
        self.links.push((source, target, value));
    }

    pub fn nodes(&self) -> &[String] {
        &self.nodes
    }

    pub fn links(&self) -> &[(usize, usize, f64)] {
        &self.links
    }

    /// `getGraph`: node ids and links by id, the input d3-sankey takes.
    pub fn graph(&self) -> (Vec<String>, Vec<LinkInput>) {
        let links = self
            .links
            .iter()
            .map(|&(s, t, value)| LinkInput {
                source: self.nodes[s].clone(),
                target: self.nodes[t].clone(),
                value,
            })
            .collect();
        (self.nodes.clone(), links)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodes_keep_first_seen_order() {
        let mut db = SankeyDb::new();
        let a = db.find_or_create_node("a");
        let b = db.find_or_create_node("b");
        assert_eq!(db.find_or_create_node("a"), a);
        db.add_link(a, b, 2.5);
        assert_eq!(db.nodes(), ["a", "b"]);
        let (ids, links) = db.graph();
        assert_eq!(ids, ["a", "b"]);
        assert_eq!(links[0].source, "a");
        assert_eq!(links[0].value, 2.5);
    }
}
