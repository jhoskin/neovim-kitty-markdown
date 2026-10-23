//! Port of `diagrams/flowchart/flowDb.ts` (mermaid 12.2.0): the `FlowDB`
//! the grammar actions fill, and `getData`, which turns it into the
//! unified renderer's [`LayoutData`].
//!
//! Vertices keep first-mention order (`Map` insertion order in JS), edges
//! keep declaration order, and subgraphs are stored in the order the
//! grammar reduces them (a nested subgraph closes before its parent).
//! `click` and `href` statements are stored and not drawn; tooltips and
//! callbacks have no meaning in a PNG.

use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::mermaid::config::defaults::MermaidConfig;
use crate::mermaid::edges::{Edge, Pattern, Thickness};
use crate::mermaid::layout::unified::LayoutData;
use crate::mermaid::shapes::{self, Dir, Look, Node};

/// `MERMAID_DOM_ID_PREFIX`.
const DOM_ID_PREFIX: &str = "flowchart-";

/// `FlowText`: a label and how it was written (`text`, `string`,
/// `markdown`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlowText {
    pub text: String,
    pub kind: String,
}

impl FlowText {
    pub fn new(text: &str, kind: &str) -> FlowText {
        FlowText {
            text: text.to_string(),
            kind: kind.to_string(),
        }
    }
}

/// `FlowVertex` of `types.ts`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlowVertex {
    pub id: String,
    pub label_type: String,
    pub dom_id: String,
    pub styles: Vec<String>,
    pub classes: Vec<String>,
    pub text: Option<String>,
    /// `type`: the bracket shape (`square`, `round`, `diamond`, ...) or a
    /// shape name from `@{ shape: ... }`.
    pub kind: Option<String>,
    pub dir: Option<String>,
    pub props: Map<String, Value>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub have_callback: bool,
    pub icon: Option<String>,
    pub form: Option<String>,
    pub pos: Option<String>,
    pub img: Option<String>,
    pub constraint: Option<String>,
    pub asset_width: Option<f64>,
    pub asset_height: Option<f64>,
}

/// What `destructLink` returns and the grammar extends into the `link`
/// value: `{ type, stroke, length, text, id }`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LinkInfo {
    pub kind: Option<String>,
    pub stroke: Option<String>,
    pub length: Option<f64>,
    pub text: Option<FlowText>,
    /// `LINK_ID` as lexed, with its trailing `@`.
    pub id: Option<String>,
}

/// `FlowEdge` of `types.ts`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlowEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    pub kind: Option<String>,
    pub text: String,
    pub label_type: String,
    pub classes: Vec<String>,
    pub is_user_defined_id: bool,
    pub interpolate: Option<String>,
    pub stroke: Option<String>,
    pub length: Option<f64>,
    pub style: Option<Vec<String>>,
    pub animate: Option<Value>,
    pub animation: Option<Value>,
}

/// `FlowClass`: a `classDef`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlowClass {
    pub id: String,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

/// `FlowSubGraph`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlowSubGraph {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    pub dir: Option<String>,
    pub label_type: String,
    pub metadata: Option<Map<String, Value>>,
}

/// One entry of the list a subgraph body reduces to: a vertex or subgraph
/// id, a separator token's text, or a `direction` statement.
#[derive(Debug, Clone, PartialEq)]
pub enum DocItem {
    Id(String),
    Dir(String),
}

/// The values `linkStyle` positions take: `default` or an index as lexed.
#[derive(Debug, Clone, PartialEq)]
pub enum LinkPos {
    Default,
    Index(String),
}

/// The config `FlowDB` reads through `getConfig()`.
#[derive(Debug, Clone, PartialEq)]
pub struct DbConfig {
    pub max_edges: f64,
    pub inherit_dir: bool,
    pub look: String,
    pub curve: String,
    pub padding: f64,
    pub min_node_width: f64,
}

impl DbConfig {
    pub fn from_config(config: &MermaidConfig) -> DbConfig {
        DbConfig {
            max_edges: *config.max_edges,
            inherit_dir: config.flowchart.inherit_dir,
            look: config.look.clone(),
            curve: config.flowchart.curve.clone(),
            padding: *config.flowchart.padding,
            min_node_width: *config.flowchart.min_node_width,
        }
    }
}

impl Default for DbConfig {
    fn default() -> DbConfig {
        DbConfig::from_config(&MermaidConfig::default())
    }
}

