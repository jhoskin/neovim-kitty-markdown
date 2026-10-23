//! Port of `diagrams/treeView/db.ts` (`addNode`, `getRoot`, `getConfig`),
//! the `populate` function of `parser.ts`, and `icons.ts` (`detectIcon`,
//! `getNodeIcon`), mermaid 12.2.0.

use serde_json::Value;

use super::parser::Ast;
use crate::mermaid::config::Resolved;
use crate::mermaid::diagrams::quadrant::db::sanitize_text;

/// `NodeType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    File,
    Directory,
}

/// A `Node`. Children are indices into [`TreeViewDb::nodes`].
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: usize,
    pub level: i64,
    pub name: String,
    pub node_type: NodeType,
    /// An iconify reference, a built-in icon name, or `none`.
    pub icon: Option<String>,
    pub css_class: Option<String>,
    pub description: Option<String>,
    pub children: Vec<usize>,
}

/// The node tree: `nodes[0]` is the root, `{ id: 0, level: -1, name: '/',
/// nodeType: 'directory' }`, and ids are indices.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeViewDb {
    pub nodes: Vec<Node>,
    /// `state.records.stack`.
    stack: Vec<usize>,
    pub title: String,
    pub acc_title: String,
    pub acc_descr: String,
}

impl Default for TreeViewDb {
    fn default() -> Self {
        TreeViewDb::new()
    }
}

impl TreeViewDb {
    pub fn new() -> TreeViewDb {
        TreeViewDb {
            nodes: vec![Node {
                id: 0,
                level: -1,
                name: "/".to_string(),
                node_type: NodeType::Directory,
                icon: None,
                css_class: None,
                description: None,
                children: Vec::new(),
            }],
            stack: vec![0],
            title: String::new(),
            acc_title: String::new(),
            acc_descr: String::new(),
        }
    }

    pub fn root(&self) -> &Node {
        &self.nodes[0]
    }

    /// `addNode`: pop the stack down to the first node of a lower level,
    /// add the node as its child, and push it.
    pub fn add_node(
        &mut self,
        level: i64,
        name: String,
        node_type: NodeType,
        css_class: Option<String>,
        icon: Option<String>,
        description: Option<String>,
    ) {
        while let Some(&top) = self.stack.last()
            && level <= self.nodes[top].level
            && self.stack.len() > 1
        {
            self.stack.pop();
        }
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            level,
            name,
            node_type,
            icon,
            css_class,
            description,
            children: Vec::new(),
        });
        if let Some(&parent) = self.stack.last() {
            self.nodes[parent].children.push(id);
        }
        self.stack.push(id);
    }

    /// `populate(ast)` after `populateCommonDb`.
    pub fn populate(&mut self, ast: &Ast) {
        if let Some(t) = &ast.acc_title {
            self.acc_title = t.clone();
        }
        if let Some(d) = &ast.acc_descr {
            self.acc_descr = d.clone();
        }
        if let Some(t) = &ast.title {
            self.title = t.clone();
        }
        for node in &ast.nodes {
            let level = node.indent.map_or(0, |i| i as i64);
            let (name, node_type) = match node.name.strip_suffix('/') {
                Some(stripped) => (stripped.to_string(), NodeType::Directory),
                None => (node.name.clone(), NodeType::File),
            };
            // `node.classAnnotation || undefined`.
            let css_class = node.class_annotation.clone().filter(|c| !c.is_empty());
            // An empty `icon()` hides the icon.
            let icon = node.icon_annotation.as_ref().map(|i| {
                if i.is_empty() {
                    "none".to_string()
                } else {
                    i.clone()
                }
            });
            let description = node
                .desc_annotation
                .as_deref()
                .filter(|d| !d.is_empty())
                .map(sanitize_description);
            self.add_node(level, name, node_type, css_class, icon, description);
        }
    }
}

