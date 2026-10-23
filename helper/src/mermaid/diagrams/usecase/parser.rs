//! Port of `usecase.parser.ts` (a Chevrotain 13 `CstParser`) and the
//! error handling of `usecase.chevrotain.ts` and `runChevrotainParse.ts`.
//!
//! The parser is a recursive-descent walk of the same rules. Each
//! `OR`/`OPTION`/`MANY` decision uses the lookahead Chevrotain computes
//! for it: Chevrotain only looks at the rest of the enclosing rule, so
//! most decisions need one token; the `statement` alternatives and the
//! metadata separator loop need up to three. Error recovery is off, so
//! the first error ends the parse, with Chevrotain's default message:
//! `MismatchedTokenException`, `NoViableAltException`,
//! `EarlyExitException`, or `NotAllInputParsedException`.
//!
//! The output is the part of the CST the visitor (`usecase.visitor.ts`)
//! reads, already shaped as its intermediate values.

use std::fmt::Write;

use super::lexer::{Kind, LexError, Positions, Token, tokenize};

/// A byte range of the source.
pub type Span = (usize, usize);

/// `LabelType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelType {
    Text,
    Markdown,
}

impl LabelType {
    pub fn as_str(self) -> &'static str {
        match self {
            LabelType::Text => "text",
            LabelType::Markdown => "markdown",
        }
    }
}

/// `DraftLabel`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub text: String,
    pub kind: LabelType,
    pub span: Span,
}

/// A metadata value: a decoded string or a boolean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetaValue {
    Str(String),
    Bool(bool),
}

/// `DraftMetadataProperty`; `loc` is the key token's location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaProp {
    pub key: String,
    pub value: MetaValue,
    pub loc: Span,
}

/// `DraftMetadata`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Metadata {
    pub properties: Vec<MetaProp>,
}

/// `UseCaseShape`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Ellipse,
    Rect,
}

/// `ParsedActorItem` and `ParsedEntity`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    pub id: String,
    pub label: Label,
    /// The id token (string tokens without their quotes).
    pub loc: Span,
    pub generated: bool,
    pub shape: Option<Shape>,
    pub metadata: Option<Metadata>,
    pub stereotype: Option<String>,
    pub classes: Vec<String>,
    /// `explicitDeclaration` of `entityName`; actors leave it unset.
    pub explicit_declaration: bool,
}

/// `ArrowType` (`ARROW_TYPE` in `usecaseTypes.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowType {
    SolidArrow = 0,
    BackArrow = 1,
    LineSolid = 2,
    CircleArrow = 3,
    CrossArrow = 4,
    CircleArrowReversed = 5,
    CrossArrowReversed = 6,
}

/// `RelationshipType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationKind {
    Association,
    Include,
    Extend,
    Generalization,
}

impl RelationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RelationKind::Association => "association",
            RelationKind::Include => "include",
            RelationKind::Extend => "extend",
            RelationKind::Generalization => "generalization",
        }
    }
}

/// `ParsedArrow`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arrow {
    pub kind: RelationKind,
    pub arrow_type: ArrowType,
    pub label: Option<Label>,
    pub minlen: u32,
}

/// `ParsedRelationTail` plus the span of the `relationTail` node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationTail {
    pub explicit_id: Option<(String, Span)>,
    pub arrow: Arrow,
    pub target: Entity,
    pub span: Span,
}

/// `DraftBoundary` as `systemBoundaryName` returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryName {
    pub id: String,
    pub label: Label,
    pub loc: Span,
    pub generated: bool,
}

/// One child of `systemBoundaryContent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryChild {
    Actors(Vec<Entity>),
    Entity(Entity),
}

/// One statement in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    AccTitle(String),
    AccDescr(String),
    Direction(String),
    Actor {
        items: Vec<Entity>,
        relation: Option<RelationTail>,
    },
    Entity {
        source: Entity,
        relation: Option<RelationTail>,
    },
    Boundary {
        name: BoundaryName,
        metadata: Option<Metadata>,
        classes: Vec<String>,
        children: Vec<BoundaryChild>,
    },
    MetadataAssignment {
        target: String,
        loc: Span,
        metadata: Metadata,
    },
    Note {
        target: String,
        target_loc: Span,
        label: Label,
    },
    Json {
        id: String,
        id_loc: Span,
        literal: Span,
        classes: Vec<String>,
    },
    ClassDef {
        ids: Vec<String>,
        styles: Vec<String>,
    },
    ClassAssign {
        targets: Vec<(String, Span)>,
        classes: Vec<String>,
    },
    Style {
        target: String,
        loc: Span,
        styles: Vec<String>,
    },
}

