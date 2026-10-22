//! The LALR(1) parse loop from `parser.parse` in jison 0.4.18.
//!
//! Error recovery is left out: no mermaid grammar has an `error`
//! production, so the first bad token throws in mermaid too. A grammar's
//! `return $2` in its start production ends the parse at the same moment
//! accept would, so the parse result is the start symbol's value.

use super::Grammar;
use super::actions::{Token, Yy};
use super::error::ParseError;
use super::lexer::{Lexed, Lexer, Loc};
use super::tables::Entry;

/// Semantic actions for one grammar. The driver pushes `terminal` values on
/// shift and calls `reduce` with the right-hand side on every reduction.
pub trait Actions: Yy {
    type Value;

    /// The semantic value of a shifted token, jison's `vstack.push(lexer.yytext)`.
    fn terminal(&mut self, yytext: &str) -> Self::Value;

    /// The semantic action of `production` (an index into
    /// `Tables::productions`). `rhs` and `locs` hold one entry per symbol on
    /// the right-hand side; `yytext` is the text of the last shifted token.
    fn reduce(
        &mut self,
        production: usize,
        rhs: &[Self::Value],
        locs: &[Loc],
        yytext: &str,
    ) -> Result<Self::Value, ParseError>;
}

/// The lookahead as jison holds it after `self.symbols_[token] || token`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Symbol {
    Id(u32),
    /// A token name the grammar does not know; it can match no table entry
    /// and jison reports it by name.
    Unknown(String),
}

const EOF_ID: u32 = 1;
const ERROR_ID: u32 = 2;

pub fn parse<A: Actions>(
    grammar: &Grammar,
    input: &str,
    actions: &mut A,
) -> Result<A::Value, ParseError> {
    let tables = &grammar.tables;
    let mut lexer = Lexer::new(grammar, input);
    let mut states: Vec<u32> = vec![0];
    let mut vstack: Vec<A::Value> = Vec::new();
    let mut lstack: Vec<Loc> = vec![lexer.yylloc];
    let mut symbol: Option<Symbol> = None;
    // The parser's own copies, refreshed on shift; the error line comes
    // from this `yylineno`, not the lexer's.
    let mut yytext = String::new();
    let mut yylineno = 0;

    loop {
        let state = *states
            .last()
            .ok_or_else(|| internal("empty state stack", &lexer))?;
        let action: Option<Vec<u32>> = if let Some(default) = tables.default_actions.get(&state) {
            Some(default.clone())
        } else {
            if symbol.is_none() {
                symbol = Some(match lexer.lex(actions)? {
                    Lexed::Eof => Symbol::Id(EOF_ID),
                    Lexed::Token(Token::Id(id)) => Symbol::Id(id),
                    Lexed::Token(Token::Name(name)) => match tables.symbol(&name) {
                        Some(id) => Symbol::Id(id),
                        None => Symbol::Unknown(name),
                    },
                    Lexed::Token(Token::FirstChar) => {
                        let name: String = lexer.yytext.chars().take(1).collect();
                        match tables.symbol(&name) {
                            Some(id) => Symbol::Id(id),
                            None => Symbol::Unknown(name),
                        }
                    }
                });
            }
            let row = tables.table.get(state as usize);
            match (&symbol, row) {
                (Some(Symbol::Id(id)), Some(row)) => match row.get(id) {
                    Some(Entry::Action(a)) => Some(a.clone()),
                    _ => None,
                },
                _ => None,
            }
        };

        let Some(action) = action.filter(|a| !a.is_empty() && a[0] != 0) else {
            let row = tables.table.get(state as usize);
            let expected: Vec<String> = row
                .map(|row| {
                    row.keys()
                        .filter(|id| **id > ERROR_ID)
                        .filter_map(|id| tables.terminal_name(*id))
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let got = match &symbol {
                Some(Symbol::Id(id)) => tables
                    .terminal_name(*id)
                    .map_or_else(|| id.to_string(), str::to_string),
                Some(Symbol::Unknown(name)) => name.clone(),
                None => String::new(),
            };
            return Err(ParseError::parse(
                yylineno + 1,
                lexer.yylloc.first_column,
                &lexer.show_position(),
                expected,
                got,
            ));
        };

        match action[0] {
            1 => {
                let next = *action
                    .get(1)
                    .ok_or_else(|| internal("shift without state", &lexer))?;
                vstack.push(actions.terminal(&lexer.yytext));
                lstack.push(lexer.yylloc);
                states.push(next);
                symbol = None;
                yytext.clone_from(&lexer.yytext);
                yylineno = lexer.yylineno;
            }
            2 => {
                let production = *action
                    .get(1)
                    .ok_or_else(|| internal("reduce without production", &lexer))?
                    as usize;
                let (lhs, len) = *tables
                    .production_sizes
                    .get(production)
                    .ok_or_else(|| internal("reduce of unknown production", &lexer))?;
                let len = len as usize;
                if vstack.len() < len || states.len() <= len {
                    return Err(internal("reduce below stack bottom", &lexer));
                }
                let first = lstack[lstack.len() - len.max(1)];
                let last = lstack[lstack.len() - 1];
                let loc = Loc {
                    first_line: first.first_line,
                    last_line: last.last_line,
                    first_column: first.first_column,
                    last_column: last.last_column,
                };
                let rhs = &vstack[vstack.len() - len..];
                let locs = &lstack[lstack.len() - len..];
                let value = actions.reduce(production, rhs, locs, &yytext)?;
                vstack.truncate(vstack.len() - len);
                lstack.truncate(lstack.len() - len);
                states.truncate(states.len() - len);
                vstack.push(value);
                lstack.push(loc);
                let top = *states
                    .last()
                    .ok_or_else(|| internal("empty state stack", &lexer))?;
                let goto = tables
                    .table
                    .get(top as usize)
                    .and_then(|row| row.get(&lhs))
                    .ok_or_else(|| internal("missing goto", &lexer))?;
                match goto {
                    Entry::Goto(s) => states.push(*s),
                    Entry::Action(_) => return Err(internal("goto cell holds an action", &lexer)),
                }
            }
            3 => {
                return vstack
                    .pop()
                    .ok_or_else(|| internal("accept with empty value stack", &lexer));
            }
            other => return Err(internal(&format!("unknown action kind {other}"), &lexer)),
        }
    }
}

/// A table inconsistency; cannot happen with tables jison generated.
fn internal(what: &str, lexer: &Lexer<'_>) -> ParseError {
    ParseError::action(
        lexer.yylineno + 1,
        lexer.yylloc.first_column,
        format!("internal parser error: {what}"),
    )
}
