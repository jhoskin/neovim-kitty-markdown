//! `diagrams/class/classDb.ts` and `classTypes.ts` (mermaid 12.2.0): the
//! class diagram database the grammar actions fill, and `getData`, which
//! turns it into the unified renderer's `LayoutData`.
//!
//! `sanitizeText` is the identity here: with the default `strict` security
//! level it only runs DOMPurify, which leaves plain text alone. The click,
//! link, and tooltip setters store their values; the scene draws no
//! interactivity, so nothing reads them back.

use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;

use crate::mermaid::edges::{Edge, Pattern, Thickness};
use crate::mermaid::shapes::{ClassMember as ShapeMember, Classifier, Dir, Look, Node, NodeExtra};

const MERMAID_DOM_ID_PREFIX: &str = "classId-";

/// `relationType` of `ClassDB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationType {
    Aggregation,
    Extension,
    Composition,
    Dependency,
    Lollipop,
    /// The grammar's `'none'` for a missing end.
    None,
}

impl RelationType {
    /// `getArrowMarker(type)`.
    pub fn marker(self) -> &'static str {
        match self {
            RelationType::Aggregation => "aggregation",
            RelationType::Extension => "extension",
            RelationType::Composition => "composition",
            RelationType::Dependency => "dependency",
            RelationType::Lollipop => "lollipop",
            RelationType::None => "none",
        }
    }
}

/// `lineType` of `ClassDB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineType {
    Line,
    DottedLine,
}

/// The `relation` object of a `ClassRelation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relation {
    pub type1: RelationType,
    pub type2: RelationType,
    pub line_type: LineType,
}

/// `ClassRelation` of `classTypes.ts`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassRelation {
    pub id1: String,
    pub id2: String,
    pub relation_title1: String,
    pub relation_title2: String,
    pub title: Option<String>,
    pub relation: Relation,
}

/// `ClassMember.memberType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberType {
    Method,
    Attribute,
}

/// `ClassMember` of `classTypes.ts`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassMember {
    pub id: String,
    pub member_type: MemberType,
    pub visibility: String,
    pub text: String,
    pub classifier: String,
    pub parameters: String,
    pub return_type: String,
}

const VISIBILITY_VALUES: [&str; 5] = ["#", "+", "~", "-", ""];

static METHOD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"([#+~-])?(.+)\((.*)\)([\s$*])?(.*)([$*])?").expect("static regex")
});

/// `parseGenericTypes` of `diagrams/common/common.ts`: `~T~` pairs become
/// `<T>`, with `~K, V~` split on commas and rejoined.
pub fn parse_generic_types(input: &str) -> String {
    let sets = split_keep_commas(input);
    let mut output: Vec<String> = Vec::new();
    let mut i = 0;
    while i < sets.len() {
        let mut this_set = sets[i].clone();
        if this_set == "," && i > 0 && i + 1 < sets.len() {
            let previous = &sets[i - 1];
            let next = &sets[i + 1];
            if previous.matches('~').count() == 1 && next.matches('~').count() == 1 {
                this_set = format!("{previous},{next}");
                i += 1;
                output.pop();
            }
        }
        output.push(process_set(&this_set));
        i += 1;
    }
    output.join("")
}

/// `input.split(/(,)/)`: the pieces with each comma kept as its own entry.
fn split_keep_commas(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for c in input.chars() {
        if c == ',' {
            out.push(std::mem::take(&mut current));
            out.push(",".to_string());
        } else {
            current.push(c);
        }
    }
    out.push(current);
    out
}