/// `FlowDB`.
#[derive(Debug, Clone)]
pub struct FlowDb {
    config: DbConfig,
    vertex_counter: usize,
    pub vertices: IndexMap<String, FlowVertex>,
    pub edges: Vec<FlowEdge>,
    /// `edges.defaultInterpolate` / `edges.defaultStyle`.
    pub default_interpolate: Option<String>,
    pub default_style: Option<Vec<String>>,
    pub classes: IndexMap<String, FlowClass>,
    pub sub_graphs: Vec<FlowSubGraph>,
    /// `subGraphLookup`: id to the index in `sub_graphs`.
    sub_graph_lookup: IndexMap<String, usize>,
    pub tooltips: IndexMap<String, String>,
    sub_count: usize,
    first_graph_flag: bool,
    direction: Option<String>,
    pub acc_title: String,
    pub acc_description: String,
}

/// `sanitizeText` under the default `securityLevel: strict` with HTML
/// labels: DOMPurify keeps ordinary markup, and the label measurer reads
/// `<br>` and entities itself, so the text passes through. The script
/// removal DOMPurify does has no effect on a PNG.
fn sanitize_text(text: &str) -> String {
    text.to_string()
}

/// `sanitizeNodeLabelType`.
fn sanitize_node_label_type(label_type: Option<&str>) -> String {
    match label_type {
        Some(t @ ("markdown" | "string" | "text")) => t.to_string(),
        _ => "markdown".to_string(),
    }
}

/// Strips one pair of surrounding double quotes, as `addVertex` and
/// `addSingleLink` do.
fn strip_quotes(text: String) -> String {
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        text[1..text.len() - 1].to_string()
    } else if text == "\"" {
        // `'"'.startsWith('"') && '"'.endsWith('"')` holds in JS and
        // `substring(1, 0)` is the empty string.
        String::new()
    } else {
        text
    }
}

/// JavaScript `String.prototype.trim`.
fn js_trim(s: &str) -> &str {
    crate::mermaid::directive::js_trim(s)
}

/// A metadata value read as a string the way JS template code reads it.
fn meta_str(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        other => Some(other.to_string()),
    }
}

/// JS truthiness of a metadata value.
fn meta_truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(_) => true,
    }
}

/// `Number(value)` for `w` and `h`.
fn meta_number(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => {
            let t = js_trim(s);
            if t.is_empty() {
                0.0
            } else {
                t.parse().unwrap_or(f64::NAN)
            }
        }
        Value::Bool(b) => f64::from(u8::from(*b)),
        _ => f64::NAN,
    }
}

/// `yaml.load(yamlData, { schema: JSON_SCHEMA })` for the shape data.
fn parse_metadata(metadata: &str) -> Result<Map<String, Value>, String> {
    let yaml = if metadata.contains('\n') {
        format!("{metadata}\n")
    } else {
        format!("{{\n{metadata}\n}}")
    };
    let value: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&yaml).map_err(|e| format!("YAMLException: {e}"))?;
    let json = serde_json::to_value(value).map_err(|e| e.to_string())?;
    match json {
        Value::Object(map) => Ok(map),
        Value::Null => Ok(Map::new()),
        _ => Ok(Map::new()),
    }
}

impl Default for FlowDb {
    fn default() -> FlowDb {
        FlowDb::new(DbConfig::default())
    }
}

impl FlowDb {
    pub fn new(config: DbConfig) -> FlowDb {
        FlowDb {
            config,
            vertex_counter: 0,
            vertices: IndexMap::new(),
            edges: Vec::new(),
            default_interpolate: None,
            default_style: None,
            classes: IndexMap::new(),
            sub_graphs: Vec::new(),
            sub_graph_lookup: IndexMap::new(),
            tooltips: IndexMap::new(),
            sub_count: 0,
            first_graph_flag: true,
            direction: None,
            acc_title: String::new(),
            acc_description: String::new(),
        }
    }

    /// `firstGraph()`: true once, for the lexer's `yy.lex.firstGraph()`.
    pub fn first_graph(&mut self) -> bool {
        if self.first_graph_flag {
            self.first_graph_flag = false;
            return true;
        }
        false
    }

