//! The semantic actions of `sequenceDiagram.jison` (mermaid 12.2.0), one
//! match arm per production index of `sequenceDiagram.tables.json`.
//!
//! The grammar builds plain objects (`{type: 'addMessage', ...}`) and
//! arrays of them, and `start -> SD document` hands the array to
//! `yy.apply`. Here every production yields a [`Val`]; statements are
//! flattened into `Vec<Stmt>` as they are built, which is what `apply`'s
//! recursion over arrays produces anyway.

use crate::mermaid::jison::{Actions, Lit, Loc, ParseError, Yy};

use super::db::{ActorRef, LineType, ParsedMessage, Placement, SequenceDb, Stmt, StmtKind};

/// A semantic value on jison's `vstack`.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Nil,
    /// A token's `yytext`, or a string a production returned.
    Text(String),
    Stmts(Vec<Stmt>),
    /// `actor` and `actor_with_config`: `{type: 'addParticipant', actor}`.
    Actor(ActorRef),
    /// `participant_statement`: the actor object with `draw` and `type`
    /// set; `destroy` is the `destroyParticipant` variant.
    Participant(ActorRef, bool),
    /// `actor_pair`: one actor, or two for `A,B`.
    ActorPair(Vec<ActorRef>),
    Placement(Placement),
    LineType(LineType),
    Msg(ParsedMessage),
}

impl Val {
    fn text(&self) -> &str {
        match self {
            Val::Text(s) => s,
            _ => "",
        }
    }

    fn stmts(self) -> Vec<Stmt> {
        match self {
            Val::Stmts(s) => s,
            _ => Vec::new(),
        }
    }

    fn actor(&self) -> ActorRef {
        match self {
            Val::Actor(a) | Val::Participant(a, _) => a.clone(),
            _ => ActorRef::default(),
        }
    }

    fn line_type(&self) -> LineType {
        match self {
            Val::LineType(t) => *t,
            _ => LineType::Solid,
        }
    }

    fn msg(&self) -> ParsedMessage {
        match self {
            Val::Msg(m) => m.clone(),
            Val::Text(t) => SequenceDb::parse_message(t),
            _ => ParsedMessage::default(),
        }
    }
}

#[derive(Debug, Default)]
pub struct SequenceActions;

impl Yy for SequenceActions {
    fn yy_call(&mut self, _name: &str, _args: &[Lit]) -> Lit {
        Lit::Undefined
    }
}

fn line_of(locs: &[Loc]) -> usize {
    locs.first().map_or(1, |l| l.first_line.max(1))
}

fn stmt(line: usize, kind: StmtKind) -> Stmt {
    Stmt { line, kind }
}

/// `{type: 'addParticipant', actor}` applied as a statement: the implicit
/// declaration every actor mention makes.
fn mention(line: usize, actor: &ActorRef) -> Stmt {
    stmt(
        line,
        StmtKind::AddParticipant(ActorRef {
            actor: actor.actor.clone(),
            ..ActorRef::default()
        }),
    )
}

/// `JavaScript Number(text)` for a `NUM` token.
fn number(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok()
}

/// `$1.substring(n)`: the title keyword and its separator dropped.
fn skip_chars(text: &str, n: usize) -> String {
    text.chars().skip(n).collect()
}

impl SequenceActions {
    fn block(&self, locs: &[Loc], start: LineType, end: LineType, label: &Val, body: Val) -> Val {
        let line = line_of(locs);
        let mut out = vec![stmt(
            line,
            StmtKind::BlockStart {
                signal_type: start,
                text: label.msg(),
            },
        )];
        out.extend(body.stmts());
        out.push(stmt(
            locs.last().map_or(line, |l| l.first_line.max(1)),
            StmtKind::BlockEnd(end),
        ));
        Val::Stmts(out)
    }

    /// `document and restOfLine par_sections` and its `else`/`option`
    /// twins: `$1.concat([{type, text}, $4])`.
    fn section(&self, locs: &[Loc], rhs: &[Val], kind: LineType) -> Val {
        let mut out = rhs[0].clone().stmts();
        out.push(stmt(
            locs.get(1).map_or(1, |l| l.first_line.max(1)),
            StmtKind::BlockStart {
                signal_type: kind,
                text: rhs[2].msg(),
            },
        ));
        out.extend(rhs[3].clone().stmts());
        Val::Stmts(out)
    }

    fn participant(&self, rhs: &[Val], draw: &str, description: Option<&Val>) -> Val {
        let mut actor = rhs[1].actor();
        actor.draw = Some(draw.to_string());
        if let Some(d) = description {
            actor.description = Some(d.msg());
        }
        Val::Participant(actor, false)
    }

