//! Port of mermaid 12.2.0 `diagrams/state/stateDb.ts`, the version 2 path
//! the unified renderer reads.
//!
//! `setRootDoc` keeps the parsed document, `getRootDocV2` runs
//! `docTranslator` over it (start and end ids per scope, concurrency
//! regions), and `extract` fills the root document's state table and the
//! `classDef` table from the top-level statements. mermaid runs
//! `getRootDocV2` and `extract` again from the renderer; the second pass
//! over a translated document changes nothing, so [`StateDb::new`] runs
//! them once.
//!
//! Only the top level reaches the state table: a nested state appears
//! there only when a top-level statement (`class`, `style`, a relation)
//! names it. `dataFetcher` reads classes and styles from that table, which
//! is why a `:::class` inside a composite state is ignored in mermaid too.

use indexmap::IndexMap;

use super::actions::{Descr, Note, StateStmt, Stmt};

/// `CONSTANTS.START_NODE` and `END_NODE`.
const EDGE_STATE: &str = "[*]";

/// `StyleClass` of `stateDb.ts`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyleClass {
    pub id: String,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

/// The parts of a root-document state entry `dataFetcher` reads.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DbState {
    pub id: String,
    pub kind: Option<String>,
    pub descriptions: Vec<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
    pub note: Option<Note>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Relation {
    pub id1: String,
    pub id2: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct StateDb {
    /// The translated root document.
    pub root_doc: Vec<Stmt>,
    pub states: IndexMap<String, DbState>,
    pub relations: Vec<Relation>,
    pub classes: IndexMap<String, StyleClass>,
    /// `startEndCount`.
    start_end_count: usize,
    /// The counter behind `generateId()`, which mermaid seeds with
    /// `Math.random`; only DOM ids carry it.
    generated_ids: usize,
}

impl StateDb {
    /// `setRootDoc(doc)` followed by `extract(getRootDocV2())`.
    pub fn new(root_doc: Vec<Stmt>) -> StateDb {
        let mut db = StateDb {
            root_doc,
            ..StateDb::default()
        };
        let mut root = StateStmt {
            id: "root".to_string(),
            doc: Some(std::mem::take(&mut db.root_doc)),
            ..StateStmt::default()
        };
        db.translate_state(&mut root, true, "root");
        db.root_doc = root.doc.unwrap_or_default();
        db.extract();
        db
    }

    /// `docTranslator(parent, node, first)` for a state statement:
    /// `parent_id` is the id of the enclosing state (`root` at the top).
    fn translate_state(&mut self, node: &mut StateStmt, first: bool, parent_id: &str) {
        if node.id == EDGE_STATE {
            node.id = format!("{parent_id}{}", if first { "_start" } else { "_end" });
            node.start = Some(first);
        } else {
            node.id = node.id.trim().to_string();
        }
        let Some(doc) = node.doc.take() else {
            return;
        };
        // Concurrency: each `--` closes the region before it.
        let mut regions: Vec<Stmt> = Vec::new();
        let mut current: Vec<Stmt> = Vec::new();
        for stmt in &doc {
            match stmt {
                Stmt::State(s) if s.is_divider() => {
                    let mut region = s.clone();
                    region.doc = Some(std::mem::take(&mut current));
                    regions.push(Stmt::State(region));
                }
                other => current.push(other.clone()),
            }
        }
        let mut doc = if !regions.is_empty() && !current.is_empty() {
            self.generated_ids += 1;
            regions.push(Stmt::State(StateStmt {
                id: format!("id-{}", self.generated_ids),
                kind: Some("divider".to_string()),
                doc: Some(current),
                ..StateStmt::default()
            }));
            regions
        } else {
            doc
        };
        let id = node.id.clone();
        for stmt in &mut doc {
            self.translate_stmt(stmt, &id);
        }
        node.doc = Some(doc);
    }

    /// `docTranslator(node, docNode, true)` for one statement of a document.
    fn translate_stmt(&mut self, stmt: &mut Stmt, parent_id: &str) {
        match stmt {
            Stmt::Relation { state1, state2, .. } => {
                self.translate_state(state1, true, parent_id);
                self.translate_state(state2, false, parent_id);
            }
            Stmt::State(s) => self.translate_state(s, true, parent_id),
            _ => {}
        }
    }

    /// `extract(statements)` over the top-level statements.
    fn extract(&mut self) {
        self.states.clear();
        self.relations.clear();
        self.classes.clear();
        self.start_end_count = 0;
        let doc = self.root_doc.clone();
        for item in &doc {
            match item {
                Stmt::State(s) => {
                    self.add_state(
                        s.id.trim(),
                        s.kind.clone(),
                        &s.description,
                        s.note.as_ref(),
                        &[],
                    );
                }
                Stmt::Relation {
                    state1,
                    state2,
                    description,
                } => self.add_relation_objs(state1, state2, description.as_deref()),
                Stmt::ClassDef { id, classes } => self.add_style_class(id.trim(), classes),
                Stmt::Style { id, style_class } => self.handle_style_def(id, style_class),
                Stmt::ApplyClass { id, style_class } => self.set_css_class(id.trim(), style_class),
                _ => {}
            }
        }
    }

    /// `addState(id, type, doc, descr, note, classes)`. A new state takes
    /// `type`; an existing one keeps its own.
    fn add_state(
        &mut self,
        id: &str,
        kind: Option<String>,
        descr: &Descr,
        note: Option<&Note>,
        classes: &[String],
    ) {
        let id = id.trim().to_string();
        // `type = DEFAULT_STATE_TYPE`: a missing type takes the default.
        let kind = kind.or_else(|| Some("default".to_string()));
        let entry = self.states.entry(id.clone()).or_insert_with(|| DbState {
            id: id.clone(),
            kind: kind.clone(),
            ..DbState::default()
        });
        if entry.kind.is_none() {
            entry.kind = kind;
        }
        let descriptions: Vec<String> = match descr {
            Descr::None => Vec::new(),
            Descr::Text(s) if s.is_empty() => Vec::new(),
            Descr::Text(s) => vec![s.clone()],
            Descr::List(l) => l.clone(),
        };
        for d in descriptions {
            self.add_description(&id, d.trim());
        }
        if let Some(note) = note
            && let Some(state) = self.states.get_mut(&id)
        {
            state.note = Some(note.clone());
        }
        for class in classes {
            self.set_css_class(&id, class.trim());
        }
    }

    /// `addDescription(id, descr)`: a leading `:` is dropped.
    fn add_description(&mut self, id: &str, descr: &str) {
        let descr = match descr.strip_prefix(':') {
            Some(rest) => rest.trim().to_string(),
            None => descr.to_string(),
        };
        if let Some(state) = self.states.get_mut(id) {
            state.descriptions.push(descr);
        }
    }

    /// `startIdIfNeeded` / `endIdIfNeeded`: a leftover `[*]` gets a numbered
    /// id. Translated documents carry none, but the counter is ported.
    fn edge_id_if_needed(&mut self, id: &str, prefix: &str) -> String {
        if id == EDGE_STATE {
            self.start_end_count += 1;
            format!("{prefix}{}", self.start_end_count)
        } else {
            id.to_string()
        }
    }

    /// `addRelationObjs(item1, item2, relationTitle)`.
    fn add_relation_objs(&mut self, item1: &StateStmt, item2: &StateStmt, title: Option<&str>) {
        let id1 = self.edge_id_if_needed(item1.id.trim(), "start");
        let id2 = self.edge_id_if_needed(item2.id.trim(), "start");
        let kind = |s: &StateStmt| {
            if s.id.trim() == EDGE_STATE {
                Some("start".to_string())
            } else {
                s.kind.clone()
            }
        };
        self.add_state(
            &id1,
            kind(item1),
            &item1.description,
            item1.note.as_ref(),
            &item1.classes,
        );
        self.add_state(
            &id2,
            kind(item2),
            &item2.description,
            item2.note.as_ref(),
            &item2.classes,
        );
        self.relations.push(Relation {
            id1,
            id2,
            title: Some(title.unwrap_or("").to_string()),
        });
    }

    /// `addStyleClass(id, styleAttributes)`: one style per comma; a
    /// `color` attribute also becomes a text style with `fill`.
    fn add_style_class(&mut self, id: &str, attributes: &str) {
        let class = self
            .classes
            .entry(id.to_string())
            .or_insert_with(|| StyleClass {
                id: id.to_string(),
                ..StyleClass::default()
            });
        if attributes.is_empty() {
            return;
        }
        for attrib in attributes.split(',') {
            // `attrib.replace(/([^;]*);/, '$1')`: the first `;` goes.
            let fixed = match attrib.find(';') {
                Some(i) => format!("{}{}", &attrib[..i], &attrib[i + 1..]),
                None => attrib.to_string(),
            };
            let fixed = fixed.trim().to_string();
            if attrib.contains("color") {
                let text = fixed
                    .replacen("fill", "bgFill", 1)
                    .replacen("color", "fill", 1);
                class.text_styles.push(text);
            }
            class.styles.push(fixed);
        }
    }

    /// `handleStyleDef(item)`: `style a,b fill:#f9f,stroke:#333` replaces
    /// the styles of each state, creating unknown ones.
    fn handle_style_def(&mut self, ids: &str, style_class: &str) {
        let styles: Vec<String> = style_class
            .split(',')
            .map(|s| s.replace(';', "").trim().to_string())
            .collect();
        for id in ids.trim().split(',') {
            let key = if self.states.contains_key(id) {
                id.to_string()
            } else {
                let trimmed = id.trim().to_string();
                self.add_state(
                    &trimmed,
                    Some("default".to_string()),
                    &Descr::None,
                    None,
                    &[],
                );
                trimmed
            };
            if let Some(state) = self.states.get_mut(&key) {
                state.styles = styles.clone();
            }
        }
    }

    /// `setCssClass(itemIds, cssClassName)`.
    fn set_css_class(&mut self, ids: &str, class: &str) {
        for id in ids.split(',') {
            let key = if self.states.contains_key(id) {
                id.to_string()
            } else {
                let trimmed = id.trim().to_string();
                self.add_state(
                    &trimmed,
                    Some("default".to_string()),
                    &Descr::None,
                    None,
                    &[],
                );
                trimmed
            };
            if let Some(state) = self.states.get_mut(&key) {
                state.classes.push(class.to_string());
            }
        }
    }

    /// `getDir(getRootDocV2())`: the last `direction` of the root document.
    pub fn direction(&self) -> String {
        doc_direction(&self.root_doc)
    }
}

/// `getDir(parsedItem, 'TB')` of `dataFetcher.ts`: the last `dir`
/// statement of a document.
pub fn doc_direction(doc: &[Stmt]) -> String {
    doc.iter()
        .rev()
        .find_map(|s| match s {
            Stmt::Dir(d) => Some(d.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "TB".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(id: &str) -> StateStmt {
        StateStmt {
            id: id.to_string(),
            kind: Some("default".to_string()),
            description: Descr::Text(String::new()),
            ..StateStmt::default()
        }
    }

    fn rel(a: &str, b: &str) -> Stmt {
        Stmt::Relation {
            state1: Box::new(state(a)),
            state2: Box::new(state(b)),
            description: None,
        }
    }

    #[test]
    fn edge_states_take_the_scope_id() {
        let mut composite = state("C");
        composite.doc = Some(vec![rel("[*]", "x"), rel("x", "[*]")]);
        let db = StateDb::new(vec![rel("[*]", "C"), Stmt::State(composite)]);
        let Stmt::Relation { state1, .. } = &db.root_doc[0] else {
            panic!("relation");
        };
        assert_eq!(state1.id, "root_start");
        assert_eq!(state1.start, Some(true));
        let Stmt::State(c) = &db.root_doc[1] else {
            panic!("state");
        };
        let inner = c.doc.as_ref().expect("doc");
        let Stmt::Relation { state1, .. } = &inner[0] else {
            panic!("relation");
        };
        assert_eq!(state1.id, "C_start");
        let Stmt::Relation { state2, .. } = &inner[1] else {
            panic!("relation");
        };
        assert_eq!(state2.id, "C_end");
        assert_eq!(state2.start, Some(false));
    }

    #[test]
    fn dividers_split_a_document_into_regions() {
        let mut divider = state("divider-id-1");
        divider.kind = Some("divider".to_string());
        let mut composite = state("C");
        composite.doc = Some(vec![rel("[*]", "a"), Stmt::State(divider), rel("[*]", "b")]);
        let db = StateDb::new(vec![Stmt::State(composite)]);
        let Stmt::State(c) = &db.root_doc[0] else {
            panic!("state");
        };
        let regions = c.doc.as_ref().expect("doc");
        assert_eq!(regions.len(), 2);
        let ids: Vec<String> = regions
            .iter()
            .map(|r| match r {
                Stmt::State(s) => s.id.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(ids, ["divider-id-1", "id-1"]);
        let Stmt::State(second) = &regions[1] else {
            panic!("state");
        };
        let Stmt::Relation { state1, .. } = &second.doc.as_ref().expect("doc")[0] else {
            panic!("relation");
        };
        assert_eq!(state1.id, "id-1_start");
    }

    #[test]
    fn class_defs_split_styles_and_text_styles() {
        let db = StateDb::new(vec![Stmt::ClassDef {
            id: "bad".to_string(),
            classes: "fill:#f00,color:white;,font-weight:bold".to_string(),
        }]);
        let class = db.classes.get("bad").expect("class");
        assert_eq!(
            class.styles,
            ["fill:#f00", "color:white", "font-weight:bold"]
        );
        assert_eq!(class.text_styles, ["fill:white"]);
    }

    #[test]
    fn classes_from_relations_and_class_statements_accumulate() {
        let mut a = state("A");
        a.classes = vec!["one".to_string()];
        let db = StateDb::new(vec![
            Stmt::Relation {
                state1: Box::new(a),
                state2: Box::new(state("B")),
                description: None,
            },
            Stmt::ApplyClass {
                id: "A,B".to_string(),
                style_class: "two".to_string(),
            },
            Stmt::Style {
                id: "B".to_string(),
                style_class: "fill:#f9f;,stroke:#333".to_string(),
            },
        ]);
        assert_eq!(db.states["A"].classes, ["one", "two"]);
        assert_eq!(db.states["B"].classes, ["two"]);
        assert_eq!(db.states["B"].styles, ["fill:#f9f", "stroke:#333"]);
    }

    #[test]
    fn direction_is_the_last_root_dir_statement() {
        let db = StateDb::new(vec![
            Stmt::Dir("LR".to_string()),
            Stmt::Dir("BT".to_string()),
        ]);
        assert_eq!(db.direction(), "BT");
        assert_eq!(doc_direction(&[]), "TB");
    }
}