    /// `addVertex(id, textObj, type, style, classes, dir, props, metadata)`.
    #[allow(clippy::too_many_arguments)]
    pub fn add_vertex(
        &mut self,
        id: &str,
        text_obj: Option<&FlowText>,
        kind: Option<&str>,
        style: Option<&[String]>,
        classes: Option<&[String]>,
        dir: Option<&str>,
        props: Option<Map<String, Value>>,
        metadata: Option<&str>,
    ) -> Result<(), String> {
        if js_trim(id).is_empty() {
            return Ok(());
        }
        let doc = match metadata {
            Some(m) => Some(parse_metadata(m)?),
            None => None,
        };

        // Metadata for a declared subgraph (`sub1@{ view: collapsed }`).
        if let Some(&index) = self.sub_graph_lookup.get(id)
            && let Some(doc) = &doc
        {
            let sg = &mut self.sub_graphs[index];
            let mut merged = sg.metadata.clone().unwrap_or_default();
            for (k, v) in doc {
                merged.insert(k.clone(), v.clone());
            }
            sg.metadata = Some(merged);
            return Ok(());
        }

        // Metadata for an edge with a user id (`e1@{ curve: linear }`).
        if let Some(edge) = self.edges.iter_mut().find(|e| e.id == id) {
            if let Some(doc) = &doc {
                if let Some(v) = doc.get("animate") {
                    edge.animate = Some(v.clone());
                }
                if let Some(v) = doc.get("animation") {
                    edge.animation = Some(v.clone());
                }
                if let Some(v) = doc.get("curve") {
                    edge.interpolate = meta_str(v);
                }
            }
            return Ok(());
        }

        if !self.vertices.contains_key(id) {
            self.vertices.insert(
                id.to_string(),
                FlowVertex {
                    id: id.to_string(),
                    label_type: "text".to_string(),
                    dom_id: format!("{DOM_ID_PREFIX}{id}-{}", self.vertex_counter),
                    ..FlowVertex::default()
                },
            );
        }
        self.vertex_counter += 1;
        let Some(vertex) = self.vertices.get_mut(id) else {
            return Ok(());
        };

        match text_obj {
            Some(t) => {
                let txt = strip_quotes(sanitize_text(js_trim(&t.text)));
                vertex.label_type = t.kind.clone();
                vertex.text = Some(txt);
            }
            None => {
                if vertex.text.is_none() {
                    vertex.text = Some(id.to_string());
                }
            }
        }
        if let Some(kind) = kind {
            vertex.kind = Some(kind.to_string());
        }
        if let Some(style) = style {
            vertex.styles.extend(style.iter().cloned());
        }
        if let Some(classes) = classes {
            vertex.classes.extend(classes.iter().cloned());
        }
        if let Some(dir) = dir {
            vertex.dir = Some(dir.to_string());
        }
        if let Some(props) = props {
            for (k, v) in props {
                vertex.props.insert(k, v);
            }
        }

        if let Some(doc) = doc {
            if let Some(shape) = doc.get("shape").and_then(meta_str)
                && !shape.is_empty()
            {
                if shape != shape.to_lowercase() || shape.contains('_') {
                    return Err(format!(
                        "No such shape: {shape}. Shape names should be lowercase."
                    ));
                }
                if shapes::lookup(&shape).is_none() {
                    return Err(format!("No such shape: {shape}."));
                }
                vertex.kind = Some(shape);
            }
            if meta_truthy(doc.get("label")) {
                vertex.text = doc.get("label").and_then(meta_str);
                vertex.label_type =
                    sanitize_node_label_type(doc.get("labelType").and_then(Value::as_str));
            }
            let label_blank = doc
                .get("label")
                .and_then(meta_str)
                .is_none_or(|l| js_trim(&l).is_empty());
            if meta_truthy(doc.get("icon")) {
                vertex.icon = doc.get("icon").and_then(meta_str);
                if label_blank && vertex.text.as_deref() == Some(id) {
                    vertex.text = Some(String::new());
                }
            }
            if meta_truthy(doc.get("form")) {
                vertex.form = doc.get("form").and_then(meta_str);
            }
            if meta_truthy(doc.get("pos")) {
                vertex.pos = doc.get("pos").and_then(meta_str);
            }
            if meta_truthy(doc.get("img")) {
                vertex.img = doc.get("img").and_then(meta_str);
                if label_blank && vertex.text.as_deref() == Some(id) {
                    vertex.text = Some(String::new());
                }
            }
            if meta_truthy(doc.get("constraint")) {
                vertex.constraint = doc.get("constraint").and_then(meta_str);
            }
            if let Some(w) = doc.get("w").filter(|v| meta_truthy(Some(v))) {
                vertex.asset_width = Some(meta_number(w));
            }
            if let Some(h) = doc.get("h").filter(|v| meta_truthy(Some(v))) {
                vertex.asset_height = Some(meta_number(h));
            }
        }
        Ok(())
    }

