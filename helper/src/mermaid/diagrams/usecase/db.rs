//! `usecaseDb.ts` and `usecaseTypes.ts` (mermaid 12.2.0): the committed
//! model and `getData`, plus `prepareUsecaseLayoutData` of
//! `usecaseRenderer.ts`, which turns it into the unified renderer's
//! layout data.
//!
//! `sanitizeText` is the identity, as in the class port: under the
//! default `strict` level it only runs DOMPurify, which leaves plain text
//! alone. `escapePlainLabel` is skipped for the same reason the other
//! ports skip it: the label measurer reads the text, not its HTML.

use indexmap::IndexMap;

use super::json::{JsonValue, flatten_rows};
use super::parser::{ArrowType, LabelType, RelationKind, Shape};
use crate::mermaid::edges::{Edge, Pattern, Thickness};
use crate::mermaid::shapes::{Dir, Look, Node, NodeExtra};

/// `ActorType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorType {
    Normal,
    Hollow,
    Awesome,
    Icon,
}

impl ActorType {
    pub fn as_str(self) -> &'static str {
        match self {
            ActorType::Normal => "normal",
            ActorType::Hollow => "hollow",
            ActorType::Awesome => "awesome",
            ActorType::Icon => "icon",
        }
    }
}

/// `BoundaryType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryType {
    Rect,
    Package,
}

