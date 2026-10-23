//! The semantic actions of `erDiagram.jison` (mermaid 12.2.0), one match
//! arm per production index of `erDiagram.tables.json`.
//!
//! The grammar calls the db (`yy.addEntity`, `yy.addRelationship`, ...)
//! while it reduces, so [`ErActions`] owns the [`ErDb`] and makes the same
//! calls in the same order. A production without an action keeps jison's
//! default `$$ = $1`. The values the statements return matter only inside
//! a subgraph, whose `document` collects them for `addSubGraph`.

use crate::mermaid::jison::{Actions, Loc, ParseError, Yy};

use super::db::{Attribute, Cardinality, ErDb, Identification, Item, RelSpec};

/// A semantic value on jison's `vstack`.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Nil,
    /// A token's `yytext` or a string a production built.
    Text(String),
    /// The arrays `document`, `line`, and the statements return.
    Items(Vec<Item>),
    /// `{ stmt: 'dir', value }`.
    Dir(String),
    /// `idList`, `stylesOpt`, `attributeKeyTypeList`.
    List(Vec<String>),
    Attribute(Attribute),
    /// `attributes`, last attribute first.
    Attributes(Vec<Attribute>),
    Cardinality(Cardinality),
    Identification(Identification),
    RelSpec(RelSpec),
    /// `subgraphHeader`: `{ id, text }`.
    Header(String, String),
}

impl Val {
    fn text(&self) -> String {
        match self {
            Val::Text(s) => s.clone(),
            _ => String::new(),
        }
    }

    fn list(&self) -> Vec<String> {
        match self {
            Val::List(l) => l.clone(),
            Val::Text(s) => vec![s.clone()],
            _ => Vec::new(),
        }
    }

    fn attributes(&self) -> Vec<Attribute> {
        match self {
            Val::Attributes(a) => a.clone(),
            Val::Attribute(a) => vec![a.clone()],
            _ => Vec::new(),
        }
    }

    fn cardinality(&self) -> Cardinality {
        match self {
            Val::Cardinality(c) => *c,
            _ => Cardinality::OnlyOne,
        }
    }

    fn rel_spec(&self) -> RelSpec {
        match self {
            Val::RelSpec(r) => *r,
            _ => RelSpec {
                card_a: Cardinality::OnlyOne,
                card_b: Cardinality::OnlyOne,
                rel_type: Identification::Identifying,
            },
        }
    }

    /// `[].concat(value)`: an array spreads, anything else is one element.
    fn concat_into(self, out: &mut Vec<Item>) {
        match self {
            Val::Items(items) => out.extend(items),
            Val::Text(s) => out.push(Item::Name(s)),
            Val::Dir(d) => out.push(Item::Dir(d)),
            Val::List(l) => out.extend(l.into_iter().map(Item::Name)),
            _ => out.push(Item::Other),
        }
    }
}

#[derive(Debug, Default)]
pub struct ErActions {
    pub db: ErDb,
}

impl ErActions {
    pub fn new() -> ErActions {
        ErActions { db: ErDb::new() }
    }
}

impl Yy for ErActions {}

fn names(names: &[&str]) -> Val {
    Val::Items(names.iter().map(|n| Item::Name(n.to_string())).collect())
}