/// `sanitizeText` on a description, which `.text()` then shows as
/// written: DOMPurify parses the text as HTML and serializes it again, so
/// a bare `&` comes back as `&amp;`, a `<` or `>` that opens no tag as
/// `&lt;` or `&gt;`, and a no-break space as `&nbsp;`. Markup is left as
/// written; the purifier's tag rules are not ported.
fn sanitize_description(text: &str) -> String {
    let amp = sanitize_text(text);
    let chars: Vec<char> = amp.chars().collect();
    let mut out = String::with_capacity(amp.len());
    for (i, c) in chars.iter().enumerate() {
        match c {
            '<' if !chars
                .get(i + 1)
                .is_some_and(|n| n.is_ascii_alphabetic() || matches!(n, '/' | '!' | '?')) =>
            {
                out.push_str("&lt;");
            }
            '>' => out.push_str("&gt;"),
            '\u{a0}' => out.push_str("&nbsp;"),
            _ => out.push(*c),
        }
    }
    out
}

/// `getConfig()`: `DEFAULT_CONFIG.treeView` merged with the site config.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeViewConfig {
    pub use_max_width: bool,
    pub row_indent: f64,
    pub padding_x: f64,
    pub padding_y: f64,
    pub line_thickness: f64,
    pub show_icons: bool,
    pub default_icon_pack: String,
    pub filename_icons: serde_json::Map<String, Value>,
    pub extension_icons: serde_json::Map<String, Value>,
}

impl TreeViewConfig {
    pub fn from_resolved(resolved: &Resolved) -> TreeViewConfig {
        TreeViewConfig::from_section(&resolved.config.tree_view)
    }

    pub fn from_section(section: &Value) -> TreeViewConfig {
        let num =
            |key: &str, default: f64| section.get(key).and_then(Value::as_f64).unwrap_or(default);
        let flag = |key: &str, default: bool| match section.get(key) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Null) | None => default,
            Some(Value::Number(n)) => n.as_f64().is_some_and(|v| v != 0.0),
            Some(Value::String(s)) => !s.is_empty(),
            Some(_) => true,
        };
        let map = |key: &str| match section.get(key) {
            Some(Value::Object(m)) => m.clone(),
            _ => serde_json::Map::new(),
        };
        TreeViewConfig {
            use_max_width: flag("useMaxWidth", true),
            row_indent: num("rowIndent", 10.0),
            padding_x: num("paddingX", 5.0),
            padding_y: num("paddingY", 5.0),
            line_thickness: num("lineThickness", 1.0),
            show_icons: flag("showIcons", false),
            default_icon_pack: section
                .get("defaultIconPack")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            filename_icons: map("filenameIcons"),
            extension_icons: map("extensionIcons"),
        }
    }
}

/// `treeViewIcons.prefix`.
pub const BUILTIN_PACK: &str = "mermaid-treeview";
/// The icons of the built-in pack.
const BUILTIN_ICONS: [&str; 2] = ["folder", "file"];

