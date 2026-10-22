//! Port of `diagrams/kanban/kanbanDb.ts` (mermaid 12.2.0): the node list
//! the grammar actions fill, the indentation rule that sorts nodes into
//! sections, the `@{ ... }` metadata, and `getData`, the section and item
//! list the renderer draws.

use std::fmt;

use serde_yaml_ng::Value as Yaml;

/// The errors `addNode` and `decorateNode` throw. mermaid reports them as
/// the diagram's error, not as a parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KanbanError {
    /// `getSection` found a node shallower than the first node.
    ItemsWithoutSection(String),
    /// The `@{ ... }` block named a shape that is not lower case.
    NoSuchShape(String),
    /// `yaml.load` rejected the `@{ ... }` block.
    Yaml(String),
    /// `decorateNode` ran before any node existed: the JavaScript dereferences
    /// `nodes[-1]`.
    NoNodeToDecorate(String),
}

impl fmt::Display for KanbanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KanbanError::ItemsWithoutSection(label) => {
                write!(
                    f,
                    "Items without section detected, found section (\"{label}\")"
                )
            }
            KanbanError::NoSuchShape(shape) => {
                write!(
                    f,
                    "No such shape: {shape}. Shape names should be lowercase."
                )
            }
            KanbanError::Yaml(message) => f.write_str(message),
            KanbanError::NoNodeToDecorate(field) => write!(
                f,
                "TypeError: Cannot set properties of undefined (setting '{field}')"
            ),
        }
    }
}

impl std::error::Error for KanbanError {}

/// `nodeType` of `kanbanDb.ts`. Only the shape of the brackets; the type
/// changes the node padding, which nothing downstream reads.
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

/// `KanbanNode` as `addNode` builds it, before `getData` reshapes it.
#[derive(Debug, Clone, PartialEq)]
pub struct KanbanNode {
    pub id: String,
    /// The indentation: the length of the leading `SPACELIST`.
    pub level: usize,
    pub label: String,
    /// `mindmap.maxNodeWidth`; `getData` drops it.
    pub width: f64,
    /// `mindmap.padding`, doubled for bracketed labels; `getData` drops it.
    pub padding: f64,
    pub icon: Option<String>,
    pub assigned: Option<String>,
    pub ticket: Option<String>,
    pub priority: Option<String>,
    pub css_classes: Option<String>,
    pub parent_id: Option<String>,
}

/// One column of `getData().nodes`: the `isGroup` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub id: String,
    pub label: String,
    pub ticket: Option<String>,
    pub level: usize,
    pub items: Vec<Item>,
}

/// One card of `getData().nodes`: the `kanbanItem` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: String,
    pub label: String,
    pub ticket: Option<String>,
    pub priority: Option<String>,
    pub assigned: Option<String>,
    pub icon: Option<String>,
    pub level: usize,
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

#[derive(Debug, Clone, Default)]
pub struct KanbanDb {
    nodes: Vec<KanbanNode>,
    /// Indices into `nodes` of the nodes `getSection` returned `null` for.
    sections: Vec<usize>,
    cnt: usize,
    defaults: NodeDefaults,
}

/// JavaScript truthiness of a YAML value.
fn truthy(value: &Yaml) -> bool {
    match value {
        Yaml::Null => false,
        Yaml::Bool(b) => *b,
        Yaml::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Yaml::String(s) => !s.is_empty(),
        Yaml::Sequence(_) | Yaml::Mapping(_) | Yaml::Tagged(_) => true,
    }
}

/// `value.toString()` for the scalars js-yaml's `JSON_SCHEMA` produces.
fn js_to_string(value: &Yaml) -> String {
    match value {
        Yaml::Null => "null".to_string(),
        Yaml::Bool(b) => b.to_string(),
        Yaml::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => i.to_string(),
            (None, Some(f)) => f.to_string(),
            _ => n.to_string(),
        },
        Yaml::String(s) => s.clone(),
        Yaml::Sequence(items) => items.iter().map(js_to_string).collect::<Vec<_>>().join(","),
        Yaml::Mapping(_) => "[object Object]".to_string(),
        Yaml::Tagged(t) => js_to_string(&t.value),
    }
}

impl KanbanDb {
    pub fn new(defaults: NodeDefaults) -> KanbanDb {
        KanbanDb {
            defaults,
            ..KanbanDb::default()
        }
    }