    /// `addSingleLink(start, end, type, id)`.
    pub fn add_single_link(
        &mut self,
        start: &str,
        end: &str,
        link: &LinkInfo,
        id: Option<&str>,
    ) -> Result<(), String> {
        let mut edge = FlowEdge {
            start: start.to_string(),
            end: end.to_string(),
            text: String::new(),
            label_type: "text".to_string(),
            interpolate: self.default_interpolate.clone(),
            ..FlowEdge::default()
        };
        if let Some(t) = &link.text {
            edge.text = strip_quotes(sanitize_text(js_trim(&t.text)));
            edge.label_type = sanitize_node_label_type(Some(&t.kind));
        }
        edge.kind = link.kind.clone();
        edge.stroke = link.stroke.clone();
        edge.length = link.length.map(|l| if l > 10.0 { 10.0 } else { l });

        match id {
            Some(id) if !self.edges.iter().any(|e| e.id == id) => {
                edge.id = id.to_string();
                edge.is_user_defined_id = true;
            }
            _ => {
                let existing = self
                    .edges
                    .iter()
                    .filter(|e| e.start == edge.start && e.end == edge.end)
                    .count();
                let counter = if existing == 0 { 0 } else { existing + 1 };
                edge.id = format!("L_{}_{}_{counter}", edge.start, edge.end);
            }
        }

        if (self.edges.len() as f64) < self.config.max_edges {
            self.edges.push(edge);
            Ok(())
        } else {
            Err(format!(
                "Edge limit exceeded. {} edges found, but the limit is {}.\n\nInitialize mermaid with maxEdges set to a higher number to allow more edges.\nYou cannot set this config via configuration inside the diagram as it is a secure config.\nYou have to call mermaid.initialize.",
                self.edges.len(),
                self.config.max_edges
            ))
        }
    }

    /// `addLink(start[], end[], linkData)`: every start to every end; only
    /// the last start to the first end keeps a user-defined id.
    pub fn add_link(
        &mut self,
        starts: &[String],
        ends: &[String],
        link: &LinkInfo,
    ) -> Result<(), String> {
        let id = link.id.as_deref().map(|i| i.replacen('@', "", 1));
        for start in starts {
            for end in ends {
                let is_last_start = starts.last() == Some(start);
                let is_first_end = ends.first() == Some(end);
                if is_last_start && is_first_end {
                    self.add_single_link(start, end, link, id.as_deref())?;
                } else {
                    self.add_single_link(start, end, link, None)?;
                }
            }
        }
        Ok(())
    }

    /// Resolves a `linkStyle` index the way `this.edges[pos]` does with the
    /// lexed string: a canonical array index or nothing.
    fn edge_index(&self, pos: &str) -> Option<usize> {
        let n: usize = pos.parse().ok()?;
        (n.to_string() == pos && n < self.edges.len()).then_some(n)
    }

    /// `updateLinkInterpolate(positions, interpolate)`.
    pub fn update_link_interpolate(
        &mut self,
        positions: &[LinkPos],
        interpolate: &str,
    ) -> Result<(), String> {
        for pos in positions {
            match pos {
                LinkPos::Default => self.default_interpolate = Some(interpolate.to_string()),
                LinkPos::Index(i) => {
                    let Some(n) = self.edge_index(i) else {
                        return Err(
                            "TypeError: Cannot set properties of undefined (setting 'interpolate')"
                                .to_string(),
                        );
                    };
                    self.edges[n].interpolate = Some(interpolate.to_string());
                }
            }
        }
        Ok(())
    }

    /// `updateLink(positions, style)`. The positions arrive as strings
    /// from the lexer, so the bounds check (`typeof pos === 'number'`)
    /// never runs and an index past the end fails on the property write.
    pub fn update_link(&mut self, positions: &[LinkPos], style: &[String]) -> Result<(), String> {
        for pos in positions {
            match pos {
                LinkPos::Default => self.default_style = Some(style.to_vec()),
                LinkPos::Index(i) => {
                    let Some(n) = self.edge_index(i) else {
                        return Err(
                            "TypeError: Cannot set properties of undefined (setting 'style')"
                                .to_string(),
                        );
                    };
                    let mut style = style.to_vec();
                    if !style.is_empty() && !style.iter().any(|s| s.starts_with("fill")) {
                        style.push("fill:none".to_string());
                    }
                    self.edges[n].style = Some(style);
                }
            }
        }
        Ok(())
    }

