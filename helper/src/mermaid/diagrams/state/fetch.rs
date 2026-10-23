//! Port of mermaid 12.2.0 `diagrams/state/dataFetcher.ts` plus the label
//! pass of `StateDB.extract` and `StateDB.getData`: the translated
//! document becomes the unified renderer's nodes and edges.
//!
//! A state is a `rect` (drawn as `roundedRect` because it has `rx` and
//! `ry`), `rectWithTitle` once it has two or more descriptions,
//! `stateStart`/`stateEnd` for a `[*]`, its `type` for fork, join, choice,
//! and divider, and `roundedWithTitle` when it has a document. A note is a
//! `note` node inside an invisible `noteGroup` cluster plus a dashed edge.
//! The `colorIndex` palette slots only paint under `redux-color` themes and
//! the `neo` look, which the helper does not render, so they are left out.

use indexmap::IndexMap;

use super::actions::{Descr, StateStmt, Stmt};
use super::db::{DbState, StateDb, StyleClass, doc_direction};
use crate::mermaid::edges::{Edge, Pattern, Thickness};
use crate::mermaid::shapes::{Dir, Node};

/// `stateCommon.ts` constants.
const CSS_DIAGRAM_STATE: &str = "statediagram-state";
const CSS_DIAGRAM_CLUSTER: &str = "statediagram-cluster";
const CSS_DIAGRAM_CLUSTER_ALT: &str = "statediagram-cluster-alt";
const CSS_DIAGRAM_NOTE: &str = "statediagram-note";
const CSS_EDGE: &str = "transition";
const CSS_EDGE_NOTE_EDGE: &str = "transition note-edge";
const G_EDGE_STYLE: &str = "fill:none";
const NOTE_ID: &str = "----note";
const PARENT_ID: &str = "----parent";

/// What the fetch needs from the config.
#[derive(Debug, Clone)]
pub struct FetchConfig {
    /// The SVG id the DOM ids are prefixed with.
    pub diagram_id: String,
    /// `flowchart.padding`, the note padding.
    pub flowchart_padding: f64,
    /// `state.wrappingWidth` and `state.minNodeWidth`, set by `getData`.
    pub wrapping_width: f64,
    pub min_node_width: f64,
}

/// `nodeDb` entry: the description is a string until a second one
/// arrives.
#[derive(Debug, Clone, PartialEq)]
enum Label {
    Text(String),
    List(Vec<String>),
}

impl Label {
    fn len(&self) -> usize {
        match self {
            Label::Text(s) => s.encode_utf16().count(),
            Label::List(l) => l.len(),
        }
    }
}

#[derive(Debug, Clone)]
struct NodeEntry {
    /// `None` is mermaid's `undefined` shape, which a note on a state not
    /// yet declared leaves behind.
    shape: Option<String>,
    description: Label,
    css_classes: String,
    css_styles: Vec<String>,
    is_group: bool,
    dir: Option<String>,
}

/// The nodes and edges `getData` returns, plus the diagram direction.
#[derive(Debug, Clone, Default)]
pub struct Fetched {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub direction: Dir,
}

struct Fetcher<'a> {
    states: &'a IndexMap<String, DbState>,
    classes: &'a IndexMap<String, StyleClass>,
    config: &'a FetchConfig,
    node_db: IndexMap<String, NodeEntry>,
    graph_item_count: usize,
    nodes: Vec<Node>,
    /// The `label` array a node kept, split by `extract` afterwards.
    labels: IndexMap<String, Label>,
    edges: Vec<Edge>,
}

/// `stateDomId(itemId, counter, type)` with the SVG id in front, as the
/// oracle's DOM shows it.
fn dom_id(diagram_id: &str, item_id: &str, counter: usize, kind: &str) -> String {
    let type_str = if kind.is_empty() {
        String::new()
    } else {
        format!("----{kind}")
    };
    format!("{diagram_id}-state-{item_id}{type_str}-{counter}")
}

fn descr_items(d: &Descr) -> Vec<String> {
    match d {
        Descr::None => Vec::new(),
        Descr::Text(s) => vec![s.clone()],
        Descr::List(l) => l.clone(),
    }
}