    /// `getSection(level)`: `null` when the level is the section level,
    /// else the last section. Throws when an earlier node sits above the
    /// first node's level.
    fn get_section(&self, level: usize) -> Result<Option<usize>, KanbanError> {
        let Some(first) = self.nodes.first() else {
            return Ok(None);
        };
        let section_level = first.level;
        let mut last_section = None;
        for (i, node) in self.nodes.iter().enumerate().rev() {
            if node.level == section_level && last_section.is_none() {
                last_section = Some(i);
            }
            if node.level < section_level {
                return Err(KanbanError::ItemsWithoutSection(node.label.clone()));
            }
        }
        match last_section {
            Some(i) if self.nodes[i].level == level => Ok(None),
            other => Ok(other),
        }
    }

    /// `addNode(level, id, descr, type, shapeData)`.
    pub fn add_node(
        &mut self,
        level: usize,
        id: &str,
        descr: &str,
        node_type: u8,
        shape_data: Option<&str>,
    ) -> Result<(), KanbanError> {
        let mut padding = self.defaults.padding;
        if matches!(
            node_type,
            node_type::ROUNDED_RECT | node_type::RECT | node_type::HEXAGON
        ) {
            padding *= 2.0;
        }
        let id = if id.is_empty() {
            let generated = format!("kbn{}", self.cnt);
            self.cnt += 1;
            generated
        } else {
            id.to_string()
        };
        let mut node = KanbanNode {
            id,
            level,
            label: descr.to_string(),
            width: self.defaults.max_node_width,
            padding,
            icon: None,
            assigned: None,
            ticket: None,
            priority: None,
            css_classes: None,
            parent_id: None,
        };

        if let Some(shape_data) = shape_data {
            // A one-line block is a flow mapping; a multi-line block is a
            // block mapping as written.
            let yaml_data = if shape_data.contains('\n') {
                format!("{shape_data}\n")
            } else {
                format!("{{\n{shape_data}\n}}")
            };
            let doc: Yaml = serde_yaml_ng::from_str(&yaml_data)
                .map_err(|e| KanbanError::Yaml(e.to_string()))?;
            let get = |key: &str| -> Option<&Yaml> {
                match &doc {
                    Yaml::Mapping(map) => map.get(Yaml::String(key.to_string())),
                    _ => None,
                }
            };
            if let Some(shape) = get("shape").filter(|v| truthy(v)) {
                let shape = js_to_string(shape);
                if shape != shape.to_lowercase() || shape.contains('_') {
                    return Err(KanbanError::NoSuchShape(shape));
                }
                // A valid lower-case shape is accepted and then ignored:
                // `doc.shape === 'kanbanItem'` can never hold for a
                // lower-case string, and `getData` sets the shape anyway.
            }
            if let Some(label) = get("label").filter(|v| truthy(v)) {
                node.label = js_to_string(label);
            }
            if let Some(icon) = get("icon").filter(|v| truthy(v)) {
                node.icon = Some(js_to_string(icon));
            }
            if let Some(assigned) = get("assigned").filter(|v| truthy(v)) {
                node.assigned = Some(js_to_string(assigned));
            }
            if let Some(ticket) = get("ticket").filter(|v| truthy(v)) {
                node.ticket = Some(js_to_string(ticket));
            }
            if let Some(priority) = get("priority").filter(|v| truthy(v)) {
                node.priority = Some(js_to_string(priority));
            }
        }

        match self.get_section(level)? {
            Some(section) => {
                node.parent_id = Some(self.nodes[section].id.clone());
            }
            None => self.sections.push(self.nodes.len()),
        }
        self.nodes.push(node);
        Ok(())
    }

    /// `decorateNode({ icon })`.
    pub fn decorate_icon(&mut self, icon: &str) -> Result<(), KanbanError> {
        let node = self
            .nodes
            .last_mut()
            .ok_or_else(|| KanbanError::NoNodeToDecorate("icon".to_string()))?;
        node.icon = Some(icon.to_string());
        Ok(())
    }

    /// `decorateNode({ class })`.
    pub fn decorate_class(&mut self, class: &str) -> Result<(), KanbanError> {
        let node = self
            .nodes
            .last_mut()
            .ok_or_else(|| KanbanError::NoNodeToDecorate("cssClasses".to_string()))?;
        node.css_classes = Some(class.to_string());
        Ok(())
    }

    /// `getSections()`.
    pub fn sections(&self) -> Vec<&KanbanNode> {
        self.sections.iter().map(|&i| &self.nodes[i]).collect()
    }

