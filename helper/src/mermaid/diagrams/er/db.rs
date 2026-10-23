//! `diagrams/er/erDb.ts` and `erTypes.ts` (mermaid 12.2.0): the entities,
//! their attributes, the relationships, the `classDef`/`class`/`style`
//! statements, the subgraphs, and `getData`, which turns them into the
//! unified renderer's nodes and edges.

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;

use crate::mermaid::diagrams::class::db::parse_generic_types;
use crate::mermaid::edges::{Edge, Pattern, Thickness};
use crate::mermaid::shapes::{Dir, ErAttribute, Look, Node, NodeExtra};

/// The text `addText` in `erBox.ts` hands `createText`: `~T~` generics
/// become `<T>`, escaped as `&lt;T&gt;` so the markdown step keeps them as
/// text (mermaid restores the characters in the DOM afterwards, which is
/// what the entity decoding of the label measurer does). Text without a
/// generic passes unchanged, markup included. The shape receives the text
/// already converted; the keys (`keys.join()`) never hold a `~`.
fn add_text_label(text: &str) -> String {
    let parsed = parse_generic_types(text);
    if parsed == text {
        return parsed;
    }
    parsed.replace('<', "&lt;").replace('>', "&gt;")
}

/// `Attribute` of `erTypes.ts`. The grammar leaves `keys` and `comment`
/// undefined when the source has none; `addAttributes` fills them in.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Attribute {
    pub type_name: String,
    pub name: String,
    pub keys: Option<Vec<String>>,
    pub comment: Option<String>,
}

/// `EntityNode` of `erTypes.ts`, the fields `addEntity` sets.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Entity {
    /// `entity-${name}-${index}`.
    pub id: String,
    pub label: String,
    pub attributes: Vec<ErAttribute>,
    /// `''` when the source gives none.
    pub alias: String,
    /// `'default'` plus one ` name` per `setClass`.
    pub css_classes: String,
    pub css_styles: Vec<String>,
}

/// `ErDB.Cardinality`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
    OnlyOne,
    MdParent,
}

impl Cardinality {
    /// The constant's value, `'ZERO_OR_ONE'` and so on.
    pub fn as_str(self) -> &'static str {
        match self {
            Cardinality::ZeroOrOne => "ZERO_OR_ONE",
            Cardinality::ZeroOrMore => "ZERO_OR_MORE",
            Cardinality::OneOrMore => "ONE_OR_MORE",
            Cardinality::OnlyOne => "ONLY_ONE",
            Cardinality::MdParent => "MD_PARENT",
        }
    }

    /// `cardA.toLowerCase()`: the edge's arrow type.
    pub fn arrow_type(self) -> String {
        self.as_str().to_lowercase()
    }
}

/// `ErDB.Identification`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identification {
    NonIdentifying,
    Identifying,
}

/// `RelSpec`: `cardA` is the cardinality written after the line (next to
/// `entityB`), `cardB` the one written before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelSpec {
    pub card_a: Cardinality,
    pub card_b: Cardinality,
    pub rel_type: Identification,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Relationship {
    /// An entity id, or a subgraph id when the name is a subgraph.
    pub entity_a: String,
    pub role_a: String,
    pub entity_b: String,
    pub rel_spec: RelSpec,
}

/// `EntityClass`: a `classDef`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntityClass {
    pub id: String,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

/// `ErSubGraph`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SubGraph {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    pub css_styles: Vec<String>,
    pub dir: Option<String>,
    pub label_type: String,
}

/// One element of the arrays `document` concatenates: an entity or
/// subgraph name, a `{ stmt: 'dir' }` object, or anything else, which
/// `addSubGraph` drops.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Name(String),
    Dir(String),
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErDb {
    pub entities: IndexMap<String, Entity>,
    pub relationships: Vec<Relationship>,
    pub classes: IndexMap<String, EntityClass>,
    /// `yy.subgraphDepth`, which the grammar actions read and write.
    pub subgraph_depth: i64,
    pub sub_graphs: Vec<SubGraph>,
    sub_graph_lookup: HashMap<String, usize>,
    sub_count: usize,
    pub direction: String,
    pub acc_title: Option<String>,
    pub acc_description: Option<String>,
}

impl Default for ErDb {
    fn default() -> ErDb {
        ErDb::new()
    }
}