impl Fetcher<'_> {
    /// `setupDoc(parent, doc, ..., altFlag)`.
    fn setup_doc(&mut self, parent: Option<&StateStmt>, doc: &[Stmt], alt_flag: bool) {
        for item in doc {
            match item {
                Stmt::State(s) => self.fetch(parent, s, alt_flag),
                Stmt::Relation {
                    state1,
                    state2,
                    description,
                } => {
                    self.fetch(parent, state1, alt_flag);
                    self.fetch(parent, state2, alt_flag);
                    let id = format!("edge{}", self.graph_item_count);
                    self.edges.push(Edge {
                        arrow_type_end: Some("arrow_barb".to_string()),
                        style: vec![G_EDGE_STYLE.to_string()],
                        label: Some(description.clone().unwrap_or_default()),
                        labelpos: Some("c".to_string()),
                        label_type: Some("markdown".to_string()),
                        thickness: Some(Thickness::Normal),
                        classes: Some(CSS_EDGE.to_string()),
                        ..Edge::new(&id, &state1.id, &state2.id)
                    });
                    self.graph_item_count += 1;
                }
                _ => {}
            }
        }
    }

    /// `insertOrUpdateNode(nodes, nodeData, classes)`.
    fn insert_or_update(&mut self, mut node: Node, label: Option<Label>) {
        if node.id.is_empty() || node.id == "</join></fork>" || node.id == "</choice>" {
            return;
        }
        if !node.css_classes.is_empty() {
            for class in node.css_classes.split(' ') {
                if let Some(def) = self.classes.get(class) {
                    node.css_compiled_styles.extend(def.styles.iter().cloned());
                }
            }
        }
        if let Some(label) = label {
            self.labels.insert(node.id.clone(), label);
        }
        match self.nodes.iter_mut().find(|n| n.id == node.id) {
            // `Object.assign` keeps the properties the new data lacks.
            Some(existing) => {
                if node.parent_id.is_none() {
                    node.parent_id = existing.parent_id.take();
                }
                *existing = node;
            }
            None => self.nodes.push(node),
        }
    }

    /// `dataFetcher(parent, parsedItem, ..., altFlag)`.
    fn fetch(&mut self, parent: Option<&StateStmt>, item: &StateStmt, alt_flag: bool) {
        let item_id = item.id.clone();
        let db_state = self.states.get(&item_id);
        let class_str = db_state.map(|s| s.classes.join(" ")).unwrap_or_default();
        let style = db_state.map(|s| s.styles.clone()).unwrap_or_default();

        if item_id != "root" {
            let mut shape = Some(match item.start {
                Some(true) => "stateStart".to_string(),
                Some(false) => "stateEnd".to_string(),
                None => "rect".to_string(),
            });
            if item.kind.as_deref() != Some("default") {
                shape = item.kind.clone();
            }
            let entry = self
                .node_db
                .entry(item_id.clone())
                .or_insert_with(|| NodeEntry {
                    shape,
                    description: Label::Text(item_id.clone()),
                    css_classes: format!("{class_str} {CSS_DIAGRAM_STATE}"),
                    css_styles: style,
                    is_group: false,
                    dir: None,
                });

            if item.description.truthy() {
                let incoming = descr_items(&item.description);
                entry.description = match &entry.description {
                    Label::List(list) => {
                        entry.shape = Some("rectWithTitle".to_string());
                        let mut list = list.clone();
                        list.extend(incoming);
                        Label::List(list)
                    }
                    Label::Text(text) if !text.is_empty() => {
                        entry.shape = Some("rectWithTitle".to_string());
                        if *text == item_id {
                            Label::List(incoming)
                        } else {
                            let mut list = vec![text.clone()];
                            list.extend(incoming);
                            Label::List(list)
                        }
                    }
                    Label::Text(_) => {
                        entry.shape = Some("rect".to_string());
                        match &item.description {
                            Descr::List(l) => Label::List(l.clone()),
                            _ => Label::Text(incoming.concat()),
                        }
                    }
                };
            }

            if entry.description.len() == 1 && entry.shape.as_deref() == Some("rectWithTitle") {
                entry.shape = Some(if entry.is_group {
                    "roundedWithTitle".to_string()
                } else {
                    "rect".to_string()
                });
            }

            if !entry.is_group
                && let Some(doc) = item.doc.as_deref()
            {
                entry.is_group = true;
                entry.dir = Some(doc_direction(doc));
                entry.shape = Some(if item.is_divider() {
                    "divider".to_string()
                } else {
                    "roundedWithTitle".to_string()
                });
                entry.css_classes = format!(
                    "{} {CSS_DIAGRAM_CLUSTER} {}",
                    entry.css_classes,
                    if alt_flag {
                        CSS_DIAGRAM_CLUSTER_ALT
                    } else {
                        ""
                    }
                );
            }

            let entry = entry.clone();
            let shape = entry
                .shape
                .clone()
                .unwrap_or_else(|| "undefined".to_string());
            let label = if shape == "divider" {
                Label::Text(String::new())
            } else {
                entry.description.clone()
            };
            let parent_id = parent.filter(|p| p.id != "root").map(|p| p.id.clone());
            let node = Node {
                id: item_id.clone(),
                dom_id: Some(dom_id(
                    &self.config.diagram_id,
                    &item_id,
                    self.graph_item_count,
                    "",
                )),
                label_style: Some(String::new()),
                label_type: Some("markdown".to_string()),
                css_classes: entry.css_classes.clone(),
                css_styles: entry.css_styles.clone(),
                shape,
                dir: entry.dir.as_deref().and_then(Dir::parse),
                is_group: entry.is_group,
                padding: Some(8.0),
                rx: Some(10.0),
                ry: Some(10.0),
                parent_id,
                center_label: true,
                ..Node::default()
            };

            if let Some(note) = &item.note {
                let note_id = format!("{item_id}{NOTE_ID}-{}", self.graph_item_count);
                let group_id = format!("{item_id}{PARENT_ID}");
                let note_node = Node {
                    id: note_id.clone(),
                    dom_id: Some(dom_id(
                        &self.config.diagram_id,
                        &item_id,
                        self.graph_item_count,
                        "note",
                    )),
                    label: Some(note.text.clone()),
                    label_style: Some(String::new()),
                    label_type: Some("markdown".to_string()),
                    css_classes: CSS_DIAGRAM_NOTE.to_string(),
                    shape: "note".to_string(),
                    padding: Some(self.config.flowchart_padding),
                    parent_id: Some(group_id.clone()),
                    ..Node::default()
                };
                let group_node = Node {
                    id: group_id.clone(),
                    dom_id: Some(dom_id(
                        &self.config.diagram_id,
                        &item_id,
                        self.graph_item_count,
                        "parent",
                    )),
                    label: Some(note.text.clone()),
                    label_style: Some(String::new()),
                    css_classes: entry.css_classes.clone(),
                    shape: "noteGroup".to_string(),
                    is_group: true,
                    padding: Some(16.0),
                    ..Node::default()
                };
                self.graph_item_count += 1;
                self.insert_or_update(group_node, None);
                self.insert_or_update(note_node, None);
                self.insert_or_update(node, Some(label));
                let (from, to) = if note.position == "left of" {
                    (note_id, item_id.clone())
                } else {
                    (item_id.clone(), note_id)
                };
                self.edges.push(Edge {
                    arrow_type_end: Some(String::new()),
                    style: vec![G_EDGE_STYLE.to_string()],
                    classes: Some(CSS_EDGE_NOTE_EDGE.to_string()),
                    pattern: Some(Pattern::Dashed),
                    labelpos: Some("c".to_string()),
                    label_type: Some("markdown".to_string()),
                    thickness: Some(Thickness::Normal),
                    ..Edge::new(&format!("{from}-{to}"), &from, &to)
                });
            } else {
                self.insert_or_update(node, Some(label));
            }
        }
        if let Some(doc) = item.doc.as_deref() {
            self.setup_doc(Some(item), doc, !alt_flag);
        }
    }
}

