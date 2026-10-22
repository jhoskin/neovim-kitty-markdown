//! The grammar actions of `gantt.jison`, keyed by production index in
//! `helper/grammars/gantt.tables.json`. The lexer actions of the grammar
//! call nothing on `yy`, so the default `Yy` is enough.

use super::db::GanttDb;
use crate::mermaid::jison::lexer::js_trim;
use crate::mermaid::jison::{Actions, Loc, ParseError, Yy};

/// JavaScript `String.prototype.substr(start)`. The grammar only skips an
/// ASCII keyword, so counting characters matches counting UTF-16 units.
fn substr(s: &str, start: usize) -> String {
    s.chars().skip(start).collect()
}

impl Yy for GanttDb {}

impl Actions for GanttDb {
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
        let db_error = |err: super::db::GanttError| {
            let loc = locs.first().copied().unwrap_or_default();
            ParseError::action(loc.first_line, loc.first_column, err.to_string())
        };
        let value = match production {
            // start: gantt document EOF { return $2; }
            1 => at(1),
            // document: /* empty */ { $$ = [] } | document line { $1.push($2); $$ = $1 }
            2 | 3 => String::new(),
            // line: SPACE statement { $$ = $2 }
            4 => at(1),
            // line: statement { $$ = $1 }
            5 => at(0),
            // line: NL | EOF { $$ = []; }
            6 | 7 => String::new(),
            8 => {
                self.set_weekday("monday");
                String::new()
            }
            9 => {
                self.set_weekday("tuesday");
                String::new()
            }
            10 => {
                self.set_weekday("wednesday");
                String::new()
            }
            11 => {
                self.set_weekday("thursday");
                String::new()
            }
            12 => {
                self.set_weekday("friday");
                String::new()
            }
            13 => {
                self.set_weekday("saturday");
                String::new()
            }
            14 => {
                self.set_weekday("sunday");
                String::new()
            }
            15 => {
                self.set_weekend("friday");
                String::new()
            }
            16 => {
                self.set_weekend("saturday");
                String::new()
            }
            // statement: dateFormat { yy.setDateFormat($1.substr(11)); $$ = $1.substr(11); }
            17 => {
                let v = substr(&at(0), 11);
                self.set_date_format(&v);
                v
            }
            18 => {
                self.enable_inclusive_end_dates();
                substr(&at(0), 18)
            }
            // statement: topAxis { yy.TopAxis(); ... }: the grammar names a
            // method `ganttDb` does not have (it exports `enableTopAxis`),
            // so mermaid 12.2.0 throws on a `topAxis` statement. The top
            // axis works through the `gantt.topAxis` config only.
            19 => {
                let loc = locs.first().copied().unwrap_or_default();
                return Err(ParseError::action(
                    loc.first_line,
                    loc.first_column,
                    "TypeError: yy.TopAxis is not a function",
                ));
            }
            20 => {
                let v = substr(&at(0), 11);
                self.set_axis_format(&v);
                v
            }
            21 => {
                let v = substr(&at(0), 13);
                self.set_tick_interval(&v);
                v
            }
            22 => {
                let v = substr(&at(0), 9);
                self.set_excludes(&v);
                v
            }
            23 => {
                let v = substr(&at(0), 9);
                self.set_includes(&v);
                v
            }
            24 => {
                let v = substr(&at(0), 12);
                self.set_today_marker(&v);
                v
            }
            // statement: weekday | weekend | clickStatement (no action, $$ = $1)
            25 | 26 | 32 => at(0),
            27 => {
                let v = substr(&at(0), 6);
                self.set_diagram_title(&v);
                v
            }
            28 => {
                let v = js_trim(&at(1)).to_string();
                self.set_acc_title(&v);
                v
            }
            29 => {
                let v = js_trim(&at(1)).to_string();
                self.set_acc_description(&v);
                v
            }
            30 => {
                let v = js_trim(&at(0)).to_string();
                self.set_acc_description(&v);
                v
            }
            31 => {
                let v = substr(&at(0), 8);
                self.add_section(&v);
                v
            }
            // statement: taskTxt taskData { yy.addTask($1, $2); $$ = 'task'; }
            33 => {
                self.add_task(&at(0), &at(1)).map_err(db_error)?;
                "task".to_string()
            }
            34 => {
                self.set_click_event(&at(0), &at(1), None);
                at(0)
            }
            35 => {
                self.set_click_event(&at(0), &at(1), Some(&at(2)));
                at(0)
            }
            36 => {
                self.set_click_event(&at(0), &at(1), None);
                self.set_link(&at(0), &at(2));
                at(0)
            }
            37 => {
                self.set_click_event(&at(0), &at(1), Some(&at(2)));
                self.set_link(&at(0), &at(3));
                at(0)
            }
            38 => {
                self.set_click_event(&at(0), &at(2), None);
                self.set_link(&at(0), &at(1));
                at(0)
            }
            39 => {
                self.set_click_event(&at(0), &at(2), Some(&at(3)));
                self.set_link(&at(0), &at(1));
                at(0)
            }
            40 => {
                self.set_link(&at(0), &at(1));
                at(0)
            }
            // clickStatementDebug: unreachable from `statement`, kept as the
            // grammar writes it.
            41..=47 => rhs.join(" "),
            other => {
                let loc = locs.first().copied().unwrap_or_default();
                return Err(ParseError::action(
                    loc.first_line,
                    loc.first_column,
                    format!("gantt grammar has no production {other}"),
                ));
            }
        };
        Ok(value)
    }
}
