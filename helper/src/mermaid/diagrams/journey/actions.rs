//! The semantic actions of `journey.jison` (mermaid 12.2.0), one match arm
//! per production index of `journey.tables.json`.
//!
//! Every statement action calls the db at once (`yy.addSection`,
//! `yy.addTask`, `yy.setDiagramTitle`), so the actions own the
//! [`JourneyDb`] and the parse result itself is unused.

use crate::mermaid::directive::js_trim;
use crate::mermaid::jison::{Actions, Lit, Loc, ParseError, Yy};

use super::db::JourneyDb;

/// A semantic value on jison's `vstack`: a token's `yytext` or nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Nil,
    Text(String),
}

impl Val {
    fn text(&self) -> &str {
        match self {
            Val::Text(s) => s,
            Val::Nil => "",
        }
    }
}

#[derive(Debug, Default)]
pub struct JourneyActions {
    pub db: JourneyDb,
}

impl JourneyActions {
    pub fn new(db: JourneyDb) -> JourneyActions {
        JourneyActions { db }
    }
}

impl Yy for JourneyActions {
    fn yy_call(&mut self, _name: &str, _args: &[Lit]) -> Lit {
        Lit::Undefined
    }
}

/// `$1.substr(n)`: the keyword and its separator dropped.
fn skip_chars(text: &str, n: usize) -> String {
    text.chars().skip(n).collect()
}

impl Actions for JourneyActions {
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
        let v = match production {
            // statement -> title: `yy.setDiagramTitle($1.substr(6))`
            8 => {
                let title = skip_chars(rhs[0].text(), 6);
                self.db.set_diagram_title(&title);
                Val::Text(title)
            }
            // statement -> acc_title acc_title_value
            9 => {
                let value = js_trim(rhs[1].text()).to_string();
                self.db.set_acc_title(&value);
                Val::Text(value)
            }
            // statement -> acc_descr acc_descr_value
            10 => {
                let value = js_trim(rhs[1].text()).to_string();
                self.db.set_acc_description(&value);
                Val::Text(value)
            }
            // statement -> acc_descr_multiline_value
            11 => {
                let value = js_trim(rhs[0].text()).to_string();
                self.db.set_acc_description(&value);
                Val::Text(value)
            }
            // statement -> section: `yy.addSection($1.substr(8))`
            12 => {
                let section = skip_chars(rhs[0].text(), 8);
                self.db.add_section(&section);
                Val::Text(section)
            }
            // statement -> taskName taskData: `yy.addTask($1, $2)`
            13 => {
                self.db.add_task(rhs[0].text(), rhs[1].text());
                Val::Text("task".to_string())
            }
            // start, document, and line productions carry no value the db
            // needs.
            _ => Val::Nil,
        };
        Ok(v)
    }
}