/// The error `parser.parse` throws, positions resolved, with the line
/// `R-177` reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFailure {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
enum ErrorKind {
    Mismatch(&'static str),
    NoViableAlt(Vec<Vec<&'static str>>),
    EarlyExit(Vec<&'static str>),
    Redundant,
}

/// A parser error and the token it points at (`EOF` at the end).
#[derive(Debug, Clone)]
struct PError {
    kind: ErrorKind,
    at: Token,
}

type PResult<T> = Result<T, PError>;

/// The second token of the `entityStatement` lookahead paths after an
/// `IDENTIFIER`.
const ENTITY_FOLLOW_ID: &[Kind] = &[
    Kind::LeftParen,
    Kind::LeftBracket,
    Kind::StereotypeStart,
    Kind::ClassSeparator,
    Kind::Identifier,
    Kind::DependencyArrow,
    Kind::Generalization,
    Kind::ForwardSolid,
    Kind::BackwardSolid,
    Kind::MarkerlessSolid,
    Kind::ForwardCircle,
    Kind::BackwardCircle,
    Kind::ForwardCross,
    Kind::BackwardCross,
    Kind::Newline,
    Kind::Eof,
];
/// The same after a quoted name, which takes no `(` or `[` label.
const ENTITY_FOLLOW_STRING: &[Kind] = &[
    Kind::StereotypeStart,
    Kind::ClassSeparator,
    Kind::Identifier,
    Kind::DependencyArrow,
    Kind::Generalization,
    Kind::ForwardSolid,
    Kind::BackwardSolid,
    Kind::MarkerlessSolid,
    Kind::ForwardCircle,
    Kind::BackwardCircle,
    Kind::ForwardCross,
    Kind::BackwardCross,
    Kind::Newline,
    Kind::Eof,
];
/// The third token after `<target> @{` in the metadata paths.
const METADATA_THIRD: &[Kind] = &[
    Kind::Newline,
    Kind::Identifier,
    Kind::PlainString,
    Kind::RightBrace,
];
const TARGETS: &[Kind] = &[Kind::Identifier, Kind::PlainString, Kind::MarkdownString];
const ARROWS: &[Kind] = &[
    Kind::DependencyArrow,
    Kind::Generalization,
    Kind::ForwardSolid,
    Kind::BackwardSolid,
    Kind::MarkerlessSolid,
    Kind::ForwardCircle,
    Kind::BackwardCircle,
    Kind::ForwardCross,
    Kind::BackwardCross,
];
const STYLE_COMPONENTS: &[Kind] = &[
    Kind::PlainString,
    Kind::Number,
    Kind::HashColor,
    Kind::CssIdentifier,
    Kind::CssEscapedComma,
    Kind::Dash,
    Kind::Dot,
    Kind::Percent,
    Kind::CssPunctuation,
    Kind::Colon,
    Kind::LeftParen,
    Kind::RightParen,
    Kind::LeftBracket,
    Kind::RightBracket,
    Kind::LeftBrace,
    Kind::RightBrace,
    Kind::At,
    Kind::MarkerlessSolid,
];

/// `forbiddenPlantUmlStatements`.
const FORBIDDEN: &[&str] = &[
    "allowmixing",
    "newpage",
    "package",
    "rectangle",
    "skinparam",
];

fn names(kinds: &[Kind]) -> Vec<&'static str> {
    kinds.iter().map(|k| k.name()).collect()
}

/// The lookahead paths of the `statement` alternatives, in the order
/// Chevrotain lists them in a `NoViableAltException`.
fn statement_paths() -> Vec<Vec<&'static str>> {
    let mut paths: Vec<Vec<&'static str>> = [
        Kind::AccTitleLine,
        Kind::AccDescrLine,
        Kind::AccDescrBlock,
        Kind::Direction,
        Kind::Actor,
        Kind::SystemBoundary,
        Kind::Note,
        Kind::JsonDeclarationStart,
        Kind::ClassDef,
        Kind::Class,
        Kind::Style,
    ]
    .iter()
    .map(|k| vec![k.name()])
    .collect();
    let metadata_paths = || {
        let mut out = Vec::new();
        for t in TARGETS {
            for third in METADATA_THIRD {
                out.push(vec![t.name(), Kind::MetadataStart.name(), third.name()]);
            }
        }
        out
    };
    paths.extend(metadata_paths());
    for t in TARGETS {
        let follow = if *t == Kind::Identifier {
            ENTITY_FOLLOW_ID
        } else {
            ENTITY_FOLLOW_STRING
        };
        for f in follow {
            paths.push(vec![t.name(), f.name()]);
        }
    }
    paths.extend(metadata_paths());
    paths
}