    /// Every node in insertion order.
    pub fn nodes(&self) -> &[KanbanNode] {
        &self.nodes
    }

    /// `getData()`: one `Section` per section with the items whose
    /// `parentId` is the section's id, in node order. Two sections with one
    /// id share their items, as the JavaScript filter does.
    pub fn get_data(&self) -> Vec<Section> {
        self.sections()
            .into_iter()
            .map(|section| Section {
                id: section.id.clone(),
                label: section.label.clone(),
                ticket: section.ticket.clone(),
                level: section.level,
                items: self
                    .nodes
                    .iter()
                    .filter(|n| n.parent_id.as_deref() == Some(section.id.as_str()))
                    .map(|n| Item {
                        id: n.id.clone(),
                        label: n.label.clone(),
                        ticket: n.ticket.clone(),
                        priority: n.priority.clone(),
                        assigned: n.assigned.clone(),
                        icon: n.icon.clone(),
                        level: n.level,
                    })
                    .collect(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> KanbanDb {
        KanbanDb::new(NodeDefaults::default())
    }

    #[test]
    fn first_node_is_a_section_and_indented_nodes_are_its_items() {
        let mut db = db();
        db.add_node(2, "Todo", "Todo", 0, None).unwrap();
        db.add_node(4, "a", "A", node_type::RECT, None).unwrap();
        db.add_node(4, "b", "B", node_type::RECT, None).unwrap();
        db.add_node(2, "Done", "Done", 0, None).unwrap();
        db.add_node(4, "c", "C", node_type::RECT, None).unwrap();
        let data = db.get_data();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].label, "Todo");
        assert_eq!(data[0].items.len(), 2);
        assert_eq!(data[1].items[0].id, "c");
        assert_eq!(db.nodes()[1].padding, 20.0);
        assert_eq!(db.nodes()[0].padding, 10.0);
    }

    #[test]
    fn metadata_is_parsed_from_the_shape_data_block() {
        let mut db = db();
        db.add_node(0, "s", "S", 0, None).unwrap();
        db.add_node(
            2,
            "i",
            "I",
            node_type::RECT,
            Some(" ticket: MC-2037, assigned: knsv, priority: 'High' "),
        )
        .unwrap();
        let item = &db.get_data()[0].items[0];
        assert_eq!(item.ticket.as_deref(), Some("MC-2037"));
        assert_eq!(item.assigned.as_deref(), Some("knsv"));
        assert_eq!(item.priority.as_deref(), Some("High"));
        // Numbers are stringified like `toString()`.
        db.add_node(2, "j", "J", node_type::RECT, Some(" ticket: 2038 "))
            .unwrap();
        assert_eq!(db.get_data()[0].items[1].ticket.as_deref(), Some("2038"));
        // A multi-line block is read as written.
        db.add_node(
            2,
            "k",
            "K",
            node_type::RECT,
            Some("\n  assigned: me\n  label: New\n"),
        )
        .unwrap();
        let item = &db.get_data()[0].items[2];
        assert_eq!(item.assigned.as_deref(), Some("me"));
        assert_eq!(item.label, "New");
    }

    #[test]
    fn shape_names_must_be_lower_case() {
        let mut db = db();
        db.add_node(0, "s", "S", 0, None).unwrap();
        let err = db
            .add_node(2, "i", "I", 0, Some(" shape: kanbanItem "))
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "No such shape: kanbanItem. Shape names should be lowercase."
        );
        db.add_node(2, "i", "I", 0, Some(" shape: rect ")).unwrap();
    }

    #[test]
    fn a_node_above_the_first_level_throws_on_the_next_node() {
        let mut db = db();
        db.add_node(2, "s", "S", 0, None).unwrap();
        db.add_node(0, "x", "Loose", 0, None).unwrap();
        let err = db.add_node(2, "y", "Y", 0, None).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Items without section detected, found section (\"Loose\")"
        );
    }

    #[test]
    fn empty_ids_are_generated_and_decorations_hit_the_last_node() {
        let mut db = db();
        assert!(db.decorate_icon("fa fa-book").is_err());
        db.add_node(0, "", "S", 0, None).unwrap();
        db.decorate_icon("fa fa-book").unwrap();
        db.decorate_class("hot").unwrap();
        assert_eq!(db.nodes()[0].id, "kbn0");
        assert_eq!(db.nodes()[0].icon.as_deref(), Some("fa fa-book"));
        assert_eq!(db.nodes()[0].css_classes.as_deref(), Some("hot"));
    }
}
