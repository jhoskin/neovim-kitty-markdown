//! The grammar actions of `kanban.jison`, keyed by production index in
//! `helper/grammars/kanban.tables.json`. The lexer actions only call
//! `yy.getLogger()`, so the default `Yy` is enough.

use super::db::{KanbanDb, get_type};
use crate::mermaid::jison::{Actions, Loc, ParseError, Yy};

/// `{ id, descr, type }`, what the `node` productions build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedNode {
    pub id: String,
    pub descr: String,
    pub node_type: u8,
}

/// A semantic value: token text, a concatenated `shapeData` string, or a
/// node object.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Val {
    pub text: String,
    pub node: Option<ParsedNode>,
}

impl Val {
    fn text(text: impl Into<String>) -> Val {
        Val {
            text: text.into(),
            node: None,
        }
    }

    fn node(id: &str, descr: &str, node_type: u8) -> Val {
        Val {
            text: String::new(),
            node: Some(ParsedNode {
                id: id.to_string(),
                descr: descr.to_string(),
                node_type,
            }),
        }
    }
}

impl Yy for KanbanDb {}

fn action_error(loc: Loc, message: impl Into<String>) -> ParseError {
    ParseError::action(loc.first_line, loc.first_column, message)
}

/// `$n.length` of a `SPACELIST`: whitespace is all in the BMP, so the
/// UTF-16 length is the character count.
fn js_length(text: &str) -> usize {
    text.chars().count()
}

impl Actions for KanbanDb {
    type Value = Val;

    fn terminal(&mut self, yytext: &str) -> Val {
        Val::text(yytext)
    }

    fn reduce(
        &mut self,
        production: usize,
        rhs: &[Val],
        locs: &[Loc],
        _yytext: &str,
    ) -> Result<Val, ParseError> {
        let loc = locs.first().copied().unwrap_or_default();
        let at = |i: usize| -> Val { rhs.get(i).cloned().unwrap_or_default() };
        let node_at = |i: usize| -> Result<ParsedNode, ParseError> {
            rhs.get(i).and_then(|v| v.node.clone()).ok_or_else(|| {
                action_error(
                    loc,
                    format!("kanban production {production}: symbol {i} is not a node"),
                )
            })
        };
        let value = match production {
            // statement: SPACELIST node shapeData
            15 => {
                let n = node_at(1)?;
                self.add_node(
                    js_length(&at(0).text),
                    &n.id,
                    &n.descr,
                    n.node_type,
                    Some(&at(2).text),
                )
                .map_err(|e| action_error(loc, e.to_string()))?;
                Val::default()
            }
            // statement: SPACELIST node
            16 => {
                let n = node_at(1)?;
                self.add_node(js_length(&at(0).text), &n.id, &n.descr, n.node_type, None)
                    .map_err(|e| action_error(loc, e.to_string()))?;
                Val::default()
            }
            // statement: SPACELIST ICON
            17 => {
                self.decorate_icon(&at(1).text)
                    .map_err(|e| action_error(loc, e.to_string()))?;
                Val::default()
            }
            // statement: SPACELIST CLASS
            18 => {
                self.decorate_class(&at(1).text)
                    .map_err(|e| action_error(loc, e.to_string()))?;
                Val::default()
            }
            // statement: node shapeData
            20 => {
                let n = node_at(0)?;
                self.add_node(0, &n.id, &n.descr, n.node_type, Some(&at(1).text))
                    .map_err(|e| action_error(loc, e.to_string()))?;
                Val::default()
            }
            // statement: node
            21 => {
                let n = node_at(0)?;
                self.add_node(0, &n.id, &n.descr, n.node_type, None)
                    .map_err(|e| action_error(loc, e.to_string()))?;
                Val::default()
            }
            // statement: ICON
            22 => {
                self.decorate_icon(&at(0).text)
                    .map_err(|e| action_error(loc, e.to_string()))?;
                Val::default()
            }
            // statement: CLASS
            23 => {
                self.decorate_class(&at(0).text)
                    .map_err(|e| action_error(loc, e.to_string()))?;
                Val::default()
            }
            // node: nodeWithId | nodeWithoutId
            25 | 26 => at(0),
            // nodeWithoutId: NODE_DSTART NODE_DESCR NODE_DEND
            27 => {
                let descr = at(1).text;
                Val::node(&descr, &descr, get_type(&at(0).text, &at(2).text))
            }
            // nodeWithId: NODE_ID
            28 => {
                let id = at(0).text;
                Val::node(&id, &id, 0)
            }
            // nodeWithId: NODE_ID NODE_DSTART NODE_DESCR NODE_DEND
            29 => Val::node(&at(0).text, &at(2).text, get_type(&at(1).text, &at(3).text)),
            // shapeData: shapeData SHAPE_DATA { $$ = $1 + $2 }
            30 => Val::text(format!("{}{}", at(0).text, at(1).text)),
            // shapeData: SHAPE_DATA
            31 => at(0),
            // start, spaceLines, mindMap, stop, document, SPACELINE and
            // SPACELIST statements: no value.
            0..=14 | 19 | 24 => Val::default(),
            other => {
                return Err(action_error(
                    loc,
                    format!("kanban grammar has no production {other}"),
                ));
            }
        };
        Ok(value)
    }
}
