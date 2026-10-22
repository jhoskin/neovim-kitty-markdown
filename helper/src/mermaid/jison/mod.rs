//! LALR(1) driver over vendored jison tables, lexer, oracle-form error text.
//!
//! mermaid 12.2.0 parses its diagrams with parsers that jison 0.4.18
//! generates from the grammars in `helper/grammars/`. This module replays
//! jison's tables instead of porting each generated parser: `tables` loads
//! the JSON that `build-tables.mjs` emits, `lexer` is a port of jison-lex,
//! `actions` translates the JavaScript in lexer actions, `driver` is the
//! parse loop, and `error` prints errors as jison does. A diagram module
//! supplies the grammar actions through the `Actions` trait.

pub mod actions;
pub mod driver;
pub mod error;
pub mod jsregex;
pub mod lexer;
pub mod tables;

use std::fmt;
use std::sync::OnceLock;

use fancy_regex::Regex;

pub use actions::{Lit, Program, Token, Yy};
pub use driver::{Actions, parse};
pub use error::{ErrorKind, ParseError};
pub use lexer::{Lexer, Loc};
pub use tables::Tables;

/// One lexer rule ready to run.
#[derive(Debug)]
pub struct CompiledRule {
    pub regex: Regex,
    pub program: Program,
}

/// A grammar's tables with its lexer rules compiled and translated.
#[derive(Debug)]
pub struct Grammar {
    pub name: &'static str,
    pub tables: Tables,
    pub rules: Vec<CompiledRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    UnknownGrammar(String),
    Json {
        grammar: String,
        message: String,
    },
    Regex {
        grammar: String,
        rule: usize,
        message: String,
    },
    Action(actions::Unsupported),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::UnknownGrammar(name) => write!(f, "no vendored grammar named {name}"),
            LoadError::Json { grammar, message } => write!(f, "{grammar}.tables.json: {message}"),
            LoadError::Regex {
                grammar,
                rule,
                message,
            } => write!(f, "{grammar} lexer rule {rule}: {message}"),
            LoadError::Action(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LoadError {}

impl Grammar {
    /// Parses and compiles one grammar from its JSON.
    pub fn load(name: &'static str, json: &str) -> Result<Grammar, LoadError> {
        let tables = Tables::from_json(json).map_err(|e| LoadError::Json {
            grammar: name.to_string(),
            message: e.to_string(),
        })?;
        let lexer = &tables.lexer;
        actions::check_include(name, lexer.action_include.as_deref()).map_err(LoadError::Action)?;
        let mut rules = Vec::with_capacity(lexer.rules.len());
        for rule in &lexer.rules {
            let regex = jsregex::compile_rule(&rule.pattern, lexer.options.case_insensitive)
                .map_err(|e| LoadError::Regex {
                    grammar: name.to_string(),
                    rule: rule.index,
                    message: e.to_string(),
                })?;
            let program =
                actions::translate(name, rule.index, &rule.action).map_err(LoadError::Action)?;
            rules.push(CompiledRule { regex, program });
        }
        Ok(Grammar {
            name,
            tables,
            rules,
        })
    }
}

static GRAMMARS: [OnceLock<Result<Grammar, LoadError>>; tables::GRAMMARS.len()] =
    [const { OnceLock::new() }; tables::GRAMMARS.len()];

/// The vendored grammar `name` (its `.jison` basename in mermaid, such as
/// `flow` or `sequenceDiagram`), loaded once per process.
pub fn grammar(name: &str) -> Result<&'static Grammar, LoadError> {
    let Some(index) = tables::GRAMMARS.iter().position(|(n, _)| *n == name) else {
        return Err(LoadError::UnknownGrammar(name.to_string()));
    };
    let (static_name, json) = tables::GRAMMARS[index];
    match GRAMMARS[index].get_or_init(|| Grammar::load(static_name, json)) {
        Ok(g) => Ok(g),
        Err(e) => Err(e.clone()),
    }
}

/// Names of every vendored grammar.
pub fn grammar_names() -> impl Iterator<Item = &'static str> {
    tables::GRAMMARS.iter().map(|(n, _)| *n)
}
