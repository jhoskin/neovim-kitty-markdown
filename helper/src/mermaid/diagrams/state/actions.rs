//! The semantic actions of `stateDiagram.jison` (mermaid 12.2.0), one
//! match arm per production index of `stateDiagram.tables.json`.
//!
//! The grammar builds plain objects (`{stmt: 'state', ...}`) into nested
//! documents, and `start -> SD document` hands the root document to
//! `yy.setRootDoc`. A production without an action is jison's default
//! `$$ = $1`, so a statement such as `hide empty description` or a
//! floating note pushes its first token's text into the document; those
//! become [`Stmt::Other`], which every later pass ignores, as mermaid's
//! `switch (item.stmt)` does.

use crate::mermaid::jison::{Actions, Lit, Loc, ParseError, Yy};

/// `StateStmt.description`: absent, one string, or the two-part array
/// `STATE_DESCR AS ID:descr` builds.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Descr {
    #[default]
    None,
    Text(String),
    List(Vec<String>),
}

impl Descr {
    /// JavaScript truthiness: `undefined` and `''` are false, an array is
    /// always true.
    pub fn truthy(&self) -> bool {
        match self {
            Descr::None => false,
            Descr::Text(s) => !s.is_empty(),
            Descr::List(_) => true,
        }
    }
}

/// `Note` of `stateDb.ts`.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    /// `left of` or `right of`.
    pub position: String,
    pub text: String,
}

/// `StateStmt` of `stateDb.ts`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StateStmt {
    pub id: String,
    /// `default`, `fork`, `join`, `choice`, or `divider`; `None` for the
    /// note statement, which sets no type.
    pub kind: Option<String>,
    pub description: Descr,
    pub doc: Option<Vec<Stmt>>,
    pub note: Option<Note>,
    /// Set by `docTranslator` on a `[*]`: `true` for a start, `false` for
    /// an end.
    pub start: Option<bool>,
    /// The `:::class` of an `idStatement`.
    pub classes: Vec<String>,
}

impl StateStmt {
    fn new(id: &str, kind: &str) -> StateStmt {
        StateStmt {
            id: id.to_string(),
            kind: Some(kind.to_string()),
            ..StateStmt::default()
        }
    }

    /// `type === 'divider'`.
    pub fn is_divider(&self) -> bool {
        self.kind.as_deref() == Some("divider")
    }
}

/// One entry of a document.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    State(StateStmt),
    Relation {
        state1: Box<StateStmt>,
        state2: Box<StateStmt>,
        description: Option<String>,
    },
    ClassDef {
        id: String,
        classes: String,
    },
    Style {
        id: String,
        style_class: String,
    },
    ApplyClass {
        id: String,
        style_class: String,
    },
    /// `{stmt: 'dir', value}`.
    Dir(String),
    Click {
        id: String,
        url: String,
        tooltip: String,
    },
    /// A value without a `stmt`: the text jison's default action pushed.
    Other,
}

/// A semantic value on jison's `vstack`.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Nil,
    Text(String),
    /// `line -> NL`: `'nl'`, which `document -> document line` skips.
    Nl,
    State(StateStmt),
    Stmt(Stmt),
    Doc(Vec<Stmt>),
}

impl Val {
    fn text(&self) -> &str {
        match self {
            Val::Text(s) => s,
            _ => "",
        }
    }

    fn state(&self) -> StateStmt {
        match self {
            Val::State(s) => s.clone(),
            _ => StateStmt::default(),
        }
    }

    fn doc(&self) -> Vec<Stmt> {
        match self {
            Val::Doc(d) => d.clone(),
            _ => Vec::new(),
        }
    }

    /// The value as `document -> document line` pushes it.
    fn into_stmt(self) -> Stmt {
        match self {
            Val::State(s) => Stmt::State(s),
            Val::Stmt(s) => s,
            _ => Stmt::Other,
        }
    }
}

/// `trimColon(str)` of `stateDb.ts`.
pub fn trim_colon(s: &str) -> String {
    match s.strip_prefix(':') {
        Some(rest) => rest.trim().to_string(),
        None => s.trim().to_string(),
    }
}

/// The parser-side `yy` of `StateDB`: only `getDividerId` keeps state.
/// `setDirection` edits the previous root document, which `setRootDoc`
/// replaces right after, so the direction comes from the `dir`
/// statements alone.
#[derive(Debug, Default)]
pub struct StateActions {
    divider_count: usize,
}

impl Yy for StateActions {
    fn yy_call(&mut self, _name: &str, _args: &[Lit]) -> Lit {
        Lit::Undefined
    }
}

impl StateActions {
    /// `getDividerId()`.
    fn divider_id(&mut self) -> String {
        self.divider_count += 1;
        format!("divider-id-{}", self.divider_count)
    }
}

fn id_statement(id: &str, classes: Option<&str>) -> Val {
    let mut s = StateStmt::new(id.trim(), "default");
    s.description = Descr::Text(String::new());
    if let Some(c) = classes {
        s.classes = vec![c.trim().to_string()];
    }
    Val::State(s)
}

