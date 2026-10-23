//! Port of `diagrams/c4/c4Db.ts` (mermaid 12.2.0).
//!
//! The grammar actions call the db directly (`yy.addPersonOrSystem(...)`,
//! `yy.addRel(...)`), so [`C4Db`] holds the shapes, the boundary tree, and
//! the relationships in source order, as the module-level arrays of
//! `c4Db.ts` do. A shape, boundary, or relationship is a JavaScript object
//! that named arguments (`$key="value"`) can extend with any key, so each
//! one keeps its attributes in an insertion-ordered [`Fields`] map.

use indexmap::IndexMap;

/// One parser attribute: a positional value or a `$key="value"` pair
/// (`ParserAttribute` in `c4Db.ts`).
#[derive(Debug, Clone, PartialEq)]
pub enum Attr {
    Str(String),
    Kv(String, String),
}

impl Attr {
    /// The value as JavaScript coerces it to a string; an object is
    /// `[object Object]`.
    pub fn js_string(&self) -> String {
        match self {
            Attr::Str(s) => s.clone(),
            Attr::Kv(..) => "[object Object]".to_string(),
        }
    }
}

/// A property value on a C4 object.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    /// `{ text }`, what `TEXT_FIELDS` and the positional text slots store.
    Text(String),
    /// A plain string (`$sprite`, `$tags`, colors, `$shape`).
    Str(String),
    /// A number: `parseInt` of an `UpdateRelStyle` offset.
    Num(f64),
}

impl Field {
    /// JavaScript truthiness.
    pub fn truthy(&self) -> bool {
        match self {
            Field::Text(_) => true,
            Field::Str(s) => !s.is_empty(),
            Field::Num(n) => *n != 0.0 && !n.is_nan(),
        }
    }

    /// `String(value)`.
    pub fn js_string(&self) -> String {
        match self {
            Field::Text(_) => "[object Object]".to_string(),
            Field::Str(s) => s.clone(),
            Field::Num(n) => js_number_string(*n),
        }
    }
}

/// `String(n)` for the integers `parseInt` returns.
fn js_number_string(n: f64) -> String {
    if n.is_nan() {
        "NaN".to_string()
    } else if n.fract() == 0.0 && n.abs() < 1e21 {
        format!("{n:.0}")
    } else {
        n.to_string()
    }
}

/// The JavaScript properties of one C4 object, in insertion order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fields(pub IndexMap<String, Field>);

impl Fields {
    pub fn get(&self, key: &str) -> Option<&Field> {
        self.0.get(key)
    }

    pub fn set(&mut self, key: &str, value: Field) {
        self.0.insert(key.to_string(), value);
    }

    /// `obj[key].text` when the property is a `{ text }` object.
    pub fn text(&self, key: &str) -> Option<&str> {
        match self.0.get(key) {
            Some(Field::Text(s)) => Some(s),
            _ => None,
        }
    }

    /// `obj[key].text = value` on an existing `{ text }`.
    pub fn set_text(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), Field::Text(value));
    }

    /// `obj[key]` when it is a plain string.
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.0.get(key) {
            Some(Field::Str(s)) => Some(s),
            _ => None,
        }
    }

    /// `obj[key] ??= { text: '' }`.
    fn default_text(&mut self, key: &str) {
        self.0
            .entry(key.to_string())
            .or_insert_with(|| Field::Text(String::new()));
    }
}

/// The fields stored as `{ text }` whichever slot they arrive through.
const TEXT_FIELDS: [&str; 4] = ["label", "descr", "techn", "type"];

/// A named override as `assignAttributes` and the slot handlers store it.
fn named(key: &str, value: &str) -> Field {
    if TEXT_FIELDS.contains(&key) {
        Field::Text(value.to_string())
    } else {
        Field::Str(value.to_string())
    }
}

/// The optional text slot of an `add*` call: a positional value becomes
/// `{ text }` under `slot`, a named one goes to its own key, and a missing
/// one leaves the old value or sets `{ text: '' }`.
fn text_slot(fields: &mut Fields, slot: &str, attr: Option<&Attr>) {
    match attr {
        None => fields.default_text(slot),
        Some(Attr::Kv(key, value)) => fields.set(key, named(key, value)),
        Some(Attr::Str(value)) => fields.set_text(slot, value.clone()),
    }
}

/// `assignAttributes(bag, { sprite, tags, link })`.
fn assign_attributes(fields: &mut Fields, slots: &[(&str, Option<&Attr>)]) {
    for (slot, attr) in slots {
        match attr {
            None => {}
            Some(Attr::Kv(key, value)) => fields.set(key, named(key, value)),
            Some(Attr::Str(value)) => fields.set(slot, Field::Str(value.clone())),
        }
    }
}