    /// `addClass(ids, style)`: `classDef a,b fill:#f9f,stroke:#333`.
    pub fn add_class(&mut self, ids: &str, style: &[String]) {
        let joined = style
            .join(",")
            .replace("\\,", "§§§")
            .replace(',', ";")
            .replace("§§§", ",");
        let style: Vec<String> = joined.split(';').map(str::to_string).collect();
        for id in ids.split(',') {
            let class = self
                .classes
                .entry(id.to_string())
                .or_insert_with(|| FlowClass {
                    id: id.to_string(),
                    ..FlowClass::default()
                });
            for s in &style {
                if s.contains("color") {
                    class.text_styles.push(s.replacen("fill", "bgFill", 1));
                }
                class.styles.push(s.clone());
            }
        }
    }

    /// `setDirection(dir)`.
    pub fn set_direction(&mut self, dir: &str) {
        let mut d = js_trim(dir).to_string();
        if d.contains('<') {
            d = "RL".to_string();
        }
        if d.contains('^') {
            d = "BT".to_string();
        }
        if d.contains('>') {
            d = "LR".to_string();
        }
        if d.contains('v') {
            d = "TB".to_string();
        }
        if d == "TD" {
            d = "TB".to_string();
        }
        self.direction = Some(d);
    }

    /// `getDirection()`.
    pub fn direction(&self) -> Option<&str> {
        self.direction.as_deref().map(js_trim)
    }

    /// `setClass(ids, className)`: vertices, edges with that id, and
    /// subgraphs.
    pub fn set_class(&mut self, ids: &str, class_name: &str) {
        for id in ids.split(',') {
            if let Some(v) = self.vertices.get_mut(id) {
                v.classes.push(class_name.to_string());
            }
            if let Some(e) = self.edges.iter_mut().find(|e| e.id == id) {
                e.classes.push(class_name.to_string());
            }
            if let Some(&i) = self.sub_graph_lookup.get(id) {
                self.sub_graphs[i].classes.push(class_name.to_string());
            }
        }
    }

    /// `setTooltip(ids, tooltip)`.
    pub fn set_tooltip(&mut self, ids: &str, tooltip: &str) {
        let tooltip = sanitize_text(tooltip);
        for id in ids.split(',') {
            self.tooltips.insert(id.to_string(), tooltip.clone());
        }
    }

    /// `setClickEvent(ids, functionName, functionArgs)`: the callback only
    /// binds under `securityLevel: loose`, and a PNG has no click; what is
    /// visible is the `clickable` class.
    pub fn set_click_event(&mut self, ids: &str) {
        for id in ids.split(',') {
            if let Some(v) = self.vertices.get_mut(id) {
                v.have_callback = true;
            }
        }
        self.set_class(ids, "clickable");
    }

    /// `setLink(ids, linkStr, target)`.
    pub fn set_link(&mut self, ids: &str, link: &str, target: Option<&str>) {
        for id in ids.split(',') {
            if let Some(v) = self.vertices.get_mut(id) {
                v.link = Some(link.to_string());
                v.link_target = target.map(str::to_string);
            }
        }
        self.set_class(ids, "clickable");
    }

    /// `addSubGraph(_id, list, _title)`. `same` is the `_id === _title`
    /// identity check of the grammar's `subgraph id` form.
    pub fn add_sub_graph(
        &mut self,
        id: Option<&FlowText>,
        list: &[DocItem],
        title: Option<&FlowText>,
        same: bool,
    ) -> Result<String, String> {
        let Some(id_text) = id else {
            return Err(
                "TypeError: Cannot read properties of undefined (reading 'text')".to_string(),
            );
        };
        let mut id: Option<String> = Some(js_trim(&id_text.text).to_string());
        let title_obj = title.cloned().unwrap_or_default();
        let title_kind = title.map(|t| t.kind.clone());
        if same && title_obj.text.chars().any(char::is_whitespace) {
            id = None;
        }

        // `uniq(list.flat())`.
        let mut dir: Option<String> = None;
        let mut node_list: Vec<String> = Vec::new();
        for item in list {
            match item {
                DocItem::Dir(d) => dir = Some(d.clone()),
                DocItem::Id(s) => {
                    if js_trim(s).is_empty() || node_list.contains(s) {
                        continue;
                    }
                    node_list.push(s.clone());
                }
            }
        }
        if dir.is_none() && self.config.inherit_dir {
            dir = self.direction().map(str::to_string);
        }

        let id = id.unwrap_or_else(|| format!("subGraph{}", self.sub_count));
        let title = sanitize_text(&title_obj.text);
        self.sub_count += 1;

        // `makeUniq`: drop members already claimed by another subgraph; a
        // subgraph never contains itself.
        let nodes: Vec<String> = node_list
            .into_iter()
            .filter(|n| !self.sub_graphs.iter().any(|sg| sg.nodes.contains(n)))
            .filter(|n| *n != id)
            .collect();

        let index = match self.sub_graphs.iter().position(|sg| sg.id == id) {
            Some(i) => {
                self.sub_graphs[i].nodes.extend(nodes);
                i
            }
            None => {
                self.sub_graphs.push(FlowSubGraph {
                    id: id.clone(),
                    nodes,
                    title: js_trim(&title).to_string(),
                    classes: Vec::new(),
                    dir,
                    label_type: sanitize_node_label_type(title_kind.as_deref()),
                    metadata: None,
                });
                self.sub_graphs.len() - 1
            }
        };
        self.sub_graph_lookup.insert(id.clone(), index);
        Ok(id)
    }

