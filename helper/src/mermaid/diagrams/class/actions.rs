//! The semantic actions of `classDiagram.jison` (mermaid 12.2.0), one match
//! arm per production index of `classDiagram.tables.json`.
//!
//! The grammar calls the db (`yy.addClass`, `yy.addRelation`, ...) while it
//! reduces, so [`ClassActions`] owns the [`ClassDb`] and makes the same
//! calls in the same order. A production without an action keeps jison's
//! default `$$ = $1`.

use crate::mermaid::jison::{Actions, Loc, ParseError, Yy};

use super::db::{ClassDb, ClassRelation, LineType, Relation, RelationType};

/// A semantic value on jison's `vstack`.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Nil,
    /// A token's `yytext` or a string a production built.
    Text(String),
    /// `members`, `classList`, `stylesOpt`.
    List(Vec<String>),
    /// `classStatements`: `[[classNames], [noteIds]]`.
    Statements(Vec<String>, Vec<String>),
    RelationType(RelationType),
    LineType(LineType),
    Relation(Relation),
    RelationStatement(ClassRelation),
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
            _ => Vec::new(),
        }
    }

    fn statements(&self) -> (Vec<String>, Vec<String>) {
        match self {
            Val::Statements(c, n) => (c.clone(), n.clone()),
            _ => (Vec::new(), Vec::new()),
        }
    }

    fn relation_type(&self) -> RelationType {
        match self {
            Val::RelationType(t) => *t,
            _ => RelationType::None,
        }
    }

    fn line_type(&self) -> LineType {
        match self {
            Val::LineType(t) => *t,
            _ => LineType::Line,
        }
    }

    fn relation(&self) -> Relation {
        match self {
            Val::Relation(r) => *r,
            _ => Relation {
                type1: RelationType::None,
                type2: RelationType::None,
                line_type: LineType::Line,
            },
        }
    }
}

#[derive(Debug, Default)]
pub struct ClassActions {
    pub db: ClassDb,
}

impl ClassActions {
    pub fn new() -> ClassActions {
        ClassActions { db: ClassDb::new() }
    }
}

impl Yy for ClassActions {}

fn relation_statement(id1: &Val, id2: &Val, relation: &Val, t1: &str, t2: &str) -> Val {
    Val::RelationStatement(ClassRelation {
        id1: id1.text(),
        id2: id2.text(),
        relation_title1: t1.to_string(),
        relation_title2: t2.to_string(),
        title: None,
        relation: relation.relation(),
    })
}

