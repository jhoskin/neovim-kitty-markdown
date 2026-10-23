//! The grammar actions of `ishikawa.jison`, keyed by production index in
//! `helper/grammars/ishikawa.tables.json`. The lexer actions only return
//! tokens, so the default `Yy` is enough.

use super::db::IshikawaDb;
use crate::mermaid::jison::{Actions, Loc, ParseError, Yy};

impl Yy for IshikawaDb {}

/// `$1.length` of a `SPACELIST`: whitespace is all in the BMP, so the
/// UTF-16 length is the character count.
fn js_length(text: &str) -> usize {
    text.chars().count()
}

/// `String.prototype.trim`: Rust's Unicode white space plus the BOM.
fn js_trim(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
}

impl Actions for IshikawaDb {
    type Value = String;

    fn terminal(&mut self, yytext: &str) -> String {
        yytext.to_string()
    }

    fn reduce(
        &mut self,
        production: usize,
        rhs: &[String],
        locs: &[Loc],
        _yytext: &str,
    ) -> Result<String, ParseError> {
        let at = |i: usize| rhs.get(i).map_or("", String::as_str);
        match production {
            // statement: SPACELIST TEXT { yy.addNode($1.length, $2.trim()) }
            15 => self.add_node(js_length(at(0)), js_trim(at(1))),
            // statement: TEXT { yy.addNode(0, $1.trim()) }
            16 => self.add_node(0, js_trim(at(0))),
            // start, spaceLines, ishikawa (`return yy` at the end), stop,
            // document, and the SPACELINE and SPACELIST statements.
            0..=14 | 17 | 18 => {}
            other => {
                let loc = locs.first().copied().unwrap_or_default();
                return Err(ParseError::action(
                    loc.first_line,
                    loc.first_column,
                    format!("ishikawa grammar has no production {other}"),
                ));
            }
        }
        Ok(String::new())
    }
}
