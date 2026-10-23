//! The semantic actions of `c4Diagram.jison` (mermaid 12.2.0), one match
//! arm per production index of `c4Diagram.tables.json`.
//!
//! Every statement calls the db at once (`yy.addPersonOrSystem(...$2)`),
//! so the actions own the [`C4Db`]. The lexer actions use no `yy` hook.

use crate::mermaid::directive::js_trim;
use crate::mermaid::jison::{Actions, Loc, ParseError, Yy};

use super::db::{Attr, C4Db};

/// A semantic value on jison's `vstack`.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Nil,
    Text(String),
    Attr(Attr),
    Attrs(Vec<Attr>),
}

impl Val {
    fn text(&self) -> &str {
        match self {
            Val::Text(s) => s,
            _ => "",
        }
    }

    fn attrs(&self) -> Vec<Attr> {
        match self {
            Val::Attrs(list) => list.clone(),
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct C4Actions {
    pub db: C4Db,
}

impl C4Actions {
    pub fn new(db: C4Db) -> C4Actions {
        C4Actions { db }
    }
}

impl Yy for C4Actions {}

/// `$1.substring(n)`.
fn substring(text: &str, n: usize) -> String {
    text.chars().skip(n).collect()
}

/// `attrs.splice(index, 0, value)`: an index past the end appends.
fn splice_insert(mut attrs: Vec<Attr>, index: usize, value: &str) -> Vec<Attr> {
    let at = index.min(attrs.len());
    attrs.insert(at, Attr::Str(value.to_string()));
    attrs
}

/// The element kind each `diagramStatement -> KEYWORD attributes`
/// production hands to its db call.
fn element_kind(production: usize) -> Option<(&'static str, bool)> {
    let kinds: [(&str, bool); 20] = [
        ("person", false),
        ("external_person", false),
        ("system", false),
        ("system_db", false),
        ("system_queue", false),
        ("external_system", false),
        ("external_system_db", false),
        ("external_system_queue", false),
        ("container", true),
        ("container_db", true),
        ("container_queue", true),
        ("external_container", true),
        ("external_container_db", true),
        ("external_container_queue", true),
        ("component", true),
        ("component_db", true),
        ("component_queue", true),
        ("external_component", true),
        ("external_component_db", true),
        ("external_component_queue", true),
    ];
    production
        .checked_sub(39)
        .and_then(|i| kinds.get(i))
        .copied()
}

impl Actions for C4Actions {
    type Value = Val;

    fn terminal(&mut self, yytext: &str) -> Val {
        Val::Text(yytext.to_string())
    }

    fn reduce(
        &mut self,
        production: usize,
        rhs: &[Val],
        _locs: &[Loc],
        _yytext: &str,
    ) -> Result<Val, ParseError> {
        let attrs = || rhs.get(1).map(Val::attrs).unwrap_or_default();
        let v = match production {
            // graphConfig -> C4_* NEWLINE statements EOF: `yy.setC4Type($1)`
            8..=12 => {
                self.db.set_c4_type(rhs[0].text());
                Val::Nil
            }
            // otherStatement -> title: `yy.setTitle($1.substring(6))`
            19 => {
                let title = substring(rhs[0].text(), 6);
                self.db.set_title(&title);
                Val::Text(title)
            }
            // otherStatement -> accDescription
            20 => {
                let descr = substring(rhs[0].text(), 15);
                self.db.set_acc_description(&descr);
                Val::Text(descr)
            }
            // acc_title acc_title_value: the grammar calls `setTitle`, so an
            // accessible title is also drawn as the diagram title.
            21 => {
                let value = js_trim(rhs[1].text()).to_string();
                self.db.set_title(&value);
                Val::Text(value)
            }
            22 => {
                let value = js_trim(rhs[1].text()).to_string();
                self.db.set_acc_description(&value);
                Val::Text(value)
            }
            23 => {
                let value = js_trim(rhs[0].text()).to_string();
                self.db.set_acc_description(&value);
                Val::Text(value)
            }
            // boundaryStart productions.
            28 => {
                let list = splice_insert(attrs(), 2, "ENTERPRISE");
                self.db.add_person_or_system_boundary(&list);
                Val::Attrs(list)
            }
            29 => {
                let list = splice_insert(attrs(), 2, "SYSTEM");
                self.db.add_person_or_system_boundary(&list);
                Val::Attrs(list)
            }
            30 => {
                let list = attrs();
                self.db.add_person_or_system_boundary(&list);
                Val::Attrs(list)
            }
            31 => {
                let list = splice_insert(attrs(), 2, "CONTAINER");
                self.db.add_container_boundary(&list);
                Val::Attrs(list)
            }
            32..=34 => {
                let kind = ["node", "nodeL", "nodeR"][production - 32];
                let list = attrs();
                self.db.add_deployment_node(kind, &list);
                Val::Attrs(list)
            }
            // boundaryStopStatement -> RBRACE
            35 => {
                self.db.pop_boundary_parse_stack();
                Val::Nil
            }
            39..=58 => {
                let list = attrs();
                if let Some((kind, has_techn)) = element_kind(production) {
                    if has_techn {
                        self.db.add_container(kind, &list);
                    } else {
                        self.db.add_person_or_system(kind, &list);
                    }
                }
                Val::Attrs(list)
            }
            60..=66 => {
                let kind =
                    ["rel", "birel", "rel_u", "rel_d", "rel_l", "rel_r", "rel_b"][production - 60];
                let list = attrs();
                self.db.add_rel(kind, &list);
                Val::Attrs(list)
            }
            // REL_INDEX attributes: `$2.splice(0, 1)` drops the index.
            67 => {
                let mut list = attrs();
                if !list.is_empty() {
                    list.remove(0);
                }
                self.db.add_rel("rel", &list);
                Val::Attrs(list)
            }
            68 => {
                let list = attrs();
                self.db.update_el_style(&list);
                Val::Attrs(list)
            }
            69 => {
                let list = attrs();
                self.db.update_rel_style(&list);
                Val::Attrs(list)
            }
            70 => {
                let list = attrs();
                self.db.update_layout_config(&list);
                Val::Attrs(list)
            }
            // attributes -> attribute: `[$1]`
            71 => match &rhs[0] {
                Val::Attr(a) => Val::Attrs(vec![a.clone()]),
                _ => Val::Attrs(Vec::new()),
            },
            // attributes -> attribute attributes: `$2.unshift($1)`
            72 => {
                let mut list = rhs[1].attrs();
                if let Val::Attr(a) = &rhs[0] {
                    list.insert(0, a.clone());
                }
                Val::Attrs(list)
            }
            // attribute -> STR | ATTRIBUTE: `$1.trim()`
            73 | 75 => Val::Attr(Attr::Str(js_trim(rhs[0].text()).to_string())),
            // attribute -> STR_KEY STR_VALUE: `{ [$1.trim()]: $2.trim() }`
            74 => Val::Attr(Attr::Kv(
                js_trim(rhs[0].text()).to_string(),
                js_trim(rhs[1].text()).to_string(),
            )),
            // attribute -> ATTRIBUTE_EMPTY: `""`
            76 => Val::Attr(Attr::Str(String::new())),
            // `diagramStatement -> boundaryStatement` and the structural
            // productions carry no value the db needs; `$$` defaults to
            // `$1`.
            _ => rhs.first().cloned().unwrap_or(Val::Nil),
        };
        Ok(v)
    }
}