impl Actions for ClassActions {
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
            // classLabel -> SQS STR SQE
            8 => at(1),
            // namespaceName / className -> alphaNumToken DOT ...
            11 | 14 => Val::Text(format!("{}.{}", text(0), text(2))),
            // namespaceName / className -> alphaNumToken namespaceName
            12 | 16 => Val::Text(format!("{}{}", text(0), text(1))),
            // className -> alphaNumToken GENERICTYPE | classLiteralName GENERICTYPE
            17 | 18 => Val::Text(format!("{}~{}~", text(0), text(1))),
            // statement -> relationStatement
            19 => {
                if let Val::RelationStatement(rel) = at(0) {
                    db.add_relation(rel);
                }
                Val::Nil
            }
            // statement -> relationStatement LABEL
            20 => {
                if let Val::RelationStatement(mut rel) = at(0) {
                    rel.title = Some(ClassDb::cleanup_label(&text(1)));
                    db.add_relation(rel);
                }
                Val::Nil
            }
            // acc_title / acc_descr
            31 => {
                db.acc_title = Some(text(1).trim().to_string());
                Val::Nil
            }
            32 => {
                db.acc_description = Some(text(1).trim().to_string());
                Val::Nil
            }
            33 => {
                db.acc_description = Some(text(0).trim().to_string());
                Val::Nil
            }
            // namespaceStatement -> namespaceIdentifier STRUCT_START classStatements STRUCT_STOP
            34 | 35 => {
                let body = if production == 34 { at(2) } else { at(3) };
                let (classes, notes) = body.statements();
                db.add_classes_to_namespace(&text(0), &classes, &notes);
                db.pop_namespace();
                Val::Nil
            }
            // namespaceIdentifier -> NAMESPACE namespaceName [classLabel]
            36 => Val::Text(db.add_namespace(&text(1), None)),
            37 => {
                let label = text(2);
                Val::Text(db.add_namespace(&text(1), Some(&label)))
            }
            // classStatements -> classStatement [NEWLINE]
            38 | 39 => Val::Statements(vec![text(0)], Vec::new()),
            // classStatements -> classStatement NEWLINE classStatements
            40 => {
                let (mut classes, notes) = at(2).statements();
                classes.insert(0, text(0));
                Val::Statements(classes, notes)
            }
            41 | 42 => Val::Statements(Vec::new(), vec![text(0)]),
            43 => {
                let (classes, mut notes) = at(2).statements();
                notes.insert(0, text(0));
                Val::Statements(classes, notes)
            }
            44 | 45 => Val::Statements(Vec::new(), Vec::new()),
            46 => at(2),
            // classStatement -> classIdentifier STYLE_SEPARATOR alphaNumToken
            48 => {
                db.set_css_class(&text(0), &text(2));
                at(0)
            }
            // classStatement -> classIdentifier STRUCT_START members STRUCT_STOP
            49 => {
                db.add_members(&text(0), &at(2).list());
                at(0)
            }
            51 => {
                db.set_css_class(&text(0), &text(2));
                db.add_members(&text(0), &at(4).list());
                at(0)
            }
            52 | 54 => {
                db.add_annotation(&text(0), &text(2));
                at(0)
            }
            53 => {
                db.add_annotation(&text(0), &text(2));
                db.add_members(&text(0), &at(5).list());
                at(0)
            }
            // classIdentifier -> CLASS className [classLabel]
            55 => {
                db.add_class(&text(1));
                at(1)
            }
            56 => {
                db.add_class(&text(1));
                db.set_class_label(&text(1), &text(2));
                at(1)
            }
            // annotationStatement -> ANNOTATION_START alphaNumToken ANNOTATION_END className
            60 => {
                db.add_annotation(&text(3), &text(1));
                at(0)
            }
            // members -> MEMBER
            61 => Val::List(vec![text(0)]),
            // members -> MEMBER members: `$2.push($1)`
            62 => {
                let mut list = at(1).list();
                list.push(text(0));
                Val::List(list)
            }
            // memberStatement -> className LABEL
            64 => {
                db.add_member(&text(0), &ClassDb::cleanup_label(&text(1)));
                at(0)
            }
            67 => relation_statement(&at(0), &at(2), &at(1), "none", "none"),
            68 => relation_statement(&at(0), &at(3), &at(2), &text(1), "none"),
            69 => relation_statement(&at(0), &at(3), &at(1), "none", &text(2)),
            70 => relation_statement(&at(0), &at(4), &at(2), &text(1), &text(3)),
            // noteStatement -> NOTE_FOR className noteText
            71 => {
                let class_name = text(1);
                Val::Text(db.add_note(&text(2), Some(&class_name)))
            }
            72 => Val::Text(db.add_note(&text(1), None)),
            // classDefStatement -> CLASSDEF classList stylesOpt
            73 => {
                db.define_class(&at(1).list(), &at(2).list());
                at(0)
            }
            74 => Val::List(vec![text(0)]),
            75 => {
                let mut list = at(0).list();
                list.push(text(2));
                Val::List(list)
            }
            76 => {
                db.set_direction("TB");
                Val::Nil
            }
            77 => {
                db.set_direction("BT");
                Val::Nil
            }
            78 => {
                db.set_direction("RL");
                Val::Nil
            }
            79 => {
                db.set_direction("LR");
                Val::Nil
            }
            80 => Val::Relation(Relation {
                type1: at(0).relation_type(),
                type2: at(2).relation_type(),
                line_type: at(1).line_type(),
            }),
            81 => Val::Relation(Relation {
                type1: RelationType::None,
                type2: at(1).relation_type(),
                line_type: at(0).line_type(),
            }),
            82 => Val::Relation(Relation {
                type1: at(0).relation_type(),
                type2: RelationType::None,
                line_type: at(1).line_type(),
            }),
            83 => Val::Relation(Relation {
                type1: RelationType::None,
                type2: RelationType::None,
                line_type: at(0).line_type(),
            }),
            84 => Val::RelationType(RelationType::Aggregation),
            85 => Val::RelationType(RelationType::Extension),
            86 => Val::RelationType(RelationType::Composition),
            87 => Val::RelationType(RelationType::Dependency),
            88 => Val::RelationType(RelationType::Lollipop),
            89 => Val::LineType(LineType::Line),
            90 => Val::LineType(LineType::DottedLine),
            // clickStatement: CALLBACK className STR [STR]
            91 | 92 | 97..=100 => {
                db.set_click_event(&text(1));
                let tooltip = match production {
                    92 | 98 => Some(text(3)),
                    100 => Some(text(4)),
                    _ => None,
                };
                db.set_tooltip(&text(1), tooltip.as_deref());
                at(0)
            }
            // LINK className STR [LINK_TARGET | STR [LINK_TARGET]]
            93 => {
                db.set_link(&text(1), &text(2), None);
                at(0)
            }
            94 => {
                db.set_link(&text(1), &text(2), Some(&text(3)));
                at(0)
            }
            95 => {
                db.set_link(&text(1), &text(2), None);
                db.set_tooltip(&text(1), Some(&text(3)));
                at(0)
            }
            96 => {
                db.set_link(&text(1), &text(2), Some(&text(4)));
                db.set_tooltip(&text(1), Some(&text(3)));
                at(0)
            }
            // CLICK className HREF STR [...]
            101 => {
                db.set_link(&text(1), &text(3), None);
                at(0)
            }
            102 => {
                db.set_link(&text(1), &text(3), Some(&text(4)));
                at(0)
            }
            103 => {
                db.set_link(&text(1), &text(3), None);
                db.set_tooltip(&text(1), Some(&text(4)));
                at(0)
            }
            104 => {
                db.set_link(&text(1), &text(3), Some(&text(5)));
                db.set_tooltip(&text(1), Some(&text(4)));
                at(0)
            }
            // styleStatement -> STYLE ALPHA stylesOpt
            105 => {
                db.set_css_style(&text(1), &at(2).list());
                at(0)
            }
            // cssClassStatement -> CSSCLASS STR ALPHA
            106 => {
                db.set_css_class(&text(1), &text(2));
                Val::Nil
            }
            107 => Val::List(vec![text(0)]),
            108 => {
                let mut list = at(0).list();
                list.push(text(2));
                Val::List(list)
            }
            // style -> style styleComponent
            110 => Val::Text(format!("{}{}", text(0), text(1))),
            // Everything else: jison's default `$$ = $1`.
            _ => at(0),
        };
        Ok(v)
    }
}
