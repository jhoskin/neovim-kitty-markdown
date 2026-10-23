//! Port of `diagrams/treemap/db.ts` (`TreeMapDB`), `utils.ts`
//! (`buildHierarchy`), the `populate` function of `parser.ts`, and
//! `getConfig()` of the treemap section.

use indexmap::IndexMap;
use serde_json::Value;

use super::parser::{Ast, ItemKind, Row};
use crate::mermaid::config::Resolved;
use crate::mermaid::shapes::util::is_label_style;

/// A `TreemapNode`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreemapNode {
    pub name: String,
    /// `[]` for a section, `undefined` for a leaf.
    pub children: Option<Vec<TreemapNode>>,
    pub value: Option<f64>,
    pub class_selector: Option<String>,
    /// The class styles as `key:value` strings, set only when not empty.
    pub css_compiled_styles: Option<Vec<String>>,
}

/// A `DiagramStyleClassDef` as `addClass` fills it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyleClass {
    pub id: String,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreemapDb {
    /// `outerNodes`: the level-0 nodes, the children of `getRoot()`.
    pub outer_nodes: Vec<TreemapNode>,
    /// Insertion order, as the JS `Map` keeps it.
    pub classes: IndexMap<String, StyleClass>,
    pub title: String,
    pub acc_title: String,
    pub acc_descr: String,
}

/// One flat item `populate` collects before `buildHierarchy`.
struct FlatItem {
    level: usize,
    name: String,
    leaf: bool,
    value: Option<f64>,
    class_selector: Option<String>,
    css_compiled_styles: Option<Vec<String>>,
}

impl TreemapDb {
    pub fn new() -> TreemapDb {
        TreemapDb::default()
    }

    /// `getRoot()`: `{ name: '', children: outerNodes }`.
    pub fn root(&self) -> TreemapNode {
        TreemapNode {
            name: String::new(),
            children: Some(self.outer_nodes.clone()),
            ..TreemapNode::default()
        }
    }

    /// `addClass(id, style)`: `\,` protects a comma, the other commas
    /// separate declarations like `;` does.
    pub fn add_class(&mut self, id: &str, style: &str) {
        let class = self
            .classes
            .entry(id.to_string())
            .or_insert_with(|| StyleClass {
                id: id.to_string(),
                ..StyleClass::default()
            });
        let protected = style
            .replace("\\,", "\u{a7}\u{a7}\u{a7}")
            .replace(',', ";")
            .replace("\u{a7}\u{a7}\u{a7}", ",");
        for s in protected.split(';') {
            // `isLabelStyle` sees the whole declaration, so only a bare
            // property name ever lands here.
            if is_label_style(s) {
                class.text_styles.push(s.to_string());
            }
            class.styles.push(s.to_string());
        }
    }

    /// `getStylesForClass(classSelector)`.
    pub fn styles_for_class(&self, class: &str) -> Vec<String> {
        self.classes
            .get(class)
            .map(|c| c.styles.clone())
            .unwrap_or_default()
    }

    /// `setDiagramTitle` and friends from `populateCommonDb`, which set a
    /// field only when the AST value is truthy.
    pub fn set_diagram_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    /// `populate(ast, db)`: the common fields, every `classDef` first, then
    /// the items as a hierarchy.
    pub fn populate(&mut self, ast: &Ast) {
        if let Some(d) = ast.acc_descr.as_deref().filter(|s| !s.is_empty()) {
            self.acc_descr = d.to_string();
        }
        if let Some(t) = ast.acc_title.as_deref().filter(|s| !s.is_empty()) {
            self.acc_title = t.to_string();
        }
        if let Some(t) = ast.title.as_deref().filter(|s| !s.is_empty()) {
            self.title = t.to_string();
        }
        for row in &ast.rows {
            if let Row::ClassDef {
                class_name,
                style_text,
            } = row
            {
                self.add_class(class_name, style_text.as_deref().unwrap_or(""));
            }
        }
        let mut items = Vec::new();
        for row in &ast.rows {
            let Row::Item { indent, item } = row else {
                continue;
            };
            let styles = item
                .class_selector
                .as_deref()
                .map(|c| self.styles_for_class(c))
                .unwrap_or_default();
            items.push(FlatItem {
                // `row.indent ? parseInt(row.indent) : 0`.
                level: indent.unwrap_or(0),
                name: item.name.clone(),
                leaf: item.kind == ItemKind::Leaf,
                value: item.value,
                class_selector: item.class_selector.clone(),
                css_compiled_styles: (!styles.is_empty()).then_some(styles),
            });
        }
        self.outer_nodes = build_hierarchy(items);
    }
}