struct Parser<'a> {
    text: &'a str,
    tokens: Vec<Token>,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str, mut tokens: Vec<Token>) -> Parser<'a> {
        tokens.push(Token {
            kind: Kind::Eof,
            start: text.len(),
            end: text.len(),
        });
        Parser {
            text,
            tokens,
            pos: 0,
        }
    }

    fn la(&self, k: usize) -> Token {
        self.tokens[(self.pos + k - 1).min(self.tokens.len() - 1)]
    }

    fn kind(&self, k: usize) -> Kind {
        self.la(k).kind
    }

    fn image(&self, t: Token) -> &'a str {
        &self.text[t.start..t.end]
    }

    fn error(&self, kind: ErrorKind) -> PError {
        PError {
            kind,
            at: self.la(1),
        }
    }

    fn advance(&mut self) -> Token {
        let t = self.la(1);
        if t.kind != Kind::Eof {
            self.pos += 1;
        }
        t
    }

    fn consume(&mut self, kind: Kind) -> PResult<Token> {
        if self.kind(1) == kind {
            Ok(self.advance())
        } else {
            Err(self.error(ErrorKind::Mismatch(kind.name())))
        }
    }

    fn consume_text(&mut self, kind: Kind) -> PResult<String> {
        let t = self.consume(kind)?;
        Ok(self.image(t).to_string())
    }

    /// `CONSUME(Word)`: any token in the `WORD` category.
    fn consume_word(&mut self) -> PResult<Token> {
        if self.kind(1).is_word() {
            Ok(self.advance())
        } else {
            Err(self.error(ErrorKind::Mismatch("WORD")))
        }
    }

    fn no_viable(&self, paths: &[&[Kind]]) -> PError {
        self.error(ErrorKind::NoViableAlt(
            paths.iter().map(|p| names(p)).collect(),
        ))
    }

    /// A `NoViableAltException` whose paths name token categories.
    fn no_viable_named(&self, paths: &[&'static str]) -> PError {
        self.error(ErrorKind::NoViableAlt(
            paths.iter().map(|p| vec![*p]).collect(),
        ))
    }

    // -- rules --------------------------------------------------------

    fn start(&mut self) -> PResult<Vec<Statement>> {
        self.consume(Kind::Usecase)?;
        self.line_end()?;
        let mut statements = Vec::new();
        while self.is_line_start() {
            if let Some(s) = self.line()? {
                statements.push(s);
            }
        }
        if self.kind(1) != Kind::Eof {
            return Err(self.error(ErrorKind::Redundant));
        }
        Ok(statements)
    }

    fn is_statement_first(&self, k: Kind) -> bool {
        matches!(
            k,
            Kind::AccTitleLine
                | Kind::AccDescrLine
                | Kind::AccDescrBlock
                | Kind::Direction
                | Kind::Actor
                | Kind::SystemBoundary
                | Kind::Note
                | Kind::JsonDeclarationStart
                | Kind::ClassDef
                | Kind::Class
                | Kind::Style
                | Kind::Identifier
                | Kind::PlainString
                | Kind::MarkdownString
        )
    }

    fn is_line_start(&self) -> bool {
        let k = self.kind(1);
        k == Kind::Newline || k == Kind::Comment || self.is_statement_first(k)
    }

    fn line_end(&mut self) -> PResult<()> {
        match self.kind(1) {
            Kind::Newline => {
                self.advance();
                Ok(())
            }
            Kind::Eof => Ok(()),
            _ => Err(self.no_viable(&[&[Kind::Newline], &[Kind::Eof]])),
        }
    }

    /// `line`: a blank line, a comment line, or a statement.
    fn line(&mut self) -> PResult<Option<Statement>> {
        match self.kind(1) {
            Kind::Newline => {
                self.advance();
                Ok(None)
            }
            Kind::Comment => {
                self.advance();
                self.line_end()?;
                Ok(None)
            }
            _ => self.statement().map(Some),
        }
    }

    fn statement(&mut self) -> PResult<Statement> {
        let (k1, k2, k3) = (self.kind(1), self.kind(2), self.kind(3));
        match k1 {
            Kind::AccTitleLine => return self.acc_title(),
            Kind::AccDescrLine | Kind::AccDescrBlock => return self.acc_descr(),
            Kind::Direction => return self.direction(),
            Kind::Actor => return self.actor_statement(),
            Kind::SystemBoundary => return self.system_boundary(),
            Kind::Note => return self.note(),
            Kind::JsonDeclarationStart => return self.json(),
            Kind::ClassDef => return self.class_def(),
            Kind::Class => return self.class_statement(),
            Kind::Style => return self.style_statement(),
            _ => {}
        }
        let target = TARGETS.contains(&k1);
        let metadata_path = target && k2 == Kind::MetadataStart && METADATA_THIRD.contains(&k3);
        // `isMetadataAssignment` gate plus the alternative's lookahead.
        if metadata_path {
            return self.metadata_assignment();
        }
        // The entity paths that start like a metadata assignment were
        // taken above; the rest need a second token that can follow a name.
        let entity_path = (k1 == Kind::Identifier && ENTITY_FOLLOW_ID.contains(&k2))
            || ((k1 == Kind::PlainString || k1 == Kind::MarkdownString)
                && ENTITY_FOLLOW_STRING.contains(&k2));
        let forbidden = k1 == Kind::Identifier
            && FORBIDDEN.contains(&self.image(self.la(1)).to_lowercase().as_str());
        if entity_path && !forbidden {
            return self.entity_statement();
        }
        Err(self.error(ErrorKind::NoViableAlt(statement_paths())))
    }

    fn acc_title(&mut self) -> PResult<Statement> {
        let t = self.advance();
        let image = self.image(t);
        let value = image[image.find(':').map_or(0, |i| i + 1)..].trim();
        let value = value.to_string();
        self.line_end()?;
        Ok(Statement::AccTitle(value))
    }

    fn acc_descr(&mut self) -> PResult<Statement> {
        let t = self.advance();
        let image = self.image(t);
        let value = if t.kind == Kind::AccDescrLine {
            image[image.find(':').map_or(0, |i| i + 1)..].trim()
        } else {
            let open = image.find('{').map_or(0, |i| i + 1);
            let close = image.rfind('}').unwrap_or(image.len());
            image[open..close.max(open)].trim()
        };
        let value = value.to_string();
        self.line_end()?;
        Ok(Statement::AccDescr(value))
    }

    fn direction(&mut self) -> PResult<Statement> {
        self.advance();
        let dirs = [Kind::Td, Kind::Tb, Kind::Bt, Kind::Lr, Kind::Rl];
        if !dirs.contains(&self.kind(1)) {
            return Err(self.no_viable(&[
                &[Kind::Td],
                &[Kind::Tb],
                &[Kind::Bt],
                &[Kind::Lr],
                &[Kind::Rl],
            ]));
        }
        let t = self.advance();
        let dir = self.image(t).to_string();
        self.line_end()?;
        Ok(Statement::Direction(dir))
    }

    fn is_relation_first(&self, k: Kind) -> bool {
        k == Kind::Identifier || ARROWS.contains(&k)
    }

    fn actor_statement(&mut self) -> PResult<Statement> {
        self.advance();
        let mut items = vec![self.actor_item()?];
        let mut relation = None;
        let k = self.kind(1);
        if k == Kind::Comma {
            while self.kind(1) == Kind::Comma {
                self.advance();
                items.push(self.actor_item()?);
            }
        } else if self.is_relation_first(k) {
            relation = Some(self.relation_tail()?);
        }
        self.line_end()?;
        Ok(Statement::Actor { items, relation })
    }

    fn actor_item(&mut self) -> PResult<Entity> {
        let mut item = self.actor_name()?;
        if self.kind(1) == Kind::MetadataStart {
            item.metadata = Some(self.metadata()?);
        }
        if self.kind(1) == Kind::StereotypeStart {
            item.stereotype = Some(self.stereotype()?);
        }
        if self.kind(1) == Kind::ClassSeparator {
            item.classes = self.class_suffix()?;
        }
        Ok(item)
    }

    fn name_alternatives(&self) -> PError {
        self.no_viable(&[
            &[Kind::Identifier],
            &[Kind::PlainString],
            &[Kind::MarkdownString],
        ])
    }

    fn actor_name(&mut self) -> PResult<Entity> {
        match self.kind(1) {
            Kind::Identifier => {
                let t = self.advance();
                let label = if self.kind(1) == Kind::LeftParen {
                    self.advance();
                    let label = self.node_label()?;
                    self.consume(Kind::RightParen)?;
                    label
                } else {
                    self.token_label(t)
                };
                Ok(self.named(t, label))
            }
            Kind::PlainString | Kind::MarkdownString => {
                let t = self.advance();
                Ok(self.generated(t))
            }
            _ => Err(self.name_alternatives()),
        }
    }

    fn named(&self, t: Token, label: Label) -> Entity {
        Entity {
            id: self.image(t).to_string(),
            label,
            loc: (t.start, t.end),
            generated: false,
            shape: None,
            metadata: None,
            stereotype: None,
            classes: Vec::new(),
            explicit_declaration: false,
        }
    }

    fn generated(&self, t: Token) -> Entity {
        let label = self.token_label(t);
        Entity {
            id: generate_id(&label.text),
            loc: label.span,
            label,
            generated: true,
            shape: None,
            metadata: None,
            stereotype: None,
            classes: Vec::new(),
            explicit_declaration: false,
        }
    }

    fn entity_statement(&mut self) -> PResult<Statement> {
        let source = self.entity_name()?;
        let relation = if self.is_relation_first(self.kind(1)) {
            Some(self.relation_tail()?)
        } else {
            None
        };
        self.line_end()?;
        Ok(Statement::Entity { source, relation })
    }

    fn entity_name(&mut self) -> PResult<Entity> {
        let mut entity = match self.kind(1) {
            Kind::Identifier => {
                let t = self.advance();
                let (label, shape) = match self.kind(1) {
                    Kind::LeftParen => {
                        self.advance();
                        let label = self.node_label()?;
                        self.consume(Kind::RightParen)?;
                        (label, Some(Shape::Ellipse))
                    }
                    Kind::LeftBracket => {
                        self.advance();
                        let label = self.node_label()?;
                        self.consume(Kind::RightBracket)?;
                        (label, Some(Shape::Rect))
                    }
                    _ => (self.token_label(t), None),
                };
                let mut e = self.named(t, label);
                e.shape = shape;
                e
            }
            Kind::PlainString | Kind::MarkdownString => {
                let t = self.advance();
                self.generated(t)
            }
            _ => return Err(self.name_alternatives()),
        };
        if self.kind(1) == Kind::MetadataStart {
            entity.metadata = Some(self.metadata()?);
        }
        if self.kind(1) == Kind::StereotypeStart {
            entity.stereotype = Some(self.stereotype()?);
        }
        if self.kind(1) == Kind::ClassSeparator {
            entity.classes = self.class_suffix()?;
        }
        entity.explicit_declaration =
            entity.shape.is_some() || entity.metadata.is_some() || entity.stereotype.is_some();
        Ok(entity)
    }

    /// `nodeLabel`: a quoted string, or unquoted label tokens rebuilt from
    /// the source slice.
    fn node_label(&mut self) -> PResult<Label> {
        match self.kind(1) {
            Kind::PlainString | Kind::MarkdownString => {
                let t = self.advance();
                Ok(self.token_label(t))
            }
            k if k.is_label_text() => {
                let first = self.advance();
                let mut last = first;
                while self.kind(1).is_label_text() {
                    last = self.advance();
                }
                let span = (first.start, last.end);
                Ok(Label {
                    text: self.text[span.0..span.1].to_string(),
                    kind: LabelType::Text,
                    span,
                })
            }
            _ => Err(self.no_viable_named(&["PLAIN_STRING", "MARKDOWN_STRING", "LABEL_TEXT"])),
        }
    }

    fn relation_tail(&mut self) -> PResult<RelationTail> {
        let start = self.la(1).start;
        let explicit_id = if self.kind(1) == Kind::Identifier && self.kind(2) == Kind::At {
            let t = self.advance();
            self.advance();
            Some((self.image(t).to_string(), (t.start, t.end)))
        } else {
            None
        };
        let arrow = self.arrow()?;
        let target = self.entity_name()?;
        let end = self.tokens[self.pos.saturating_sub(1)].end;
        Ok(RelationTail {
            explicit_id,
            arrow,
            target,
            span: (start, end),
        })
    }

    fn arrow(&mut self) -> PResult<Arrow> {
        let t = self.la(1);
        match t.kind {
            Kind::DependencyArrow => {
                self.advance();
                self.consume(Kind::Colon)?;
                let kind = match self.kind(1) {
                    Kind::Include => RelationKind::Include,
                    Kind::Extend => RelationKind::Extend,
                    _ => return Err(self.no_viable(&[&[Kind::Include], &[Kind::Extend]])),
                };
                let word = self.advance();
                Ok(Arrow {
                    kind,
                    arrow_type: ArrowType::SolidArrow,
                    label: Some(Label {
                        text: kind.as_str().to_string(),
                        kind: LabelType::Text,
                        span: (word.start, word.end),
                    }),
                    minlen: 1,
                })
            }
            Kind::Generalization => {
                self.advance();
                Ok(Arrow {
                    kind: RelationKind::Generalization,
                    ..association(ArrowType::SolidArrow, None, 1)
                })
            }
            Kind::ForwardSolid => {
                self.advance();
                Ok(association(
                    ArrowType::SolidArrow,
                    None,
                    solid_minlen(self.image(t)),
                ))
            }
            Kind::BackwardSolid => {
                self.advance();
                if self.image(t) == "<--" && self.has_labeled_right(&[Kind::MarkerlessSolid]) {
                    let label = self.edge_label()?;
                    let close = self.consume(Kind::MarkerlessSolid)?;
                    let minlen = solid_minlen(self.image(close));
                    Ok(association(ArrowType::BackArrow, Some(label), minlen))
                } else {
                    Ok(association(
                        ArrowType::BackArrow,
                        None,
                        solid_minlen(self.image(t)),
                    ))
                }
            }
            Kind::MarkerlessSolid => {
                self.advance();
                let closers = [
                    Kind::ForwardSolid,
                    Kind::MarkerlessSolid,
                    Kind::ForwardCircle,
                    Kind::ForwardCross,
                ];
                if self.image(t) == "--" && self.has_labeled_right(&closers) {
                    let label = self.edge_label()?;
                    if !closers.contains(&self.kind(1)) {
                        return Err(self.no_viable(&[
                            &[Kind::ForwardSolid],
                            &[Kind::MarkerlessSolid],
                            &[Kind::ForwardCircle],
                            &[Kind::ForwardCross],
                        ]));
                    }
                    let last = self.advance();
                    let (arrow_type, minlen) = match last.kind {
                        Kind::ForwardSolid => {
                            (ArrowType::SolidArrow, solid_minlen(self.image(last)))
                        }
                        Kind::ForwardCircle => (ArrowType::CircleArrow, 1),
                        Kind::ForwardCross => (ArrowType::CrossArrow, 1),
                        _ => (ArrowType::LineSolid, solid_minlen(self.image(last))),
                    };
                    Ok(association(arrow_type, Some(label), minlen))
                } else {
                    Ok(association(
                        ArrowType::LineSolid,
                        None,
                        solid_minlen(self.image(t)),
                    ))
                }
            }
            Kind::ForwardCircle => {
                self.advance();
                Ok(association(ArrowType::CircleArrow, None, 1))
            }
            Kind::ForwardCross => {
                self.advance();
                Ok(association(ArrowType::CrossArrow, None, 1))
            }
            Kind::BackwardCircle | Kind::BackwardCross => {
                self.advance();
                let arrow_type = if t.kind == Kind::BackwardCircle {
                    ArrowType::CircleArrowReversed
                } else {
                    ArrowType::CrossArrowReversed
                };
                let label = if self.has_labeled_right(&[Kind::MarkerlessSolid]) {
                    let label = self.edge_label()?;
                    self.consume(Kind::MarkerlessSolid)?;
                    Some(label)
                } else {
                    None
                };
                Ok(association(arrow_type, label, 1))
            }
            _ => Err(self.no_viable(&ARROWS.iter().map(std::slice::from_ref).collect::<Vec<_>>())),
        }
    }

    /// `hasLabeledRight(allowed)`: label tokens up to one of `allowed`.
    fn has_labeled_right(&self, allowed: &[Kind]) -> bool {
        let is_label =
            |k: Kind| k.is_label_text() || k == Kind::PlainString || k == Kind::MarkdownString;
        if !is_label(self.kind(1)) {
            return false;
        }
        let mut i = 2;
        loop {
            let k = self.kind(i);
            if allowed.contains(&k) {
                return true;
            }
            if !is_label(k) {
                return false;
            }
            i += 1;
        }
    }

    fn edge_label(&mut self) -> PResult<Label> {
        self.node_label()
    }

    fn metadata(&mut self) -> PResult<Metadata> {
        self.consume(Kind::MetadataStart)?;
        while self.kind(1) == Kind::Newline {
            self.advance();
        }
        let mut properties = Vec::new();
        if matches!(self.kind(1), Kind::Identifier | Kind::PlainString) {
            properties.push(self.metadata_property()?);
            while self.separator_iterates() {
                self.metadata_separator();
                properties.push(self.metadata_property()?);
            }
            if self.kind(1) == Kind::Comma {
                self.advance();
            }
            while self.kind(1) == Kind::Newline {
                self.advance();
            }
        }
        self.consume(Kind::RightBrace)?;
        Ok(Metadata { properties })
    }

    /// The `MANY2(metadataSeparator metadataProperty)` lookahead: the
    /// paths Chevrotain keeps at depth three, ambiguous ones included.
    fn separator_iterates(&self) -> bool {
        use Kind::{Comma as C, Newline as N};
        let p = |k: Kind| matches!(k, Kind::Identifier | Kind::PlainString);
        let (a, b, c) = (self.kind(1), self.kind(2), self.kind(3));
        match (a, b) {
            (C, x) if p(x) => true,
            (C, N) => p(c) || c == N,
            (N, x) if p(x) => true,
            (N, C) => true,
            (N, N) => p(c) || c == C || c == N,
            _ => false,
        }
    }

    fn metadata_separator(&mut self) {
        if self.kind(1) == Kind::Comma {
            self.advance();
            while self.kind(1) == Kind::Newline {
                self.advance();
            }
        } else {
            while self.kind(1) == Kind::Newline {
                self.advance();
            }
            if self.kind(1) == Kind::Comma {
                self.advance();
            }
            while self.kind(1) == Kind::Newline {
                self.advance();
            }
        }
    }

    fn metadata_property(&mut self) -> PResult<MetaProp> {
        let key = match self.kind(1) {
            Kind::Identifier | Kind::PlainString => self.advance(),
            _ => return Err(self.no_viable(&[&[Kind::Identifier], &[Kind::PlainString]])),
        };
        self.consume(Kind::Colon)?;
        let value = match self.kind(1) {
            Kind::Identifier | Kind::PlainString => {
                let t = self.advance();
                MetaValue::Str(self.decode_plain(t))
            }
            Kind::True => {
                self.advance();
                MetaValue::Bool(true)
            }
            Kind::False => {
                self.advance();
                MetaValue::Bool(false)
            }
            _ => {
                return Err(self.no_viable(&[
                    &[Kind::Identifier],
                    &[Kind::PlainString],
                    &[Kind::True],
                    &[Kind::False],
                ]));
            }
        };
        Ok(MetaProp {
            key: self.decode_plain(key),
            value,
            loc: (key.start, key.end),
        })
    }

    fn decode_plain(&self, t: Token) -> String {
        let image = self.image(t);
        if t.kind == Kind::PlainString {
            image[1..image.len() - 1].to_string()
        } else {
            image.to_string()
        }
    }

    fn system_boundary(&mut self) -> PResult<Statement> {
        self.advance();
        let name = match self.kind(1) {
            Kind::Identifier => {
                let t = self.advance();
                let label = match self.kind(1) {
                    Kind::LeftParen => {
                        self.advance();
                        let label = self.node_label()?;
                        self.consume(Kind::RightParen)?;
                        label
                    }
                    Kind::LeftBracket => {
                        self.advance();
                        let label = self.node_label()?;
                        self.consume(Kind::RightBracket)?;
                        label
                    }
                    _ => self.token_label(t),
                };
                BoundaryName {
                    id: self.image(t).to_string(),
                    label,
                    loc: (t.start, t.end),
                    generated: false,
                }
            }
            Kind::PlainString | Kind::MarkdownString => {
                let t = self.advance();
                let label = self.token_label(t);
                BoundaryName {
                    id: generate_id(&label.text),
                    loc: label.span,
                    label,
                    generated: true,
                }
            }
            _ => return Err(self.name_alternatives()),
        };
        let metadata = if self.kind(1) == Kind::MetadataStart {
            Some(self.metadata()?)
        } else {
            None
        };
        let classes = if self.kind(1) == Kind::ClassSeparator {
            self.class_suffix()?
        } else {
            Vec::new()
        };
        self.line_end()?;
        let mut children = Vec::new();
        loop {
            match self.kind(1) {
                Kind::Newline => {
                    self.advance();
                }
                Kind::Comment => {
                    self.advance();
                    self.line_end()?;
                }
                Kind::Actor => {
                    self.advance();
                    let mut items = vec![self.actor_item()?];
                    while self.kind(1) == Kind::Comma {
                        self.advance();
                        items.push(self.actor_item()?);
                    }
                    self.line_end()?;
                    children.push(BoundaryChild::Actors(items));
                }
                Kind::Identifier | Kind::PlainString | Kind::MarkdownString => {
                    let entity = self.entity_name()?;
                    self.line_end()?;
                    children.push(BoundaryChild::Entity(entity));
                }
                _ => break,
            }
        }
        self.consume(Kind::End)?;
        self.line_end()?;
        Ok(Statement::Boundary {
            name,
            metadata,
            classes,
            children,
        })
    }

    fn metadata_assignment(&mut self) -> PResult<Statement> {
        let t = self.advance();
        let (target, loc) = if t.kind == Kind::Identifier {
            (self.image(t).to_string(), (t.start, t.end))
        } else {
            let label = self.token_label(t);
            (generate_id(&label.text), label.span)
        };
        let metadata = self.metadata()?;
        self.line_end()?;
        Ok(Statement::MetadataAssignment {
            target,
            loc,
            metadata,
        })
    }

    fn note(&mut self) -> PResult<Statement> {
        self.advance();
        self.consume(Kind::For)?;
        let target = self.consume(Kind::Identifier)?;
        let label = self.node_label()?;
        self.line_end()?;
        Ok(Statement::Note {
            target: self.image(target).to_string(),
            target_loc: (target.start, target.end),
            label,
        })
    }

    fn stereotype(&mut self) -> PResult<String> {
        self.consume(Kind::StereotypeStart)?;
        let t = self.consume(Kind::StereotypeText)?;
        self.consume(Kind::StereotypeEnd)?;
        Ok(self.image(t).trim().to_string())
    }

    fn class_suffix(&mut self) -> PResult<Vec<String>> {
        self.consume(Kind::ClassSeparator)?;
        let mut classes = vec![self.consume_text(Kind::Identifier)?];
        while self.kind(1) == Kind::Comma {
            self.advance();
            classes.push(self.consume_text(Kind::Identifier)?);
        }
        Ok(classes)
    }

    fn json(&mut self) -> PResult<Statement> {
        let start = self.advance();
        let literal = self.consume(Kind::JsonObjectLiteral)?;
        let classes = if self.kind(1) == Kind::ClassSeparator {
            self.class_suffix()?
        } else {
            Vec::new()
        };
        self.line_end()?;
        // `/^json[\t ]+(\w+)/`: the id follows the keyword and blanks.
        let image = self.image(start);
        let after = image[4..].trim_start_matches([' ', '\t']);
        let id_start = start.start + (image.len() - after.len());
        let id_len = after
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
            .count();
        Ok(Statement::Json {
            id: after[..id_len].to_string(),
            id_loc: (id_start, id_start + id_len),
            literal: (literal.start, literal.end),
            classes,
        })
    }

    fn class_def(&mut self) -> PResult<Statement> {
        self.advance();
        let mut ids = vec![self.consume_text(Kind::Identifier)?];
        while self.kind(1) == Kind::Comma {
            self.advance();
            ids.push(self.consume_text(Kind::Identifier)?);
        }
        let styles = self.styles()?;
        self.line_end()?;
        Ok(Statement::ClassDef { ids, styles })
    }

    fn class_statement(&mut self) -> PResult<Statement> {
        self.advance();
        let first = self.consume(Kind::Identifier)?;
        let mut targets = vec![(self.image(first).to_string(), (first.start, first.end))];
        while self.kind(1) == Kind::Comma {
            self.advance();
            let t = self.consume(Kind::Identifier)?;
            targets.push((self.image(t).to_string(), (t.start, t.end)));
        }
        let mut classes = vec![self.consume_text(Kind::Identifier)?];
        while self.kind(1) == Kind::Comma {
            self.advance();
            classes.push(self.consume_text(Kind::Identifier)?);
        }
        self.line_end()?;
        Ok(Statement::ClassAssign { targets, classes })
    }

    fn style_statement(&mut self) -> PResult<Statement> {
        self.advance();
        let target = self.consume(Kind::Identifier)?;
        let styles = self.styles()?;
        self.line_end()?;
        Ok(Statement::Style {
            target: self.image(target).to_string(),
            loc: (target.start, target.end),
            styles,
        })
    }

    fn styles(&mut self) -> PResult<Vec<String>> {
        let mut out = vec![self.style_value()?];
        while self.kind(1) == Kind::Comma {
            self.advance();
            out.push(self.style_value()?);
        }
        Ok(out)
    }

    fn is_style_component(k: Kind) -> bool {
        k.is_word() || STYLE_COMPONENTS.contains(&k)
    }

    /// `styleValue`: the source slice from the property to the last
    /// component, with `\,` unescaped.
    fn style_value(&mut self) -> PResult<String> {
        let first = match self.kind(1) {
            k if k.is_word() => self.advance(),
            Kind::CssIdentifier => self.advance(),
            Kind::MarkerlessSolid => {
                let t = self.advance();
                self.consume_word()?;
                t
            }
            _ => {
                return Err(self.no_viable_named(&["WORD", "CSS_IDENTIFIER", "MARKERLESS_SOLID"]));
            }
        };
        self.consume(Kind::Colon)?;
        if !Self::is_style_component(self.kind(1)) {
            let mut paths = vec!["WORD"];
            paths.extend(names(STYLE_COMPONENTS));
            return Err(self.error(ErrorKind::EarlyExit(paths)));
        }
        let mut last = self.advance();
        while Self::is_style_component(self.kind(1)) {
            last = self.advance();
        }
        Ok(self.text[first.start..last.end].replace("\\,", ","))
    }

    /// `tokenLabel`: the token text without its quotes or backticks.
    fn token_label(&self, t: Token) -> Label {
        let (kind, trim) = match t.kind {
            Kind::MarkdownString => (LabelType::Markdown, 2),
            Kind::PlainString => (LabelType::Text, 1),
            _ => (LabelType::Text, 0),
        };
        let span = (
            t.start + trim,
            t.end.saturating_sub(trim).max(t.start + trim),
        );
        Label {
            text: self.text[span.0..span.1].to_string(),
            kind,
            span,
        }
    }
}