impl ErDb {
    pub fn new() -> ErDb {
        ErDb {
            entities: IndexMap::new(),
            relationships: Vec::new(),
            classes: IndexMap::new(),
            subgraph_depth: 0,
            sub_graphs: Vec::new(),
            sub_graph_lookup: HashMap::new(),
            sub_count: 0,
            direction: "TB".to_string(),
            acc_title: None,
            acc_description: None,
        }
    }

    /// `addEntity(name, alias = '')`: creates the entity on first use; a
    /// later alias only sticks when the entity has none yet.
    pub fn add_entity(&mut self, name: &str, alias: &str) -> String {
        let size = self.entities.len();
        let entity = self
            .entities
            .entry(name.to_string())
            .or_insert_with(|| Entity {
                id: format!("entity-{name}-{size}"),
                label: name.to_string(),
                attributes: Vec::new(),
                alias: alias.to_string(),
                css_classes: "default".to_string(),
                css_styles: Vec::new(),
            });
        if entity.alias.is_empty() && !alias.is_empty() {
            entity.alias = alias.to_string();
        }
        entity.id.clone()
    }

    /// `addAttributes(entityName, attribs)`. The right-recursive
    /// `attributes` rule builds the list last attribute first, so the loop
    /// runs backwards.
    pub fn add_attributes(&mut self, entity_name: &str, attribs: &[Attribute]) {
        self.add_entity(entity_name, "");
        let Some(entity) = self.entities.get_mut(entity_name) else {
            return;
        };
        for attrib in attribs.iter().rev() {
            entity.attributes.push(ErAttribute {
                type_name: attrib.type_name.clone(),
                name: attrib.name.clone(),
                keys: attrib.keys.clone().unwrap_or_default(),
                comment: attrib.comment.clone().unwrap_or_default(),
            });
        }
    }

    /// `addRelationship(entA, rolA, entB, rSpec)`: a name that is a
    /// subgraph id links the subgraph, any other name an entity.
    pub fn add_relationship(&mut self, ent_a: &str, rol_a: &str, ent_b: &str, spec: RelSpec) {
        let mut resolve = |name: &str| {
            if self.sub_graph_lookup.contains_key(name) {
                name.to_string()
            } else {
                self.add_entity(name, "")
            }
        };
        let entity_a = resolve(ent_a);
        let entity_b = resolve(ent_b);
        self.relationships.push(Relationship {
            entity_a,
            role_a: rol_a.to_string(),
            entity_b,
            rel_spec: spec,
        });
    }

    pub fn set_direction(&mut self, dir: &str) {
        self.direction = dir.to_string();
    }