/// `processSet` of `common.ts`.
fn process_set(input: &str) -> String {
    let tilde_count = input.matches('~').count();
    if tilde_count <= 1 {
        return input.to_string();
    }
    let mut body = input;
    let mut has_starting_tilde = false;
    if !tilde_count.is_multiple_of(2) && input.starts_with('~') {
        body = &input[1..];
        has_starting_tilde = true;
    }
    let mut chars: Vec<char> = body.chars().collect();
    loop {
        let first = chars.iter().position(|&c| c == '~');
        let last = chars.iter().rposition(|&c| c == '~');
        match (first, last) {
            (Some(f), Some(l)) if f != l => {
                chars[f] = '<';
                chars[l] = '>';
            }
            _ => break,
        }
    }
    let mut out: String = chars.into_iter().collect();
    if has_starting_tilde {
        out.insert(0, '~');
    }
    out
}

/// JavaScript `String.prototype.trim` without the regex: Unicode white space.
fn js_trim(s: &str) -> &str {
    s.trim()
}

impl ClassMember {
    /// `new ClassMember(input, memberType)`.
    pub fn new(input: &str, member_type: MemberType) -> ClassMember {
        let mut m = ClassMember {
            id: String::new(),
            member_type,
            visibility: String::new(),
            text: String::new(),
            classifier: String::new(),
            parameters: String::new(),
            return_type: String::new(),
        };
        m.parse_member(input);
        m
    }

    /// `parseMember(input)`.
    fn parse_member(&mut self, input: &str) {
        let mut potential_classifier = String::new();
        if self.member_type == MemberType::Method {
            if let Some(caps) = METHOD_RE.captures(input) {
                let group = |i: usize| caps.get(i).map_or("", |m| m.as_str());
                let detected = js_trim(group(1));
                if VISIBILITY_VALUES.contains(&detected) {
                    self.visibility = detected.to_string();
                }
                self.id = group(2).to_string();
                self.parameters = js_trim(group(3)).to_string();
                potential_classifier = js_trim(group(4)).to_string();
                self.return_type = js_trim(group(5)).to_string();
                if potential_classifier.is_empty()
                    && let Some(last) = self.return_type.chars().last()
                    && (last == '$' || last == '*')
                {
                    potential_classifier = last.to_string();
                    self.return_type.pop();
                }
            }
        } else {
            let chars: Vec<char> = input.chars().collect();
            let length = chars.len();
            let first = chars.first().map(|c| c.to_string()).unwrap_or_default();
            let last = chars.last().copied();
            if VISIBILITY_VALUES.contains(&first.as_str()) {
                self.visibility = first;
            }
            if let Some(last) = last
                && (last == '$' || last == '*')
            {
                potential_classifier = last.to_string();
            }
            let start = if self.visibility.is_empty() { 0 } else { 1 };
            let end = if potential_classifier.is_empty() {
                length
            } else {
                length.saturating_sub(1)
            };
            self.id = if start < end {
                chars[start..end].iter().collect()
            } else {
                String::new()
            };
        }
        self.classifier = potential_classifier;
        // "Preserve one space only".
        self.id = if self.id.starts_with(' ') {
            format!(" {}", js_trim(&self.id))
        } else {
            js_trim(&self.id).to_string()
        };

        let mut combined = String::new();
        if !self.visibility.is_empty() {
            combined.push('\\');
            combined.push_str(&self.visibility);
        }
        combined.push_str(&parse_generic_types(&self.id));
        if self.member_type == MemberType::Method {
            combined.push('(');
            combined.push_str(&parse_generic_types(&self.parameters));
            combined.push(')');
            if !self.return_type.is_empty() {
                combined.push_str(" : ");
                combined.push_str(&parse_generic_types(&self.return_type));
            }
        }
        self.text = combined.replace('<', "&lt;").replace('>', "&gt;");
        if self.text.starts_with("\\&lt;") {
            self.text = self.text.replacen("\\&lt;", "~", 1);
        }
    }

    /// `parseClassifier()`, as the shape's typed classifier.
    pub fn shape_classifier(&self) -> Classifier {
        match self.classifier.as_str() {
            "*" => Classifier::Abstract,
            "$" => Classifier::Static,
            _ => Classifier::None,
        }
    }

