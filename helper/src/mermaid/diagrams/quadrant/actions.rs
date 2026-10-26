//! The semantic actions of `parser/quadrant.jison` (mermaid 12.2.0) over
//! the vendored LALR tables, dispatched by production name and shape so a
//! regenerated table with the same grammar needs no edits here.

use super::db::{Db, TextObj};
use crate::mermaid::jison::{self, Actions, Grammar, Loc, ParseError, Yy};

/// A semantic value on the parser stack: a token text, a `text`
/// production's object, or the `stylesOpt` array.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Undefined,
    Str(String),
    Text(TextObj),
    Styles(Vec<String>),
}

impl Value {
    /// `'' + value` for the string concatenations in the grammar.
    fn as_str(&self) -> &str {
        match self {
            Value::Str(s) => s,
            Value::Text(t) => &t.text,
            Value::Undefined => "undefined",
            Value::Styles(_) => "",
        }
    }

    fn styles(&self) -> Vec<String> {
        match self {
            Value::Styles(s) => s.clone(),
            _ => Vec::new(),
        }
    }

    fn text(&self) -> TextObj {
        match self {
            Value::Text(t) => t.clone(),
            other => TextObj {
                text: other.as_str().to_string(),
                markdown: false,
            },
        }
    }
}

pub struct QuadrantActions<'a> {
    grammar: &'static Grammar,
    db: &'a mut Db,
}

impl<'a> QuadrantActions<'a> {
    pub fn new(db: &'a mut Db) -> Result<QuadrantActions<'a>, jison::LoadError> {
        Ok(QuadrantActions {
            grammar: jison::grammar("quadrant")?,
            db,
        })
    }

    pub fn grammar(&self) -> &'static Grammar {
        self.grammar
    }

    /// `(1)|(0(.\d+)?)` matched the token, so this is a plain decimal
    /// unless the unescaped `.` let a stray character through; JS gets `NaN`
    /// then and draws nothing sensible, so 0 is the kindest reading.
    fn coordinate(v: &Value) -> f64 {
        v.as_str().parse::<f64>().unwrap_or(0.0)
    }

    fn throw(locs: &[Loc], message: String) -> ParseError {
        let loc = locs.first().copied().unwrap_or_default();
        ParseError::action(loc.first_line, loc.first_column, message)
    }
}

impl Yy for QuadrantActions<'_> {}