impl Actions for ErActions {
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
        let at = |i: usize| rhs.get(i).cloned().unwrap_or(Val::Nil);
        let text = |i: usize| rhs.get(i).map(Val::text).unwrap_or_default();
        let list = |i: usize| rhs.get(i).map(Val::list).unwrap_or_default();
        let attrs = |i: usize| rhs.get(i).map(Val::attributes).unwrap_or_default();
        let spec = |i: usize| rhs.get(i).map(Val::rel_spec);
        let db = &mut self.db;
        let v = match production {
            // document -> /* empty */ ; line -> NEWLINE | EOF
            2 | 5 | 6 => Val::Items(Vec::new()),
            // document -> document line
            3 => {
                let mut items = Vec::new();
                at(0).concat_into(&mut items);
                at(1).concat_into(&mut items);
                Val::Items(items)
            }
            // entityName relSpec entityName COLON role
            7 => {
                let (a, b) = (text(0), text(2));
                db.add_entity(&a, "");
                db.add_entity(&b, "");
                if let Some(s) = spec(1) {
                    db.add_relationship(&a, &text(4), &b, s);
                }
                names(&[&a, &b])
            }
            // entityName ::: idList relSpec entityName ::: idList COLON role
            8 => {
                let (a, b) = (text(0), text(4));
                db.add_entity(&a, "");
                db.add_entity(&b, "");
                if let Some(s) = spec(3) {
                    db.add_relationship(&a, &text(8), &b, s);
                }
                db.set_class(std::slice::from_ref(&a), &list(2));
                db.set_class(std::slice::from_ref(&b), &list(6));
                names(&[&a, &b])
            }
            // entityName ::: idList relSpec entityName COLON role
            9 => {
                let (a, b) = (text(0), text(4));
                db.add_entity(&a, "");
                db.add_entity(&b, "");
                if let Some(s) = spec(3) {
                    db.add_relationship(&a, &text(6), &b, s);
                }
                db.set_class(std::slice::from_ref(&a), &list(2));
                names(&[&a, &b])
            }
            // entityName relSpec entityName ::: idList COLON role
            10 => {
                let (a, b) = (text(0), text(2));
                db.add_entity(&a, "");
                db.add_entity(&b, "");
                if let Some(s) = spec(1) {
                    db.add_relationship(&a, &text(6), &b, s);
                }
                db.set_class(std::slice::from_ref(&b), &list(4));
                names(&[&a, &b])
            }
            // entityName BLOCK_START attributes BLOCK_STOP
            11 => {
                let a = text(0);
                db.add_entity(&a, "");
                db.add_attributes(&a, &attrs(2));
                names(&[&a])
            }
            // entityName ::: idList BLOCK_START attributes BLOCK_STOP
            12 => {
                let a = text(0);
                db.add_entity(&a, "");
                db.add_attributes(&a, &attrs(4));
                db.set_class(std::slice::from_ref(&a), &list(2));
                names(&[&a])
            }
            // entityName BLOCK_START BLOCK_STOP ; entityName
            13 | 15 => {
                let a = text(0);
                db.add_entity(&a, "");
                names(&[&a])
            }
            // entityName ::: idList BLOCK_START BLOCK_STOP ; entityName ::: idList
            14 | 16 => {
                let a = text(0);
                db.add_entity(&a, "");
                db.set_class(std::slice::from_ref(&a), &list(2));
                names(&[&a])
            }
            // entityName SQS entityName SQE BLOCK_START attributes BLOCK_STOP
            17 => {
                let a = text(0);
                db.add_entity(&a, &text(2));
                db.add_attributes(&a, &attrs(5));
                names(&[&a])
            }
            // entityName SQS entityName SQE ::: idList BLOCK_START attributes BLOCK_STOP
            18 => {
                let a = text(0);
                db.add_entity(&a, &text(2));
                db.add_attributes(&a, &attrs(7));
                db.set_class(std::slice::from_ref(&a), &list(5));
                names(&[&a])
            }
            // entityName SQS entityName SQE BLOCK_START BLOCK_STOP
            19 => {
                let a = text(0);
                db.add_entity(&a, &text(2));
                names(&[&a])
            }
            // entityName SQS entityName SQE ::: idList BLOCK_START BLOCK_STOP
            20 => {
                let a = text(0);
                db.add_entity(&a, &text(2));
                db.set_class(std::slice::from_ref(&a), &list(5));
                names(&[&a])
            }
            // entityName SQS entityName SQE: no `$$`, so `$$ = $1`.
            21 => {
                db.add_entity(&text(0), &text(2));
                at(0)
            }
            // entityName SQS entityName SQE ::: idList
            22 => {
                let a = text(0);
                db.add_entity(&a, &text(2));
                db.set_class(std::slice::from_ref(&a), &list(5));
                at(0)
            }
            // title title_value ; acc_title acc_title_value
            23 | 24 => {
                let t = text(1).trim().to_string();
                db.acc_title = Some(t.clone());
                Val::Text(t)
            }
            // acc_descr acc_descr_value
            25 => {
                let t = text(1).trim().to_string();
                db.acc_description = Some(t.clone());
                Val::Text(t)
            }
            // acc_descr_multiline_value
            26 => {
                let t = text(0).trim().to_string();
                db.acc_description = Some(t.clone());
                Val::Text(t)
            }
            // statement -> direction
            27 => {
                let Val::Dir(dir) = at(0) else {
                    return Ok(Val::Nil);
                };
                if db.subgraph_depth == 0 {
                    db.set_direction(&dir);
                    Val::Items(Vec::new())
                } else {
                    Val::Dir(dir)
                }
            }
            // subgraphHeader document END
            31 => {
                // `(yy.subgraphDepth || 1) - 1`
                let depth = if db.subgraph_depth == 0 {
                    1
                } else {
                    db.subgraph_depth
                };
                db.subgraph_depth = depth - 1;
                let (id, title) = match at(0) {
                    Val::Header(id, title) => (id, title),
                    _ => (String::new(), String::new()),
                };
                let mut items = Vec::new();
                at(1).concat_into(&mut items);
                Val::Text(db.add_sub_graph(&id, &items, &title))
            }
            // SUBGRAPH entityName separator
            32 => {
                db.subgraph_depth += 1;
                Val::Header(text(1), text(1))
            }
            // SUBGRAPH entityName SQS subgraphTitle SQE separator
            33 => {
                db.subgraph_depth += 1;
                Val::Header(text(1), text(3))
            }
            // subgraphTitle -> subgraphTitle entityName
            35 => Val::Text(format!("{} {}", text(0), text(1))),
            36 => Val::Dir("TB".to_string()),
            37 => Val::Dir("BT".to_string()),
            38 => Val::Dir("RL".to_string()),
            39 => Val::Dir("LR".to_string()),
            // CLASSDEF idList stylesOpt separator: `$$ = $CLASSDEF`
            40 => {
                db.add_class(&list(1), &list(2));
                at(0)
            }
            // idList -> UNICODE_TEXT | STYLE_TEXT ; stylesOpt -> style
            41 | 42 | 47 => Val::List(vec![text(0)]),
            // idList -> idList COMMA (UNICODE_TEXT | STYLE_TEXT) ;
            // stylesOpt -> stylesOpt COMMA style ;
            // attributeKeyTypeList -> attributeKeyTypeList , attributeKeyType
            43 | 44 | 48 | 73 => {
                let mut l = list(0);
                l.push(text(2));
                Val::List(l)
            }
            // CLASS idList idList: `$$ = $CLASS`
            45 => {
                db.set_class(&list(1), &list(2));
                at(0)
            }
            // STYLE idList stylesOpt separator: `$$ = $STYLE`
            46 => {
                db.add_css_styles(&list(1), &list(2));
                at(0)
            }
            // style -> style styleComponent ; attributeType -> ATTRIBUTE_WORD '?'
            50 | 70 => Val::Text(format!("{}{}", text(0), text(1))),
            // entityName -> ENTITY_NAME ; attributeComment -> COMMENT ;
            // role -> WORD | ENTITY_NAME
            58 | 75 | 84 | 85 => Val::Text(text(0).replace('"', "")),
            // attributes -> attribute
            63 => Val::Attributes(attrs(0)),
            // attributes -> attribute attributes: `$2.push($1)`
            64 => {
                let mut a = attrs(1);
                a.extend(attrs(0));
                Val::Attributes(a)
            }
            // attribute -> attributeType attributeName [keys] [comment]
            65..=68 => Val::Attribute(Attribute {
                type_name: text(0),
                name: text(1),
                keys: matches!(production, 66 | 68).then(|| list(2)),
                comment: match production {
                    67 => Some(text(2)),
                    68 => Some(text(3)),
                    _ => None,
                },
            }),
            // attributeKeyTypeList -> attributeKeyType
            72 => Val::List(vec![text(0)]),
            // relSpec -> cardinality relType cardinality
            76 => {
                let rel_type = match at(1) {
                    Val::Identification(i) => i,
                    _ => Identification::Identifying,
                };
                Val::RelSpec(RelSpec {
                    card_a: at(2).cardinality(),
                    rel_type,
                    card_b: at(0).cardinality(),
                })
            }
            77 => Val::Cardinality(Cardinality::ZeroOrOne),
            78 => Val::Cardinality(Cardinality::ZeroOrMore),
            79 => Val::Cardinality(Cardinality::OneOrMore),
            80 => Val::Cardinality(Cardinality::OnlyOne),
            81 => Val::Cardinality(Cardinality::MdParent),
            82 => Val::Identification(Identification::NonIdentifying),
            83 => Val::Identification(Identification::Identifying),
            // Every other production: `$$ = $1`.
            _ => at(0),
        };
        Ok(v)
    }
}