    fn to_shape(&self) -> ShapeMember {
        ShapeMember {
            text: self.text.clone(),
            classifier: self.shape_classifier(),
        }
    }
}

/// `ClassNode` of `classTypes.ts`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassNode {
    pub id: String,
    pub type_param: String,
    pub label: String,
    pub text: String,
    pub css_classes: String,
    pub methods: Vec<ClassMember>,
    pub members: Vec<ClassMember>,
    pub annotations: Vec<String>,
    pub styles: Vec<String>,
    pub dom_id: String,
    pub parent: Option<String>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub have_callback: bool,
    pub tooltip: Option<String>,
}

/// `ClassNote`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassNote {
    pub id: String,
    pub class: Option<String>,
    pub text: String,
    pub index: usize,
    pub parent: Option<String>,
}

/// `Interface`: the far end of a lollipop relation.
#[derive(Debug, Clone, PartialEq)]
pub struct Interface {
    pub id: String,
    pub label: String,
    pub class_id: String,
}

/// `NamespaceNode`, without the class and note maps nothing reads.
#[derive(Debug, Clone, PartialEq)]
pub struct NamespaceNode {
    pub id: String,
    pub label: String,
    pub dom_id: String,
    pub parent: Option<String>,
    pub children: Vec<String>,
    pub explicit: bool,
}

/// `StyleClass`: a `classDef`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyleClass {
    pub id: String,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

/// The theme and config values `getData` reads through `getConfig()`.
#[derive(Debug, Clone, PartialEq)]
pub struct DataConfig {
    /// `class.padding`, undefined by default.
    pub padding: Option<f64>,
    /// `class.hierarchicalNamespaces` (default true).
    pub hierarchical: bool,
    pub note_bkg_color: String,
    pub note_border_color: String,
}

/// `ClassDB`. Maps keep JavaScript `Map` insertion order.
#[derive(Debug, Clone, Default)]
pub struct ClassDb {
    pub relations: Vec<ClassRelation>,
    pub classes: IndexMap<String, ClassNode>,
    pub style_classes: IndexMap<String, StyleClass>,
    pub notes: IndexMap<String, ClassNote>,
    pub interfaces: Vec<Interface>,
    pub namespaces: IndexMap<String, NamespaceNode>,
    namespace_counter: usize,
    namespace_stack: Vec<String>,
    /// The module-level `classCounter`, reset per render in the oracle
    /// because each mmdc run loads mermaid afresh.
    class_counter: usize,
    pub direction: String,
    pub acc_title: Option<String>,
    pub acc_description: Option<String>,
}

impl ClassDb {
    pub fn new() -> ClassDb {
        ClassDb {
            direction: "TB".to_string(),
            ..ClassDb::default()
        }
    }

    /// `splitClassNameAndType(id)`.
    pub fn split_class_name_and_type(id: &str) -> (String, String) {
        if let Some(pos) = id.find('~')
            && pos > 0
        {
            let mut split = id.split('~');
            let class_name = split.next().unwrap_or_default().to_string();
            let generic = split.next().unwrap_or_default().to_string();
            return (class_name, generic);
        }
        (id.to_string(), String::new())
    }

    /// `setClassLabel(id, label)`.
    pub fn set_class_label(&mut self, id: &str, label: &str) {
        let (class_name, _) = Self::split_class_name_and_type(id);
        if let Some(c) = self.classes.get_mut(&class_name) {
            c.label = label.to_string();
            c.text = if c.type_param.is_empty() {
                label.to_string()
            } else {
                format!("{label}<{}>", c.type_param)
            };
        }
    }