impl BoundaryType {
    pub fn as_str(self) -> &'static str {
        match self {
            BoundaryType::Rect => "rect",
            BoundaryType::Package => "package",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Actor {
    pub id: String,
    pub label: String,
    pub label_type: LabelType,
    pub actor_type: ActorType,
    pub icon: Option<String>,
    pub business: bool,
    pub stereotype: Option<String>,
    pub parent_id: Option<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UseCase {
    pub id: String,
    pub label: String,
    pub label_type: LabelType,
    pub shape: Shape,
    pub business: bool,
    pub stereotype: Option<String>,
    pub parent_id: Option<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Boundary {
    pub id: String,
    pub label: String,
    pub label_type: LabelType,
    pub boundary_type: BoundaryType,
    pub members: Vec<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Relationship {
    pub id: String,
    pub explicit_id: bool,
    pub source: String,
    pub target: String,
    pub kind: RelationKind,
    pub arrow_type: ArrowType,
    pub label: Option<(String, LabelType)>,
    pub minlen: u32,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub animate: bool,
    pub animation: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub id: String,
    pub target: String,
    pub label: String,
    pub label_type: LabelType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsonNode {
    pub id: String,
    pub value: IndexMap<String, JsonValue>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

/// `UsecaseFields`: the committed model.
#[derive(Debug, Clone, PartialEq)]
pub struct UsecaseDb {
    pub actors: IndexMap<String, Actor>,
    pub use_cases: IndexMap<String, UseCase>,
    pub boundaries: IndexMap<String, Boundary>,
    pub relationships: Vec<Relationship>,
    pub notes: IndexMap<String, Note>,
    pub json_nodes: IndexMap<String, JsonNode>,
    pub class_defs: IndexMap<String, Vec<String>>,
    /// `TB`, `BT`, `LR`, or `RL` (`TD` is stored as `TB`).
    pub direction: String,
    pub acc_title: String,
    pub acc_description: String,
}

impl Default for UsecaseDb {
    fn default() -> UsecaseDb {
        UsecaseDb {
            actors: IndexMap::new(),
            use_cases: IndexMap::new(),
            boundaries: IndexMap::new(),
            relationships: Vec::new(),
            notes: IndexMap::new(),
            json_nodes: IndexMap::new(),
            class_defs: IndexMap::new(),
            // `DEFAULT_DIRECTION`.
            direction: "LR".to_string(),
            acc_title: String::new(),
            acc_description: String::new(),
        }
    }
}

/// The `usecase` config keys `getData` reads.
#[derive(Debug, Clone, PartialEq)]
pub struct DataConfig {
    pub wrapping_width: f64,
    pub min_node_width: f64,
}

/// The node kinds the renderer styles differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Actor,
    UseCase,
    Note,
    Json,
    Boundary,
}

/// `usecaseNodeDomId(modelId)`: `usecase-<id>` with runs of other
/// characters as `_`, trimmed.
pub fn usecase_node_dom_id(id: &str) -> String {
    let mut out = String::new();
    let mut in_run = false;
    for c in id.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push('_');
            in_run = true;
        }
    }
    let trimmed = out.trim_matches('_');
    let id = if trimmed.is_empty() {
        "element"
    } else {
        trimmed
    };
    format!("usecase-{id}")
}

/// `escapeMarkdownMarkers`.
fn escape_markdown_markers(label: &str) -> String {
    let mut out = String::new();
    for c in label.chars() {
        if matches!(c, '*' | '[' | '\\' | ']' | '_' | '`') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn class_names(names: &[&str], extra: &[String]) -> String {
    names
        .iter()
        .copied()
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .chain(extra.iter().filter(|n| !n.is_empty()).cloned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// What `getData` and `prepareUsecaseLayoutData` produce: the nodes and
/// edges, each node's kind, and the direction.
#[derive(Debug, Clone)]
pub struct Data {
    pub nodes: Vec<Node>,
    pub kinds: Vec<NodeKind>,
    pub edges: Vec<Edge>,
    /// The relationship type of each edge, `note` for note connectors.
    pub edge_kinds: Vec<&'static str>,
    pub direction: Dir,
}

impl UsecaseDb {
    /// `getCompiledStyles(classNames)`: the `default` class first, then the
    /// named classes; a later declaration of a property replaces the value
    /// and keeps the first position.
    pub fn compiled_styles(&self, classes: &[String]) -> Vec<String> {
        let mut compiled: IndexMap<String, String> = IndexMap::new();
        for name in std::iter::once("default").chain(classes.iter().map(String::as_str)) {
            let Some(styles) = self.class_defs.get(name) else {
                continue;
            };
            for raw in styles {
                let style = raw.trim();
                let property = style.split_once(':').map_or(style, |(p, _)| p).trim();
                if !property.is_empty() {
                    compiled.insert(property.to_string(), style.to_string());
                }
            }
        }
        compiled.into_values().collect()
    }

    /// `getData()` followed by `prepareUsecaseLayoutData(data, id)`.
    pub fn get_data(&self, cfg: &DataConfig) -> Data {
        let mut nodes = Vec::new();
        let mut kinds = Vec::new();
        let mut edges = Vec::new();
        let mut edge_kinds = Vec::new();

        for actor in self.actors.values() {
            let shape = match actor.actor_type {
                ActorType::Hollow => "usecaseActorHollow",
                ActorType::Awesome => "usecaseActorAwesome",
                ActorType::Icon => "usecaseActorIcon",
                ActorType::Normal => "usecaseActor",
            };
            let variant = format!("usecase-actor-{}", actor.actor_type.as_str());
            nodes.push(Node {
                label_type: Some(actor.label_type.as_str().to_string()),
                padding: Some(10.0),
                css_classes: class_names(
                    &[
                        "default",
                        "usecase-actor",
                        &variant,
                        if actor.business {
                            "usecase-business"
                        } else {
                            ""
                        },
                    ],
                    &actor.classes,
                ),
                css_styles: actor.styles.clone(),
                css_compiled_styles: self.compiled_styles(&actor.classes),
                parent_id: actor.parent_id.clone(),
                extra: NodeExtra::Usecase {
                    stereotype: actor.stereotype.clone(),
                    business: actor.business,
                    rows: Vec::new(),
                },
                ..Node::new(&actor.id, &actor.label, shape)
            });
            kinds.push(NodeKind::Actor);
        }

        for use_case in self.use_cases.values() {
            let (shape, shape_class) = match use_case.shape {
                Shape::Ellipse if use_case.business => ("usecaseBusiness", "usecase-ellipse"),
                Shape::Ellipse => ("usecaseEllipse", "usecase-ellipse"),
                Shape::Rect => ("rect", "usecase-rect"),
            };
            let mut node = Node {
                label_type: Some(use_case.label_type.as_str().to_string()),
                padding: Some(if use_case.shape == Shape::Ellipse {
                    20.0
                } else {
                    10.0
                }),
                css_classes: class_names(
                    &[
                        "default",
                        "usecase-element",
                        shape_class,
                        if use_case.business {
                            "usecase-business"
                        } else {
                            ""
                        },
                    ],
                    &use_case.classes,
                ),
                css_styles: use_case.styles.clone(),
                css_compiled_styles: self.compiled_styles(&use_case.classes),
                parent_id: use_case.parent_id.clone(),
                extra: NodeExtra::Usecase {
                    stereotype: use_case.stereotype.clone(),
                    business: use_case.business,
                    rows: Vec::new(),
                },
                ..Node::new(&use_case.id, &use_case.label, shape)
            };
            // `prepareUsecaseLayoutData` folds the stereotype of an
            // ellipse or rectangle into a Markdown label.
            if (shape == "usecaseEllipse" || shape == "rect")
                && let Some(stereotype) = &use_case.stereotype
            {
                let label = if use_case.label_type == LabelType::Text {
                    escape_markdown_markers(&use_case.label)
                } else {
                    use_case.label.clone()
                };
                node.label = Some(format!(
                    "«{}»<br/>{label}",
                    escape_markdown_markers(stereotype)
                ));
                node.label_type = Some("markdown".to_string());
                node.extra = NodeExtra::Usecase {
                    stereotype: None,
                    business: use_case.business,
                    rows: Vec::new(),
                };
            }
            nodes.push(node);
            kinds.push(NodeKind::UseCase);
        }

        for note in self.notes.values() {
            nodes.push(Node {
                label_type: Some(note.label_type.as_str().to_string()),
                padding: Some(10.0),
                css_classes: "default usecase-note".to_string(),
                css_compiled_styles: self.compiled_styles(&[]),
                ..Node::new(&note.id, &note.label, "note")
            });
            kinds.push(NodeKind::Note);
        }

        for json in self.json_nodes.values() {
            let rows = flatten_rows(&json.value)
                .into_iter()
                .map(|r| (r.key, r.value))
                .collect();
            nodes.push(Node {
                label_type: Some("text".to_string()),
                padding: Some(10.0),
                css_classes: class_names(&["default", "usecase-json-table"], &json.classes),
                css_styles: json.styles.clone(),
                css_compiled_styles: self.compiled_styles(&json.classes),
                extra: NodeExtra::Usecase {
                    stereotype: None,
                    business: false,
                    rows,
                },
                ..Node::new(&json.id, &json.id, "usecaseJsonTable")
            });
            kinds.push(NodeKind::Json);
        }

        for boundary in self.boundaries.values() {
            let type_class = format!("system-boundary-{}", boundary.boundary_type.as_str());
            nodes.push(Node {
                label_type: Some(boundary.label_type.as_str().to_string()),
                is_group: true,
                padding: Some(20.0),
                css_classes: class_names(
                    &["default", "system-boundary", &type_class],
                    &boundary.classes,
                ),
                css_styles: boundary.styles.clone(),
                css_compiled_styles: self.compiled_styles(&boundary.classes),
                ..Node::new(&boundary.id, &boundary.label, "usecaseSystemBoundary")
            });
            kinds.push(NodeKind::Boundary);
        }

        for r in &self.relationships {
            let (start, end, pattern, label) = match r.kind {
                RelationKind::Include | RelationKind::Extend => (
                    "none",
                    "arrow_point",
                    Pattern::Dotted,
                    Some((r.kind.as_str().to_string(), LabelType::Text)),
                ),
                RelationKind::Generalization => ("none", "extension", Pattern::Solid, None),
                RelationKind::Association => {
                    let (s, e) = match r.arrow_type {
                        ArrowType::SolidArrow => ("none", "arrow_point"),
                        ArrowType::BackArrow => ("arrow_point", "none"),
                        ArrowType::CircleArrow => ("none", "arrow_circle"),
                        ArrowType::CrossArrow => ("none", "arrow_cross"),
                        ArrowType::CircleArrowReversed => ("arrow_circle", "none"),
                        ArrowType::CrossArrowReversed => ("arrow_cross", "none"),
                        ArrowType::LineSolid => ("none", "none"),
                    };
                    (s, e, Pattern::Solid, r.label.clone())
                }
            };
            let animation = (r.animate || r.animation.is_some()).then(|| {
                format!(
                    "edge-animation-{}",
                    r.animation.as_deref().unwrap_or("fast")
                )
            });
            let type_class = format!("relationship-{}", r.kind.as_str());
            let mut extra = r.classes.clone();
            extra.extend(animation);
            edges.push(Edge {
                label: label.as_ref().map(|(t, _)| t.clone()),
                label_type: label.as_ref().map(|(_, k)| k.as_str().to_string()),
                classes: Some(class_names(
                    &["default", "relationship", &type_class],
                    &extra,
                )),
                style: r.styles.clone(),
                css_compiled_styles: self.compiled_styles(&r.classes),
                arrow_type_start: Some(start.to_string()),
                arrow_type_end: Some(end.to_string()),
                thickness: Some(Thickness::Normal),
                pattern: Some(pattern),
                labelpos: Some("c".to_string()),
                minlen: Some(f64::from(r.minlen)),
                look: Look::Classic,
                ..Edge::new(&r.id, &r.source, &r.target)
            });
            edge_kinds.push(r.kind.as_str());
        }

        for note in self.notes.values() {
            edges.push(Edge {
                classes: Some("default relationship relationship-note".to_string()),
                css_compiled_styles: self.compiled_styles(&[]),
                arrow_type_start: Some("none".to_string()),
                arrow_type_end: Some("none".to_string()),
                thickness: Some(Thickness::Normal),
                pattern: Some(Pattern::Dotted),
                labelpos: Some("c".to_string()),
                minlen: Some(1.0),
                look: Look::Classic,
                ..Edge::new(&format!("{}-edge", note.id), &note.id, &note.target)
            });
            edge_kinds.push("note");
        }

        for (node, kind) in nodes.iter_mut().zip(&kinds) {
            node.look = Look::Classic;
            node.dom_id = Some(usecase_node_dom_id(&node.id));
            if node.wrapping_width.is_none() {
                node.wrapping_width = Some(cfg.wrapping_width);
            }
            // Actors are glyphs with a caption, not boxes.
            if !node.is_group && *kind != NodeKind::Actor && node.min_width.is_none() {
                node.min_width = Some(cfg.min_node_width);
            }
        }

        Data {
            nodes,
            kinds,
            edges,
            edge_kinds,
            direction: Dir::parse(&self.direction).unwrap_or(Dir::LR),
        }
    }
}
