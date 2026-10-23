//! The grammar actions of `venn.jison`, keyed by production index in
//! `helper/grammars/venn.tables.json`, and the `yy` hooks its lexer
//! actions use: `getIndentMode`, `setIndentMode`, and the
//! `consumeIndentText` property the `<bol>` rule sets on the db.

use super::db::VennDb;
use crate::mermaid::jison::{Actions, Lit, Loc, ParseError, Yy};

/// A semantic value: token text, an identifier list, a style field, or a
/// style field list.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Val {
    #[default]
    None,
    Text(String),
    List(Vec<String>),
    Field(String, String),
    Fields(Vec<(String, String)>),
}

impl Val {
    fn text(&self) -> &str {
        match self {
            Val::Text(t) => t,
            _ => "",
        }
    }

    fn list(&self) -> Vec<String> {
        match self {
            Val::List(l) => l.clone(),
            Val::Text(t) => vec![t.clone()],
            _ => Vec::new(),
        }
    }
}

impl Yy for VennDb {
    fn yy_call(&mut self, name: &str, args: &[Lit]) -> Lit {
        match name {
            "getIndentMode" => Lit::Bool(self.indent_mode()),
            "setIndentMode" => {
                let enabled = args.first().is_some_and(Lit::truthy);
                self.set_indent_mode(enabled);
                Lit::Undefined
            }
            _ => Lit::Undefined,
        }
    }

    fn yy_get(&mut self, name: &str) -> Lit {
        match name {
            // The db functions exist, so `if (yy.setIndentMode)` holds.
            "getIndentMode" | "setIndentMode" => Lit::Bool(true),
            "consumeIndentText" => Lit::Bool(self.consume_indent_text),
            _ => Lit::Undefined,
        }
    }

    fn yy_set(&mut self, name: &str, value: Lit) {
        if name == "consumeIndentText" {
            self.consume_indent_text = value.truthy();
        }
    }
}

fn action_error(loc: Loc, message: impl Into<String>) -> ParseError {
    ParseError::action(loc.first_line, loc.first_column, message)
}

/// `parseFloat($NUMERIC)`; the token regex only matches decimal numbers.
fn parse_float(text: &str) -> f64 {
    text.parse().unwrap_or(f64::NAN)
}

impl VennDb {
    /// The checks and the `addSubsetData` call every `union` production
    /// shares.
    fn union(
        &mut self,
        loc: Loc,
        list: &[String],
        label: Option<&str>,
        size: Option<f64>,
    ) -> Result<(), ParseError> {
        if list.len() < 2 {
            return Err(action_error(loc, "union requires multiple identifiers"));
        }
        self.validate_union_identifiers(list)
            .map_err(|e| action_error(loc, e))?;
        self.add_subset_data(list, label, size);
        self.set_indent_mode(true);
        Ok(())
    }

    /// `var cs = yy.getCurrentSets(); if (!cs) throw ...; yy.addTextData(cs, ...)`.
    fn indented_text(&mut self, loc: Loc, id: &str, label: Option<&str>) -> Result<(), ParseError> {
        let Some(cs) = self.current_sets().map(<[String]>::to_vec) else {
            return Err(action_error(loc, "text requires set"));
        };
        self.add_text_data(&cs, id, label);
        Ok(())
    }
}

impl Actions for VennDb {
    type Value = Val;

    fn terminal(&mut self, yytext: &str) -> Val {
        Val::Text(yytext.to_string())
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
        let text = |i: usize| -> String { at(i).text().to_string() };
        let value = match production {
            // statement: TITLE; `$1.substr(6)` drops `title` and one space.
            8 => {
                let title: String = text(0).chars().skip(6).collect();
                self.set_diagram_title(&title);
                Val::Text(title)
            }
            // statement: SET identifier [BRACKET_LABEL] [COLON NUMERIC]
            9 => {
                self.add_subset_data(&at(1).list(), None, None);
                self.set_indent_mode(true);
                Val::None
            }
            10 => {
                self.add_subset_data(&at(1).list(), Some(&text(2)), None);
                self.set_indent_mode(true);
                Val::None
            }
            11 => {
                self.add_subset_data(&at(1).list(), None, Some(parse_float(&text(3))));
                self.set_indent_mode(true);
                Val::None
            }
            12 => {
                self.add_subset_data(&at(1).list(), Some(&text(2)), Some(parse_float(&text(4))));
                self.set_indent_mode(true);
                Val::None
            }
            // statement: UNION identifierList [BRACKET_LABEL] [COLON NUMERIC]
            13 => {
                self.union(loc, &at(1).list(), None, None)?;
                Val::None
            }
            14 => {
                self.union(loc, &at(1).list(), Some(&text(2)), None)?;
                Val::None
            }
            15 => {
                self.union(loc, &at(1).list(), None, Some(parse_float(&text(3))))?;
                Val::None
            }
            16 => {
                self.union(
                    loc,
                    &at(1).list(),
                    Some(&text(2)),
                    Some(parse_float(&text(4))),
                )?;
                Val::None
            }
            // statement: TEXT identifierList (IDENTIFIER | STRING | NUMERIC)
            17..=19 => {
                self.add_text_data(&at(1).list(), &text(2), None);
                Val::None
            }
            // statement: TEXT identifierList (IDENTIFIER | STRING) BRACKET_LABEL
            20 | 21 => {
                self.add_text_data(&at(1).list(), &text(2), Some(&text(3)));
                Val::None
            }
            // statement: INDENT_TEXT indentedTextTail
            22 => at(1),
            // statement: STYLE identifierList stylesOpt
            23 => {
                let fields = match at(2) {
                    Val::Fields(f) => f,
                    _ => Vec::new(),
                };
                self.add_style_data(&at(1).list(), &fields);
                Val::None
            }
            // indentedTextTail: (IDENTIFIER | STRING | NUMERIC) [BRACKET_LABEL]
            24..=26 => {
                self.indented_text(loc, &text(0), None)?;
                Val::None
            }
            27 | 28 => {
                self.indented_text(loc, &text(0), Some(&text(1)))?;
                Val::None
            }
            // stylesOpt: styleField
            29 => match at(0) {
                Val::Field(k, v) => Val::Fields(vec![(k, v)]),
                _ => Val::Fields(Vec::new()),
            },
            // stylesOpt: stylesOpt COMMA styleField
            30 => {
                let mut fields = match at(0) {
                    Val::Fields(f) => f,
                    _ => Vec::new(),
                };
                if let Val::Field(k, v) = at(2) {
                    fields.push((k, v));
                }
                Val::Fields(fields)
            }
            // styleField: IDENTIFIER COLON styleValue
            31 => Val::Field(text(0), text(2)),
            // styleValue: STRING
            32 => at(0),
            // styleValue: valueTokens, joined with spaces
            33 => Val::Text(at(0).list().join(" ")),
            // valueTokens: valueToken
            34 => Val::List(vec![text(0)]),
            // valueTokens: valueTokens valueToken
            35 => {
                let mut list = at(0).list();
                list.push(text(1));
                Val::List(list)
            }
            // valueToken and identifier: the token text
            36..=40 | 43 | 44 => at(0),
            // identifierList: identifier
            41 => Val::List(vec![text(0)]),
            // identifierList: identifierList COMMA identifier
            42 => {
                let mut list = at(0).list();
                list.push(text(2));
                Val::List(list)
            }
            // start, optNewlines, document, line: no value the db reads.
            0..=7 => Val::None,
            other => {
                return Err(action_error(
                    loc,
                    format!("venn grammar has no production {other}"),
                ));
            }
        };
        Ok(value)
    }
}