    /// `addClass(id)`.
    pub fn add_class(&mut self, id: &str) {
        let (class_name, type_param) = Self::split_class_name_and_type(id);
        if self.classes.contains_key(&class_name) {
            return;
        }
        let text = if type_param.is_empty() {
            class_name.clone()
        } else {
            format!("{class_name}&lt;{type_param}&gt;")
        };
        let node = ClassNode {
            id: class_name.clone(),
            type_param,
            label: class_name.clone(),
            text,
            css_classes: "default".to_string(),
            methods: Vec::new(),
            members: Vec::new(),
            annotations: Vec::new(),
            styles: Vec::new(),
            dom_id: format!("{MERMAID_DOM_ID_PREFIX}{class_name}-{}", self.class_counter),
            parent: None,
            link: None,
            link_target: None,
            have_callback: false,
            tooltip: None,
        };
        self.classes.insert(class_name, node);
        self.class_counter += 1;
    }

    fn add_interface(&mut self, label: &str, class_id: &str) {
        let id = format!("interface{}", self.interfaces.len());
        self.interfaces.push(Interface {
            id,
            label: label.to_string(),
            class_id: class_id.to_string(),
        });
    }

    /// `addRelation(classRelation)`.
    pub fn add_relation(&mut self, mut rel: ClassRelation) {
        let is_end = |t: RelationType| t != RelationType::None;
        if rel.relation.type1 == RelationType::Lollipop && !is_end(rel.relation.type2) {
            self.add_class(&rel.id2.clone());
            self.add_interface(&rel.id1.clone(), &rel.id2.clone());
            rel.id1 = format!("interface{}", self.interfaces.len() - 1);
        } else if rel.relation.type2 == RelationType::Lollipop && !is_end(rel.relation.type1) {
            self.add_class(&rel.id1.clone());
            self.add_interface(&rel.id2.clone(), &rel.id1.clone());
            rel.id2 = format!("interface{}", self.interfaces.len() - 1);
        } else {
            self.add_class(&rel.id1.clone());
            self.add_class(&rel.id2.clone());
        }
        rel.id1 = Self::split_class_name_and_type(&rel.id1).0;
        rel.id2 = Self::split_class_name_and_type(&rel.id2).0;
        rel.relation_title1 = js_trim(&rel.relation_title1).to_string();
        rel.relation_title2 = js_trim(&rel.relation_title2).to_string();
        self.relations.push(rel);
    }

    /// `addAnnotation(className, annotation)`.
    pub fn add_annotation(&mut self, class_name: &str, annotation: &str) {
        let (name, _) = Self::split_class_name_and_type(class_name);
        if let Some(c) = self.classes.get_mut(&name) {
            c.annotations.push(annotation.to_string());
        }
    }

    /// `addMember(className, member)`.
    pub fn add_member(&mut self, class_name: &str, member: &str) {
        self.add_class(class_name);
        let (name, _) = Self::split_class_name_and_type(class_name);
        let Some(the_class) = self.classes.get_mut(&name) else {
            return;
        };
        let member_string = js_trim(member);
        if member_string.starts_with("<<") && member_string.ends_with(">>") {
            // Both markers are present, so the string is at least `<<>>`.
            let inner = &member_string[2..member_string.len() - 2];
            the_class.annotations.push(inner.to_string());
        } else if member_string.find(')').is_some_and(|p| p > 0) {
            the_class
                .methods
                .push(ClassMember::new(member_string, MemberType::Method));
        } else if !member_string.is_empty() {
            the_class
                .members
                .push(ClassMember::new(member_string, MemberType::Attribute));
        }
    }

    /// `addMembers(className, members)`: the grammar collects the body
    /// lines last first, so they are reversed back.
    pub fn add_members(&mut self, class_name: &str, members: &[String]) {
        for member in members.iter().rev() {
            self.add_member(class_name, member);
        }
    }

    /// `addNote(text, className)`.
    pub fn add_note(&mut self, text: &str, class_name: Option<&str>) -> String {
        let index = self.notes.len();
        let id = format!("note{index}");
        self.notes.insert(
            id.clone(),
            ClassNote {
                id: id.clone(),
                class: class_name.map(str::to_string),
                text: text.to_string(),
                index,
                parent: None,
            },
        );
        id
    }

