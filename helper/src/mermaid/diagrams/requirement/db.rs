//! `requirementDb.ts` (mermaid 12.2.0): requirements, elements,
//! relationships, `classDef`/`class`/`style`, and `getData` into the
//! unified renderer's [`Node`] and [`Edge`] lists.

use indexmap::IndexMap;

use crate::mermaid::edges::{Edge, Pattern, Thickness};
use crate::mermaid::shapes::{Dir, Node, NodeExtra, RequirementData};

/// The `RequirementType` values the grammar hands `addRequirement`.
pub const REQUIREMENT: &str = "Requirement";
pub const FUNCTIONAL_REQUIREMENT: &str = "Functional Requirement";
pub const INTERFACE_REQUIREMENT: &str = "Interface Requirement";
pub const PERFORMANCE_REQUIREMENT: &str = "Performance Requirement";
pub const PHYSICAL_REQUIREMENT: &str = "Physical Requirement";
pub const DESIGN_CONSTRAINT: &str = "Design Constraint";

/// `Relationships.CONTAINS`, the one relationship drawn solid with the
/// contains marker at its start.
pub const CONTAINS: &str = "contains";

/// `Requirement` of `types.ts`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Requirement {
    pub name: String,
    pub type_name: String,
    pub requirement_id: String,
    pub text: String,
    pub risk: String,
    pub verify_method: String,
    pub css_styles: Vec<String>,
    pub classes: Vec<String>,
}

/// `Element` of `types.ts`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ElementDef {
    pub name: String,
    pub type_name: String,
    pub doc_ref: String,
    pub css_styles: Vec<String>,
    pub classes: Vec<String>,
}

/// `Relation` of `types.ts`.
#[derive(Debug, Clone, PartialEq)]
pub struct Relation {
    pub type_name: String,
    pub src: String,
    pub dst: String,
}

/// `RequirementClass` of `types.ts`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RequirementClass {
    pub id: String,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

/// The fields `setNewReq*` collect until `addRequirement` reads them.
#[derive(Debug, Clone, PartialEq, Default)]
struct Latest {
    requirement_id: String,
    text: String,
    risk: String,
    verify_method: String,
    element_type: String,
    doc_ref: String,
}

/// `RequirementDB`. The maps keep insertion order, as JS `Map`s do, so
/// `getData` lists nodes in the order the diagram defines them.
#[derive(Debug, Clone, Default)]
pub struct RequirementDb {
    pub relations: Vec<Relation>,
    pub requirements: IndexMap<String, Requirement>,
    pub elements: IndexMap<String, ElementDef>,
    pub classes: IndexMap<String, RequirementClass>,
    pub direction: String,
    pub acc_title: Option<String>,
    pub acc_description: Option<String>,
    latest: Latest,
}

/// `classes: ['default']`, the initial class list of every node.
fn default_classes() -> Vec<String> {
    vec!["default".to_string()]
}

/// The node-level `cssStyles` and `classes` a style statement reaches.
fn node_styles<'a>(
    requirements: &'a mut IndexMap<String, Requirement>,
    elements: &'a mut IndexMap<String, ElementDef>,
    id: &str,
) -> Option<(&'a mut Vec<String>, &'a mut Vec<String>)> {
    // `this.requirements.get(id) ?? this.elements.get(id)`
    if let Some(r) = requirements.get_mut(id) {
        return Some((&mut r.css_styles, &mut r.classes));
    }
    elements
        .get_mut(id)
        .map(|e| (&mut e.css_styles, &mut e.classes))
}

impl RequirementDb {
    pub fn new() -> RequirementDb {
        RequirementDb {
            direction: "TB".to_string(),
            ..RequirementDb::default()
        }
    }

    pub fn set_direction(&mut self, dir: &str) {
        self.direction = dir.to_string();
    }

    /// `addRequirement(name, type)`: a new name takes the collected
    /// fields; a known name keeps its own. Either way the fields reset.
    pub fn add_requirement(&mut self, name: &str, type_name: &str) {
        if !self.requirements.contains_key(name) {
            let latest = &self.latest;
            self.requirements.insert(
                name.to_string(),
                Requirement {
                    name: name.to_string(),
                    type_name: type_name.to_string(),
                    requirement_id: latest.requirement_id.clone(),
                    text: latest.text.clone(),
                    risk: latest.risk.clone(),
                    verify_method: latest.verify_method.clone(),
                    css_styles: Vec::new(),
                    classes: default_classes(),
                },
            );
        }
        self.reset_latest_requirement();
    }

    fn reset_latest_requirement(&mut self) {
        self.latest.requirement_id.clear();
        self.latest.text.clear();
        self.latest.risk.clear();
        self.latest.verify_method.clear();
    }