/// A person, system, container, or component (`C4Shape`).
#[derive(Debug, Clone, PartialEq)]
pub struct C4Shape {
    pub alias: String,
    /// `typeC4Shape.text`, for example `external_system_db`.
    pub type_c4_shape: String,
    pub parent_boundary: String,
    pub wrap: bool,
    pub fields: Fields,
}

impl C4Shape {
    pub fn label(&self) -> &str {
        self.fields.text("label").unwrap_or("")
    }
}

/// A boundary or a deployment node (`C4Boundary`).
#[derive(Debug, Clone, PartialEq)]
pub struct C4Boundary {
    pub alias: String,
    pub parent_boundary: String,
    pub wrap: bool,
    /// `node`, `nodeL`, or `nodeR` for a deployment node.
    pub node_type: Option<String>,
    pub fields: Fields,
}

/// A relationship (`C4Rel`).
#[derive(Debug, Clone, PartialEq)]
pub struct C4Rel {
    /// `rel`, `birel`, `rel_u`, `rel_d`, `rel_l`, `rel_r`, or `rel_b`.
    pub kind: String,
    pub from: String,
    pub to: String,
    pub wrap: bool,
    pub fields: Fields,
}

#[derive(Debug, Clone, PartialEq)]
pub struct C4Db {
    pub shapes: Vec<C4Shape>,
    pub boundaries: Vec<C4Boundary>,
    pub rels: Vec<C4Rel>,
    pub title: String,
    pub acc_description: Option<String>,
    pub c4_type: Option<String>,
    pub c4_shape_in_row: i64,
    pub c4_boundary_in_row: i64,
    /// `wrapEnabled`: the root `wrap` config `init` hands to `setWrap`.
    wrap_enabled: bool,
    boundary_parse_stack: Vec<String>,
    current_boundary_parse: String,
    parent_boundary_parse: String,
}

/// `createGlobalBoundary()`.
fn global_boundary() -> C4Boundary {
    let mut fields = Fields::default();
    fields.set_text("label", "global".to_string());
    fields.set_text("type", "global".to_string());
    C4Boundary {
        alias: "global".to_string(),
        parent_boundary: String::new(),
        wrap: false,
        node_type: None,
        fields,
    }
}

/// JavaScript `parseInt(value)` in base 10, `NaN` when no digit leads.
pub fn js_parse_int(value: &str) -> f64 {
    let s = value.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let (sign, rest) = match s.strip_prefix('-') {
        Some(r) => (-1.0, r),
        None => (1.0, s.strip_prefix('+').unwrap_or(s)),
    };
    if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        let digits: String = hex.chars().take_while(char::is_ascii_hexdigit).collect();
        return if digits.is_empty() {
            f64::NAN
        } else {
            u64::from_str_radix(&digits, 16).map_or(f64::NAN, |v| sign * v as f64)
        };
    }
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return f64::NAN;
    }
    digits.parse::<f64>().map_or(f64::NAN, |v| sign * v)
}

impl Default for C4Db {
    fn default() -> C4Db {
        C4Db::new(false)
    }
}

impl C4Db {
    /// `clear()` followed by `setWrap(wrap)` from the diagram's `init`.
    pub fn new(wrap: bool) -> C4Db {
        C4Db {
            shapes: Vec::new(),
            boundaries: vec![global_boundary()],
            rels: Vec::new(),
            title: String::new(),
            acc_description: None,
            c4_type: None,
            c4_shape_in_row: 4,
            c4_boundary_in_row: 2,
            wrap_enabled: wrap,
            boundary_parse_stack: vec![String::new()],
            current_boundary_parse: "global".to_string(),
            parent_boundary_parse: String::new(),
        }
    }