/// A map value as JS reads it: a non-empty string, else nothing.
fn map_icon(map: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// `detectIcon`: an exact filename match first, then the extension from
/// the last dot (not a leading one), lower-cased, with or without the dot.
fn detect_icon(name: &str, config: &TreeViewConfig) -> Option<String> {
    if let Some(icon) = map_icon(&config.filename_icons, name) {
        return Some(icon);
    }
    let dot = name.rfind('.')?;
    if dot == 0 {
        return None;
    }
    let ext = name[dot..].to_lowercase();
    // `extensionIcons?.[ext] ?? extensionIcons?.[ext.slice(1)]`: a present
    // empty string stops the `??` and then counts as no match.
    match config.extension_icons.get(&ext) {
        Some(Value::Null) | None => map_icon(&config.extension_icons, &ext[1..]),
        Some(_) => map_icon(&config.extension_icons, &ext),
    }
}

/// `qualifyIcon`: built-ins win, then the `defaultIconPack`.
fn qualify_icon(icon: &str, default_icon_pack: &str) -> String {
    if icon.contains(':') {
        return icon.to_string();
    }
    if BUILTIN_ICONS.contains(&icon) || default_icon_pack.is_empty() {
        return format!("{BUILTIN_PACK}:{icon}");
    }
    format!("{default_icon_pack}:{icon}")
}

/// `getNodeIcon`: the fully qualified icon to draw for a node, if any.
pub fn node_icon(node: &Node, config: &TreeViewConfig) -> Option<String> {
    match node.icon.as_deref() {
        Some("none") => return None,
        Some(icon) if !icon.is_empty() => {
            return Some(qualify_icon(icon, &config.default_icon_pack));
        }
        _ => {}
    }
    if !config.show_icons {
        return None;
    }
    if node.node_type == NodeType::File
        && let Some(detected) = detect_icon(&node.name, config)
    {
        if detected == "none" {
            return None;
        }
        return Some(qualify_icon(&detected, &config.default_icon_pack));
    }
    let builtin = match node.node_type {
        NodeType::Directory => "folder",
        NodeType::File => "file",
    };
    Some(format!("{BUILTIN_PACK}:{builtin}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::diagrams::treeview::parser;
    use serde_json::json;

    fn db(text: &str) -> TreeViewDb {
        let ast = parser::parse(text).unwrap_or_else(|e| panic!("{}", e.message));
        let mut db = TreeViewDb::new();
        db.populate(&ast);
        db
    }

    fn names(db: &TreeViewDb, id: usize) -> Vec<&str> {
        db.nodes[id]
            .children
            .iter()
            .map(|c| db.nodes[*c].name.as_str())
            .collect()
    }

    #[test]
    fn indentation_builds_the_hierarchy() {
        let db = db("treeView-beta\n  a/\n    b\n   c\n  d\ne\n");
        assert_eq!(names(&db, 0), vec!["a", "d", "e"]);
        assert_eq!(names(&db, 1), vec!["b", "c"]);
        assert_eq!(db.nodes[1].node_type, NodeType::Directory);
        assert_eq!(db.nodes[2].node_type, NodeType::File);
    }

    #[test]
    fn annotations_fill_the_node() {
        let db = db("treeView-beta\n  a :::highlight icon() ## x & y\n  b icon(folder)\n");
        let a = &db.nodes[1];
        assert_eq!(a.css_class.as_deref(), Some("highlight"));
        assert_eq!(a.icon.as_deref(), Some("none"));
        assert_eq!(a.description.as_deref(), Some("x &amp; y"));
        assert_eq!(db.nodes[2].icon.as_deref(), Some("folder"));
    }

    #[test]
    fn icons_resolve_like_get_node_icon() {
        let config = TreeViewConfig::from_section(&json!({
            "showIcons": true,
            "defaultIconPack": "pack",
            "filenameIcons": { "Dockerfile": "docker" },
            "extensionIcons": { ".ts": "typescript", "txt": "none", ".md": "" }
        }));
        let db = db(
            "treeView-beta\n  src/\n  a.TS\n  Dockerfile\n  n.txt\n  r.md\n  .env\n  x icon(file)\n  y icon(rust)\n  z icon(none)\n",
        );
        let icons: Vec<Option<String>> = db.nodes.iter().map(|n| node_icon(n, &config)).collect();
        let s = |v: &str| Some(v.to_string());
        assert_eq!(
            icons,
            vec![
                s("mermaid-treeview:folder"),
                s("mermaid-treeview:folder"),
                s("pack:typescript"),
                s("pack:docker"),
                None,
                s("mermaid-treeview:file"),
                s("mermaid-treeview:file"),
                s("mermaid-treeview:file"),
                s("pack:rust"),
                None,
            ]
        );
        let hidden = TreeViewConfig::from_section(&json!({}));
        assert_eq!(node_icon(&db.nodes[1], &hidden), None);
        assert_eq!(node_icon(&db.nodes[7], &hidden), s("mermaid-treeview:file"));
    }

    #[test]
    fn descriptions_escape_what_the_purifier_escapes() {
        assert_eq!(sanitize_description("a < b > c"), "a &lt; b &gt; c");
        assert_eq!(sanitize_description("x&nbsp;y"), "x&nbsp;y");
    }
}