    /// `getCompiledStyles(classDefs)`: the `classDef` styles, then their
    /// text styles, of each class the list names.
    fn compiled_styles<'a>(&self, class_defs: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        let mut compiled = Vec::new();
        for name in class_defs {
            let Some(class) = self.classes.get(name) else {
                continue;
            };
            compiled.extend(class.styles.iter().map(|s| s.trim().to_string()));
            compiled.extend(class.text_styles.iter().map(|s| s.trim().to_string()));
        }
        compiled
    }

    /// `addCssStyles(ids, styles)`: the `style` statement.
    pub fn add_css_styles(&mut self, ids: &[String], styles: &[String]) {
        for id in ids {
            if let Some(entity) = self.entities.get_mut(id) {
                entity.css_styles.extend(styles.iter().cloned());
            }
            if let Some(&i) = self.sub_graph_lookup.get(id) {
                self.sub_graphs[i].css_styles.extend(styles.iter().cloned());
            }
        }
    }

    /// `addClass(ids, style)`: the `classDef` statement. A declaration
    /// that mentions `color` also goes to the text styles, with its first
    /// `fill` renamed `bgFill`.
    pub fn add_class(&mut self, ids: &[String], styles: &[String]) {
        for id in ids {
            let class = self
                .classes
                .entry(id.clone())
                .or_insert_with(|| EntityClass {
                    id: id.clone(),
                    ..EntityClass::default()
                });
            for s in styles {
                if s.contains("color") {
                    class.text_styles.push(s.replacen("fill", "bgFill", 1));
                }
                class.styles.push(s.clone());
            }
        }
    }

    /// `setClass(ids, classNames)`: the `class` statement and `:::`.
    pub fn set_class(&mut self, ids: &[String], class_names: &[String]) {
        for id in ids {
            if let Some(entity) = self.entities.get_mut(id) {
                for name in class_names {
                    entity.css_classes.push(' ');
                    entity.css_classes.push_str(name);
                }
            }
            if let Some(&i) = self.sub_graph_lookup.get(id) {
                self.sub_graphs[i]
                    .classes
                    .extend(class_names.iter().cloned());
            }
        }
    }

    /// `addSubGraph(_id, list, _title)`: keeps the first occurrence of each
    /// name, takes the last `direction` of the body, and drops names that
    /// an earlier subgraph already holds. Returns the id.
    pub fn add_sub_graph(&mut self, id: &str, list: &[Item], title: &str) -> String {
        let id = id.trim().to_string();
        let mut seen = HashSet::new();
        let mut dir = None;
        let mut node_list = Vec::new();
        for item in list {
            match item {
                Item::Dir(d) => dir = Some(d.clone()),
                Item::Name(name) => {
                    let trimmed = name.trim();
                    if !trimmed.is_empty() && seen.insert(trimmed.to_string()) {
                        node_list.push(name.clone());
                    }
                }
                Item::Other => {}
            }
        }
        // `common.sanitizeText` only strips script and unsafe markup; the
        // titles the grammar admits pass through unchanged.
        self.sub_count += 1;
        let existing: HashSet<&String> = self.sub_graphs.iter().flat_map(|s| &s.nodes).collect();
        let nodes: Vec<String> = node_list
            .into_iter()
            .filter(|n| !existing.contains(n))
            .collect();
        let sub_graph = SubGraph {
            id: id.clone(),
            nodes,
            title: title.trim().to_string(),
            classes: Vec::new(),
            css_styles: Vec::new(),
            dir,
            // `sanitizeNodeLabelType(_title?.type)`: the grammar passes no
            // type, which maps to `markdown`.
            label_type: "markdown".to_string(),
        };
        self.sub_graphs.push(sub_graph);
        self.sub_graph_lookup
            .insert(id.clone(), self.sub_graphs.len() - 1);
        id
    }

    /// `getData()`: the subgraphs as group nodes (last first), the entities
    /// as `erBox` nodes, and one edge per relationship.
    pub fn get_data(&self) -> (Vec<Node>, Vec<Edge>, Dir) {
        let mut nodes = Vec::new();
        let mut parent_db: HashMap<&str, &str> = HashMap::new();
        for sg in self.sub_graphs.iter().rev() {
            for id in &sg.nodes {
                parent_db.insert(id, &sg.id);
            }
        }
        for sg in self.sub_graphs.iter().rev() {
            nodes.push(Node {
                id: sg.id.clone(),
                label: Some(sg.title.clone()),
                label_style: Some(String::new()),
                label_type: Some(sg.label_type.clone()),
                parent_id: parent_db.get(sg.id.as_str()).map(|p| p.to_string()),
                padding: Some(8.0),
                css_compiled_styles: self.compiled_styles(sg.classes.iter().map(String::as_str)),
                css_styles: sg.css_styles.clone(),
                css_classes: sg.classes.join(" "),
                shape: "rect".to_string(),
                dir: sg.dir.as_deref().and_then(Dir::parse),
                is_group: true,
                look: Look::Classic,
                ..Node::default()
            });
        }

        for (key, entity) in &self.entities {
            if self.sub_graph_lookup.contains_key(key) {
                continue;
            }
            // `erBox` draws a table only when there are attributes; then every
            // text goes through `addText` and its generic-type step.
            let table = !entity.attributes.is_empty();
            let text = |t: &str| {
                if table {
                    add_text_label(t)
                } else {
                    t.to_string()
                }
            };
            nodes.push(Node {
                id: entity.id.clone(),
                label: Some(text(&entity.label)),
                label_type: Some("markdown".to_string()),
                parent_id: parent_db.get(key.as_str()).map(|p| p.to_string()),
                css_classes: entity.css_classes.clone(),
                css_styles: entity.css_styles.clone(),
                css_compiled_styles: self.compiled_styles(entity.css_classes.split(' ')),
                shape: "erBox".to_string(),
                look: Look::Classic,
                extra: NodeExtra::Entity {
                    alias: Some(text(&entity.alias)).filter(|a| !a.is_empty()),
                    attributes: entity
                        .attributes
                        .iter()
                        .map(|a| ErAttribute {
                            type_name: add_text_label(&a.type_name),
                            name: add_text_label(&a.name),
                            keys: a.keys.clone(),
                            comment: add_text_label(&a.comment),
                        })
                        .collect(),
                },
                ..Node::default()
            });
        }

        let edges = self
            .relationships
            .iter()
            .enumerate()
            .map(|(count, rel)| Edge {
                curve: Some("basis".to_string()),
                label: Some(rel.role_a.clone()),
                labelpos: Some("c".to_string()),
                thickness: Some(Thickness::Normal),
                classes: Some("relationshipLine".to_string()),
                arrow_type_start: Some(rel.rel_spec.card_b.arrow_type()),
                arrow_type_end: Some(rel.rel_spec.card_a.arrow_type()),
                pattern: Some(if rel.rel_spec.rel_type == Identification::Identifying {
                    Pattern::Solid
                } else {
                    Pattern::Dashed
                }),
                look: Look::Classic,
                label_type: Some("markdown".to_string()),
                // `getEdgeId(a, b, { prefix: 'id', counter })`.
                ..Edge::new(
                    &format!("id_{}_{}_{count}", rel.entity_a, rel.entity_b),
                    &rel.entity_a,
                    &rel.entity_b,
                )
            })
            .collect();

        let dir = Dir::parse(&self.direction).unwrap_or_default();
        (nodes, edges, dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(a: Cardinality, b: Cardinality, t: Identification) -> RelSpec {
        RelSpec {
            card_a: a,
            card_b: b,
            rel_type: t,
        }
    }

    #[test]
    fn entities_get_ids_in_creation_order_and_keep_the_first_alias() {
        let mut db = ErDb::new();
        assert_eq!(db.add_entity("A", ""), "entity-A-0");
        assert_eq!(db.add_entity("B", "Bee"), "entity-B-1");
        db.add_entity("B", "Other");
        db.add_entity("A", "Ay");
        assert_eq!(db.entities["A"].alias, "Ay");
        assert_eq!(db.entities["B"].alias, "Bee");
    }

    #[test]
    fn relationships_map_cardinalities_to_arrow_types() {
        let mut db = ErDb::new();
        db.add_relationship(
            "CUSTOMER",
            "places",
            "ORDER",
            spec(
                Cardinality::ZeroOrMore,
                Cardinality::OnlyOne,
                Identification::NonIdentifying,
            ),
        );
        let (nodes, edges, dir) = db.get_data();
        assert_eq!(dir, Dir::TB);
        assert_eq!(nodes.len(), 2);
        let e = &edges[0];
        assert_eq!(e.id, "id_entity-CUSTOMER-0_entity-ORDER-1_0");
        assert_eq!(e.arrow_type_start.as_deref(), Some("only_one"));
        assert_eq!(e.arrow_type_end.as_deref(), Some("zero_or_more"));
        assert_eq!(e.pattern, Some(Pattern::Dashed));
        assert_eq!(e.label.as_deref(), Some("places"));
    }

    #[test]
    fn class_defs_compile_with_text_styles() {
        let mut db = ErDb::new();
        db.add_entity("A", "");
        db.add_class(
            &["cool".to_string()],
            &["fill:#bbf".to_string(), "color:#fff".to_string()],
        );
        db.set_class(&["A".to_string()], &["cool".to_string()]);
        db.add_css_styles(&["A".to_string()], &["stroke:#f00".to_string()]);
        let (nodes, _, _) = db.get_data();
        assert_eq!(nodes[0].css_classes, "default cool");
        assert_eq!(
            nodes[0].css_compiled_styles,
            vec!["fill:#bbf", "color:#fff", "color:#fff"]
        );
        assert_eq!(nodes[0].css_styles, vec!["stroke:#f00"]);
    }

    #[test]
    fn subgraphs_parent_their_entities_and_keep_members_unique() {
        let mut db = ErDb::new();
        db.add_entity("A", "");
        db.add_entity("B", "");
        let inner = db.add_sub_graph(
            "inner",
            &[Item::Name("B".into()), Item::Dir("LR".into())],
            "Inner",
        );
        db.add_sub_graph(
            "outer",
            &[
                Item::Name("A".into()),
                Item::Name("A".into()),
                Item::Name("B".into()),
                Item::Name(inner),
            ],
            "outer",
        );
        assert_eq!(db.sub_graphs[1].nodes, vec!["A", "inner"]);
        let (nodes, _, _) = db.get_data();
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["outer", "inner", "entity-A-0", "entity-B-1"]);
        assert_eq!(nodes[1].parent_id.as_deref(), Some("outer"));
        assert_eq!(nodes[1].dir, Some(Dir::LR));
        assert_eq!(nodes[2].parent_id.as_deref(), Some("outer"));
        assert_eq!(nodes[3].parent_id.as_deref(), Some("inner"));
    }
}