impl Actions for QuadrantActions<'_> {
    type Value = Value;

    fn terminal(&mut self, yytext: &str) -> Value {
        Value::Str(yytext.to_string())
    }

    fn reduce(
        &mut self,
        production: usize,
        rhs: &[Value],
        locs: &[Loc],
        _yytext: &str,
    ) -> Result<Value, ParseError> {
        let Some(p) = self.grammar.tables.productions.get(production) else {
            return Err(Self::throw(
                locs,
                format!("quadrant: unknown production {production}"),
            ));
        };
        // jison's default action: `$$ = $1`.
        let first = rhs.first().cloned().unwrap_or(Value::Undefined);
        let names: Vec<&str> = p.rhs.iter().map(String::as_str).collect();
        let value = match (p.lhs.as_str(), names.as_slice()) {
            ("idString" | "alphaNum" | "style", [_, _]) => {
                Value::Str(format!("{}{}", rhs[0].as_str(), rhs[1].as_str()))
            }
            ("stylesOpt", [_]) => Value::Styles(vec![rhs[0].as_str().trim().to_string()]),
            ("stylesOpt", [_, "COMMA", _]) => {
                let mut styles = rhs[0].styles();
                styles.push(rhs[2].as_str().trim().to_string());
                Value::Styles(styles)
            }
            ("classDefStatement", ["CLASSDEF", "SPACE", "idString", "SPACE", "stylesOpt"]) => {
                self.db
                    .add_class(rhs[2].as_str(), &rhs[4].styles())
                    .map_err(|e| Self::throw(locs, e.0))?;
                first
            }
            ("statement", ["classDefStatement"]) => Value::Undefined,
            ("statement", ["title", "title_value"]) => {
                let title = rhs[1].as_str().trim().to_string();
                self.db.set_diagram_title(&title);
                Value::Str(title)
            }
            ("statement", ["acc_title", "acc_title_value"]) => {
                let v = rhs[1].as_str().trim().to_string();
                self.db.acc_title = v.clone();
                Value::Str(v)
            }
            ("statement", ["acc_descr", "acc_descr_value"])
            | ("statement", ["acc_descr_multiline_value"]) => {
                let v = rhs.last().map(|v| v.as_str().trim()).unwrap_or_default();
                self.db.acc_description = v.to_string();
                Value::Str(v.to_string())
            }
            ("points", ["text", "point_start", "point_x", "point_y"]) => {
                self.add_point(&rhs[0], "", &rhs[2], &rhs[3], &[], locs)?
            }
            ("points", ["text", "class_name", "point_start", "point_x", "point_y"]) => {
                let class = rhs[1].as_str().to_string();
                self.add_point(&rhs[0], &class, &rhs[3], &rhs[4], &[], locs)?
            }
            ("points", ["text", "point_start", "point_x", "point_y", "stylesOpt"]) => {
                let styles = rhs[4].styles();
                self.add_point(&rhs[0], "", &rhs[2], &rhs[3], &styles, locs)?
            }
            (
                "points",
                [
                    "text",
                    "class_name",
                    "point_start",
                    "point_x",
                    "point_y",
                    "stylesOpt",
                ],
            ) => {
                let class = rhs[1].as_str().to_string();
                let styles = rhs[5].styles();
                self.add_point(&rhs[0], &class, &rhs[3], &rhs[4], &styles, locs)?
            }
            ("axisDetails", ["X-AXIS", "text", "AXIS-TEXT-DELIMITER", "text"]) => {
                self.db.set_x_axis_left_text(&rhs[1].text());
                self.db.set_x_axis_right_text(&rhs[3].text());
                first
            }
            ("axisDetails", ["X-AXIS", "text", "AXIS-TEXT-DELIMITER"]) => {
                let mut t = rhs[1].text();
                t.text.push_str(" ⟶ ");
                self.db.set_x_axis_left_text(&t);
                first
            }
            ("axisDetails", ["X-AXIS", "text"]) => {
                self.db.set_x_axis_left_text(&rhs[1].text());
                first
            }
            ("axisDetails", ["Y-AXIS", "text", "AXIS-TEXT-DELIMITER", "text"]) => {
                self.db.set_y_axis_bottom_text(&rhs[1].text());
                self.db.set_y_axis_top_text(&rhs[3].text());
                first
            }
            ("axisDetails", ["Y-AXIS", "text", "AXIS-TEXT-DELIMITER"]) => {
                let mut t = rhs[1].text();
                t.text.push_str(" ⟶ ");
                self.db.set_y_axis_bottom_text(&t);
                first
            }
            ("axisDetails", ["Y-AXIS", "text"]) => {
                self.db.set_y_axis_bottom_text(&rhs[1].text());
                first
            }
            ("quadrantDetails", ["QUADRANT_1", "text"]) => {
                self.db.set_quadrant1_text(&rhs[1].text());
                first
            }
            ("quadrantDetails", ["QUADRANT_2", "text"]) => {
                self.db.set_quadrant2_text(&rhs[1].text());
                first
            }
            ("quadrantDetails", ["QUADRANT_3", "text"]) => {
                self.db.set_quadrant3_text(&rhs[1].text());
                first
            }
            ("quadrantDetails", ["QUADRANT_4", "text"]) => {
                self.db.set_quadrant4_text(&rhs[1].text());
                first
            }
            ("text", ["alphaNumToken"]) | ("text", ["STR"]) => Value::Text(TextObj {
                text: rhs[0].as_str().to_string(),
                markdown: false,
            }),
            ("text", ["MD_STR"]) => Value::Text(TextObj {
                text: rhs[0].as_str().to_string(),
                markdown: true,
            }),
            ("text", ["text", "textNoTagsToken"]) => {
                let mut t = rhs[0].text();
                t.text.push_str(rhs[1].as_str());
                Value::Text(t)
            }
            _ => first,
        };
        Ok(value)
    }
}

impl QuadrantActions<'_> {
    fn add_point(
        &mut self,
        text: &Value,
        class_name: &str,
        x: &Value,
        y: &Value,
        styles: &[String],
        locs: &[Loc],
    ) -> Result<Value, ParseError> {
        self.db
            .add_point(
                &text.text(),
                class_name,
                Self::coordinate(x),
                Self::coordinate(y),
                styles,
            )
            .map_err(|e| Self::throw(locs, e.0))?;
        Ok(Value::Undefined)
    }
}