    /// `getCompiledStyles(classDefs)`.
    fn compiled_styles(&self, class_defs: &[String]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for name in class_defs {
            if let Some(class) = self.classes.get(name) {
                out.extend(class.styles.iter().cloned());
                out = out.iter().map(|s| js_trim(s).to_string()).collect();
                out.extend(class.text_styles.iter().cloned());
                out = out.iter().map(|s| js_trim(s).to_string()).collect();
            }
        }
        out
    }

    /// `getTypeFromVertex(vertex)`.
    fn type_from_vertex(vertex: &FlowVertex) -> String {
        if vertex.img.is_some() {
            return "imageSquare".to_string();
        }
        if vertex.icon.is_some() {
            return match vertex.form.as_deref() {
                Some("circle") => "iconCircle",
                Some("square") => "iconSquare",
                Some("rounded") => "iconRounded",
                _ => "icon",
            }
            .to_string();
        }
        match vertex.kind.as_deref() {
            None | Some("square") => "squareRect".to_string(),
            Some("round") => "roundedRect".to_string(),
            Some(other) => other.to_string(),
        }
    }

    /// `destructEdgeType(type)`.
    fn destruct_edge_type(kind: Option<&str>) -> (String, String) {
        match kind {
            Some(t @ ("arrow_point" | "arrow_circle" | "arrow_cross")) => {
                ("none".to_string(), t.to_string())
            }
            Some(t @ ("double_arrow_point" | "double_arrow_circle" | "double_arrow_cross")) => {
                let s = t.replacen("double_", "", 1);
                (s.clone(), s)
            }
            _ => ("none".to_string(), "arrow_point".to_string()),
        }
    }