/// `buildHierarchy(items)`: each item goes under the nearest earlier
/// section with a smaller level; a leaf never takes children.
fn build_hierarchy(items: Vec<FlatItem>) -> Vec<TreemapNode> {
    // An arena of nodes with child indices, turned into the tree at the end.
    struct Slot {
        node: TreemapNode,
        children: Vec<usize>,
    }
    let mut slots: Vec<Slot> = Vec::new();
    let mut roots = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for item in items {
        let node = TreemapNode {
            name: item.name,
            children: if item.leaf { None } else { Some(Vec::new()) },
            value: if item.leaf { item.value } else { None },
            class_selector: item.class_selector,
            css_compiled_styles: item.css_compiled_styles,
        };
        let index = slots.len();
        slots.push(Slot {
            node,
            children: Vec::new(),
        });
        while stack.last().is_some_and(|&(_, level)| level >= item.level) {
            stack.pop();
        }
        match stack.last() {
            None => roots.push(index),
            Some(&(parent, _)) => slots[parent].children.push(index),
        }
        if !item.leaf {
            stack.push((index, item.level));
        }
    }
    fn materialize(slots: &[Slot], index: usize) -> TreemapNode {
        let slot = &slots[index];
        let mut node = slot.node.clone();
        if let Some(children) = node.children.as_mut() {
            *children = slot
                .children
                .iter()
                .map(|&c| materialize(slots, c))
                .collect();
        }
        node
    }
    roots.iter().map(|&i| materialize(&slots, i)).collect()
}

/// `TreeMapDB.getConfig()`: the schema defaults merged with the user's
/// treemap section.
#[derive(Debug, Clone, PartialEq)]
pub struct TreemapConfig {
    /// `config.padding ?? 10`: the inner padding between cells.
    pub padding: f64,
    /// `config.diagramPadding ?? 8`.
    pub diagram_padding: f64,
    /// `config.showValues !== false`.
    pub show_values: bool,
    /// `config.nodeWidth`; the renderer multiplies a truthy value by 10.
    pub node_width: f64,
    pub node_height: f64,
    /// `config.valueFormat || ','`.
    pub value_format: String,
    pub use_max_width: bool,
}

impl TreemapConfig {
    pub fn from_resolved(resolved: &Resolved) -> TreemapConfig {
        let section = &resolved.config.treemap;
        let num = |key: &str| section.get(key).and_then(Value::as_f64);
        // `??`: a missing or null value takes the fallback.
        let nullish = |key: &str, default: f64| num(key).unwrap_or(default);
        // `||`: a falsy value takes the fallback (the renderer's 960 and
        // 500 when the multiplied size is not truthy).
        let truthy_num = |key: &str| num(key).filter(|v| *v != 0.0 && !v.is_nan()).unwrap_or(0.0);
        let value_format = match section.get("valueFormat") {
            Some(Value::String(s)) if !s.is_empty() => s.clone(),
            _ => ",".to_string(),
        };
        TreemapConfig {
            padding: nullish("padding", 10.0),
            diagram_padding: nullish("diagramPadding", 8.0),
            show_values: section.get("showValues") != Some(&Value::Bool(false)),
            node_width: truthy_num("nodeWidth"),
            node_height: truthy_num("nodeHeight"),
            value_format,
            use_max_width: match section.get("useMaxWidth") {
                Some(Value::Bool(b)) => *b,
                Some(Value::Null) | None => false,
                Some(_) => true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::diagrams::treemap::parser::parse;

    fn db_for(text: &str) -> TreemapDb {
        let mut db = TreemapDb::new();
        db.populate(&parse(text).unwrap());
        db
    }

    #[test]
    fn indentation_builds_the_tree() {
        let db = db_for(
            "treemap\n\"A\"\n  \"A1\": 1\n  \"A2\"\n    \"A21\": 2\n  \"A3\": 3\n\"B\"\n \"B1\": 4\n",
        );
        assert_eq!(db.outer_nodes.len(), 2);
        let a = &db.outer_nodes[0];
        let names: Vec<_> = a
            .children
            .as_ref()
            .unwrap()
            .iter()
            .map(|n| n.name.as_str())
            .collect();
        assert_eq!(names, ["A1", "A2", "A3"]);
        assert_eq!(
            a.children.as_ref().unwrap()[1].children.as_ref().unwrap()[0].value,
            Some(2.0)
        );
        assert_eq!(db.outer_nodes[1].children.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn a_deeper_leaf_after_a_leaf_joins_the_section_above() {
        let db = db_for("treemap\n\"A\"\n  \"x\": 1\n    \"y\": 2\n");
        let children = db.outer_nodes[0].children.as_ref().unwrap();
        assert_eq!(children.len(), 2);
        assert!(children[1].children.is_none());
    }

    #[test]
    fn class_styles_reach_the_nodes() {
        let db = db_for(
            "treemap\n\"A\":::hot\n  \"B\": 1:::hot\n  \"C\": 2:::cold\nclassDef hot fill:red,color:blue\\,x\n",
        );
        assert_eq!(db.classes["hot"].styles, ["fill:red", "color:blue,x"]);
        let a = &db.outer_nodes[0];
        assert_eq!(
            a.css_compiled_styles.as_deref(),
            Some(&["fill:red".to_string(), "color:blue,x".to_string()][..])
        );
        let c = &a.children.as_ref().unwrap()[1];
        assert_eq!(c.class_selector.as_deref(), Some("cold"));
        assert_eq!(c.css_compiled_styles, None);
    }
}