/// Parses `text`, the text `Diagram.fromText` hands the parser (its
/// newline included), into `db` as `parser.parse(text)` does.
pub fn parse_into(db: &mut Db, text: &str) -> Result<(), ParseError> {
    let mut actions =
        QuadrantActions::new(db).map_err(|e| ParseError::action(1, 0, e.to_string()))?;
    let grammar = actions.grammar();
    jison::parse(grammar, text, &mut actions).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::config::QuadrantChartConfig;
    use crate::mermaid::theme::ThemeVariables;

    fn parse(code: &str) -> Result<Db, ParseError> {
        let mut db = Db::new();
        parse_into(&mut db, &format!("{code}\n"))?;
        Ok(db)
    }

    #[test]
    fn every_grammar_production_is_handled_or_defaulted() {
        let grammar = jison::grammar("quadrant").unwrap();
        let actioned: Vec<&str> = grammar
            .tables
            .productions
            .iter()
            .filter(|p| p.action.as_deref().is_some_and(|a| a.contains("yy.")))
            .map(|p| p.lhs.as_str())
            .collect();
        for lhs in actioned {
            assert!(
                matches!(
                    lhs,
                    "classDefStatement"
                        | "statement"
                        | "points"
                        | "axisDetails"
                        | "quadrantDetails"
                ),
                "production {lhs} calls yy but has no action here"
            );
        }
    }

    #[test]
    fn the_docs_example_parses() {
        let db = parse(
            "quadrantChart\n    title Reach and engagement of campaigns\n    x-axis Low Reach --> High Reach\n    y-axis Low Engagement --> High Engagement\n    quadrant-1 We should expand\n    quadrant-2 Need to promote\n    quadrant-3 Re-evaluate\n    quadrant-4 May be improved\n    Campaign A: [0.3, 0.6]\n    Campaign B: [0.45, 0.23]",
        )
        .unwrap();
        assert_eq!(db.diagram_title(), "Reach and engagement of campaigns");
        let build = db.quadrant_data(&QuadrantChartConfig::default(), &ThemeVariables::default());
        assert_eq!(build.quadrants[0].text.text, "We should expand");
        assert_eq!(build.quadrants[2].text.text, "Re-evaluate");
        assert_eq!(build.axis_labels.len(), 4);
        assert_eq!(build.axis_labels[0].text, "Low Reach");
        assert_eq!(build.axis_labels[3].text, "High Engagement");
        assert_eq!(build.points.len(), 2);
        assert_eq!(build.points[0].text.text, "Campaign B");
        assert_eq!(build.points[1].text.text, "Campaign A");
    }

    #[test]
    fn point_styles_classes_and_half_axes() {
        let db = parse(
            "quadrantChart\n  x-axis Urgent\n  y-axis Slow -->\n  Point A:::class1: [0.9, 0.0] radius: 12\n  Point B: [0.8, 0.1] color: #ff3300, radius: 10\n  Point C:::class2: [0.5, 0.4]\n  classDef class1 color: #109060\n  classDef class2 color: #908342, radius : 10, stroke-color: #310085, stroke-width: 10px\n",
        )
        .unwrap();
        let build = db.quadrant_data(&QuadrantChartConfig::default(), &ThemeVariables::default());
        assert_eq!(build.axis_labels[0].text, "Urgent");
        assert_eq!(build.axis_labels[1].text, "Slow ⟶");
        let c = &build.points[0];
        assert_eq!(c.text.text, "Point C");
        assert_eq!(c.fill.as_deref(), Some("#908342"));
        assert_eq!(c.radius, 10.0);
        assert_eq!(c.stroke_color.as_deref(), Some("#310085"));
        assert_eq!(c.stroke_width, "10px");
        let b = &build.points[1];
        assert_eq!(b.fill.as_deref(), Some("#ff3300"));
        assert_eq!(b.radius, 10.0);
        let a = &build.points[2];
        assert_eq!(a.fill.as_deref(), Some("#109060"));
        assert_eq!(a.radius, 12.0);
    }

    #[test]
    fn a_bad_style_is_an_action_error_with_mermaids_text() {
        let err = parse("quadrantChart\n  Point A: [0.9, 0.0] radius: 12px\n").unwrap_err();
        assert_eq!(err.kind, jison::ErrorKind::Action);
        assert_eq!(
            err.message,
            "value for radius 12px is invalid, please use a valid number"
        );
    }

    #[test]
    fn a_syntax_error_is_a_parse_error() {
        let err = parse("quadrantChart\n  x-axis\n").unwrap_err();
        assert_eq!(err.kind, jison::ErrorKind::Parse);
        assert!(
            err.message.starts_with("Parse error on line 2:"),
            "{}",
            err.message
        );
    }
}
