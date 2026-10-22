//! Grammar actions of `xychart.jison`, by production index in
//! `helper/grammars/xychart.tables.json`.
//!
//! The lexer actions need nothing from `yy`, so [`Yy`] is the no-op
//! default. A production without an action gets jison's default `$$ = $1`.

use crate::mermaid::jison::lexer::js_trim;
use crate::mermaid::jison::{Actions, Loc, ParseError, Yy};

use super::db::{DataPoint, Db, TextValue};

/// The semantic values the grammar passes around.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Undefined,
    /// A token's `yytext` or an `alphaNum` concatenation.
    Str(String),
    /// A `text` nonterminal: `{ text, type }`.
    Text(TextValue),
    /// `commaSeparatedTexts`.
    Texts(Vec<TextValue>),
    /// `dataPoint`.
    Point(DataPoint),
    /// `dataPoints` and `plotData`.
    Points(Vec<DataPoint>),
}

pub struct XyActions<'a> {
    pub db: &'a mut Db,
}

impl Yy for XyActions<'_> {}

fn str_of(rhs: &[Val], i: usize) -> &str {
    match rhs.get(i) {
        Some(Val::Str(s)) => s,
        _ => "",
    }
}

fn text_of(rhs: &[Val], i: usize) -> TextValue {
    match rhs.get(i) {
        Some(Val::Text(t)) => t.clone(),
        _ => TextValue {
            text: String::new(),
        },
    }
}

fn texts_of(rhs: &[Val], i: usize) -> Vec<TextValue> {
    match rhs.get(i) {
        Some(Val::Texts(t)) => t.clone(),
        _ => Vec::new(),
    }
}

fn points_of(rhs: &[Val], i: usize) -> Vec<DataPoint> {
    match rhs.get(i) {
        Some(Val::Points(p)) => p.clone(),
        _ => Vec::new(),
    }
}

/// `Number(yytext)` for a `NUMBER_WITH_DECIMAL` token.
fn number_of(rhs: &[Val], i: usize) -> f64 {
    str_of(rhs, i).parse().unwrap_or(f64::NAN)
}

fn empty_text() -> TextValue {
    TextValue {
        text: String::new(),
    }
}

impl Actions for XyActions<'_> {
    type Value = Val;

    fn terminal(&mut self, yytext: &str) -> Val {
        Val::Str(yytext.to_string())
    }

    fn reduce(
        &mut self,
        production: usize,
        rhs: &[Val],
        _locs: &[Loc],
        _yytext: &str,
    ) -> Result<Val, ParseError> {
        let value = match production {
            // chartConfig: CHART_ORIENTATION
            5 => {
                self.db.set_orientation(str_of(rhs, 0));
                Val::Undefined
            }
            // statement: title text
            9 => {
                let text = text_of(rhs, 1);
                self.db.set_diagram_title(js_trim(&text.text));
                Val::Undefined
            }
            // statement: LINE plotData | LINE text plotData
            12 => {
                self.db.set_line_data(&empty_text(), &points_of(rhs, 1));
                Val::Undefined
            }
            13 => {
                self.db.set_line_data(&text_of(rhs, 1), &points_of(rhs, 2));
                Val::Undefined
            }
            // statement: BAR plotData | BAR text plotData
            14 => {
                self.db.set_bar_data(&empty_text(), &points_of(rhs, 1));
                Val::Undefined
            }
            15 => {
                self.db.set_bar_data(&text_of(rhs, 1), &points_of(rhs, 2));
                Val::Undefined
            }
            // acc_title and acc_descr: nothing renders them.
            16..=18 => Val::Undefined,
            // plotData: SQUARE_BRACES_START dataPoints SQUARE_BRACES_END
            19 => rhs.get(1).cloned().unwrap_or(Val::Undefined),
            // dataPoints: dataPoint COMMA dataPoints
            20 => {
                let mut points = Vec::with_capacity(1 + points_of(rhs, 2).len());
                if let Some(Val::Point(p)) = rhs.first() {
                    points.push(p.clone());
                }
                points.extend(points_of(rhs, 2));
                Val::Points(points)
            }
            // dataPoints: dataPoint
            21 => match rhs.first() {
                Some(Val::Point(p)) => Val::Points(vec![p.clone()]),
                _ => Val::Points(Vec::new()),
            },
            // dataPoint: NUMBER_WITH_DECIMAL STR | NUMBER_WITH_DECIMAL
            22 => Val::Point(DataPoint {
                value: number_of(rhs, 0),
                label: str_of(rhs, 1).to_string(),
            }),
            23 => Val::Point(DataPoint {
                value: number_of(rhs, 0),
                label: String::new(),
            }),
            // parseXAxis: text | text xAxisData | xAxisData
            24 | 25 => {
                self.db.set_x_axis_title(&text_of(rhs, 0));
                Val::Undefined
            }
            26 => {
                self.db.set_x_axis_title(&empty_text());
                Val::Undefined
            }
            // xAxisData: bandData | NUMBER ARROW_DELIMITER NUMBER
            27 => {
                self.db.set_x_axis_band(&texts_of(rhs, 0));
                Val::Undefined
            }
            28 => {
                self.db
                    .set_x_axis_range_data(number_of(rhs, 0), number_of(rhs, 2));
                Val::Undefined
            }
            // bandData: SQUARE_BRACES_START commaSeparatedTexts SQUARE_BRACES_END
            29 => rhs.get(1).cloned().unwrap_or(Val::Undefined),
            // commaSeparatedTexts: text COMMA commaSeparatedTexts | text
            30 => {
                let mut texts = vec![text_of(rhs, 0)];
                texts.extend(texts_of(rhs, 2));
                Val::Texts(texts)
            }
            31 => Val::Texts(vec![text_of(rhs, 0)]),
            // parseYAxis: text | text yAxisData | yAxisData
            32 | 33 => {
                self.db.set_y_axis_title(&text_of(rhs, 0));
                Val::Undefined
            }
            34 => {
                self.db.set_y_axis_title(&empty_text());
                Val::Undefined
            }
            // yAxisData: NUMBER ARROW_DELIMITER NUMBER
            35 => {
                self.db
                    .set_y_axis_range_data(number_of(rhs, 0), number_of(rhs, 2));
                Val::Undefined
            }
            // text: alphaNum | STR | MD_STR
            39..=41 => Val::Text(TextValue {
                text: str_of(rhs, 0).to_string(),
            }),
            // alphaNum: alphaNum alphaNumToken
            43 => Val::Str(format!("{}{}", str_of(rhs, 0), str_of(rhs, 1))),
            // Every other production, including alphaNumToken, is `$$ = $1`.
            _ => rhs.first().cloned().unwrap_or(Val::Undefined),
        };
        Ok(value)
    }
}