impl Actions for StateActions {
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
        let first = || rhs.first().cloned().unwrap_or(Val::Nil);
        let v = match production {
            // start -> SPACE start | NL start | SD document
            1..=3 => rhs[1].clone(),
            // document -> (empty)
            4 => Val::Doc(Vec::new()),
            // document -> document line
            5 => {
                let mut doc = rhs[0].doc();
                if rhs[1] != Val::Nl {
                    doc.push(rhs[1].clone().into_stmt());
                }
                Val::Doc(doc)
            }
            // line -> SPACE statement
            6 => rhs[1].clone(),
            // line -> statement
            7 => rhs[0].clone(),
            // line -> NL
            8 => Val::Nl,
            // statement -> idStatement
            12 => rhs[0].clone(),
            // statement -> idStatement DESCR
            13 => {
                let mut s = rhs[0].state();
                s.description = Descr::Text(trim_colon(rhs[1].text()));
                Val::State(s)
            }
            // statement -> idStatement '-->' idStatement [DESCR]
            14 | 15 => Val::Stmt(Stmt::Relation {
                state1: Box::new(rhs[0].state()),
                state2: Box::new(rhs[2].state()),
                description: rhs.get(3).map(|d| trim_colon(d.text())),
            }),
            // statement -> COMPOSIT_STATE STRUCT_START document STRUCT_STOP
            19 => {
                let mut s = StateStmt::new(rhs[0].text(), "default");
                s.description = Descr::Text(String::new());
                s.doc = Some(rhs[2].doc());
                Val::State(s)
            }
            // statement -> STATE_DESCR AS ID
            20 => {
                let raw_id = rhs[2].text();
                let description = rhs[0].text().trim().to_string();
                let mut s = StateStmt::new(raw_id, "default");
                if raw_id.contains(':') {
                    let mut parts = raw_id.split(':');
                    s.id = parts.next().unwrap_or("").to_string();
                    let second = parts.next().unwrap_or("").to_string();
                    s.description = Descr::List(vec![description, second]);
                } else {
                    s.description = Descr::Text(description);
                }
                Val::State(s)
            }
            // statement -> STATE_DESCR AS ID STRUCT_START document STRUCT_STOP
            21 => {
                let mut s = StateStmt::new(rhs[2].text(), "default");
                s.description = Descr::Text(rhs[0].text().to_string());
                s.doc = Some(rhs[4].doc());
                Val::State(s)
            }
            // statement -> FORK | JOIN | CHOICE
            22 => Val::State(StateStmt::new(rhs[0].text(), "fork")),
            23 => Val::State(StateStmt::new(rhs[0].text(), "join")),
            24 => Val::State(StateStmt::new(rhs[0].text(), "choice")),
            // statement -> CONCURRENT
            25 => {
                let id = self.divider_id();
                Val::State(StateStmt::new(&id, "divider"))
            }
            // statement -> note notePosition ID NOTE_TEXT
            26 => Val::State(StateStmt {
                id: rhs[2].text().trim().to_string(),
                note: Some(Note {
                    position: rhs[1].text().trim().to_string(),
                    text: rhs[3].text().trim().to_string(),
                }),
                ..StateStmt::default()
            }),
            // statement -> acc_title acc_title_value | acc_descr acc_descr_value
            29 | 30 => Val::Text(rhs[1].text().trim().to_string()),
            // statement -> CLICK idStatement STRING STRING NL
            32 => Val::Stmt(Stmt::Click {
                id: rhs[1].state().id,
                url: rhs[2].text().to_string(),
                tooltip: rhs[3].text().to_string(),
            }),
            // statement -> CLICK idStatement HREF STRING NL
            33 => Val::Stmt(Stmt::Click {
                id: rhs[1].state().id,
                url: rhs[3].text().to_string(),
                tooltip: String::new(),
            }),
            // classDefStatement -> classDef CLASSDEF_ID|DEFAULT CLASSDEF_STYLEOPTS
            34 | 35 => Val::Stmt(Stmt::ClassDef {
                id: rhs[1].text().trim().to_string(),
                classes: rhs[2].text().trim().to_string(),
            }),
            // styleStatement -> style STYLE_IDS STYLEDEF_STYLEOPTS
            36 => Val::Stmt(Stmt::Style {
                id: rhs[1].text().trim().to_string(),
                style_class: rhs[2].text().trim().to_string(),
            }),
            // cssClassStatement -> class CLASSENTITY_IDS STYLECLASS
            37 => Val::Stmt(Stmt::ApplyClass {
                id: rhs[1].text().trim().to_string(),
                style_class: rhs[2].text().trim().to_string(),
            }),
            // direction -> direction_tb | direction_bt | direction_rl | direction_lr
            38 => Val::Stmt(Stmt::Dir("TB".to_string())),
            39 => Val::Stmt(Stmt::Dir("BT".to_string())),
            40 => Val::Stmt(Stmt::Dir("RL".to_string())),
            41 => Val::Stmt(Stmt::Dir("LR".to_string())),
            // idStatement -> ID | EDGE_STATE
            44 | 45 => id_statement(rhs[0].text(), None),
            // idStatement -> ID|EDGE_STATE STYLE_SEPARATOR ID
            46 | 47 => id_statement(rhs[0].text(), Some(rhs[2].text())),
            // Everything else: jison's default `$$ = $1`.
            _ => first(),
        };
        Ok(v)
    }
}