    fn participant_stmt(&self, line: usize, value: &Val, create: bool) -> Vec<Stmt> {
        match value {
            Val::Participant(actor, destroy) => {
                let kind = if create {
                    StmtKind::CreateParticipant(actor.clone())
                } else if *destroy {
                    StmtKind::DestroyParticipant(actor.clone())
                } else {
                    StmtKind::AddParticipant(actor.clone())
                };
                vec![stmt(line, kind)]
            }
            other => other.clone().stmts(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn signal(
        &self,
        locs: &[Loc],
        from: &Val,
        to: &Val,
        signal_type: &Val,
        msg: &Val,
        activate: bool,
        central_connection: Option<LineType>,
        trailing: &[StmtKind],
    ) -> Val {
        let line = line_of(locs);
        let from = from.actor();
        let to = to.actor();
        let mut out = vec![
            mention(line, &from),
            mention(line, &to),
            stmt(
                line,
                StmtKind::AddMessage {
                    from: from.actor.clone(),
                    to: to.actor.clone(),
                    signal_type: signal_type.line_type(),
                    msg: msg.msg(),
                    activate,
                    central_connection,
                },
            ),
        ];
        out.extend(trailing.iter().cloned().map(|k| stmt(line, k)));
        Val::Stmts(out)
    }

    /// `links actor text2` and its siblings: `[$2, {type, actor, text}]`.
    fn actor_text(
        &self,
        locs: &[Loc],
        rhs: &[Val],
        make: fn(String, ParsedMessage) -> StmtKind,
    ) -> Val {
        let line = line_of(locs);
        let actor = rhs[1].actor();
        Val::Stmts(vec![
            mention(line, &actor),
            stmt(line, make(actor.actor, rhs[2].msg())),
        ])
    }
}

impl Actions for SequenceActions {
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
        let line = line_of(locs);
        let v = match production {
            // start -> SPACE start | NEWLINE start | SD document
            1..=3 => rhs[1].clone(),
            // document / box_section -> (empty)
            4 | 10 => Val::Stmts(Vec::new()),
            // document -> document line; box_section -> box_section box_line
            5 | 11 => {
                let mut list = rhs[0].clone().stmts();
                list.extend(rhs[1].clone().stmts());
                Val::Stmts(list)
            }
            // line -> SPACE statement
            6 => rhs[1].clone(),
            // line -> statement
            7 => rhs[0].clone(),
            // line -> NEWLINE | INVALID; box_line -> NEWLINE
            8 | 9 | 14 => Val::Stmts(Vec::new()),
            // box_line -> SPACE participant_statement
            12 => Val::Stmts(self.participant_stmt(line, &rhs[1], false)),
            // box_line -> participant_statement; statement -> participant_statement
            13 | 15 => Val::Stmts(self.participant_stmt(line, &rhs[0], false)),
            // statement -> create participant_statement
            16 => Val::Stmts(self.participant_stmt(line, &rhs[1], true)),
            // statement -> box restOfLine box_section end
            17 => {
                let mut out = vec![stmt(
                    line,
                    StmtKind::BoxStart(SequenceDb::parse_box_data(rhs[1].text())),
                )];
                out.extend(rhs[2].clone().stmts());
                out.push(stmt(line, StmtKind::BoxEnd));
                Val::Stmts(out)
            }
            // statement -> signal NEWLINE
            18 => rhs[0].clone(),
            // autonumber NUM NUM NEWLINE
            19 => Val::Stmts(vec![stmt(
                line,
                StmtKind::SequenceIndex {
                    start: number(rhs[1].text()),
                    step: number(rhs[2].text()),
                    visible: true,
                },
            )]),
            // autonumber NUM NEWLINE
            20 => Val::Stmts(vec![stmt(
                line,
                StmtKind::SequenceIndex {
                    start: number(rhs[1].text()),
                    step: Some(1.0),
                    visible: true,
                },
            )]),
            // autonumber off NEWLINE
            21 => Val::Stmts(vec![stmt(
                line,
                StmtKind::SequenceIndex {
                    start: None,
                    step: None,
                    visible: false,
                },
            )]),
            // autonumber NEWLINE
            22 => Val::Stmts(vec![stmt(
                line,
                StmtKind::SequenceIndex {
                    start: None,
                    step: None,
                    visible: true,
                },
            )]),
            // activate actor NEWLINE
            23 => Val::Stmts(vec![stmt(
                line,
                StmtKind::ActiveStart(rhs[1].actor().actor),
            )]),
            // deactivate actor NEWLINE
            24 => Val::Stmts(vec![stmt(line, StmtKind::ActiveEnd(rhs[1].actor().actor))]),
            // note/links/link/properties/details statements + NEWLINE
            25..=29 => rhs[0].clone(),
            // title
            30 => Val::Stmts(vec![stmt(
                line,
                StmtKind::Title(skip_chars(rhs[0].text(), 6)),
            )]),
            // legacy_title
            31 => Val::Stmts(vec![stmt(
                line,
                StmtKind::Title(skip_chars(rhs[0].text(), 7)),
            )]),
            // acc_title acc_title_value
            32 => Val::Stmts(vec![stmt(
                line,
                StmtKind::AccTitle(rhs[1].text().trim().to_string()),
            )]),
            // acc_descr acc_descr_value
            33 => Val::Stmts(vec![stmt(
                line,
                StmtKind::AccDescription(rhs[1].text().trim().to_string()),
            )]),
            // acc_descr_multiline_value
            34 => Val::Stmts(vec![stmt(
                line,
                StmtKind::AccDescription(rhs[0].text().trim().to_string()),
            )]),
            35 => self.block(
                locs,
                LineType::LoopStart,
                LineType::LoopEnd,
                &rhs[1],
                rhs[2].clone(),
            ),
            36 => self.block(
                locs,
                LineType::RectStart,
                LineType::RectEnd,
                &rhs[1],
                rhs[2].clone(),
            ),
            37 => self.block(
                locs,
                LineType::OptStart,
                LineType::OptEnd,
                &rhs[1],
                rhs[2].clone(),
            ),
            38 => self.block(
                locs,
                LineType::AltStart,
                LineType::AltEnd,
                &rhs[1],
                rhs[2].clone(),
            ),
            39 => self.block(
                locs,
                LineType::ParStart,
                LineType::ParEnd,
                &rhs[1],
                rhs[2].clone(),
            ),
            40 => self.block(
                locs,
                LineType::ParOverStart,
                LineType::ParEnd,
                &rhs[1],
                rhs[2].clone(),
            ),
            41 => self.block(
                locs,
                LineType::CriticalStart,
                LineType::CriticalEnd,
                &rhs[1],
                rhs[2].clone(),
            ),
            42 => self.block(
                locs,
                LineType::BreakStart,
                LineType::BreakEnd,
                &rhs[1],
                rhs[2].clone(),
            ),
            // option_sections / par_sections / else_sections -> document
            43 | 45 | 47 => rhs[0].clone(),
            44 => self.section(locs, rhs, LineType::CriticalOption),
            46 => self.section(locs, rhs, LineType::ParAnd),
            48 => self.section(locs, rhs, LineType::AltElse),
            // participant actor AS restOfLine NEWLINE
            49 | 54 => self.participant(rhs, "participant", Some(&rhs[3])),
            // participant actor NEWLINE
            50 | 55 => self.participant(rhs, "participant", None),
            // participant_actor actor AS restOfLine NEWLINE
            51 | 56 => self.participant(rhs, "actor", Some(&rhs[3])),
            // participant_actor actor NEWLINE
            52 | 57 => self.participant(rhs, "actor", None),
            // destroy actor NEWLINE
            53 => Val::Participant(rhs[1].actor(), true),
            // note placement actor text2
            58 => {
                let actor = rhs[2].actor();
                Val::Stmts(vec![
                    mention(line, &actor),
                    stmt(
                        line,
                        StmtKind::AddNote {
                            actors: vec![actor.actor],
                            placement: match rhs[1] {
                                Val::Placement(p) => p,
                                _ => Placement::LeftOf,
                            },
                            text: rhs[3].msg(),
                        },
                    ),
                ])
            }
            // note over actor_pair text2
            59 => {
                let pair = match &rhs[2] {
                    Val::ActorPair(p) => p.clone(),
                    other => vec![other.actor()],
                };
                let mut out: Vec<Stmt> = pair.iter().map(|a| mention(line, a)).collect();
                let names: Vec<String> = pair.iter().map(|a| a.actor.clone()).collect();
                let actors = if names.len() >= 2 {
                    names[..2].to_vec()
                } else {
                    vec![names[0].clone(), names[0].clone()]
                };
                out.push(stmt(
                    line,
                    StmtKind::AddNote {
                        actors,
                        placement: Placement::Over,
                        text: rhs[3].msg(),
                    },
                ));
                Val::Stmts(out)
            }
            60 => self.actor_text(locs, rhs, |actor, text| StmtKind::AddLinks { actor, text }),
            61 => self.actor_text(locs, rhs, |actor, text| StmtKind::AddALink { actor, text }),
            62 => self.actor_text(locs, rhs, |actor, text| StmtKind::AddProperties {
                actor,
                text,
            }),
            63 => self.actor_text(locs, rhs, |actor, text| StmtKind::AddDetails {
                actor,
                text,
            }),
            // spaceList
            64 | 65 => Val::Nil,
            // actor_pair -> actor , actor
            66 => Val::ActorPair(vec![rhs[0].actor(), rhs[2].actor()]),
            // actor_pair -> actor
            67 => Val::ActorPair(vec![rhs[0].actor()]),
            68 => Val::Placement(Placement::LeftOf),
            69 => Val::Placement(Placement::RightOf),
            // actor signaltype + actor text2
            70 => self.signal(
                locs,
                &rhs[0],
                &rhs[3],
                &rhs[1],
                &rhs[4],
                true,
                None,
                &[StmtKind::ActiveStart(rhs[3].actor().actor)],
            ),
            // actor signaltype - actor text2
            71 => self.signal(
                locs,
                &rhs[0],
                &rhs[3],
                &rhs[1],
                &rhs[4],
                false,
                None,
                &[StmtKind::ActiveEnd(rhs[0].actor().actor)],
            ),
            // actor signaltype () actor text2
            72 => self.signal(
                locs,
                &rhs[0],
                &rhs[3],
                &rhs[1],
                &rhs[4],
                true,
                Some(LineType::CentralConnection),
                &[StmtKind::CentralConnection(rhs[3].actor().actor)],
            ),
            // actor () signaltype actor text2
            73 => self.signal(
                locs,
                &rhs[0],
                &rhs[3],
                &rhs[2],
                &rhs[4],
                false,
                Some(LineType::CentralConnectionReverse),
                &[StmtKind::CentralConnectionReverse(rhs[0].actor().actor)],
            ),
            // actor () signaltype () actor text2
            74 => self.signal(
                locs,
                &rhs[0],
                &rhs[4],
                &rhs[2],
                &rhs[5],
                true,
                Some(LineType::CentralConnectionDual),
                &[
                    StmtKind::CentralConnection(rhs[4].actor().actor),
                    StmtKind::CentralConnectionReverse(rhs[0].actor().actor),
                ],
            ),
            // actor signaltype actor text2
            75 => self.signal(locs, &rhs[0], &rhs[2], &rhs[1], &rhs[3], false, None, &[]),
            // actor_with_config -> ACTOR config_object
            76 => Val::Actor(ActorRef {
                actor: rhs[0].text().to_string(),
                config: Some(rhs[1].text().to_string()),
                ..ActorRef::default()
            }),
            // config_object -> CONFIG_START CONFIG_CONTENT CONFIG_END
            77 => Val::Text(rhs[1].text().trim().to_string()),
            // actor -> ACTOR | link
            78 | 79 => Val::Actor(ActorRef {
                actor: rhs[0].text().to_string(),
                ..ActorRef::default()
            }),
            80 => Val::LineType(LineType::SolidOpen),
            81 => Val::LineType(LineType::DottedOpen),
            82 => Val::LineType(LineType::Solid),
            83 => Val::LineType(LineType::SolidTop),
            84 => Val::LineType(LineType::SolidBottom),
            85 => Val::LineType(LineType::StickTop),
            86 => Val::LineType(LineType::StickBottom),
            87 => Val::LineType(LineType::SolidTopDotted),
            88 => Val::LineType(LineType::SolidBottomDotted),
            89 => Val::LineType(LineType::StickTopDotted),
            90 => Val::LineType(LineType::StickBottomDotted),
            91 => Val::LineType(LineType::SolidArrowTopReverse),
            92 => Val::LineType(LineType::SolidArrowBottomReverse),
            93 => Val::LineType(LineType::StickArrowTopReverse),
            94 => Val::LineType(LineType::StickArrowBottomReverse),
            95 => Val::LineType(LineType::SolidArrowTopReverseDotted),
            96 => Val::LineType(LineType::SolidArrowBottomReverseDotted),
            97 => Val::LineType(LineType::StickArrowTopReverseDotted),
            98 => Val::LineType(LineType::StickArrowBottomReverseDotted),
            99 => Val::LineType(LineType::BidirectionalSolid),
            100 => Val::LineType(LineType::Dotted),
            101 => Val::LineType(LineType::BidirectionalDotted),
            102 => Val::LineType(LineType::SolidCross),
            103 => Val::LineType(LineType::DottedCross),
            104 => Val::LineType(LineType::SolidPoint),
            105 => Val::LineType(LineType::DottedPoint),
            // text2 -> TXT: `yy.parseMessage($1.trim().substring(1))`
            106 => Val::Msg(SequenceDb::parse_message(&skip_chars(
                rhs[0].text().trim(),
                1,
            ))),
            _ => Val::Nil,
        };
        Ok(v)
    }
}