fn association(arrow_type: ArrowType, label: Option<Label>, minlen: u32) -> Arrow {
    Arrow {
        kind: RelationKind::Association,
        arrow_type,
        label,
        minlen,
    }
}

/// `solidMinlen`: one less than the dashes, at least 1.
fn solid_minlen(image: &str) -> u32 {
    let dashes = image.matches('-').count();
    let dashes = if dashes == 0 { 2 } else { dashes };
    dashes.saturating_sub(1).max(1) as u32
}

/// `generateId`: every non-word character becomes `_`.
pub fn generate_id(label: &str) -> String {
    label
        .chars()
        .flat_map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                vec![c]
            } else {
                // `\W` matches per UTF-16 unit.
                vec!['_'; c.len_utf16()]
            }
        })
        .collect()
}

fn format_paths(paths: &[Vec<&'static str>]) -> String {
    let mut out = String::new();
    for (i, p) in paths.iter().enumerate() {
        let _ = write!(out, "\n  {}. [{}]", i + 1, p.join(", "));
    }
    out
}

/// Parses `text` (the diagram text with the newline `fromText` appends)
/// into statements, or the error `parser.parse` throws.
pub fn parse(text: &str) -> Result<Vec<Statement>, ParseFailure> {
    let positions = Positions::new(text);
    let tokens = match tokenize(text) {
        Ok(t) => t,
        Err(LexError { start, end }) => {
            let (line, column) = positions.line_column(start);
            let ch = text[start..].chars().next().unwrap_or(' ');
            let (s, e) = (positions.offset(start), positions.offset(end));
            return Err(ParseFailure {
                line,
                column,
                message: format!(
                    "Error lexing usecase diagram: unexpected character: ->{ch}<- at offset: {s}, skipped {} characters. at line {line}, column {column} [{s},{e})",
                    e - s
                ),
            });
        }
    };
    let mut parser = Parser::new(text, tokens);
    match parser.start() {
        Ok(statements) => Ok(statements),
        Err(err) => {
            let t = err.at;
            let image = &text[t.start..t.end];
            let body = match &err.kind {
                ErrorKind::Mismatch(name) => {
                    format!("Expecting token of type --> {name} <-- but found --> '{image}' <--")
                }
                ErrorKind::NoViableAlt(paths) => format!(
                    "Expecting: one of these possible Token sequences:{}\nbut found: '{image}'",
                    format_paths(paths)
                ),
                ErrorKind::EarlyExit(paths) => format!(
                    "Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <{}>\nbut found: '{image}'",
                    paths
                        .iter()
                        .map(|p| format!("[{p}]"))
                        .collect::<Vec<_>>()
                        .join(" ,")
                ),
                ErrorKind::Redundant => {
                    format!("Redundant input, expecting EOF but found: {image}")
                }
            };
            let (line, column, start, end) = if t.kind == Kind::Eof {
                // EOF has no location; mermaid derives one from the input.
                let start = positions.utf16_len();
                let (line, _) = positions.line_column(text.len());
                let last_break = text.rfind(['\n', '\r']);
                let column = match last_break {
                    Some(i) => start - positions.offset(i),
                    None => start + 1,
                };
                (line, column, start, start)
            } else {
                let (line, column) = positions.line_column(t.start);
                (
                    line,
                    column,
                    positions.offset(t.start),
                    positions.offset(t.end),
                )
            };
            Err(ParseFailure {
                line,
                column,
                message: format!(
                    "Error parsing usecase diagram: {body} at line {line}, column {column} [{start},{end})"
                ),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(text: &str) -> String {
        parse(&format!("{text}\n")).unwrap_err().message
    }

    #[test]
    fn redundant_input() {
        assert_eq!(
            err("usecase-beta\nend"),
            "Error parsing usecase diagram: Redundant input, expecting EOF but found: end at line 2, column 1 [13,16)"
        );
    }

    #[test]
    fn mismatched_and_no_viable() {
        assert_eq!(
            err("usecase-beta\nA ( x"),
            "Error parsing usecase diagram: Expecting token of type --> RPAREN <-- but found --> '\n' <-- at line 2, column 6 [18,19)"
        );
        assert_eq!(
            err("usecase-beta foo"),
            "Error parsing usecase diagram: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: 'foo' at line 1, column 14 [13,16)"
        );
        assert_eq!(
            err("usecase-beta\nnote for A"),
            "Error parsing usecase diagram: Expecting: one of these possible Token sequences:\n  1. [PLAIN_STRING]\n  2. [MARKDOWN_STRING]\n  3. [LABEL_TEXT]\nbut found: '\n' at line 2, column 11 [23,24)"
        );
        assert_eq!(
            err("usecase-beta\nclassDef x"),
            "Error parsing usecase diagram: Expecting: one of these possible Token sequences:\n  1. [WORD]\n  2. [CSS_IDENTIFIER]\n  3. [MARKERLESS_SOLID]\nbut found: '\n' at line 2, column 11 [23,24)"
        );
    }

    #[test]
    fn statement_alternatives_list_every_path() {
        let message = err("usecase-beta\npackage foo");
        assert!(message.contains("\n  79. [MARKDOWN_STRING, METADATA_START, RBRACE]\nbut found: 'package' at line 2, column 1 [13,20)"));
        assert!(message.contains("\n  24. [IDENTIFIER, LPAREN]\n"));
    }

    #[test]
    fn lexer_error() {
        assert_eq!(
            err("usecase-beta\nA \"x"),
            "Error lexing usecase diagram: unexpected character: ->\"<- at offset: 15, skipped 1 characters. at line 2, column 3 [15,16)"
        );
    }

    #[test]
    fn labelled_markerless_edge() {
        let statements = parse("usecase-beta\nA -- \"opens\" ---> B\nA -- lbl\n").unwrap();
        let Statement::Entity {
            relation: Some(r), ..
        } = &statements[0]
        else {
            panic!("not an edge");
        };
        assert_eq!(r.arrow.arrow_type, ArrowType::SolidArrow);
        assert_eq!(r.arrow.minlen, 2);
        assert_eq!(r.arrow.label.as_ref().unwrap().text, "opens");
        let Statement::Entity {
            relation: Some(r), ..
        } = &statements[1]
        else {
            panic!("not an edge");
        };
        assert_eq!(r.arrow.arrow_type, ArrowType::LineSolid);
        assert_eq!(r.target.id, "lbl");
    }

    /// The metadata separator loop iterates on the ambiguous three-token
    /// paths, so blank lines before `}` fail on the next property.
    #[test]
    fn metadata_blank_lines_before_the_brace() {
        assert_eq!(
            err("usecase-beta\nactor A@{\n  type: hollow\n\n\n}"),
            "Error parsing usecase diagram: Expecting: one of these possible Token sequences:\n  1. [IDENTIFIER]\n  2. [PLAIN_STRING]\nbut found: '}' at line 6, column 1 [40,41)"
        );
        assert!(parse("usecase-beta\nactor A@{\n  type: hollow,\n\n  business: true\n}\n").is_ok());
    }

    /// The class suffix loop takes every `, <id>`, so a second actor after
    /// a class list is read as a class.
    #[test]
    fn class_suffix_takes_the_following_ids() {
        let statements = parse("usecase-beta\nactor A:::x, B\n").unwrap();
        let Statement::Actor { items, .. } = &statements[0] else {
            panic!("not an actor");
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].classes, ["x", "B"]);
        assert_eq!(
            err("usecase-beta\nactor A:::x, \"B\""),
            "Error parsing usecase diagram: Expecting token of type --> IDENTIFIER <-- but found --> '\"B\"' <-- at line 2, column 14 [26,29)"
        );
    }

    /// A missing `end` points at the end of the input.
    #[test]
    fn missing_end_reports_eof() {
        assert_eq!(
            err("usecase-beta\nsystemBoundary S\n  A"),
            "Error parsing usecase diagram: Expecting token of type --> END <-- but found --> '' <-- at line 4, column 1 [34,34)"
        );
    }

    #[test]
    fn style_values_need_a_component() {
        assert!(
            err("usecase-beta\nA --> B\nstyle A fill:").starts_with(
                "Error parsing usecase diagram: Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[WORD] ,[PLAIN_STRING] ,[NUMBER] ,"
            )
        );
    }
}