/// The error `extract` throws for a composite state with a description.
pub fn group_description_error(id: &str) -> String {
    format!("Group nodes can only have label. Remove the additional description for node [{id}]")
}

/// `dataFetcher(undefined, rootDoc, ...)`, the label split of `extract`,
/// and `getData`. Fails with the `extract` error text.
pub fn fetch(db: &StateDb, config: &FetchConfig) -> Result<Fetched, String> {
    let mut f = Fetcher {
        states: &db.states,
        classes: &db.classes,
        config,
        node_db: IndexMap::new(),
        graph_item_count: 0,
        nodes: Vec::new(),
        labels: IndexMap::new(),
        edges: Vec::new(),
    };
    let root = StateStmt {
        id: "root".to_string(),
        doc: Some(db.root_doc.clone()),
        ..StateStmt::default()
    };
    f.fetch(None, &root, true);

    let Fetcher {
        mut nodes,
        labels,
        edges,
        ..
    } = f;
    for node in &mut nodes {
        match labels.get(&node.id) {
            Some(Label::List(list)) => {
                node.description = list.iter().skip(1).cloned().collect();
                if node.is_group && !node.description.is_empty() {
                    return Err(group_description_error(&node.id));
                }
                node.label = Some(list.first().cloned().unwrap_or_default());
            }
            Some(Label::Text(text)) => node.label = Some(text.clone()),
            None => {}
        }
        node.wrapping_width = Some(config.wrapping_width);
        if !node.is_group {
            node.min_width = Some(config.min_node_width);
        }
    }
    Ok(Fetched {
        nodes,
        edges,
        direction: Dir::parse(&db.direction()).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::diagrams::state::parse_source;

    fn config() -> FetchConfig {
        FetchConfig {
            diagram_id: "my-svg".to_string(),
            flowchart_padding: 15.0,
            wrapping_width: 120.0,
            min_node_width: 120.0,
        }
    }

    fn fetched(src: &str) -> Fetched {
        let db = parse_source(src).expect("parses");
        fetch(&db, &config()).expect("fetches")
    }

    fn node<'a>(f: &'a Fetched, id: &str) -> &'a Node {
        f.nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap_or_else(|| panic!("no node {id}"))
    }

    #[test]
    fn chain_with_start_and_end() {
        let f = fetched("stateDiagram-v2\n[*] --> A\nA --> [*]\n");
        let ids: Vec<&str> = f.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["root_start", "A", "root_end"]);
        assert_eq!(node(&f, "root_start").shape, "stateStart");
        assert_eq!(node(&f, "root_end").shape, "stateEnd");
        assert_eq!(node(&f, "A").shape, "rect");
        assert_eq!(node(&f, "A").dom_id(), "my-svg-state-A-1");
        assert_eq!(node(&f, "A").min_width, Some(120.0));
        let edges: Vec<(&str, &str, &str)> = f
            .edges
            .iter()
            .map(|e| (e.id.as_str(), e.start.as_str(), e.end.as_str()))
            .collect();
        assert_eq!(
            edges,
            [("edge0", "root_start", "A"), ("edge1", "A", "root_end")]
        );
    }

    #[test]
    fn descriptions_pick_rect_then_rect_with_title() {
        let f = fetched(
            "stateDiagram-v2\nstate \"Long\" as s2\ns1 : one\ns2 : two\ns2 : three\ns1 --> s2\n",
        );
        let s1 = node(&f, "s1");
        assert_eq!(
            (s1.shape.as_str(), s1.label.as_deref()),
            ("rect", Some("one"))
        );
        let s2 = node(&f, "s2");
        assert_eq!(s2.shape, "rectWithTitle");
        assert_eq!(s2.label.as_deref(), Some("Long"));
        assert_eq!(s2.description, ["two", "three"]);
    }

    #[test]
    fn composites_are_groups_with_alternating_classes() {
        let f = fetched("stateDiagram-v2\nstate A {\n  state B {\n    x\n  }\n}\n");
        let a = node(&f, "A");
        assert!(a.is_group);
        assert_eq!(a.shape, "roundedWithTitle");
        assert_eq!(a.css_classes, " statediagram-state statediagram-cluster ");
        let b = node(&f, "B");
        assert_eq!(b.parent_id.as_deref(), Some("A"));
        assert_eq!(
            b.css_classes,
            " statediagram-state statediagram-cluster statediagram-cluster-alt"
        );
        assert_eq!(b.min_width, None);
        assert_eq!(node(&f, "x").parent_id.as_deref(), Some("B"));
    }

    #[test]
    fn notes_add_a_group_a_note_and_a_dashed_edge() {
        let f = fetched("stateDiagram-v2\nA\nnote left of A : hi\n");
        let ids: Vec<&str> = f.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["A", "A----parent", "A----note-0"]);
        let note = node(&f, "A----note-0");
        assert_eq!(note.parent_id.as_deref(), Some("A----parent"));
        assert_eq!(note.padding, Some(15.0));
        let e = &f.edges[0];
        assert_eq!((e.start.as_str(), e.end.as_str()), ("A----note-0", "A"));
        assert_eq!(e.id, "A----note-0-A");
        assert_eq!(e.pattern, Some(Pattern::Dashed));
    }

    #[test]
    fn class_defs_compile_into_node_styles() {
        let f = fetched(
            "stateDiagram-v2\nclassDef hot fill:#f00\n[*] --> A:::hot\nstyle A stroke:#00f\n",
        );
        let a = node(&f, "A");
        assert_eq!(a.css_classes, "hot statediagram-state");
        assert_eq!(a.css_compiled_styles, ["fill:#f00"]);
        assert_eq!(a.css_styles, ["stroke:#00f"]);
    }

    #[test]
    fn group_with_a_description_is_an_error() {
        let db =
            parse_source("stateDiagram-v2\nstate \"T\" as A {\n x\n}\nA : more\n").expect("parses");
        assert_eq!(
            fetch(&db, &config()).expect_err("rejected"),
            group_description_error("A")
        );
    }
}