    /// `cleanupLabel(label)`.
    pub fn cleanup_label(label: &str) -> String {
        let label = label.strip_prefix(':').unwrap_or(label);
        js_trim(label).to_string()
    }

    /// `setCssClass(ids, className)`.
    pub fn set_css_class(&mut self, ids: &str, class_name: &str) {
        for raw in ids.split(',') {
            let mut id = raw.to_string();
            if raw.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                id = format!("{MERMAID_DOM_ID_PREFIX}{id}");
            }
            let (id, _) = Self::split_class_name_and_type(&id);
            if let Some(node) = self.classes.get_mut(&id) {
                node.css_classes.push(' ');
                node.css_classes.push_str(class_name);
            }
        }
    }

    /// `defineClass(ids, style)`.
    pub fn define_class(&mut self, ids: &[String], style: &[String]) {
        for id in ids {
            let entry = self
                .style_classes
                .entry(id.clone())
                .or_insert_with(|| StyleClass {
                    id: id.clone(),
                    ..StyleClass::default()
                });
            for s in style {
                if s.contains("color") {
                    entry.text_styles.push(s.replacen("fill", "bgFill", 1));
                }
                entry.styles.push(s.clone());
            }
            // `value.cssClasses.includes(id)`: a substring test on the
            // space-separated string, as in mermaid.
            for value in self.classes.values_mut() {
                if value.css_classes.contains(id.as_str()) {
                    value
                        .styles
                        .extend(style.iter().flat_map(|s| s.split(',').map(str::to_string)));
                }
            }
        }
    }

    /// `setTooltip(ids, tooltip)`.
    pub fn set_tooltip(&mut self, ids: &str, tooltip: Option<&str>) {
        let Some(tooltip) = tooltip else { return };
        for id in ids.split(',') {
            let (name, _) = Self::split_class_name_and_type(id);
            if let Some(node) = self.classes.get_mut(&name) {
                node.tooltip = Some(tooltip.to_string());
            }
        }
    }

    /// `setLink(ids, linkStr, target)`.
    pub fn set_link(&mut self, ids: &str, link: &str, target: Option<&str>) {
        for raw in ids.split(',') {
            let mut id = raw.to_string();
            if raw.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                id = format!("{MERMAID_DOM_ID_PREFIX}{id}");
            }
            let (id, _) = Self::split_class_name_and_type(&id);
            if let Some(node) = self.classes.get_mut(&id) {
                node.link = Some(link.to_string());
                node.link_target = Some(target.unwrap_or("_blank").to_string());
            }
        }
        self.set_css_class(ids, "clickable");
    }

    /// `setClickEvent(ids, functionName, functionArgs)`. The callback binding
    /// only runs at `securityLevel: 'loose'` in a browser; the flag stays.
    pub fn set_click_event(&mut self, ids: &str) {
        for id in ids.split(',') {
            let (name, _) = Self::split_class_name_and_type(id);
            if let Some(node) = self.classes.get_mut(&name) {
                node.have_callback = true;
            }
        }
        self.set_css_class(ids, "clickable");
    }

    pub fn set_direction(&mut self, dir: &str) {
        self.direction = dir.to_string();
    }

    /// `addNamespace(id, label)`.
    pub fn add_namespace(&mut self, id: &str, label: Option<&str>) -> String {
        let qualified = match self.namespace_stack.last() {
            Some(prefix) => format!("{prefix}.{id}"),
            None => id.to_string(),
        };
        self.namespace_stack.push(qualified.clone());
        let label = label.filter(|l| !l.is_empty());
        if let Some(existing) = self.namespaces.get_mut(&qualified) {
            existing.explicit = true;
            if let Some(label) = label {
                existing.label = label.to_string();
            }
            return qualified;
        }
        let parts: Vec<&str> = qualified.split('.').collect();
        let mut ancestors: Vec<String> = Vec::with_capacity(parts.len());
        for (i, part) in parts.iter().enumerate() {
            if i == 0 {
                ancestors.push((*part).to_string());
            } else {
                ancestors.push(format!("{}.{part}", ancestors[i - 1]));
            }
        }
        for i in 0..ancestors.len() {
            let current = ancestors[i].clone();
            let parent = (i > 0).then(|| ancestors[i - 1].clone());
            let is_leaf = i == ancestors.len() - 1;
            let node_label = match label {
                Some(l) if is_leaf => l.to_string(),
                _ => parts[i].to_string(),
            };
            if !self.namespaces.contains_key(&current) {
                let dom_id = format!(
                    "{MERMAID_DOM_ID_PREFIX}{current}-{}",
                    self.namespace_counter
                );
                self.namespace_counter += 1;
                self.namespaces.insert(
                    current.clone(),
                    NamespaceNode {
                        id: current.clone(),
                        label: node_label,
                        dom_id,
                        parent: parent.clone(),
                        children: Vec::new(),
                        explicit: is_leaf,
                    },
                );
            } else if is_leaf && let Some(ns) = self.namespaces.get_mut(&current) {
                ns.explicit = true;
            }
            if let Some(parent) = parent {
                self.link_parent_child(&parent, &current);
            }
        }
        qualified
    }

    fn link_parent_child(&mut self, parent_id: &str, child_id: &str) {
        if !self.namespaces.contains_key(parent_id) || !self.namespaces.contains_key(child_id) {
            return;
        }
        if let Some(parent) = self.namespaces.get_mut(parent_id)
            && !parent.children.iter().any(|c| c == child_id)
        {
            parent.children.push(child_id.to_string());
        }
        if let Some(child) = self.namespaces.get_mut(child_id)
            && child.parent.is_none()
        {
            child.parent = Some(parent_id.to_string());
        }
    }

    pub fn pop_namespace(&mut self) {
        self.namespace_stack.pop();
    }

    /// `addClassesToNamespace(id, classNames, noteNames)`.
    pub fn add_classes_to_namespace(&mut self, id: &str, class_names: &[String], notes: &[String]) {
        if !self.namespaces.contains_key(id) {
            return;
        }
        for name in class_names {
            let (class_name, _) = Self::split_class_name_and_type(name);
            if let Some(node) = self.classes.get_mut(&class_name) {
                node.parent = Some(id.to_string());
            }
        }
        for note in notes {
            if let Some(n) = self.notes.get_mut(note) {
                n.parent = Some(id.to_string());
            }
        }
    }

    /// `setCssStyle(id, styles)`.
    pub fn set_css_style(&mut self, id: &str, styles: &[String]) {
        let Some(class) = self.classes.get_mut(id) else {
            return;
        };
        for s in styles {
            if s.contains(',') {
                class.styles.extend(s.split(',').map(str::to_string));
            } else {
                class.styles.push(s.clone());
            }
        }
    }

    /// `resolveExplicitAncestor(id)`.
    fn resolve_explicit_ancestor(&self, id: Option<&str>) -> Option<String> {
        let mut current = id.map(str::to_string);
        while let Some(c) = current {
            let ns = self.namespaces.get(&c)?;
            if ns.explicit {
                return Some(c);
            }
            current = ns.parent.clone();
        }
        None
    }

    /// `getData()`: nodes (namespaces, classes, notes, interfaces) and edges
    /// (note links, then relations) for the unified renderer.
    pub fn get_data(&self, cfg: &DataConfig) -> (Vec<Node>, Vec<Edge>, Dir) {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let hierarchical = cfg.hierarchical;

        for ns in self.namespaces.values() {
            if !hierarchical && !ns.explicit {
                continue;
            }
            nodes.push(Node {
                label: Some(if hierarchical {
                    ns.label.clone()
                } else {
                    ns.id.clone()
                }),
                is_group: true,
                padding: Some(cfg.padding.unwrap_or(16.0)),
                parent_id: if hierarchical {
                    ns.parent.clone()
                } else {
                    None
                },
                look: Look::Classic,
                ..Node::new(&ns.id, "", "rect")
            });
        }

        for class in self.classes.values() {
            let parent_id = if hierarchical {
                class.parent.clone()
            } else {
                self.resolve_explicit_ancestor(class.parent.as_deref())
            };
            nodes.push(Node {
                id: class.id.clone(),
                dom_id: Some(class.dom_id.clone()),
                // `addText` draws `node.text`, the label with its generic.
                label: Some(class.text.clone()),
                css_classes: class.css_classes.clone(),
                css_styles: class.styles.clone(),
                shape: "classBox".to_string(),
                parent_id,
                look: Look::Classic,
                extra: NodeExtra::Class {
                    annotations: class.annotations.clone(),
                    members: class.members.iter().map(ClassMember::to_shape).collect(),
                    methods: class.methods.iter().map(ClassMember::to_shape).collect(),
                },
                ..Node::default()
            });
        }

        for note in self.notes.values() {
            let parent_id = if hierarchical {
                note.parent.clone()
            } else {
                self.resolve_explicit_ancestor(note.parent.as_deref())
            };
            nodes.push(Node {
                label: Some(note.text.clone()),
                padding: Some(cfg.padding.unwrap_or(6.0)),
                css_styles: vec![
                    "text-align: left".to_string(),
                    "white-space: nowrap".to_string(),
                    format!("fill: {}", cfg.note_bkg_color),
                    format!("stroke: {}", cfg.note_border_color),
                ],
                parent_id,
                label_type: Some("markdown".to_string()),
                look: Look::Classic,
                ..Node::new(&note.id, "", "note")
            });
            let class_id = note
                .class
                .as_deref()
                .and_then(|c| self.classes.get(c))
                .map(|c| c.id.clone());
            if let Some(class_id) = class_id {
                edges.push(Edge {
                    classes: Some("relation".to_string()),
                    arrow_type_start: Some("none".to_string()),
                    arrow_type_end: Some("none".to_string()),
                    label_style: vec![String::new()],
                    style: vec!["fill: none".to_string()],
                    thickness: Some(Thickness::Normal),
                    pattern: Some(Pattern::Dotted),
                    look: Look::Classic,
                    ..Edge::new(&format!("edgeNote{}", note.index), &note.id, &class_id)
                });
            }
        }

        for interface in &self.interfaces {
            nodes.push(Node {
                // `opacity: 0;` in mermaid; the trailing `;` is dropped so the
                // declaration parser reads the value.
                css_styles: vec!["opacity: 0".to_string()],
                look: Look::Classic,
                ..Node::new(&interface.id, &interface.label, "rect")
            });
        }

        for (i, rel) in self.relations.iter().enumerate() {
            let counter = i + 1;
            let title = |t: &str| {
                (t != "none")
                    .then(|| t.to_string())
                    .filter(|t| !t.is_empty())
            };
            edges.push(Edge {
                label: rel.title.clone(),
                labelpos: Some("c".to_string()),
                thickness: Some(Thickness::Normal),
                classes: Some("relation".to_string()),
                arrow_type_start: Some(rel.relation.type1.marker().to_string()),
                arrow_type_end: Some(rel.relation.type2.marker().to_string()),
                start_label_right: title(&rel.relation_title1),
                end_label_left: title(&rel.relation_title2),
                label_style: vec!["display: inline-block".to_string()],
                pattern: Some(if rel.relation.line_type == LineType::DottedLine {
                    Pattern::Dashed
                } else {
                    Pattern::Solid
                }),
                look: Look::Classic,
                label_type: Some("markdown".to_string()),
                ..Edge::new(
                    &format!("id_{}_{}_{counter}", rel.id1, rel.id2),
                    &rel.id1,
                    &rel.id2,
                )
            });
        }

        let dir = Dir::parse(&self.direction).unwrap_or_default();
        (nodes, edges, dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_with_visibility_parameters_and_return_type() {
        let m = ClassMember::new("+deposit(amount) bool", MemberType::Method);
        assert_eq!(m.visibility, "+");
        assert_eq!(m.id, "deposit");
        assert_eq!(m.parameters, "amount");
        assert_eq!(m.return_type, "bool");
        assert_eq!(m.text, "\\+deposit(amount) : bool");
    }

    #[test]
    fn classifiers_on_methods_and_attributes() {
        let m = ClassMember::new("someAbstractMethod()*", MemberType::Method);
        assert_eq!(m.classifier, "*");
        assert_eq!(m.shape_classifier(), Classifier::Abstract);
        let m = ClassMember::new("+increment()$ int", MemberType::Method);
        assert_eq!(m.classifier, "$");
        assert_eq!(m.return_type, "int");
        let m = ClassMember::new("+getCount() int$", MemberType::Method);
        assert_eq!(m.classifier, "$");
        assert_eq!(m.return_type, "int");
        let a = ClassMember::new("+String staticField$", MemberType::Attribute);
        assert_eq!(a.classifier, "$");
        assert_eq!(a.id, "String staticField");
        assert_eq!(a.text, "\\+String staticField");
    }

    #[test]
    fn generics_become_escaped_angle_brackets() {
        let a = ClassMember::new("List~int~ position", MemberType::Attribute);
        assert_eq!(a.text, "List&lt;int&gt; position");
        let m = ClassMember::new("setPoints(List~int~ points)", MemberType::Method);
        assert_eq!(m.text, "setPoints(List&lt;int&gt; points)");
        assert_eq!(parse_generic_types("Map~K, V~"), "Map<K, V>");
        assert_eq!(parse_generic_types("List~List~int~~"), "List<List<int>>");
    }

    #[test]
    fn generic_class_text_and_relations() {
        let mut db = ClassDb::new();
        db.add_class("Square~Shape~");
        let c = &db.classes["Square"];
        assert_eq!(c.type_param, "Shape");
        assert_eq!(c.text, "Square&lt;Shape&gt;");
        assert_eq!(c.dom_id, "classId-Square-0");
    }

    #[test]
    fn members_block_is_reversed_back_into_source_order() {
        let mut db = ClassDb::new();
        db.add_class("A");
        // The grammar builds `[last, ..., first]`.
        db.add_members("A", &["b()".to_string(), "+int a".to_string()]);
        let c = &db.classes["A"];
        assert_eq!(c.members[0].text, "\\+int a");
        assert_eq!(c.methods[0].text, "b()");
    }

    #[test]
    fn lollipop_makes_an_interface_node() {
        let mut db = ClassDb::new();
        db.add_relation(ClassRelation {
            id1: "Engine".into(),
            id2: "Startable".into(),
            relation_title1: "none".into(),
            relation_title2: "none".into(),
            title: None,
            relation: Relation {
                type1: RelationType::None,
                type2: RelationType::Lollipop,
                line_type: LineType::Line,
            },
        });
        assert_eq!(db.interfaces[0].label, "Startable");
        assert_eq!(db.relations[0].id2, "interface0");
        assert!(!db.classes.contains_key("Startable"));
    }

    #[test]
    fn nested_namespaces_create_ancestors() {
        let mut db = ClassDb::new();
        let q = db.add_namespace("A.B", None);
        assert_eq!(q, "A.B");
        assert!(!db.namespaces["A"].explicit);
        assert!(db.namespaces["A.B"].explicit);
        assert_eq!(db.namespaces["A.B"].parent.as_deref(), Some("A"));
        assert_eq!(db.namespaces["A"].children, vec!["A.B".to_string()]);
    }
}