    pub fn set_new_req_id(&mut self, id: &str) {
        self.latest.requirement_id = id.to_string();
    }

    pub fn set_new_req_text(&mut self, text: &str) {
        self.latest.text = text.to_string();
    }

    pub fn set_new_req_risk(&mut self, risk: &str) {
        self.latest.risk = risk.to_string();
    }

    pub fn set_new_req_verify_method(&mut self, method: &str) {
        self.latest.verify_method = method.to_string();
    }

    /// `addElement(name)`.
    pub fn add_element(&mut self, name: &str) {
        if !self.elements.contains_key(name) {
            self.elements.insert(
                name.to_string(),
                ElementDef {
                    name: name.to_string(),
                    type_name: self.latest.element_type.clone(),
                    doc_ref: self.latest.doc_ref.clone(),
                    css_styles: Vec::new(),
                    classes: default_classes(),
                },
            );
        }
        self.latest.element_type.clear();
        self.latest.doc_ref.clear();
    }

    pub fn set_new_element_type(&mut self, type_name: &str) {
        self.latest.element_type = type_name.to_string();
    }

    pub fn set_new_element_doc_ref(&mut self, doc_ref: &str) {
        self.latest.doc_ref = doc_ref.to_string();
    }

    pub fn add_relationship(&mut self, type_name: &str, src: &str, dst: &str) {
        self.relations.push(Relation {
            type_name: type_name.to_string(),
            src: src.to_string(),
            dst: dst.to_string(),
        });
    }

    /// `setCssStyle(ids, styles)`: the loop `return`s at the first id that
    /// names no node, so later ids are skipped too.
    pub fn set_css_style(&mut self, ids: &[String], styles: &[String]) {
        for id in ids {
            let Some((css, _)) = node_styles(&mut self.requirements, &mut self.elements, id) else {
                return;
            };
            for s in styles {
                if s.contains(',') {
                    css.extend(s.split(',').map(str::to_string));
                } else {
                    css.push(s.clone());
                }
            }
        }
    }

    /// `setClass(ids, classNames)`: the class joins the node's list, and a
    /// class defined already adds its styles now.
    pub fn set_class(&mut self, ids: &[String], class_names: &[String]) {
        for id in ids {
            let Some((css, classes)) = node_styles(&mut self.requirements, &mut self.elements, id)
            else {
                continue;
            };
            for class in class_names {
                classes.push(class.clone());
                if let Some(c) = self.classes.get(class) {
                    css.extend(c.styles.iter().cloned());
                }
            }
        }
    }

    /// `defineClass(ids, style)`: also appends the styles to every node
    /// that already lists the class.
    pub fn define_class(&mut self, ids: &[String], style: &[String]) {
        for id in ids {
            let class = self
                .classes
                .entry(id.clone())
                .or_insert_with(|| RequirementClass {
                    id: id.clone(),
                    ..RequirementClass::default()
                });
            for s in style {
                if s.contains("color") {
                    class.text_styles.push(s.replacen("fill", "bgFill", 1));
                }
                class.styles.push(s.clone());
            }
            let split: Vec<String> = style
                .iter()
                .flat_map(|s| s.split(',').map(str::to_string))
                .collect();
            for r in self.requirements.values_mut() {
                if r.classes.contains(id) {
                    r.css_styles.extend(split.iter().cloned());
                }
            }
            for e in self.elements.values_mut() {
                if e.classes.contains(id) {
                    e.css_styles.extend(split.iter().cloned());
                }
            }
        }
    }