    pub fn set_c4_type(&mut self, c4_type: &str) {
        self.c4_type = Some(c4_type.to_string());
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn set_acc_description(&mut self, description: &str) {
        self.acc_description = Some(description.to_string());
    }

    /// `addRel(type, from, to, label, techn, descr, sprite, tags, link)`.
    pub fn add_rel(&mut self, kind: &str, attrs: &[Attr]) {
        let (Some(from), Some(to), Some(label)) = (attrs.first(), attrs.get(1), attrs.get(2))
        else {
            return;
        };
        let (from, to, label) = (from.js_string(), to.js_string(), label.js_string());
        let index = match self.rels.iter().position(|r| r.from == from && r.to == to) {
            Some(i) => i,
            None => {
                self.rels.push(C4Rel {
                    kind: String::new(),
                    from: from.clone(),
                    to: to.clone(),
                    wrap: false,
                    fields: Fields::default(),
                });
                self.rels.len() - 1
            }
        };
        let wrap = self.wrap_enabled;
        let rel = &mut self.rels[index];
        rel.kind = kind.to_string();
        rel.fields.set_text("label", label);
        text_slot(&mut rel.fields, "techn", attrs.get(3));
        text_slot(&mut rel.fields, "descr", attrs.get(4));
        assign_attributes(
            &mut rel.fields,
            &[
                ("sprite", attrs.get(5)),
                ("tags", attrs.get(6)),
                ("link", attrs.get(7)),
            ],
        );
        rel.wrap = wrap;
    }

    /// The shape with `alias`, created at the end when it is new.
    fn shape_entry(&mut self, alias: &str) -> &mut C4Shape {
        let index = match self.shapes.iter().position(|s| s.alias == alias) {
            Some(i) => i,
            None => {
                self.shapes.push(C4Shape {
                    alias: alias.to_string(),
                    type_c4_shape: String::new(),
                    parent_boundary: String::new(),
                    wrap: false,
                    fields: Fields::default(),
                });
                self.shapes.len() - 1
            }
        };
        &mut self.shapes[index]
    }

    /// `addPersonOrSystem(typeC4Shape, alias, label, descr, sprite, tags,
    /// link)`.
    pub fn add_person_or_system(&mut self, type_c4_shape: &str, attrs: &[Attr]) {
        self.add_element(type_c4_shape, attrs, false);
    }

    /// `addContainer` and `addComponent`: the same as
    /// `addPersonOrSystem` with a `techn` slot before `descr`.
    pub fn add_container(&mut self, type_c4_shape: &str, attrs: &[Attr]) {
        self.add_element(type_c4_shape, attrs, true);
    }

    fn add_element(&mut self, type_c4_shape: &str, attrs: &[Attr], has_techn: bool) {
        let alias = attrs.first().map_or_else(String::new, Attr::js_string);
        let label = attrs.get(1).map_or_else(String::new, Attr::js_string);
        let parent = self.current_boundary_parse.clone();
        let wrap = self.wrap_enabled;
        let shape = self.shape_entry(&alias);
        shape.fields.set_text("label", label);
        let rest = if has_techn {
            text_slot(&mut shape.fields, "techn", attrs.get(2));
            3
        } else {
            2
        };
        text_slot(&mut shape.fields, "descr", attrs.get(rest));
        assign_attributes(
            &mut shape.fields,
            &[
                ("sprite", attrs.get(rest + 1)),
                ("tags", attrs.get(rest + 2)),
                ("link", attrs.get(rest + 3)),
            ],
        );
        shape.type_c4_shape = type_c4_shape.to_string();
        shape.parent_boundary = parent;
        shape.wrap = wrap;
    }

    fn boundary_entry(&mut self, alias: &str) -> &mut C4Boundary {
        let index = match self.boundaries.iter().position(|b| b.alias == alias) {
            Some(i) => i,
            None => {
                self.boundaries.push(C4Boundary {
                    alias: alias.to_string(),
                    parent_boundary: String::new(),
                    wrap: false,
                    node_type: None,
                    fields: Fields::default(),
                });
                self.boundaries.len() - 1
            }
        };
        &mut self.boundaries[index]
    }

    /// `addPersonOrSystemBoundary(alias, label, type, tags, link)`; the
    /// type defaults to `system`.
    pub fn add_person_or_system_boundary(&mut self, attrs: &[Attr]) {
        self.add_boundary(attrs, "system", None);
    }

    /// `addContainerBoundary`: the type defaults to `container`.
    pub fn add_container_boundary(&mut self, attrs: &[Attr]) {
        self.add_boundary(attrs, "container", None);
    }

    /// `addDeploymentNode(nodeType, alias, label, type, descr, sprite,
    /// tags, link)`; the type defaults to `node`.
    pub fn add_deployment_node(&mut self, node_type: &str, attrs: &[Attr]) {
        self.add_boundary(attrs, "node", Some(node_type));
    }

    fn add_boundary(&mut self, attrs: &[Attr], default_type: &str, node_type: Option<&str>) {
        let alias = attrs.first().map_or_else(String::new, Attr::js_string);
        let label = attrs.get(1).map_or_else(String::new, Attr::js_string);
        let parent = self.current_boundary_parse.clone();
        let wrap = self.wrap_enabled;
        let boundary = self.boundary_entry(&alias);
        boundary.fields.set_text("label", label);
        match attrs.get(2) {
            None => boundary.fields.set_text("type", default_type.to_string()),
            Some(Attr::Kv(key, value)) => boundary.fields.set(key, named(key, value)),
            Some(Attr::Str(value)) => boundary.fields.set_text("type", value.clone()),
        }
        if node_type.is_some() {
            // Deployment nodes carry `descr` and `sprite` before `tags`.
            text_slot(&mut boundary.fields, "descr", attrs.get(3));
            assign_attributes(
                &mut boundary.fields,
                &[("tags", attrs.get(5)), ("link", attrs.get(6))],
            );
        } else {
            assign_attributes(
                &mut boundary.fields,
                &[("tags", attrs.get(3)), ("link", attrs.get(4))],
            );
        }
        if let Some(kind) = node_type {
            boundary.node_type = Some(kind.to_string());
        }
        boundary.parent_boundary = parent;
        boundary.wrap = wrap;

        self.parent_boundary_parse = std::mem::replace(&mut self.current_boundary_parse, alias);
        self.boundary_parse_stack
            .push(self.parent_boundary_parse.clone());
    }

    /// `popBoundaryParseStack()`.
    pub fn pop_boundary_parse_stack(&mut self) {
        self.current_boundary_parse = self.parent_boundary_parse.clone();
        self.boundary_parse_stack.pop();
        // `boundaryParseStack.pop()!` on an empty stack is `undefined`;
        // the empty string stands in for it.
        self.parent_boundary_parse = self.boundary_parse_stack.pop().unwrap_or_default();
        self.boundary_parse_stack
            .push(self.parent_boundary_parse.clone());
    }

    /// `updateElStyle(typeC4Shape, elementName, bgColor, fontColor,
    /// borderColor, shadowing, shape, sprite, techn, legendText,
    /// legendSprite)`: every value is stored as a plain string.
    pub fn update_el_style(&mut self, attrs: &[Attr]) {
        let Some(Attr::Str(name)) = attrs.first() else {
            return;
        };
        let fields = if let Some(s) = self.shapes.iter_mut().find(|s| &s.alias == name) {
            &mut s.fields
        } else if let Some(b) = self.boundaries.iter_mut().find(|b| &b.alias == name) {
            &mut b.fields
        } else {
            return;
        };
        let slots = [
            "bgColor",
            "fontColor",
            "borderColor",
            "shadowing",
            "shape",
            "sprite",
            "techn",
            "legendText",
            "legendSprite",
        ];
        for (slot, attr) in slots.iter().zip(attrs.iter().skip(1)) {
            match attr {
                Attr::Kv(key, value) => fields.set(key, Field::Str(value.clone())),
                Attr::Str(value) => fields.set(slot, Field::Str(value.clone())),
            }
        }
    }

    /// `updateRelStyle(typeC4Shape, from, to, textColor, lineColor,
    /// offsetX, offsetY)`: the offset slots go through `parseInt`.
    pub fn update_rel_style(&mut self, attrs: &[Attr]) {
        let (Some(Attr::Str(from)), Some(Attr::Str(to))) = (attrs.first(), attrs.get(1)) else {
            return;
        };
        let Some(rel) = self
            .rels
            .iter_mut()
            .find(|r| &r.from == from && &r.to == to)
        else {
            return;
        };
        let slots = [
            ("textColor", false),
            ("lineColor", false),
            ("offsetX", true),
            ("offsetY", true),
        ];
        for ((slot, numeric), attr) in slots.iter().zip(attrs.iter().skip(2)) {
            let (key, value) = match attr {
                Attr::Kv(key, value) => (key.as_str(), value),
                Attr::Str(value) => (*slot, value),
            };
            let field = if *numeric {
                Field::Num(js_parse_int(value))
            } else {
                Field::Str(value.clone())
            };
            rel.fields.set(key, field);
        }
    }

    /// `updateLayoutConfig(typeC4Shape, c4ShapeInRow, c4BoundaryInRow)`: a
    /// named argument counts by position, whatever its key.
    pub fn update_layout_config(&mut self, attrs: &[Attr]) {
        let value = |attr: Option<&Attr>| match attr {
            Some(Attr::Str(v)) | Some(Attr::Kv(_, v)) => js_parse_int(v),
            None => f64::NAN,
        };
        let shape_in_row = value(attrs.first());
        let boundary_in_row = value(attrs.get(1));
        if shape_in_row >= 1.0 {
            self.c4_shape_in_row = shape_in_row as i64;
        }
        if boundary_in_row >= 1.0 {
            self.c4_boundary_in_row = boundary_in_row as i64;
        }
    }

    /// `getC4ShapeArray(parentBoundary)`.
    pub fn shapes_in(&self, parent_boundary: &str) -> Vec<usize> {
        self.shapes
            .iter()
            .enumerate()
            .filter(|(_, s)| s.parent_boundary == parent_boundary)
            .map(|(i, _)| i)
            .collect()
    }

    /// `getBoundaries(parentBoundary)`.
    pub fn boundaries_in(&self, parent_boundary: &str) -> Vec<usize> {
        self.boundaries
            .iter()
            .enumerate()
            .filter(|(_, b)| b.parent_boundary == parent_boundary)
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Attr {
        Attr::Str(v.to_string())
    }

    fn kv(k: &str, v: &str) -> Attr {
        Attr::Kv(k.to_string(), v.to_string())
    }

    #[test]
    fn a_shape_keeps_its_boundary_and_named_attributes() {
        let mut db = C4Db::new(false);
        db.add_person_or_system_boundary(&[s("b0"), s("Bank"), s("ENTERPRISE")]);
        db.add_person_or_system(
            "person",
            &[
                s("a"),
                s("Customer"),
                kv("sprite", "users"),
                kv("descr", "d"),
            ],
        );
        db.pop_boundary_parse_stack();
        db.add_container("container", &[s("c"), s("Web"), s("Java")]);
        let a = &db.shapes[0];
        assert_eq!(a.parent_boundary, "b0");
        assert_eq!(a.fields.text("descr"), Some("d"));
        assert_eq!(a.fields.str("sprite"), Some("users"));
        let c = &db.shapes[1];
        assert_eq!(c.parent_boundary, "global");
        assert_eq!(c.fields.text("techn"), Some("Java"));
        assert_eq!(c.fields.text("descr"), Some(""));
        assert_eq!(db.boundaries[1].fields.text("type"), Some("ENTERPRISE"));
    }

    #[test]
    fn nested_boundaries_pop_back_to_their_parent() {
        let mut db = C4Db::new(false);
        db.add_person_or_system_boundary(&[s("b0"), s("B0")]);
        db.add_person_or_system_boundary(&[s("b1"), s("B1")]);
        db.add_person_or_system("system", &[s("x"), s("X")]);
        db.pop_boundary_parse_stack();
        db.add_person_or_system("system", &[s("y"), s("Y")]);
        db.pop_boundary_parse_stack();
        db.add_person_or_system("system", &[s("z"), s("Z")]);
        assert_eq!(db.boundaries[1].parent_boundary, "global");
        assert_eq!(db.boundaries[2].parent_boundary, "b0");
        assert_eq!(db.shapes[0].parent_boundary, "b1");
        assert_eq!(db.shapes[1].parent_boundary, "b0");
        assert_eq!(db.shapes[2].parent_boundary, "global");
    }

    #[test]
    fn only_the_offset_slots_go_through_parse_int() {
        let mut db = C4Db::new(false);
        db.add_rel("rel", &[s("a"), s("b"), s("Uses")]);
        db.update_rel_style(&[
            s("a"),
            s("b"),
            kv("textColor", "blue"),
            kv("offsetY", "-40"),
            kv("offsetX", "x"),
        ]);
        let rel = &db.rels[0];
        assert_eq!(rel.fields.str("textColor"), Some("blue"));
        // A named offset in the `lineColor` slot stays a string, as in
        // `c4Db.ts`; only the offset slots run `parseInt`.
        assert_eq!(rel.fields.get("offsetY"), Some(&Field::Str("-40".into())));
        assert!(matches!(rel.fields.get("offsetX"), Some(Field::Num(n)) if n.is_nan()));
    }

    #[test]
    fn layout_config_ignores_values_below_one() {
        let mut db = C4Db::new(false);
        db.update_layout_config(&[kv("c4ShapeInRow", "3"), kv("c4BoundaryInRow", "0")]);
        assert_eq!(db.c4_shape_in_row, 3);
        assert_eq!(db.c4_boundary_in_row, 2);
    }

    #[test]
    fn parse_int_matches_javascript() {
        assert_eq!(js_parse_int(" -50px"), -50.0);
        assert_eq!(js_parse_int("5"), 5.0);
        assert!(js_parse_int("x5").is_nan());
        assert_eq!(js_parse_int("0x1A"), 26.0);
    }
}
