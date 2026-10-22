//! Serde view of `helper/grammars/<name>.tables.json`, the LALR(1) tables
//! and lexer rules that `build-tables.mjs` dumps from jison 0.4.18.
//!
//! The field names and encodings mirror lib/jison.js so the driver can be
//! read next to the JavaScript template: `symbols_` maps a symbol name to
//! its id, `terminals_` maps a terminal id back to its name, and
//! `table[state][symbol]` is `[1, state]` (shift), `[2, production]`
//! (reduce), `[3]` (accept) or a bare state (goto on a nonterminal).

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::Deserialize;

/// One grammar's tables, deserialized once per process by `super::grammar`.
#[derive(Debug, Deserialize)]
pub struct Tables {
    pub grammar: String,
    pub mermaid_version: String,
    pub jison_version: String,
    pub start: String,
    #[serde(rename = "symbols_")]
    pub symbols: IndexMap<String, u32>,
    /// Keyed by id. A `BTreeMap` iterates in ascending numeric order, which
    /// is the order JavaScript enumerates integer-like keys and therefore
    /// the order jison lists expected tokens in a parse error.
    #[serde(rename = "terminals_")]
    pub terminals: BTreeMap<u32, String>,
    /// `[lhs symbol id, rhs length]` per production; index 0 is `$accept`.
    #[serde(rename = "productions_")]
    pub production_sizes: Vec<(u32, u32)>,
    /// The productions with their symbol names and raw JavaScript action, so
    /// a diagram module can be checked against the grammar it implements.
    pub productions: Vec<Production>,
    pub table: Vec<BTreeMap<u32, Entry>>,
    #[serde(rename = "defaultActions")]
    pub default_actions: BTreeMap<u32, Vec<u32>>,
    pub lexer: LexerTables,
}

#[derive(Debug, Deserialize)]
pub struct Production {
    pub lhs: String,
    pub rhs: Vec<String>,
    pub action: Option<String>,
}

/// One cell of the parse table in jison's encoding.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Entry {
    /// Goto target after a reduce pushed a nonterminal.
    Goto(u32),
    /// `[1, state]`, `[2, production]` or `[3]`.
    Action(Vec<u32>),
}

#[derive(Debug, Deserialize)]
pub struct LexerTables {
    pub options: LexerOptions,
    pub macros: IndexMap<String, String>,
    /// The `%{ ... %}` block before the rules, raw.
    pub action_include: Option<String>,
    pub conditions: IndexMap<String, Condition>,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Default, Deserialize)]
pub struct LexerOptions {
    pub flex: bool,
    pub case_insensitive: bool,
}

#[derive(Debug, Deserialize)]
pub struct Condition {
    /// Rule indices active in this start condition, in rule order.
    pub rules: Vec<usize>,
    pub inclusive: bool,
}

#[derive(Debug, Deserialize)]
pub struct Rule {
    pub index: usize,
    /// JavaScript regex source after macro expansion, without the `^(?:`
    /// `)` wrapper jison-lex adds.
    pub pattern: String,
    /// `None` for a rule without `<...>`, which jison-lex adds to every
    /// inclusive start condition.
    pub conditions: Option<Vec<String>>,
    /// Raw JavaScript action source.
    pub action: String,
}

impl Tables {
    pub fn from_json(json: &str) -> Result<Tables, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Symbol id of `name`, or `None` when the grammar has no such symbol
    /// (jison then uses the name itself as the symbol, which can match no
    /// table entry).
    pub fn symbol(&self, name: &str) -> Option<u32> {
        self.symbols.get(name).copied()
    }

    pub fn terminal_name(&self, id: u32) -> Option<&str> {
        self.terminals.get(&id).map(String::as_str)
    }
}

/// The vendored grammars, named after their `.jison` file in mermaid.
pub const GRAMMARS: [(&str, &str); 18] = [
    ("block", include_str!("../../../grammars/block.tables.json")),
    (
        "c4Diagram",
        include_str!("../../../grammars/c4Diagram.tables.json"),
    ),
    (
        "classDiagram",
        include_str!("../../../grammars/classDiagram.tables.json"),
    ),
    (
        "erDiagram",
        include_str!("../../../grammars/erDiagram.tables.json"),
    ),
    ("flow", include_str!("../../../grammars/flow.tables.json")),
    ("gantt", include_str!("../../../grammars/gantt.tables.json")),
    (
        "ishikawa",
        include_str!("../../../grammars/ishikawa.tables.json"),
    ),
    (
        "journey",
        include_str!("../../../grammars/journey.tables.json"),
    ),
    (
        "kanban",
        include_str!("../../../grammars/kanban.tables.json"),
    ),
    (
        "mindmap",
        include_str!("../../../grammars/mindmap.tables.json"),
    ),
    (
        "quadrant",
        include_str!("../../../grammars/quadrant.tables.json"),
    ),
    (
        "requirementDiagram",
        include_str!("../../../grammars/requirementDiagram.tables.json"),
    ),
    (
        "sankey",
        include_str!("../../../grammars/sankey.tables.json"),
    ),
    (
        "sequenceDiagram",
        include_str!("../../../grammars/sequenceDiagram.tables.json"),
    ),
    (
        "stateDiagram",
        include_str!("../../../grammars/stateDiagram.tables.json"),
    ),
    (
        "timeline",
        include_str!("../../../grammars/timeline.tables.json"),
    ),
    ("venn", include_str!("../../../grammars/venn.tables.json")),
    (
        "xychart",
        include_str!("../../../grammars/xychart.tables.json"),
    ),
];

/// The raw JSON of a vendored grammar.
pub fn json(name: &str) -> Option<&'static str> {
    GRAMMARS.iter().find(|(n, _)| *n == name).map(|(_, j)| *j)
}