    /// `getData()`: one `requirementBox` node per requirement, then one per
    /// element, and one edge per relationship labelled `<<type>>`.
    ///
    /// A relationship whose end names no node gets `start` or `end`
    /// `undefined` in mermaid, which the layout cannot place; the port
    /// leaves such an edge out.
    pub fn get_data(&self) -> (Vec<Node>, Vec<Edge>, Dir) {
        let mut nodes = Vec::new();
        for r in self.requirements.values() {
            nodes.push(Node {
                id: r.name.clone(),
                css_styles: r.css_styles.clone(),
                css_classes: r.classes.join(" "),
                shape: "requirementBox".to_string(),
                extra: NodeExtra::Requirement(RequirementData::Requirement {
                    type_name: r.type_name.clone(),
                    name: r.name.clone(),
                    requirement_id: r.requirement_id.clone(),
                    text: r.text.clone(),
                    risk: r.risk.clone(),
                    verify_method: r.verify_method.clone(),
                }),
                ..Node::default()
            });
        }
        for e in self.elements.values() {
            nodes.push(Node {
                id: e.name.clone(),
                css_styles: e.css_styles.clone(),
                css_classes: e.classes.join(" "),
                shape: "requirementBox".to_string(),
                extra: NodeExtra::Requirement(RequirementData::Element {
                    name: e.name.clone(),
                    type_name: e.type_name.clone(),
                    doc_ref: e.doc_ref.clone(),
                }),
                ..Node::default()
            });
        }

        let known =
            |name: &str| self.requirements.contains_key(name) || self.elements.contains_key(name);
        let mut edges = Vec::new();
        for (counter, relation) in self.relations.iter().enumerate() {
            if !known(&relation.src) || !known(&relation.dst) {
                continue;
            }
            let is_contains = relation.type_name == CONTAINS;
            edges.push(Edge {
                id: format!("{}-{}-{counter}", relation.src, relation.dst),
                start: relation.src.clone(),
                end: relation.dst.clone(),
                label: Some(format!("&lt;&lt;{}&gt;&gt;", relation.type_name)),
                label_type: Some("markdown".to_string()),
                classes: Some("relationshipLine".to_string()),
                style: vec![
                    "fill:none".to_string(),
                    if is_contains {
                        String::new()
                    } else {
                        "stroke-dasharray: 10,7".to_string()
                    },
                ],
                labelpos: Some("c".to_string()),
                thickness: Some(Thickness::Normal),
                pattern: Some(if is_contains {
                    Pattern::Solid
                } else {
                    Pattern::Dashed
                }),
                arrow_type_start: Some(if is_contains {
                    "requirement_contains".to_string()
                } else {
                    String::new()
                }),
                arrow_type_end: Some(if is_contains {
                    String::new()
                } else {
                    "requirement_arrow".to_string()
                }),
                ..Edge::default()
            });
        }
        let direction = Dir::parse(&self.direction).unwrap_or_default();
        (nodes, edges, direction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_requirement_takes_the_collected_fields_once() {
        let mut db = RequirementDb::new();
        db.set_new_req_id("1");
        db.set_new_req_text("text");
        db.add_requirement("r", REQUIREMENT);
        db.set_new_req_id("2");
        db.add_requirement("r", DESIGN_CONSTRAINT);
        db.add_requirement("s", REQUIREMENT);
        assert_eq!(db.requirements["r"].requirement_id, "1");
        assert_eq!(db.requirements["r"].type_name, REQUIREMENT);
        assert_eq!(db.requirements["s"].requirement_id, "");
    }

    #[test]
    fn class_styles_reach_nodes_before_and_after_the_class_def() {
        let mut db = RequirementDb::new();
        db.add_requirement("a", REQUIREMENT);
        db.add_element("b");
        db.set_class(&ids(&["a"]), &ids(&["important"]));
        db.define_class(&ids(&["important"]), &ids(&["fill:#f96", "stroke:#333"]));
        db.set_class(&ids(&["b"]), &ids(&["important"]));
        assert_eq!(
            db.requirements["a"].css_styles,
            ids(&["fill:#f96", "stroke:#333"])
        );
        assert_eq!(
            db.elements["b"].css_styles,
            ids(&["fill:#f96", "stroke:#333"])
        );
        assert_eq!(db.elements["b"].classes, ids(&["default", "important"]));
    }

    #[test]
    fn set_css_style_stops_at_an_unknown_id() {
        let mut db = RequirementDb::new();
        db.add_requirement("a", REQUIREMENT);
        db.add_requirement("c", REQUIREMENT);
        db.set_css_style(&ids(&["a", "missing", "c"]), &ids(&["fill:red"]));
        assert_eq!(db.requirements["a"].css_styles, ids(&["fill:red"]));
        assert!(db.requirements["c"].css_styles.is_empty());
    }

    #[test]
    fn edges_carry_the_relationship_markers() {
        let mut db = RequirementDb::new();
        db.add_requirement("a", REQUIREMENT);
        db.add_element("b");
        db.add_relationship(CONTAINS, "a", "b");
        db.add_relationship("traces", "b", "a");
        db.add_relationship("traces", "b", "nowhere");
        let (nodes, edges, dir) = db.get_data();
        assert_eq!(nodes.len(), 2);
        assert_eq!(dir, Dir::TB);
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].id, "a-b-0");
        assert_eq!(
            edges[0].arrow_type_start.as_deref(),
            Some("requirement_contains")
        );
        assert_eq!(
            edges[1].arrow_type_end.as_deref(),
            Some("requirement_arrow")
        );
        assert_eq!(edges[1].pattern, Some(Pattern::Dashed));
        assert_eq!(edges[1].label.as_deref(), Some("&lt;&lt;traces&gt;&gt;"));
    }
}