    /// `getData()`: the subgraphs (last reduced first), then the vertices,
    /// then the edges, with collapsed subgraphs drawn as one node.
    pub fn get_data(&self) -> LayoutData {
        let look = Look::parse(&self.config.look).unwrap_or_default();
        let mut nodes: Vec<Node> = Vec::new();
        let mut edges: Vec<Edge> = Vec::new();

        let is_sub_graph = |id: &str| self.sub_graph_lookup.contains_key(id);
        let mut sub_graph_parent: IndexMap<String, String> = IndexMap::new();
        for sg in &self.sub_graphs {
            for child in &sg.nodes {
                if is_sub_graph(child) {
                    sub_graph_parent.insert(child.clone(), sg.id.clone());
                }
            }
        }
        let is_collapsed = |id: &str| {
            self.sub_graph_lookup
                .get(id)
                .and_then(|&i| self.sub_graphs[i].metadata.as_ref())
                .and_then(|m| m.get("view"))
                .is_some_and(|v| v == "collapsed")
        };
        let outermost_collapsed = |id: &str| -> Option<String> {
            let mut result = None;
            let mut seen: Vec<String> = Vec::new();
            let mut current = Some(id.to_string());
            while let Some(c) = current {
                if seen.contains(&c) {
                    break;
                }
                if is_collapsed(&c) {
                    result = Some(c.clone());
                }
                current = sub_graph_parent.get(&c).cloned();
                seen.push(c);
            }
            result
        };
        let mut hidden: Vec<String> = Vec::new();
        let mut collapsed_ancestor: IndexMap<String, String> = IndexMap::new();
        for sg in &self.sub_graphs {
            let Some(ancestor) = outermost_collapsed(&sg.id) else {
                continue;
            };
            if sg.id != ancestor {
                hidden.push(sg.id.clone());
                collapsed_ancestor.insert(sg.id.clone(), ancestor.clone());
            }
            for child in &sg.nodes {
                if *child == ancestor {
                    continue;
                }
                hidden.push(child.clone());
                collapsed_ancestor.insert(child.clone(), ancestor.clone());
            }
        }

        let mut parent_db: IndexMap<String, String> = IndexMap::new();
        let mut sub_graph_db: IndexMap<String, bool> = IndexMap::new();
        for sg in self.sub_graphs.iter().rev() {
            if hidden.contains(&sg.id) {
                continue;
            }
            if !sg.nodes.is_empty() {
                sub_graph_db.insert(sg.id.clone(), true);
            }
            for id in &sg.nodes {
                parent_db.insert(id.clone(), sg.id.clone());
            }
        }

        for sg in self.sub_graphs.iter().rev() {
            if hidden.contains(&sg.id) {
                continue;
            }
            let collapsed = is_collapsed(&sg.id);
            nodes.push(Node {
                id: sg.id.clone(),
                label: Some(sg.title.clone()),
                label_style: Some(String::new()),
                label_type: Some(sg.label_type.clone()),
                parent_id: parent_db.get(&sg.id).cloned(),
                padding: Some(8.0),
                css_compiled_styles: self.compiled_styles(&sg.classes),
                css_classes: sg.classes.join(" "),
                shape: if collapsed { "collapsedGroup" } else { "rect" }.to_string(),
                dir: sg.dir.as_deref().and_then(Dir::parse),
                is_group: !collapsed,
                look,
                ..Node::default()
            });
        }

        let padding = if self.config.padding != 0.0 && !self.config.padding.is_nan() {
            self.config.padding
        } else {
            8.0
        };
        for vertex in self.vertices.values() {
            if hidden.contains(&vertex.id) {
                continue;
            }
            // `addNodeFromVertex`.
            if let Some(node) = nodes.iter_mut().find(|n| n.id == vertex.id) {
                node.css_styles = vertex.styles.clone();
                node.css_compiled_styles = self.compiled_styles(&vertex.classes);
                node.css_classes = vertex.classes.join(" ");
                continue;
            }
            let is_group = sub_graph_db.get(&vertex.id).copied().unwrap_or(false);
            let mut class_defs = vec!["default".to_string(), "node".to_string()];
            class_defs.extend(vertex.classes.iter().cloned());
            let mut node = Node {
                id: vertex.id.clone(),
                label: vertex.text.clone(),
                label_type: Some(vertex.label_type.clone()),
                label_style: Some(String::new()),
                parent_id: parent_db.get(&vertex.id).cloned(),
                padding: Some(padding),
                css_styles: vertex.styles.clone(),
                css_compiled_styles: self.compiled_styles(&class_defs),
                css_classes: format!("default {}", vertex.classes.join(" ")),
                dir: vertex.dir.as_deref().and_then(Dir::parse),
                dom_id: Some(vertex.dom_id.clone()),
                look,
                ..Node::default()
            };
            if is_group {
                node.is_group = true;
                node.shape = "rect".to_string();
            } else {
                node.shape = Self::type_from_vertex(vertex);
                node.min_width = Some(self.config.min_node_width);
            }
            nodes.push(node);
        }

        for (index, raw) in self.edges.iter().enumerate() {
            let (start_type, end_type) = Self::destruct_edge_type(raw.kind.as_deref());
            let mut styles: Vec<String> = self.default_style.clone().unwrap_or_default();
            let start = collapsed_ancestor
                .get(&raw.start)
                .cloned()
                .unwrap_or_else(|| raw.start.clone());
            let end = collapsed_ancestor
                .get(&raw.end)
                .cloned()
                .unwrap_or_else(|| raw.end.clone());
            if start == end
                && (collapsed_ancestor.contains_key(&raw.start)
                    || collapsed_ancestor.contains_key(&raw.end))
            {
                continue;
            }
            if let Some(style) = &raw.style {
                styles.extend(style.iter().cloned());
            }
            let invisible = raw.stroke.as_deref() == Some("invisible");
            let no_arrows = invisible || raw.kind.as_deref() == Some("arrow_open");
            let id = if raw.id.is_empty() {
                format!("L_{start}_{end}_{index}")
            } else {
                raw.id.clone()
            };
            edges.push(Edge {
                id,
                start,
                end,
                label: Some(raw.text.clone()),
                label_type: Some(raw.label_type.clone()),
                labelpos: Some("c".to_string()),
                thickness: raw.stroke.as_deref().and_then(Thickness::parse),
                minlen: raw.length,
                classes: Some(if invisible {
                    String::new()
                } else {
                    "edge-thickness-normal edge-pattern-solid flowchart-link".to_string()
                }),
                arrow_type_start: Some(if no_arrows {
                    "none".to_string()
                } else {
                    start_type
                }),
                arrow_type_end: Some(if no_arrows {
                    "none".to_string()
                } else {
                    end_type
                }),
                css_compiled_styles: self.compiled_styles(&raw.classes),
                label_style: styles.clone(),
                style: styles,
                pattern: raw.stroke.as_deref().and_then(Pattern::parse),
                look,
                curve: raw
                    .interpolate
                    .clone()
                    .or_else(|| self.default_interpolate.clone())
                    .or_else(|| Some(self.config.curve.clone()))
                    .filter(|c| !c.is_empty()),
                ..Edge::default()
            });
        }

        LayoutData {
            nodes,
            edges,
            ..LayoutData::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> FlowText {
        FlowText::new(t, "text")
    }

    #[test]
    fn vertices_keep_first_mention_order_and_dom_ids_count_every_mention() {
        let mut db = FlowDb::default();
        db.add_vertex(
            "A",
            Some(&text("Start")),
            Some("square"),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        db.add_vertex("B", None, None, None, None, None, None, None)
            .unwrap();
        db.add_vertex("A", None, None, None, None, None, None, None)
            .unwrap();
        db.add_vertex("C", None, None, None, None, None, None, None)
            .unwrap();
        let ids: Vec<&str> = db.vertices.keys().map(String::as_str).collect();
        assert_eq!(ids, ["A", "B", "C"]);
        assert_eq!(db.vertices["A"].dom_id, "flowchart-A-0");
        assert_eq!(db.vertices["C"].dom_id, "flowchart-C-3");
        assert_eq!(db.vertices["B"].text.as_deref(), Some("B"));
    }

    #[test]
    fn repeated_links_number_from_two() {
        let mut db = FlowDb::default();
        let link = LinkInfo::default();
        for _ in 0..3 {
            db.add_single_link("A", "B", &link, None).unwrap();
        }
        let ids: Vec<&str> = db.edges.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["L_A_B_0", "L_A_B_2", "L_A_B_3"]);
    }

    #[test]
    fn class_def_splits_on_commas_and_copies_color_to_text_styles() {
        let mut db = FlowDb::default();
        db.add_class("a,b", &["fill:#f9f".to_string(), "color:red".to_string()]);
        assert_eq!(db.classes["a"].styles, ["fill:#f9f", "color:red"]);
        assert_eq!(db.classes["b"].text_styles, ["color:red"]);
    }

    #[test]
    fn link_style_adds_fill_none_and_rejects_a_missing_index() {
        let mut db = FlowDb::default();
        db.add_single_link("A", "B", &LinkInfo::default(), None)
            .unwrap();
        db.update_link(&[LinkPos::Index("0".into())], &["stroke:red".into()])
            .unwrap();
        assert_eq!(
            db.edges[0].style.as_deref(),
            Some(&["stroke:red".to_string(), "fill:none".to_string()][..])
        );
        assert!(
            db.update_link(&[LinkPos::Index("3".into())], &["stroke:red".into()])
                .is_err()
        );
    }

    #[test]
    fn shape_metadata_is_checked() {
        let mut db = FlowDb::default();
        db.add_vertex("A", None, None, None, None, None, None, Some("shape: Rect"))
            .expect_err("upper case");
        db.add_vertex("B", None, None, None, None, None, None, Some("shape: nope"))
            .expect_err("unknown");
        db.add_vertex(
            "C",
            None,
            None,
            None,
            None,
            None,
            None,
            Some("shape: cyl, label: 'DB'"),
        )
        .unwrap();
        assert_eq!(db.vertices["C"].kind.as_deref(), Some("cyl"));
        assert_eq!(db.vertices["C"].text.as_deref(), Some("DB"));
        assert_eq!(db.vertices["C"].label_type, "markdown");
    }

    #[test]
    fn subgraph_data_comes_first_last_declared_first() {
        let mut db = FlowDb::default();
        db.add_vertex("a", None, None, None, None, None, None, None)
            .unwrap();
        db.add_sub_graph(
            Some(&text("one")),
            &[DocItem::Id("a".into())],
            Some(&text("one")),
            true,
        )
        .unwrap();
        db.add_vertex("b", None, None, None, None, None, None, None)
            .unwrap();
        db.add_sub_graph(
            Some(&text("two")),
            &[DocItem::Id("b".into()), DocItem::Dir("LR".into())],
            Some(&text("Two")),
            false,
        )
        .unwrap();
        let data = db.get_data();
        let ids: Vec<&str> = data.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["two", "one", "a", "b"]);
        assert_eq!(data.nodes[0].dir, Some(Dir::LR));
        assert_eq!(data.nodes[2].parent_id.as_deref(), Some("one"));
        assert_eq!(data.nodes[2].css_classes, "default ");
    }
}
