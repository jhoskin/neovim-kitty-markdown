//! The semantic actions of `requirementDiagram.jison` (mermaid 12.2.0),
//! one match arm per production index of `requirementDiagram.tables.json`.
//!
//! The body productions are right-recursive, so jison reduces the last
//! `id:`/`text:` line first and the `setNewReq*` calls run bottom-up; the
//! port reduces in the same order, so a repeated field keeps its first
//! value as in mermaid. A production without an action keeps jison's
//! default `$$ = $1`.

use crate::mermaid::jison::{Actions, Loc, ParseError, Yy};

use super::db::{
    DESIGN_CONSTRAINT, FUNCTIONAL_REQUIREMENT, INTERFACE_REQUIREMENT, PERFORMANCE_REQUIREMENT,
    PHYSICAL_REQUIREMENT, REQUIREMENT, RequirementDb,
};

/// A semantic value on jison's `vstack`.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Nil,
    /// A token's `yytext` or a string a production built.
    Text(String),
    /// `idList`, `stylesOpt`.
    List(Vec<String>),
}

impl Val {
    fn text(&self) -> String {
        match self {
            Val::Text(s) => s.clone(),
            _ => String::new(),
        }
    }

    fn list(&self) -> Vec<String> {
        match self {
            Val::List(l) => l.clone(),
            Val::Text(s) => vec![s.clone()],
            Val::Nil => Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct RequirementActions {
    pub db: RequirementDb,
}

impl RequirementActions {
    pub fn new() -> RequirementActions {
        RequirementActions {
            db: RequirementDb::new(),
        }
    }
}

impl Yy for RequirementActions {}

fn text_val(s: &str) -> Val {
    Val::Text(s.to_string())
}

impl Actions for RequirementActions {
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
        let at = |i: usize| rhs.get(i).cloned().unwrap_or(Val::Nil);
        let text = |i: usize| rhs.get(i).map(Val::text).unwrap_or_default();
        let db = &mut self.db;
        let v = match production {
            // directive -> acc_title acc_title_value | acc_descr acc_descr_value
            4 => {
                let t = text(1).trim().to_string();
                db.acc_title = Some(t.clone());
                Val::Text(t)
            }
            5 => {
                let t = text(1).trim().to_string();
                db.acc_description = Some(t.clone());
                Val::Text(t)
            }
            // directive -> acc_descr_multiline_value
            6 => {
                let t = text(0).trim().to_string();
                db.acc_description = Some(t.clone());
                Val::Text(t)
            }
            // diagram -> /* empty */
            7 => Val::List(Vec::new()),
            // direction -> direction_tb | _bt | _rl | _lr
            17 => {
                db.set_direction("TB");
                Val::Nil
            }
            18 => {
                db.set_direction("BT");
                Val::Nil
            }
            19 => {
                db.set_direction("RL");
                Val::Nil
            }
            20 => {
                db.set_direction("LR");
                Val::Nil
            }
            // requirementDef -> requirementType requirementName STRUCT_START NEWLINE requirementBody
            21 => {
                db.add_requirement(&text(1), &text(0));
                at(0)
            }
            // requirementDef -> requirementType requirementName STYLE_SEPARATOR idList ...
            22 => {
                db.add_requirement(&text(1), &text(0));
                db.set_class(&[text(1)], &at(3).list());
                at(0)
            }
            // requirementBody -> ID|TEXT|RISK|VERIFYMTHD COLONSEP value NEWLINE requirementBody
            23 => {
                db.set_new_req_id(&text(2));
                at(0)
            }
            24 => {
                db.set_new_req_text(&text(2));
                at(0)
            }
            25 => {
                db.set_new_req_risk(&text(2));
                at(0)
            }
            26 => {
                db.set_new_req_verify_method(&text(2));
                at(0)
            }
            // requirementType -> REQUIREMENT | ...
            29 => text_val(REQUIREMENT),
            30 => text_val(FUNCTIONAL_REQUIREMENT),
            31 => text_val(INTERFACE_REQUIREMENT),
            32 => text_val(PERFORMANCE_REQUIREMENT),
            33 => text_val(PHYSICAL_REQUIREMENT),
            34 => text_val(DESIGN_CONSTRAINT),
            // riskLevel
            35 => text_val("Low"),
            36 => text_val("Medium"),
            37 => text_val("High"),
            // verifyType
            38 => text_val("Analysis"),
            39 => text_val("Demonstration"),
            40 => text_val("Inspection"),
            41 => text_val("Test"),
            // elementDef -> ELEMENT elementName STRUCT_START NEWLINE elementBody
            42 => {
                db.add_element(&text(1));
                at(0)
            }
            43 => {
                db.add_element(&text(1));
                db.set_class(&[text(1)], &at(3).list());
                at(0)
            }
            // elementBody -> TYPE|DOCREF COLONSEP value NEWLINE elementBody
            44 => {
                db.set_new_element_type(&text(2));
                at(0)
            }
            45 => {
                db.set_new_element_doc_ref(&text(2));
                at(0)
            }
            // relationshipDef -> id END_ARROW_L relationship LINE id
            48 => {
                db.add_relationship(&text(2), &text(4), &text(0));
                at(0)
            }
            // relationshipDef -> id LINE relationship END_ARROW_R id
            49 => {
                db.add_relationship(&text(2), &text(0), &text(4));
                at(0)
            }
            // relationship -> CONTAINS | ...
            50 => text_val("contains"),
            51 => text_val("copies"),
            52 => text_val("derives"),
            53 => text_val("satisfies"),
            54 => text_val("verifies"),
            55 => text_val("refines"),
            56 => text_val("traces"),
            // classDefStatement -> CLASSDEF idList stylesOpt
            57 => {
                db.define_class(&at(1).list(), &at(2).list());
                at(0)
            }
            // classStatement -> CLASS idList idList
            58 => {
                db.set_class(&at(1).list(), &at(2).list());
                at(0)
            }
            // classStatement -> id STYLE_SEPARATOR idList
            59 => {
                db.set_class(&[text(0)], &at(2).list());
                at(0)
            }
            // idList -> ALPHA | id
            60 | 62 => Val::List(vec![text(0)]),
            // idList -> idList COMMA ALPHA | idList COMMA id
            61 | 63 => {
                let mut list = at(0).list();
                list.push(text(2));
                Val::List(list)
            }
            // styleStatement -> STYLE idList stylesOpt
            64 => {
                db.set_css_style(&at(1).list(), &at(2).list());
                at(0)
            }
            // stylesOpt -> style
            65 => Val::List(vec![text(0)]),
            // stylesOpt -> stylesOpt COMMA style
            66 => {
                let mut list = at(0).list();
                list.push(text(2));
                Val::List(list)
            }
            // style -> style styleComponent
            68 => Val::Text(format!("{}{}", text(0), text(1))),
            // Everything else: jison's default `$$ = $1`.
            _ => at(0),
        };
        Ok(v)
    }
}
