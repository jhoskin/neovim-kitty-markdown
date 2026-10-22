//! The grammar actions of `timeline.jison`, keyed by production index in
//! `helper/grammars/timeline.tables.json`. The lexer actions only switch
//! states, so the default `Yy` is enough.

use super::db::TimelineDb;
use crate::mermaid::jison::lexer::js_trim;
use crate::mermaid::jison::{Actions, Loc, ParseError, Yy};

/// JavaScript `String.prototype.substr(start)`. The grammar only skips an
/// ASCII keyword, so counting characters matches counting UTF-16 units.
fn substr(s: &str, start: usize) -> String {
    s.chars().skip(start).collect()
}

impl Yy for TimelineDb {}

impl Actions for TimelineDb {
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
        let at = |i: usize| rhs.get(i).cloned().unwrap_or_default();
        let action_error = |message: String| {
            let loc = locs.first().copied().unwrap_or_default();
            ParseError::action(loc.first_line, loc.first_column, message)
        };
        let value = match production {
            // start: timeline_header document EOF { return $2; }
            1 => at(1),
            // timeline_header: timeline
            2 => at(0),
            3 => {
                self.set_direction("LR");
                at(0)
            }
            4 => {
                self.set_direction("TD");
                at(0)
            }
            // document: /* empty */ | document line
            5 | 6 => String::new(),
            // line: SPACE statement { $$ = $2 }
            7 => at(1),
            // line: statement { $$ = $1 }
            8 => at(0),
            // line: NEWLINE | EOF { $$ = []; }
            9 | 10 => String::new(),
            // statement: title { setDiagramTitle($1.substr(6)); $$ = $1.substr(6); }
            11 => {
                let v = substr(&at(0), 6);
                self.set_diagram_title(&v);
                v
            }
            12 => {
                let v = js_trim(&at(1)).to_string();
                self.set_acc_title(&v);
                v
            }
            13 => {
                let v = js_trim(&at(1)).to_string();
                self.set_acc_description(&v);
                v
            }
            14 => {
                let v = js_trim(&at(0)).to_string();
                self.set_acc_description(&v);
                v
            }
            // statement: section { addSection($1.substr(8)); $$ = $1.substr(8); }
            15 => {
                let v = substr(&at(0), 8);
                self.add_section(&v);
                v
            }
            // statement: period_statement | event_statement
            16 | 17 => at(0),
            // period_statement: period { addTask($1, 0, ''); $$ = $1; }
            18 => {
                let v = at(0);
                self.add_task(&v, 0.0, "");
                v
            }
            // event_statement: event { addEvent($1.substr(2)); $$ = $1; }
            19 => {
                let v = at(0);
                self.add_event(&substr(&v, 2))
                    .map_err(|e| action_error(e.to_string()))?;
                v
            }
            other => {
                return Err(action_error(format!(
                    "timeline grammar has no production {other}"
                )));
            }
        };
        Ok(value)
    }
}
