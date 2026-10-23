//! A hand-written lexer and parser for the language of `wardley.langium`
//! plus `common.langium` (`@mermaid-js/parser` 2.0.1, mermaid 12.2.0).
//!
//! Lexer. Langium's `DefaultTokenBuilder` puts the keywords first, longest
//! first: the one-character keywords `[ , ] @ / ( ) - { }`, where `-` has
//! `ARROW`, `LINK_ARROW`, and `YAML` as its `LONGER_ALT`. The terminals
//! follow in grammar order: `WARDLEY_NUMBER`, `ARROW`, `LINK_PORT`,
//! `LINK_ARROW`, `LINK_LABEL`, `STRATEGY`, the `KW_*` terminals,
//! `NAME_WITH_SPACES`, the skipped `WS`, then the reachable common ones:
//! `ACC_DESCR`, `ACC_TITLE`, `TITLE`, `INT`, `STRING`, `ID`, `NEWLINE`,
//! `WHITESPACE` (never reached, `WS` matches first), and the hidden `YAML`,
//! `DIRECTIVE`, `SINGLE_LINE_COMMENT`. The `KW_*` rules are terminals, not
//! keywords, so they have no `LONGER_ALT`: `components` lexes as
//! `KW_COMPONENT` and `s`, and `buyer` as `STRATEGY` and `er`. At each
//! position the first token type that matches wins; a position where none
//! matches is skipped and reported as one lexer error per run of skipped
//! characters.
//!
//! Parser. An LL walk of `Wardley` with the error recovery of Chevrotain's
//! recovering parser, so the error list matches the oracle's:
//!
//! - `CONSUME` records a mismatch, then inserts the expected token when the
//!   current token can follow it, or deletes the current token when the
//!   next one is the expected one. Otherwise the rule throws.
//! - A rule that throws re-syncs: it finds the first token from the current
//!   one that is in the follow set of any rule on the stack. When that
//!   token can follow the current rule in its caller, the rule skips to it
//!   and returns; otherwise it rethrows to its caller.
//! - At the exit of a repetition whose next element is a terminal, the
//!   in-repetition recovery skips to the expected terminal or to the next
//!   token that starts an iteration, and reports the token it started at.
//! - Every decision is LL(1) except two that Langium's LL(*) lookahead
//!   decides: `Decorator?` is taken for `(` followed by `STRATEGY`, and a
//!   `TitleAndAccessibilities` iteration needs the `EOL` after its token.
//!
//! `MermaidParseError` joins both lists as `Parsing failed: <lexer errors>
//! <parser errors>`. After an error the AST is not used, so a recovered
//! rule leaves its partial result out.

use std::fmt::Write;

use crate::mermaid::theme::khroma::js_is_whitespace;

/// The `Wardley` AST after Langium's value converters.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ast {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// `size=Size`: the last one wins.
    pub size: Option<Size>,
    /// `evolution=Evolution`: the last one wins.
    pub evolution: Option<Vec<EvolutionStage>>,
    pub anchors: Vec<Placed>,
    pub components: Vec<Component>,
    pub links: Vec<Link>,
    pub evolves: Vec<Evolve>,
    pub pipelines: Vec<Pipeline>,
    pub notes: Vec<Placed>,
    /// `annotations+=Annotations`, the box positions.
    pub annotations: Vec<Coordinates>,
    /// `annotation+=Annotation`, the numbered markers.
    pub annotation: Vec<Annotation>,
    pub accelerators: Vec<Placed>,
    pub deaccelerators: Vec<Placed>,
}

/// `Size: KW_SIZE '[' width=INT ',' height=INT ']'`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

/// `EvolutionStage: name ('@' boundary)? ('/' secondName)?`.
#[derive(Debug, Clone, PartialEq)]
pub struct EvolutionStage {
    pub name: String,
    pub boundary: Option<f64>,
    pub second_name: Option<String>,
}

/// An `Anchor`, `Note`, `Accelerator`, or `Deaccelerator`: a name (or the
/// note text) and the two numbers in brackets, in source order.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub name: String,
    pub first: f64,
    pub second: f64,
    /// 1-based source line of the statement, for the errors `populateDb`
    /// throws.
    pub line: u32,
}

/// `Label: KW_LABEL '[' '-'? offsetX=INT ',' '-'? offsetY=INT ']'` with the
/// signs applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelOffset {
    pub x: f64,
    pub y: f64,
}

/// `Component`.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub name: String,
    pub visibility: f64,
    pub evolution: f64,
    pub label: Option<LabelOffset>,
    /// `Decorator: '(' strategy=STRATEGY ')'`.
    pub strategy: Option<String>,
    pub inertia: bool,
    pub line: u32,
}

/// `Link`.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub from: String,
    pub from_port: Option<String>,
    pub arrow: Option<String>,
    pub to: String,
    pub to_port: Option<String>,
    /// `LINK_LABEL` after its value converter: the `;` dropped, trimmed.
    pub link_label: Option<String>,
}

/// `Evolve: KW_EVOLVE component target=WARDLEY_NUMBER`.
#[derive(Debug, Clone, PartialEq)]
pub struct Evolve {
    pub component: String,
    pub target: f64,
    pub line: u32,
}

/// `Pipeline: KW_PIPELINE parent '{' ... '}'`.
#[derive(Debug, Clone, PartialEq)]
pub struct Pipeline {
    pub parent: String,
    pub components: Vec<PipelineComponent>,
    pub line: u32,
}

/// `PipelineComponent: KW_COMPONENT name '[' evolution ']' Label?`.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineComponent {
    pub name: String,
    pub evolution: f64,
    pub label: Option<LabelOffset>,
    pub line: u32,
}

/// `Annotations: KW_ANNOTATIONS '[' x ',' y ']'`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coordinates {
    pub x: f64,
    pub y: f64,
    pub line: u32,
}

/// `Annotation: KW_ANNOTATION number ',' '[' x ',' y ']' text`.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub number: f64,
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub line: u32,
}

/// What `MermaidParseError` carries: the joined message and the position of
/// the first error (lexer errors first, as mermaid prints them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFailure {
    pub message: String,
    pub line: u32,
    /// 1-based; 0 when Chevrotain has no position (the EOF token).
    pub column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    LBracket,
    Comma,
    RBracket,
    At,
    Slash,
    LParen,
    RParen,
    Dash,
    LBrace,
    RBrace,
    WardleyNumber,
    Arrow,
    LinkPort,
    LinkArrow,
    LinkLabel,
    Strategy,
    KwWardley,
    KwSize,
    KwEvolution,
    KwAnchor,
    KwComponent,
    KwLabel,
    KwInertia,
    KwEvolve,
    KwPipeline,
    KwNote,
    KwAnnotations,
    KwAnnotation,
    KwAccelerator,
    KwDeaccelerator,
    Name,
    AccDescr,
    AccTitle,
    Title,
    Int,
    Str,
    Id,
    Newline,
    Eof,
}

impl Kind {
    /// Chevrotain's `tokenType.name`: the keyword text for keywords, the
    /// terminal rule name otherwise.
    fn name(self) -> &'static str {
        match self {
            Kind::LBracket => "[",
            Kind::Comma => ",",
            Kind::RBracket => "]",
            Kind::At => "@",
            Kind::Slash => "/",
            Kind::LParen => "(",
            Kind::RParen => ")",
            Kind::Dash => "-",
            Kind::LBrace => "{",
            Kind::RBrace => "}",
            Kind::WardleyNumber => "WARDLEY_NUMBER",
            Kind::Arrow => "ARROW",
            Kind::LinkPort => "LINK_PORT",
            Kind::LinkArrow => "LINK_ARROW",
            Kind::LinkLabel => "LINK_LABEL",
            Kind::Strategy => "STRATEGY",
            Kind::KwWardley => "KW_WARDLEY",
            Kind::KwSize => "KW_SIZE",
            Kind::KwEvolution => "KW_EVOLUTION",
            Kind::KwAnchor => "KW_ANCHOR",
            Kind::KwComponent => "KW_COMPONENT",
            Kind::KwLabel => "KW_LABEL",
            Kind::KwInertia => "KW_INERTIA",
            Kind::KwEvolve => "KW_EVOLVE",
            Kind::KwPipeline => "KW_PIPELINE",
            Kind::KwNote => "KW_NOTE",
            Kind::KwAnnotations => "KW_ANNOTATIONS",
            Kind::KwAnnotation => "KW_ANNOTATION",
            Kind::KwAccelerator => "KW_ACCELERATOR",
            Kind::KwDeaccelerator => "KW_DEACCELERATOR",
            Kind::Name => "NAME_WITH_SPACES",
            Kind::AccDescr => "ACC_DESCR",
            Kind::AccTitle => "ACC_TITLE",
            Kind::Title => "TITLE",
            Kind::Int => "INT",
            Kind::Str => "STRING",
            Kind::Id => "ID",
            Kind::Newline => "NEWLINE",
            Kind::Eof => "EOF",
        }
    }
}

/// A set of token kinds, one bit per [`Kind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Set(u64);

impl Set {
    const EMPTY: Set = Set(0);

    const fn of(kinds: &[Kind]) -> Set {
        let mut bits = 0;
        let mut i = 0;
        while i < kinds.len() {
            bits |= 1 << kinds[i] as u64;
            i += 1;
        }
        Set(bits)
    }

    const fn with(self, other: Set) -> Set {
        Set(self.0 | other.0)
    }

    fn has(self, kind: Kind) -> bool {
        self.0 & (1 << kind as u64) != 0
    }

    fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// `TitleAndAccessibilities` starts.
const TITLES: Set = Set::of(&[Kind::AccDescr, Kind::AccTitle, Kind::Title]);
/// `EOL: NEWLINE+ | EOF`.
const EOL: Set = Set::of(&[Kind::Newline, Kind::Eof]);
/// The first tokens of a `Statement`.
const STATEMENTS: Set = Set::of(&[
    Kind::KwSize,
    Kind::KwEvolution,
    Kind::KwAnchor,
    Kind::KwComponent,
    Kind::Str,
    Kind::Id,
    Kind::Name,
    Kind::KwEvolve,
    Kind::KwPipeline,
    Kind::KwNote,
    Kind::KwAnnotations,
    Kind::KwAnnotation,
    Kind::KwAccelerator,
    Kind::KwDeaccelerator,
]);
/// The first tokens of an iteration of the `Wardley` loop, which is also
/// the follow set of `TitleAndAccessibilities` and `Statement`.
const LOOP: Set = Set::of(&[Kind::Newline]).with(TITLES).with(STATEMENTS);
/// What can follow a component's `]`, its `Label`, or its `Decorator`.
const AFTER_COMPONENT_LABEL: Set = Set::of(&[Kind::LParen, Kind::KwInertia]).with(EOL);
/// `CoordinateValue: WARDLEY_NUMBER | INT`.
const COORDINATE: Set = Set::of(&[Kind::WardleyNumber, Kind::Int]);

#[derive(Debug, Clone)]
struct Token {
    kind: Kind,
    /// Character range in the input.
    start: usize,
    end: usize,
    line: u32,
    column: u32,
}

#[derive(Debug, Clone)]
struct LexError {
    line: u32,
    column: u32,
    /// UTF-16 offset, as Chevrotain counts JS string indices.
    offset: usize,
    skipped: usize,
    first: char,
}

/// A hidden terminal: matched and dropped.
const HIDDEN: Option<Kind> = None;

/// A token matcher: the end of the match at a position, if any.
type Matcher<'a> = fn(&Lexer<'a>, usize) -> Option<usize>;

/// The terminals after `STRATEGY` that are plain words, in grammar order.
const WORD_TERMINALS: [(Kind, &str); 14] = [
    (Kind::KwWardley, "wardley-beta"),
    (Kind::KwSize, "size"),
    (Kind::KwEvolution, "evolution"),
    (Kind::KwAnchor, "anchor"),
    (Kind::KwComponent, "component"),
    (Kind::KwLabel, "label"),
    (Kind::KwInertia, "inertia"),
    (Kind::KwEvolve, "evolve"),
    (Kind::KwPipeline, "pipeline"),
    (Kind::KwNote, "note"),
    (Kind::KwAnnotations, "annotations"),
    (Kind::KwAnnotation, "annotation"),
    (Kind::KwAccelerator, "accelerator"),
    (Kind::KwDeaccelerator, "deaccelerator"),
];

struct Lexer<'a> {
    chars: &'a [char],
    /// UTF-16 offset of each character, plus one past the end.
    utf16: Vec<usize>,
}

impl<'a> Lexer<'a> {
    fn new(chars: &'a [char]) -> Lexer<'a> {
        let mut utf16 = Vec::with_capacity(chars.len() + 1);
        let mut offset = 0;
        for c in chars {
            utf16.push(offset);
            offset += c.len_utf16();
        }
        utf16.push(offset);
        Lexer { chars, utf16 }
    }

    fn at(&self, i: usize) -> Option<char> {
        self.chars.get(i).copied()
    }

    fn starts_with(&self, i: usize, s: &str) -> bool {
        s.chars()
            .enumerate()
            .all(|(k, c)| self.at(i + k) == Some(c))
    }

    fn literal(&self, i: usize, word: &str) -> Option<usize> {
        self.starts_with(i, word).then(|| i + word.chars().count())
    }

    /// The end of `[^\n\r]*` from `i`.
    fn line_end(&self, i: usize) -> usize {
        let mut j = i;
        while let Some(c) = self.at(j)
            && c != '\n'
            && c != '\r'
        {
            j += 1;
        }
        j
    }

    /// `(?:[^\n\r]*?(?=%%)|[^\n\r]*)` from `i`: up to the first `%%` on
    /// the line when there is one, else the rest of the line.
    fn rest_of_line_before_comment(&self, i: usize) -> usize {
        let end = self.line_end(i);
        let mut j = i;
        while j + 1 < end {
            if self.chars[j] == '%' && self.chars[j + 1] == '%' {
                return j;
            }
            j += 1;
        }
        end
    }

    /// `\r?\n`.
    fn newline(&self, i: usize) -> Option<usize> {
        match self.at(i) {
            Some('\n') => Some(i + 1),
            Some('\r') if self.at(i + 1) == Some('\n') => Some(i + 2),
            _ => None,
        }
    }

    /// `[ \t]+`.
    fn whitespace(&self, i: usize) -> Option<usize> {
        let mut j = i;
        while matches!(self.at(j), Some(' ') | Some('\t')) {
            j += 1;
        }
        (j > i).then_some(j)
    }

    fn blanks(&self, i: usize) -> usize {
        self.whitespace(i).unwrap_or(i)
    }

    /// `\s*` with JS whitespace.
    fn js_spaces(&self, i: usize) -> usize {
        let mut j = i;
        while let Some(c) = self.at(j)
            && js_is_whitespace(c)
        {
            j += 1;
        }
        j
    }

    fn digits(&self, i: usize) -> usize {
        let mut j = i;
        while self.at(j).is_some_and(|c| c.is_ascii_digit()) {
            j += 1;
        }
        j
    }

    /// `WARDLEY_NUMBER`: `[0-9]+\.[0-9]+`, greedy on both runs.
    fn wardley_number(&self, i: usize) -> Option<usize> {
        let int_end = self.digits(i);
        if int_end == i || self.at(int_end) != Some('.') {
            return None;
        }
        let frac_end = self.digits(int_end + 1);
        (frac_end > int_end + 1).then_some(frac_end)
    }

    /// `ARROW`: `->`.
    fn arrow(&self, i: usize) -> Option<usize> {
        self.literal(i, "->")
    }

    /// `LINK_PORT`: `'+<>' | '+>' | '+<'`.
    fn link_port(&self, i: usize) -> Option<usize> {
        self.literal(i, "+<>")
            .or_else(|| self.literal(i, "+>"))
            .or_else(|| self.literal(i, "+<"))
    }

    /// `LINK_ARROW`: `-->`, `-.->`, `>`, or `+'...'` followed by `<>`, `<`,
    /// or `>`. The quoted part is `[^']*`, which may span lines.
    fn link_arrow(&self, i: usize) -> Option<usize> {
        if let Some(end) = self
            .literal(i, "-->")
            .or_else(|| self.literal(i, "-.->"))
            .or_else(|| self.literal(i, ">"))
        {
            return Some(end);
        }
        if !self.starts_with(i, "+'") {
            return None;
        }
        let mut j = i + 2;
        while self.at(j)? != '\'' {
            j += 1;
        }
        let after = j + 1;
        self.literal(after, "<>")
            .or_else(|| self.literal(after, "<"))
            .or_else(|| self.literal(after, ">"))
    }

    /// `LINK_LABEL`: `;[^\n\r]+`.
    fn link_label(&self, i: usize) -> Option<usize> {
        if self.at(i) != Some(';') {
            return None;
        }
        let end = self.line_end(i + 1);
        (end > i + 1).then_some(end)
    }

    /// `STRATEGY`: `'build' | 'buy' | 'outsource' | 'market'`.
    fn strategy(&self, i: usize) -> Option<usize> {
        ["build", "buy", "outsource", "market"]
            .iter()
            .find_map(|w| self.literal(i, w))
    }

    /// `[A-Za-z0-9_()&]|-(?!>)`: one name character.
    fn name_char(&self, i: usize) -> bool {
        match self.at(i) {
            Some(c) if c.is_ascii_alphanumeric() || matches!(c, '_' | '(' | ')' | '&') => true,
            Some('-') => self.at(i + 1) != Some('>'),
            _ => false,
        }
    }

    fn name_chars(&self, i: usize) -> usize {
        let mut j = i;
        while self.name_char(j) {
            j += 1;
        }
        j
    }

    /// `NAME_WITH_SPACES`: `(?!title\s|accTitle|accDescr)[A-Za-z]` and name
    /// characters, then any number of blank-separated words that start with
    /// a letter or `(`. Nothing follows the pattern, so the greedy path is
    /// the match.
    fn name_with_spaces(&self, i: usize) -> Option<usize> {
        let title_ahead = self.starts_with(i, "title")
            && self.at(i + "title".len()).is_some_and(js_is_whitespace);
        if title_ahead || self.starts_with(i, "accTitle") || self.starts_with(i, "accDescr") {
            return None;
        }
        if !self.at(i).is_some_and(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        let mut end = self.name_chars(i + 1);
        while let Some(blank_end) = self.whitespace(end)
            && self
                .at(blank_end)
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '(')
        {
            end = self.name_chars(blank_end + 1);
        }
        Some(end)
    }

    /// `[\t ]*accDescr(?:[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)|\s*{([^}]*)})`.
    fn acc_descr(&self, i: usize) -> Option<usize> {
        let i = self.blanks(i);
        if !self.starts_with(i, "accDescr") {
            return None;
        }
        let j = i + "accDescr".len();
        let after_blanks = self.blanks(j);
        if self.at(after_blanks) == Some(':') {
            return Some(self.rest_of_line_before_comment(after_blanks + 1));
        }
        let brace = self.js_spaces(j);
        if self.at(brace) != Some('{') {
            return None;
        }
        let mut k = brace + 1;
        while let Some(c) = self.at(k) {
            if c == '}' {
                return Some(k + 1);
            }
            k += 1;
        }
        None
    }

    /// `[\t ]*accTitle[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)`.
    fn acc_title(&self, i: usize) -> Option<usize> {
        let i = self.blanks(i);
        if !self.starts_with(i, "accTitle") {
            return None;
        }
        let colon = self.blanks(i + "accTitle".len());
        (self.at(colon) == Some(':')).then(|| self.rest_of_line_before_comment(colon + 1))
    }

    /// `[\t ]*title(?:[\t ][^\n\r]*?(?=%%)|[\t ][^\n\r]*|)`.
    fn title(&self, i: usize) -> Option<usize> {
        let i = self.blanks(i);
        if !self.starts_with(i, "title") {
            return None;
        }
        let j = i + "title".len();
        match self.at(j) {
            Some(' ') | Some('\t') => Some(self.rest_of_line_before_comment(j + 1)),
            _ => Some(j),
        }
    }

    /// `INT`: `0|[1-9][0-9]*(?!\.)`. The lookahead belongs to the second
    /// alternative only, so `0` matches before a dot, and a longer run
    /// before a dot backtracks by one digit as the regex engine does.
    fn int(&self, i: usize) -> Option<usize> {
        match self.at(i)? {
            '0' => Some(i + 1),
            '1'..='9' => {
                let end = self.digits(i);
                if self.at(end) != Some('.') {
                    Some(end)
                } else if end - i > 1 {
                    Some(end - 1)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// `"([^"\\]|\\.)*"|'([^'\\]|\\.)*'`; `.` excludes line terminators.
    fn string(&self, i: usize) -> Option<usize> {
        let quote = self.at(i)?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let mut j = i + 1;
        loop {
            match self.at(j)? {
                c if c == quote => return Some(j + 1),
                '\\' => match self.at(j + 1)? {
                    '\n' | '\r' | '\u{2028}' | '\u{2029}' => return None,
                    _ => j += 2,
                },
                _ => j += 1,
            }
        }
    }

    /// `ID`: `[\w]([-\w]*\w)?` with ASCII `\w`: the longest run of word
    /// characters and dashes that starts and ends with a word character.
    fn id(&self, i: usize) -> Option<usize> {
        let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
        if !self.at(i).is_some_and(word) {
            return None;
        }
        let mut j = i + 1;
        let mut last_word = j;
        while let Some(c) = self.at(j)
            && (word(c) || c == '-')
        {
            j += 1;
            if word(c) {
                last_word = j;
            }
        }
        Some(last_word)
    }

    /// `(?:\r?\n|(?!\S))` after a block: a newline, the end, or whitespace.
    fn block_end(&self, i: usize) -> Option<usize> {
        if let Some(end) = self.newline(i) {
            return Some(end);
        }
        match self.at(i) {
            None => Some(i),
            Some(c) if js_is_whitespace(c) => Some(i),
            Some(_) => None,
        }
    }

    /// `---[\t ]*\r?\n(?:[\S\s]*?\r?\n)?---(?:\r?\n|(?!\S))`.
    fn yaml(&self, i: usize) -> Option<usize> {
        if !self.starts_with(i, "---") {
            return None;
        }
        let first_newline = self.newline(self.blanks(i + 3))?;
        let mut k = first_newline;
        loop {
            if self.starts_with(k, "---")
                && (k == first_newline || self.at(k - 1) == Some('\n'))
                && let Some(end) = self.block_end(k + 3)
            {
                return Some(end);
            }
            k += 1;
            if k > self.chars.len() {
                return None;
            }
        }
    }

    /// `[\t ]*%%{[\S\s]*?}%%(?:\r?\n|(?!\S))`; `WS` comes first in the
    /// token order, so the prefix never matters.
    fn directive(&self, i: usize) -> Option<usize> {
        if !self.starts_with(i, "%%{") {
            return None;
        }
        let mut k = i + 3;
        while k + 3 <= self.chars.len() {
            if self.starts_with(k, "}%%")
                && let Some(end) = self.block_end(k + 3)
            {
                return Some(end);
            }
            k += 1;
        }
        None
    }

    /// `[\t ]*%%[^\n\r]*`.
    fn comment(&self, i: usize) -> Option<usize> {
        self.starts_with(i, "%%").then(|| self.line_end(i + 2))
    }

    /// `-` with its `LONGER_ALT`s: the first of `ARROW`, `LINK_ARROW`, and
    /// `YAML` that matches a longer image wins.
    fn dash(&self, i: usize) -> Option<(Option<Kind>, usize)> {
        let end = self.literal(i, "-")?;
        let longer: [(Option<Kind>, Matcher<'a>); 3] = [
            (Some(Kind::Arrow), Lexer::arrow),
            (Some(Kind::LinkArrow), Lexer::link_arrow),
            (HIDDEN, Lexer::yaml),
        ];
        Some(
            longer
                .iter()
                .find_map(|(kind, m)| m(self, i).filter(|e| *e > end).map(|e| (*kind, e)))
                .unwrap_or((Some(Kind::Dash), end)),
        )
    }

    /// The first token type that matches at `i`, in Chevrotain's order.
    fn next(&self, i: usize) -> Option<(Option<Kind>, usize)> {
        let punctuation = [
            (Kind::LBracket, '['),
            (Kind::Comma, ','),
            (Kind::RBracket, ']'),
            (Kind::At, '@'),
            (Kind::Slash, '/'),
            (Kind::LParen, '('),
            (Kind::RParen, ')'),
        ];
        let c = self.at(i)?;
        if let Some((kind, _)) = punctuation.iter().find(|(_, p)| *p == c) {
            return Some((Some(*kind), i + 1));
        }
        if c == '-' {
            return self.dash(i);
        }
        if c == '{' {
            return Some((Some(Kind::LBrace), i + 1));
        }
        if c == '}' {
            return Some((Some(Kind::RBrace), i + 1));
        }
        let before_words: [(Option<Kind>, Matcher<'a>); 6] = [
            (Some(Kind::WardleyNumber), Lexer::wardley_number),
            (Some(Kind::Arrow), Lexer::arrow),
            (Some(Kind::LinkPort), Lexer::link_port),
            (Some(Kind::LinkArrow), Lexer::link_arrow),
            (Some(Kind::LinkLabel), Lexer::link_label),
            (Some(Kind::Strategy), Lexer::strategy),
        ];
        if let Some(found) = before_words
            .iter()
            .find_map(|(kind, m)| m(self, i).map(|end| (*kind, end)))
        {
            return Some(found);
        }
        if let Some(found) = WORD_TERMINALS
            .iter()
            .find_map(|(kind, word)| self.literal(i, word).map(|end| (Some(*kind), end)))
        {
            return Some(found);
        }
        let after_words: [(Option<Kind>, Matcher<'a>); 12] = [
            (Some(Kind::Name), Lexer::name_with_spaces),
            (HIDDEN, Lexer::whitespace),
            (Some(Kind::AccDescr), Lexer::acc_descr),
            (Some(Kind::AccTitle), Lexer::acc_title),
            (Some(Kind::Title), Lexer::title),
            (Some(Kind::Int), Lexer::int),
            (Some(Kind::Str), Lexer::string),
            (Some(Kind::Id), Lexer::id),
            (Some(Kind::Newline), Lexer::newline),
            (HIDDEN, Lexer::yaml),
            (HIDDEN, Lexer::directive),
            (HIDDEN, Lexer::comment),
        ];
        after_words
            .iter()
            .find_map(|(kind, m)| m(self, i).map(|end| (*kind, end)))
    }

    fn tokenize(&self) -> (Vec<Token>, Vec<LexError>) {
        let mut tokens = Vec::new();
        let mut errors = Vec::new();
        let mut i = 0;
        let mut line = 1;
        let mut line_start = 0;
        let mut pending: Option<LexError> = None;
        let is_line_break = |k: usize| {
            self.chars[k] == '\n' || (self.chars[k] == '\r' && self.at(k + 1) != Some('\n'))
        };
        while i < self.chars.len() {
            match self.next(i) {
                Some((kind, end)) => {
                    if let Some(err) = pending.take() {
                        errors.push(err);
                    }
                    if let Some(kind) = kind {
                        tokens.push(Token {
                            kind,
                            start: i,
                            end,
                            line,
                            column: (self.utf16[i] - self.utf16[line_start] + 1) as u32,
                        });
                    }
                    for k in i..end {
                        if is_line_break(k) {
                            line += 1;
                            line_start = k + 1;
                        }
                    }
                    i = end;
                }
                None => {
                    match &mut pending {
                        Some(err) => err.skipped += 1,
                        None => {
                            pending = Some(LexError {
                                line,
                                column: (self.utf16[i] - self.utf16[line_start] + 1) as u32,
                                offset: self.utf16[i],
                                skipped: 1,
                                first: self.chars[i],
                            })
                        }
                    }
                    if is_line_break(i) {
                        line += 1;
                        line_start = i + 1;
                    }
                    i += 1;
                }
            }
        }
        if let Some(err) = pending {
            errors.push(err);
        }
        tokens.push(Token {
            kind: Kind::Eof,
            start: self.chars.len(),
            end: self.chars.len(),
            line: 0,
            column: 0,
        });
        (tokens, errors)
    }
}

struct ParseError {
    line: u32,
    column: u32,
    message: String,
}

/// A recognition exception on its way up the rule stack.
struct Thrown;

type Parsed<T> = Result<T, Thrown>;

/// The `name=(STRING | ID | NAME_WITH_SPACES)` alternatives as Chevrotain
/// lists them.
const NAME_SEQUENCES: [&str; 3] = ["STRING", "ID", "NAME_WITH_SPACES"];
/// `EvolutionStage` lists `ID` first.
const STAGE_NAME_SEQUENCES: [&str; 3] = ["ID", "STRING", "NAME_WITH_SPACES"];

struct Parser<'a> {
    chars: &'a [char],
    tokens: Vec<Token>,
    pos: usize,
    errors: Vec<ParseError>,
    /// The re-sync set of every rule on the stack: the tokens that can
    /// follow it in its caller. The top rule's is `EOF`.
    frames: Vec<Set>,
}

impl Parser<'_> {
    fn la(&self, k: usize) -> &Token {
        &self.tokens[(self.pos + k - 1).min(self.tokens.len() - 1)]
    }

    fn kind(&self, k: usize) -> Kind {
        self.la(k).kind
    }

    fn image(&self, t: &Token) -> String {
        self.chars[t.start..t.end].iter().collect()
    }

    fn line(&self) -> u32 {
        self.la(1).line
    }

    fn advance(&mut self) -> Token {
        let t = self.la(1).clone();
        if t.kind != Kind::Eof {
            self.pos += 1;
        }
        t
    }

    fn error_at(&mut self, t: &Token, message: String) {
        self.errors.push(ParseError {
            line: t.line,
            column: t.column,
            message,
        });
    }

    /// `MismatchedTokenException` from Langium's parser error provider.
    fn mismatch_message(&self, expected: Kind, actual: &Token) -> String {
        format!(
            "Expecting token of type '{}' but found `{}`.",
            expected.name(),
            self.image(actual)
        )
    }

    /// `CONSUME` with Chevrotain's in-rule recovery. `follows` is what can
    /// come after the token in the full rule stack, for the single token
    /// insertion. Returns `None` for an inserted token.
    fn consume(&mut self, kind: Kind, follows: Set) -> Parsed<Option<Token>> {
        if self.kind(1) == kind {
            return Ok(Some(self.advance()));
        }
        let actual = self.la(1).clone();
        let message = self.mismatch_message(kind, &actual);
        self.error_at(&actual, message);
        if !follows.is_empty() && follows.has(self.kind(1)) {
            Ok(None)
        } else if self.kind(2) == kind {
            self.advance();
            Ok(Some(self.advance()))
        } else {
            Err(Thrown)
        }
    }

    /// `NoViableAltException` from Chevrotain's default error provider,
    /// one line per token sequence.
    fn no_viable_alt(&mut self, sequences: &[&str]) -> Thrown {
        let t = self.la(1).clone();
        let mut message = "Expecting: one of these possible Token sequences:".to_string();
        for (i, seq) in sequences.iter().enumerate() {
            let _ = write!(message, "\n  {}. [{seq}]", i + 1);
        }
        let _ = write!(message, "\nbut found: '{}'", self.image(&t));
        self.error_at(&t, message);
        Thrown
    }

    /// `EarlyExitException`: an `AT_LEAST_ONE` that cannot start.
    fn early_exit(&mut self, paths: &str) -> Thrown {
        let t = self.la(1).clone();
        let message = format!(
            "Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <{paths}>\nbut found: '{}'",
            self.image(&t)
        );
        self.error_at(&t, message);
        Thrown
    }

    /// `findReSyncTokenType`: the kind of the first token from `LA(1)`
    /// that can follow any rule on the stack. `EOF` follows the top rule,
    /// so the scan ends.
    fn resync_kind(&self) -> Kind {
        let union = self.frames.iter().fold(Set::EMPTY, |acc, s| acc.with(*s));
        let mut k = 1;
        while !union.has(self.kind(k)) && self.kind(k) != Kind::Eof {
            k += 1;
        }
        self.kind(k)
    }

    /// A rule invocation with `invokeRuleCatch`: on a thrown error, skip to
    /// the re-sync token when it can follow this rule, else rethrow.
    /// Returns `None` for a recovered rule.
    fn rule<T>(
        &mut self,
        resync: Set,
        body: impl FnOnce(&mut Self) -> Parsed<T>,
    ) -> Parsed<Option<T>> {
        self.frames.push(resync);
        let result = match body(self) {
            Ok(value) => Ok(Some(value)),
            Err(Thrown) => {
                let target = self.resync_kind();
                if resync.has(target) {
                    while self.kind(1) != target {
                        self.advance();
                    }
                    Ok(None)
                } else {
                    Err(Thrown)
                }
            }
        };
        self.frames.pop();
        result
    }

    /// A `MANY` (`mandatory` is `None`) or `AT_LEAST_ONE` loop. `starts`
    /// is the iteration lookahead. `after` is the terminal right after the
    /// loop and what can follow that terminal, for the in-repetition
    /// recovery at the loop exit.
    fn repetition(
        &mut self,
        mandatory: Option<&str>,
        starts: fn(&Self) -> bool,
        after: Option<(Kind, Set)>,
        body: &mut dyn FnMut(&mut Self) -> Parsed<()>,
    ) -> Parsed<()> {
        loop {
            if let Some(paths) = mandatory {
                if !starts(self) {
                    return Err(self.early_exit(paths));
                }
                body(self)?;
            }
            let mut not_stuck = true;
            while not_stuck && starts(self) {
                let before = self.pos;
                body(self)?;
                not_stuck = self.pos > before;
            }
            // `attemptInRepetitionRecovery` and
            // `shouldInRepetitionRecoveryBeTried`.
            let Some((expected, follows)) = after else {
                return Ok(());
            };
            if !not_stuck || self.kind(1) == expected {
                return Ok(());
            }
            let can_insert = !follows.is_empty() && follows.has(self.kind(1));
            if can_insert || self.kind(2) == expected {
                return Ok(());
            }
            // `tryInRepetitionRecovery`.
            let resync = self.resync_kind();
            let saved = self.pos;
            let offending = self.la(1).clone();
            loop {
                let current = self.kind(1);
                if current == expected {
                    let message = self.mismatch_message(expected, &offending);
                    self.error_at(&offending, message);
                    return Ok(());
                }
                if starts(self) {
                    let message = self.mismatch_message(expected, &offending);
                    self.error_at(&offending, message);
                    break;
                }
                if current == resync {
                    self.pos = saved;
                    return Ok(());
                }
                self.advance();
            }
        }
    }

    /// `EOL: NEWLINE+ | EOF`. The `NEWLINE+` is greedy: the LL(*)
    /// lookahead resolves its conflict with the caller's follow set to the
    /// iteration.
    fn eol(&mut self, resync: Set) -> Parsed<()> {
        self.rule(resync, |p| match p.kind(1) {
            Kind::Newline => {
                while p.kind(1) == Kind::Newline {
                    p.advance();
                }
                Ok(())
            }
            Kind::Eof => Ok(()),
            _ => Err(p.no_viable_alt(&["NEWLINE", "EOF"])),
        })
        .map(|_| ())
    }

    /// `name=(STRING | ID | NAME_WITH_SPACES)` in the order `sequences`
    /// lists, converted.
    fn name(&mut self, sequences: &[&str; 3]) -> Parsed<String> {
        match self.kind(1) {
            Kind::Str => {
                let t = self.advance();
                Ok(convert_string(&self.image(&t)))
            }
            Kind::Id | Kind::Name => {
                // `convertID` drops a leading `^`, which `ID` cannot start
                // with; a name keeps its image.
                let t = self.advance();
                Ok(self.image(&t))
            }
            _ => Err(self.no_viable_alt(sequences)),
        }
    }

    fn number(&mut self, kind: Kind, follows: Set) -> Parsed<f64> {
        Ok(self
            .consume(kind, follows)?
            .map_or(f64::NAN, |t| convert_number(&self.image(&t))))
    }

    /// `'[' a=WARDLEY_NUMBER ',' b=WARDLEY_NUMBER ']'` with `after` the
    /// follow set of the `]`.
    fn pair(&mut self, after: Set) -> Parsed<(f64, f64)> {
        let wn = Set::of(&[Kind::WardleyNumber]);
        self.consume(Kind::LBracket, wn)?;
        let a = self.number(Kind::WardleyNumber, Set::of(&[Kind::Comma]))?;
        self.consume(Kind::Comma, wn)?;
        let b = self.number(Kind::WardleyNumber, Set::of(&[Kind::RBracket]))?;
        self.consume(Kind::RBracket, after)?;
        Ok((a, b))
    }

    /// `CoordinateValue returns number: WARDLEY_NUMBER | INT`.
    fn coordinate(&mut self, resync: Set) -> Parsed<f64> {
        Ok(self
            .rule(resync, |p| {
                if COORDINATE.has(p.kind(1)) {
                    let t = p.advance();
                    Ok(convert_number(&p.image(&t)))
                } else {
                    Err(p.no_viable_alt(&["WARDLEY_NUMBER", "INT"]))
                }
            })?
            .unwrap_or(f64::NAN))
    }

    /// `'[' x=CoordinateValue ',' y=CoordinateValue ']'`.
    fn coordinates(&mut self, after: Set) -> Parsed<(f64, f64)> {
        self.consume(Kind::LBracket, COORDINATE)?;
        let x = self.coordinate(Set::of(&[Kind::Comma]))?;
        self.consume(Kind::Comma, COORDINATE)?;
        let y = self.coordinate(Set::of(&[Kind::RBracket]))?;
        self.consume(Kind::RBracket, after)?;
        Ok((x, y))
    }

    /// `Label`; `after` is what follows it in its caller.
    fn label(&mut self, after: Set) -> Parsed<Option<LabelOffset>> {
        self.rule(after, |p| {
            let signed = Set::of(&[Kind::Dash, Kind::Int]);
            p.advance();
            p.consume(Kind::LBracket, signed)?;
            let neg_x = p.kind(1) == Kind::Dash;
            if neg_x {
                p.advance();
            }
            let x = p.number(Kind::Int, Set::of(&[Kind::Comma]))?;
            p.consume(Kind::Comma, signed)?;
            let neg_y = p.kind(1) == Kind::Dash;
            if neg_y {
                p.advance();
            }
            let y = p.number(Kind::Int, Set::of(&[Kind::RBracket]))?;
            p.consume(Kind::RBracket, after)?;
            let sign = |neg: bool| if neg { -1.0 } else { 1.0 };
            Ok(LabelOffset {
                x: sign(neg_x) * x,
                y: sign(neg_y) * y,
            })
        })
    }

    /// `TitleAndAccessibilities: ((accDescr=ACC_DESCR | accTitle=ACC_TITLE
    /// | title=TITLE) EOL)+`. The iteration lookahead reads the `EOL` too,
    /// so a title token that is not followed by a newline or the end fails
    /// the first iteration with an `EarlyExitException`.
    fn title_and_accessibilities(&mut self, ast: &mut Ast) -> Parsed<()> {
        self.rule(LOOP, |p| {
            p.repetition(
                Some("[ACC_DESCR] ,[ACC_TITLE] ,[TITLE]"),
                |p| TITLES.has(p.kind(1)) && EOL.has(p.kind(2)),
                None,
                &mut |p| {
                    let t = p.advance();
                    let image = p.image(&t);
                    match t.kind {
                        Kind::AccDescr => ast.acc_descr = Some(convert_acc_descr(&image)),
                        Kind::AccTitle => ast.acc_title = Some(convert_acc_title(&image)),
                        _ => ast.title = Some(convert_title(&image)),
                    }
                    p.eol(TITLES)
                },
            )
        })
        .map(|_| ())
    }

    /// `Size`.
    fn size(&mut self) -> Parsed<Size> {
        self.advance();
        let int = Set::of(&[Kind::Int]);
        self.consume(Kind::LBracket, int)?;
        let width = self.number(Kind::Int, Set::of(&[Kind::Comma]))?;
        self.consume(Kind::Comma, int)?;
        let height = self.number(Kind::Int, Set::of(&[Kind::RBracket]))?;
        self.consume(Kind::RBracket, EOL)?;
        self.eol(Set::EMPTY)?;
        Ok(Size { width, height })
    }

    /// `EvolutionStage`; `after` is what follows it in `Evolution`.
    fn stage(&mut self, after: Set) -> Parsed<Option<EvolutionStage>> {
        self.rule(after, |p| {
            let name = p.name(&STAGE_NAME_SEQUENCES)?;
            let boundary = if p.kind(1) == Kind::At {
                p.advance();
                Some(p.number(Kind::WardleyNumber, Set::of(&[Kind::Slash]).with(after))?)
            } else {
                None
            };
            let second_name = if p.kind(1) == Kind::Slash {
                p.advance();
                Some(p.name(&STAGE_NAME_SEQUENCES)?)
            } else {
                None
            };
            Ok(EvolutionStage {
                name,
                boundary,
                second_name,
            })
        })
    }

    /// `Evolution: KW_EVOLUTION stages+=EvolutionStage (ARROW
    /// stages+=EvolutionStage)+ EOL`.
    fn evolution(&mut self) -> Parsed<Vec<EvolutionStage>> {
        self.advance();
        let mut stages = Vec::new();
        stages.extend(self.stage(Set::of(&[Kind::Arrow]))?);
        let in_loop = Set::of(&[Kind::Arrow]).with(EOL);
        self.repetition(
            Some("[ARROW]"),
            |p| p.kind(1) == Kind::Arrow,
            None,
            &mut |p| {
                p.advance();
                stages.extend(p.stage(in_loop)?);
                Ok(())
            },
        )?;
        self.eol(Set::EMPTY)?;
        Ok(stages)
    }

    /// `Anchor`, `Accelerator`, and `Deaccelerator`: a keyword, a name, and
    /// a pair.
    fn placed(&mut self) -> Parsed<Placed> {
        let line = self.line();
        self.advance();
        let name = self.name(&NAME_SEQUENCES)?;
        let (first, second) = self.pair(EOL)?;
        self.eol(Set::EMPTY)?;
        Ok(Placed {
            name,
            first,
            second,
            line,
        })
    }

    /// `Component`.
    fn component(&mut self) -> Parsed<Component> {
        let line = self.line();
        self.advance();
        let name = self.name(&NAME_SEQUENCES)?;
        let (visibility, evolution) =
            self.pair(Set::of(&[Kind::KwLabel]).with(AFTER_COMPONENT_LABEL))?;
        let label = if self.kind(1) == Kind::KwLabel {
            self.label(AFTER_COMPONENT_LABEL)?
        } else {
            None
        };
        // `Decorator?` by the LL(*) lookahead: the inertia alternative
        // also starts with `(`.
        let strategy = if self.kind(1) == Kind::LParen && self.kind(2) == Kind::Strategy {
            self.rule(AFTER_COMPONENT_LABEL, |p| {
                p.advance();
                let t = p.advance();
                p.consume(Kind::RParen, AFTER_COMPONENT_LABEL)?;
                Ok(p.image(&t))
            })?
        } else {
            None
        };
        let inertia = match self.kind(1) {
            Kind::KwInertia => {
                self.advance();
                true
            }
            Kind::LParen => {
                self.advance();
                let found = self
                    .consume(Kind::KwInertia, Set::of(&[Kind::RParen]))?
                    .is_some();
                self.consume(Kind::RParen, EOL)?;
                found
            }
            _ => false,
        };
        self.eol(Set::EMPTY)?;
        Ok(Component {
            name,
            visibility,
            evolution,
            label,
            strategy,
            inertia,
            line,
        })
    }

    /// `Link`.
    fn link(&mut self) -> Parsed<Link> {
        let from = self.name(&NAME_SEQUENCES)?;
        let optional = |p: &mut Self, kinds: &[Kind]| {
            kinds.contains(&p.kind(1)).then(|| {
                let t = p.advance();
                p.image(&t)
            })
        };
        let from_port = optional(self, &[Kind::LinkPort]);
        let arrow = optional(self, &[Kind::LinkArrow, Kind::Arrow]);
        let to = self.name(&NAME_SEQUENCES)?;
        let to_port = optional(self, &[Kind::LinkPort]);
        let link_label = optional(self, &[Kind::LinkLabel]).map(|image| convert_link_label(&image));
        self.eol(Set::EMPTY)?;
        Ok(Link {
            from,
            from_port,
            arrow,
            to,
            to_port,
            link_label,
        })
    }

    /// `Evolve`.
    fn evolve(&mut self) -> Parsed<Evolve> {
        let line = self.line();
        self.advance();
        let component = self.name(&NAME_SEQUENCES)?;
        let target = self.number(Kind::WardleyNumber, EOL)?;
        self.eol(Set::EMPTY)?;
        Ok(Evolve {
            component,
            target,
            line,
        })
    }

    /// `PipelineComponent`.
    fn pipeline_component(&mut self) -> Parsed<PipelineComponent> {
        let line = self.line();
        self.advance();
        let name = self.name(&NAME_SEQUENCES)?;
        let wn = Set::of(&[Kind::WardleyNumber]);
        self.consume(Kind::LBracket, wn)?;
        let evolution = self.number(Kind::WardleyNumber, Set::of(&[Kind::RBracket]))?;
        self.consume(Kind::RBracket, Set::of(&[Kind::KwLabel]).with(EOL))?;
        let label = if self.kind(1) == Kind::KwLabel {
            self.label(EOL)?
        } else {
            None
        };
        self.eol(Set::EMPTY)?;
        Ok(PipelineComponent {
            name,
            evolution,
            label,
            line,
        })
    }

    /// `Pipeline: KW_PIPELINE parent '{' NEWLINE+ components+=
    /// PipelineComponent+ '}' EOL`.
    fn pipeline(&mut self) -> Parsed<Pipeline> {
        let line = self.line();
        self.advance();
        let parent = self.name(&NAME_SEQUENCES)?;
        self.consume(Kind::LBrace, Set::of(&[Kind::Newline]))?;
        self.repetition(
            Some("[NEWLINE]"),
            |p| p.kind(1) == Kind::Newline,
            None,
            &mut |p| {
                p.advance();
                Ok(())
            },
        )?;
        let mut components = Vec::new();
        let resync = Set::of(&[Kind::KwComponent, Kind::RBrace]);
        self.repetition(
            Some("[KW_COMPONENT]"),
            |p| p.kind(1) == Kind::KwComponent,
            Some((Kind::RBrace, EOL)),
            &mut |p| {
                components.extend(p.rule(resync, Self::pipeline_component)?);
                Ok(())
            },
        )?;
        self.consume(Kind::RBrace, EOL)?;
        self.eol(Set::EMPTY)?;
        Ok(Pipeline {
            parent,
            components,
            line,
        })
    }

    /// `Note: KW_NOTE text=STRING '[' visibility ',' evolution ']'`.
    fn note(&mut self) -> Parsed<Placed> {
        let line = self.line();
        self.advance();
        let name = self
            .consume(Kind::Str, Set::of(&[Kind::LBracket]))?
            .map_or_else(String::new, |t| convert_string(&self.image(&t)));
        let (first, second) = self.pair(EOL)?;
        self.eol(Set::EMPTY)?;
        Ok(Placed {
            name,
            first,
            second,
            line,
        })
    }

    /// `Annotations: KW_ANNOTATIONS '[' x ',' y ']'`.
    fn annotations(&mut self) -> Parsed<Coordinates> {
        let line = self.line();
        self.advance();
        let (x, y) = self.coordinates(EOL)?;
        self.eol(Set::EMPTY)?;
        Ok(Coordinates { x, y, line })
    }

    /// `Annotation: KW_ANNOTATION number=INT ',' '[' x ',' y ']'
    /// text=STRING`.
    fn annotation(&mut self) -> Parsed<Annotation> {
        let line = self.line();
        self.advance();
        let number = self.number(Kind::Int, Set::of(&[Kind::Comma]))?;
        self.consume(Kind::Comma, Set::of(&[Kind::LBracket]))?;
        let (x, y) = self.coordinates(Set::of(&[Kind::Str]))?;
        let text = self
            .consume(Kind::Str, EOL)?
            .map_or_else(String::new, |t| convert_string(&self.image(&t)));
        self.eol(Set::EMPTY)?;
        Ok(Annotation {
            number,
            x,
            y,
            text,
            line,
        })
    }

    /// `fragment Statement`: one alternative per first token, each a rule
    /// that nothing follows inside `Statement`.
    fn statement(&mut self, ast: &mut Ast) -> Parsed<()> {
        self.rule(LOOP, |p| {
            let e = Set::EMPTY;
            match p.kind(1) {
                Kind::KwSize => {
                    if let Some(size) = p.rule(e, Self::size)? {
                        ast.size = Some(size);
                    }
                }
                Kind::KwEvolution => {
                    if let Some(stages) = p.rule(e, Self::evolution)? {
                        ast.evolution = Some(stages);
                    }
                }
                Kind::KwAnchor => ast.anchors.extend(p.rule(e, Self::placed)?),
                Kind::KwComponent => ast.components.extend(p.rule(e, Self::component)?),
                Kind::KwEvolve => ast.evolves.extend(p.rule(e, Self::evolve)?),
                Kind::KwPipeline => ast.pipelines.extend(p.rule(e, Self::pipeline)?),
                Kind::KwNote => ast.notes.extend(p.rule(e, Self::note)?),
                Kind::KwAnnotations => ast.annotations.extend(p.rule(e, Self::annotations)?),
                Kind::KwAnnotation => ast.annotation.extend(p.rule(e, Self::annotation)?),
                Kind::KwAccelerator => ast.accelerators.extend(p.rule(e, Self::placed)?),
                Kind::KwDeaccelerator => ast.deaccelerators.extend(p.rule(e, Self::placed)?),
                _ => ast.links.extend(p.rule(e, Self::link)?),
            }
            Ok(())
        })
        .map(|_| ())
    }

    /// The `Wardley` entry rule: `NEWLINE* KW_WARDLEY (NEWLINE |
    /// TitleAndAccessibilities | Statement)*`, then Chevrotain's check that
    /// all input was parsed.
    fn wardley(&mut self) -> Ast {
        let mut ast = Ast::default();
        let _ = self.rule(Set::of(&[Kind::Eof]), |p| {
            p.repetition(
                None,
                |p| p.kind(1) == Kind::Newline,
                Some((Kind::KwWardley, LOOP)),
                &mut |p| {
                    p.advance();
                    Ok(())
                },
            )?;
            p.consume(Kind::KwWardley, LOOP)?;
            p.repetition(
                None,
                |p| LOOP.has(p.kind(1)),
                Some((Kind::Eof, Set::EMPTY)),
                &mut |p| match p.kind(1) {
                    Kind::Newline => {
                        p.advance();
                        Ok(())
                    }
                    k if TITLES.has(k) => p.title_and_accessibilities(&mut ast),
                    _ => p.statement(&mut ast),
                },
            )
        });
        if self.kind(1) != Kind::Eof {
            let t = self.la(1).clone();
            let message = format!("Expecting end of file but found `{}`.", self.image(&t));
            self.error_at(&t, message);
        }
        ast
    }
}

/// Langium's `convertString`: the quotes go, a backslash escape becomes
/// the control character for `b f n r t v 0` and the character itself
/// otherwise.
fn convert_string(image: &str) -> String {
    let inner: Vec<char> = image.chars().collect();
    let mut out = String::new();
    let mut i = 1;
    while i + 1 < inner.len() {
        let c = inner[i];
        if c == '\\' && i + 2 < inner.len() {
            i += 1;
            out.push(match inner[i] {
                'b' => '\u{8}',
                'f' => '\u{c}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'v' => '\u{b}',
                '0' => '\0',
                other => other,
            });
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// `convertNumber` and `convertInt`: `Number(input)` and `parseInt`, which
/// agree on the images `WARDLEY_NUMBER` and `INT` match.
fn convert_number(image: &str) -> f64 {
    image.parse().unwrap_or(f64::NAN)
}

/// `WardleyValueConverter`: `LINK_LABEL` is `input.substring(1).trim()`.
fn convert_link_label(image: &str) -> String {
    js_trim(image.get(1..).unwrap_or("")).to_string()
}

/// `[\t ]{2,}` to one space, after a JS `trim`.
fn collapse_blanks(s: &str) -> String {
    let mut out = String::new();
    let mut run = 0;
    for c in s.chars() {
        if c == ' ' || c == '\t' {
            run += 1;
            if run == 1 {
                out.push(c);
            } else if run == 2 {
                out.pop();
                out.push(' ');
            }
        } else {
            run = 0;
            out.push(c);
        }
    }
    out
}

pub(super) fn js_trim(s: &str) -> &str {
    s.trim_matches(js_is_whitespace)
}

/// The image from the first occurrence of `word`, as `RegExp.exec` finds
/// it.
fn from_word<'s>(image: &'s str, word: &str) -> &'s str {
    image.find(word).map_or("", |i| &image[i + word.len()..])
}

/// `AbstractMermaidValueConverter.runCommonConverter` with `titleRegex`
/// `/title([\t ][^\n\r]*|)/`: group 1 trimmed, blank runs collapsed.
fn convert_title(image: &str) -> String {
    collapse_blanks(js_trim(from_word(image, "title")))
}

/// `accessibilityTitleRegex` `/accTitle[\t ]*:([^\n\r]*)/`.
fn convert_acc_title(image: &str) -> String {
    let after = from_word(image, "accTitle")
        .split_once(':')
        .map(|(_, r)| r)
        .unwrap_or("");
    collapse_blanks(js_trim(after))
}

/// `accessibilityDescrRegex` `/accDescr(?:[\t ]*:([^\n\r]*)|\s*{([^}]*)})/`:
/// the single-line form like a title, the block form with every line
/// trimmed, blank runs collapsed, and blank lines dropped.
fn convert_acc_descr(image: &str) -> String {
    let after_keyword = from_word(image, "accDescr");
    let colon_form = after_keyword.trim_start_matches([' ', '\t']);
    if let Some(rest) = colon_form.strip_prefix(':') {
        return collapse_blanks(js_trim(rest));
    }
    let inner = colon_form
        .trim_start_matches(js_is_whitespace)
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or("");
    let mut lines: Vec<String> = Vec::new();
    for line in inner.split(['\n', '\r']) {
        let trimmed = js_trim(line);
        if trimmed.is_empty() {
            continue;
        }
        lines.push(collapse_blanks(trimmed));
    }
    lines.join("\n")
}

/// `parse('wardley', text)`: the AST, or the `MermaidParseError` text with
/// the position of its first error.
pub fn parse(text: &str) -> Result<Ast, ParseFailure> {
    let chars: Vec<char> = text.chars().collect();
    let lexer = Lexer::new(&chars);
    let (tokens, lex_errors) = lexer.tokenize();
    let mut parser = Parser {
        chars: &chars,
        tokens,
        pos: 0,
        errors: Vec::new(),
        frames: Vec::new(),
    };
    let ast = parser.wardley();
    if lex_errors.is_empty() && parser.errors.is_empty() {
        return Ok(ast);
    }
    let lexer_text = lex_errors
        .iter()
        .map(|e| {
            format!(
                "Lexer error on line {}, column {}: unexpected character: ->{}<- at offset: {}, skipped {} characters.",
                e.line, e.column, e.first, e.offset, e.skipped
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let parser_text = parser
        .errors
        .iter()
        .map(|e| {
            format!(
                "Parse error on line {}, column {}: {}",
                location(e.line),
                location(e.column),
                e.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let (line, column) = match (lex_errors.first(), parser.errors.first()) {
        (Some(e), _) => (e.line, e.column),
        (None, Some(e)) => (e.line, e.column),
        (None, None) => (0, 0),
    };
    Err(ParseFailure {
        message: format!("Parsing failed: {lexer_text} {parser_text}"),
        line,
        column,
    })
}

/// `formatLocation`: Chevrotain marks a missing position (the EOF token)
/// with `NaN`, printed as `?`.
fn location(v: u32) -> String {
    if v == 0 {
        "?".to_string()
    } else {
        v.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(text: &str) -> Ast {
        parse(text).unwrap_or_else(|e| panic!("{text:?}: {}", e.message))
    }

    #[test]
    fn statements_convert_their_values() {
        let ast = ok(concat!(
            "wardley-beta\ntitle  Tea   Shop \nsize [1100, 800]\n",
            "evolution Genesis@0.2 / Concept -> 'Custom' -> Product@1.0\n",
            "anchor Business [0.95, 0.63]\n",
            "component \"Mobile App\" [0.80, 0.85] label [-60, 10] (build) (inertia)\n",
            "A +'flow'> B +<; note text \n",
            "evolve Mobile App 0.9\n",
            "pipeline Business {\n component SQL DB [0.50] label [5, -5]\n}\n",
            "annotations [0.1, 50]\nannotation 2,[1, 0.5] 'x'\n",
        ));
        assert_eq!(ast.title.as_deref(), Some("Tea Shop"));
        assert_eq!(
            ast.size,
            Some(Size {
                width: 1100.0,
                height: 800.0
            })
        );
        let stages = ast.evolution.unwrap();
        assert_eq!(
            (
                stages[0].name.as_str(),
                stages[0].boundary,
                stages[0].second_name.as_deref()
            ),
            ("Genesis", Some(0.2), Some("Concept"))
        );
        assert_eq!(stages[1].name, "Custom");
        let c = &ast.components[0];
        assert_eq!(c.name, "Mobile App");
        assert_eq!(c.label, Some(LabelOffset { x: -60.0, y: 10.0 }));
        assert_eq!(c.strategy.as_deref(), Some("build"));
        assert!(c.inertia);
        let link = &ast.links[0];
        assert_eq!(link.arrow.as_deref(), Some("+'flow'>"));
        assert_eq!(link.to_port.as_deref(), Some("+<"));
        assert_eq!(link.link_label.as_deref(), Some("note text"));
        assert_eq!(ast.evolves[0].component, "Mobile App");
        let pc = &ast.pipelines[0].components[0];
        assert_eq!((pc.name.as_str(), pc.evolution), ("SQL DB", 0.5));
        assert_eq!(pc.label, Some(LabelOffset { x: 5.0, y: -5.0 }));
        assert_eq!((ast.annotations[0].x, ast.annotations[0].y), (0.1, 50.0));
        assert_eq!(ast.annotation[0].text, "x");
    }

    #[test]
    fn keyword_terminals_have_no_longer_alternative() {
        // `buyer` is `STRATEGY` then a name, so the component has no name.
        assert!(parse("wardley-beta\ncomponent buyer [0.5, 0.5]\n").is_err());
        let ast = ok("wardley-beta\nA-B -> C (x)\nfoo-->bar\n");
        assert_eq!(
            (ast.links[0].from.as_str(), ast.links[0].to.as_str()),
            ("A-B", "C (x)")
        );
        assert_eq!(
            (ast.links[1].from.as_str(), ast.links[1].to.as_str()),
            ("foo-", "bar")
        );
    }

    /// An input and the oracle's error message, line, and column.
    type OracleCase = (&'static str, Option<(&'static str, u32, u32)>);

    /// Inputs and `parse('wardley', input)` results from the oracle's
    /// `@mermaid-js/parser`: `None` when it parses, else the
    /// `MermaidParseError` text and the first error position.
    #[test]
    fn errors_match_the_langium_parser() {
        let cases: &[OracleCase] = &[
            ("wardley-beta\ncomponent A [0.5, 0.5]\n", None),
            (
                "wardley-beta\ntitle Tea Shop\nanchor Business [0.95, 0.63]\ncomponent Cup of Tea [0.79, 0.61]\nBusiness -> Cup of Tea\nevolve Cup of Tea 0.9\nnote \"hi there\" [0.3, 0.49]\n",
                None,
            ),
            (
                "wardley-beta\ncomponent \"Mobile App\" [0.80, 0.85] label [-60, 10] (build)\ncomponent Database [0.50, 0.45] (buy) (inertia)\ncomponent X [0.1, 0.2] inertia\n",
                None,
            ),
            (
                "wardley-beta\nA -> B\nA --> B\nA -> B; my label\nA -.-> B\nA +> B\nA +< B\nA +<> B\nA +'text'> B\nA +'t'< B\nA +'t'<> B\nA > B\nA+<>->B\nA B\n",
                Some((
                    "Parsing failed:  Parse error on line 14, column 4: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    14,
                    4,
                )),
            ),
            (
                "wardley-beta\nevolution Genesis / Concept -> Custom@0.4 / Emerging -> Product -> Commodity@1.0\n",
                None,
            ),
            (
                "wardley-beta\nsize [1100, 800]\npipeline Database {\n  component \"File System\" [0.25]\n  component SQL DB [0.50] label [5, -5]\n}\n",
                None,
            ),
            (
                "wardley-beta\nannotations [0.10, 0.90]\nannotation 1,[0.60, 0.65] \"Critical\"\nannotation 2,[1, 50] 'x'\naccelerator \"AI Adoption\" [0.55, 0.25]\ndeaccelerator Legacy [0.15, 0.75]\n",
                None,
            ),
            (
                "wardley-beta\naccTitle: hello\naccDescr: world\naccDescr {\n multi\n line\n}\ntitle   a   b  \n",
                None,
            ),
            ("wardley-beta\n  title x\n component A [0.5,0.5]\n", None),
            (
                "wardley-beta\ncomponent buyer [0.5, 0.5]\ncomponent components [0.5, 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 11: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: 'buy'\nParse error on line 2, column 17: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '['\nParse error on line 3, column 11: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: 'component'",
                    2,
                    11,
                )),
            ),
            (
                "wardley-beta\ncomponent real-time processing [0.55, 0.40]\nend-user -> real-time processing\n",
                None,
            ),
            (
                "wardley-beta\ncomponent _x [0.5, 0.5]\ncomponent 3D Printer [0.5, 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 3, column 11: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '3'\nParse error on line 3, column 22: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '['",
                    3,
                    11,
                )),
            ),
            (
                "wardley-beta\ncomponent A [1, 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 14: Expecting token of type 'WARDLEY_NUMBER' but found `1`.",
                    2,
                    14,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, 0.5\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 22: Expecting token of type ']' but found `\n`.",
                    2,
                    22,
                )),
            ),
            (
                "wardley-beta\ncomponent A 0.5, 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 13: Expecting token of type '[' but found `0.5`.",
                    2,
                    13,
                )),
            ),
            (
                "wardley-beta\ncomponent [0.5, 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 11: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '['",
                    2,
                    11,
                )),
            ),
            (
                "wardley-beta\nfoo bar baz [\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 13: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '['",
                    2,
                    13,
                )),
            ),
            (
                "wardley-beta\n[\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 1: Expecting token of type 'EOF' but found `[`.",
                    2,
                    1,
                )),
            ),
            (
                "wardley\n",
                Some((
                    "Parsing failed:  Parse error on line 1, column 1: Expecting token of type 'KW_WARDLEY' but found `wardley`.\nParse error on line 1, column 8: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    1,
                    1,
                )),
            ),
            (
                "\n",
                Some((
                    "Parsing failed:  Parse error on line ?, column ?: Expecting token of type 'KW_WARDLEY' but found ``.",
                    0,
                    0,
                )),
            ),
            (
                "wardley-beta\nnote hi [0.5, 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 6: Expecting token of type 'STRING' but found `hi`.\nParse error on line 2, column 9: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '['",
                    2,
                    6,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, 0.5] (foo)\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 25: Expecting token of type 'KW_INERTIA' but found `foo)`.\nParse error on line 2, column 29: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    2,
                    25,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, 0.5] label\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 29: Expecting token of type '[' but found `\n`.",
                    2,
                    29,
                )),
            ),
            (
                "wardley-beta\npipeline A {\n}\n",
                Some((
                    "Parsing failed:  Parse error on line 3, column 1: Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[KW_COMPONENT]>\nbut found: '}'",
                    3,
                    1,
                )),
            ),
            (
                "wardley-beta\npipeline A {\n component B [0.5, 0.5]\n}\n",
                Some((
                    "Parsing failed:  Parse error on line 3, column 18: Expecting token of type ']' but found `,`.\nParse error on line 4, column 1: Expecting token of type 'EOF' but found `}`.",
                    3,
                    18,
                )),
            ),
            (
                "wardley-beta\nevolution A\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 12: Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[ARROW]>\nbut found: '\n'",
                    2,
                    12,
                )),
            ),
            (
                "wardley-beta\nsize [1, 2] x\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 13: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: 'x'\nParse error on line 2, column 14: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    2,
                    13,
                )),
            ),
            (
                "wardley-beta\ntitle x y\ncomponent A [0.5, 0.5] $\n",
                Some((
                    "Parsing failed: Lexer error on line 3, column 24: unexpected character: ->$<- at offset: 46, skipped 1 characters. ",
                    3,
                    24,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, 0.5] component B [0.5, 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 24: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: 'component'",
                    2,
                    24,
                )),
            ),
            (
                "wardley-beta\nA ->\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 5: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    2,
                    5,
                )),
            ),
            ("wardley-beta\nA -> B C\n", None),
            (
                "wardley-beta\nannotation 1, [0.5, 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 25: Expecting token of type 'STRING' but found `\n`.",
                    2,
                    25,
                )),
            ),
            (
                "wardley-beta\nevolve A\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 9: Expecting token of type 'WARDLEY_NUMBER' but found `\n`.",
                    2,
                    9,
                )),
            ),
            ("wardley-beta\ntitle\n", None),
            ("wardley-beta\ncomponent A [0.5, 0.5]", None),
            (
                "wardley-beta\ncomponent A [0.5, 0.5] (inertia\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 32: Expecting token of type ')' but found `\n`.",
                    2,
                    32,
                )),
            ),
            (
                "wardley-beta\naccelerator A [1, 2]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 16: Expecting token of type 'WARDLEY_NUMBER' but found `1`.",
                    2,
                    16,
                )),
            ),
            (
                "wardley-beta\naccDescr {x} foo\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 1: Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[ACC_DESCR] ,[ACC_TITLE] ,[TITLE]>\nbut found: 'accDescr {x}'\nParse error on line 2, column 1: Expecting end of file but found `accDescr {x}`.",
                    2,
                    1,
                )),
            ),
            (
                "wardley-beta\n}\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 1: Expecting token of type 'EOF' but found `}`.",
                    2,
                    1,
                )),
            ),
            (
                "wardley-beta\n} }\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 1: Expecting token of type 'EOF' but found `}`.",
                    2,
                    1,
                )),
            ),
            (
                "wardley-beta\n}",
                Some((
                    "Parsing failed:  Parse error on line 2, column 1: Expecting end of file but found `}`.",
                    2,
                    1,
                )),
            ),
            (
                "foo [\nwardley-beta\n",
                Some((
                    "Parsing failed:  Parse error on line 1, column 1: Expecting token of type 'KW_WARDLEY' but found `foo`.\nParse error on line 1, column 5: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '['\nParse error on line 2, column 1: Expecting token of type 'EOF' but found `wardley-beta`.",
                    1,
                    1,
                )),
            ),
            (
                "x\n\nwardley-beta\n",
                Some((
                    "Parsing failed:  Parse error on line 1, column 1: Expecting token of type 'KW_WARDLEY' but found `x`.\nParse error on line 1, column 2: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'\nParse error on line 3, column 1: Expecting token of type 'EOF' but found `wardley-beta`.",
                    1,
                    1,
                )),
            ),
            (
                "}\n\n}\nwardley-beta\n",
                Some((
                    "Parsing failed:  Parse error on line 1, column 1: Expecting token of type 'KW_WARDLEY' but found `}`.\nParse error on line 3, column 1: Expecting token of type 'KW_WARDLEY' but found `}`.",
                    1,
                    1,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 18: Expecting token of type ',' but found `0.5`.",
                    2,
                    18,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, , 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 19: Expecting token of type 'WARDLEY_NUMBER' but found `,`.",
                    2,
                    19,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, 0.5] (build\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 30: Expecting token of type ')' but found `\n`.",
                    2,
                    30,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, 0.5] label [1, 2\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 35: Expecting token of type ']' but found `\n`.",
                    2,
                    35,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, 0.5] label [x, 2]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 31: Expecting token of type 'INT' but found `x`.\nParse error on line 2, column 32: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: ','",
                    2,
                    31,
                )),
            ),
            (
                "wardley-beta\nevolution A@x -> B\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 13: Expecting token of type 'WARDLEY_NUMBER' but found `x`.",
                    2,
                    13,
                )),
            ),
            (
                "wardley-beta\nevolution A -> \n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 16: Expecting: one of these possible Token sequences:\n  1. [ID]\n  2. [STRING]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    2,
                    16,
                )),
            ),
            (
                "wardley-beta\nevolution A -> B / \n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 20: Expecting: one of these possible Token sequences:\n  1. [ID]\n  2. [STRING]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    2,
                    20,
                )),
            ),
            (
                "wardley-beta\nannotations [x, 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 14: Expecting: one of these possible Token sequences:\n  1. [WARDLEY_NUMBER]\n  2. [INT]\nbut found: 'x'\nParse error on line 2, column 15: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: ','",
                    2,
                    14,
                )),
            ),
            (
                "wardley-beta\nannotation x\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 12: Expecting token of type 'INT' but found `x`.\nParse error on line 2, column 13: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    2,
                    12,
                )),
            ),
            (
                "wardley-beta\npipeline A\ncomponent B [0.5]\n}\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 11: Expecting token of type '{' but found `\n`.",
                    2,
                    11,
                )),
            ),
            (
                "wardley-beta\npipeline A {\n component B [0.5]\n foo\n}\n",
                Some((
                    "Parsing failed:  Parse error on line 4, column 2: Expecting token of type '}' but found `foo`.\nParse error on line 4, column 5: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'\nParse error on line 5, column 1: Expecting token of type 'EOF' but found `}`.",
                    4,
                    2,
                )),
            ),
            (
                "wardley-beta\npipeline A {\n component B [0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line ?, column ?: Expecting token of type '}' but found ``.",
                    0,
                    0,
                )),
            ),
            (
                "wardley-beta\npipeline A {\n component B [0.5] }\n",
                Some((
                    "Parsing failed:  Parse error on line 3, column 20: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '}'",
                    3,
                    20,
                )),
            ),
            (
                "wardley-beta\nnote \"x\" [0.5, 0.5] y\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 21: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: 'y'\nParse error on line 2, column 22: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    2,
                    21,
                )),
            ),
            (
                "wardley-beta\ntitle a\n accTitle: b\n%% c\ncomponent A [0.5, 0.5] %% d\n",
                None,
            ),
            (
                "wardley-beta\nsize [10.5, 2]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 7: Expecting token of type 'INT' but found `10.5`.",
                    2,
                    7,
                )),
            ),
            (
                "wardley-beta\n@\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 1: Expecting token of type 'EOF' but found `@`.",
                    2,
                    1,
                )),
            ),
            (
                "wardley-beta\nA -> B\n} x\nC -> D\n",
                Some((
                    "Parsing failed:  Parse error on line 3, column 1: Expecting token of type 'EOF' but found `}`.\nParse error on line 3, column 4: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    3,
                    1,
                )),
            ),
            ("  wardley-beta\n", None),
            (
                "wardley-beta extra\n",
                Some((
                    "Parsing failed:  Parse error on line 1, column 19: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    1,
                    19,
                )),
            ),
            (
                "wardley-betaX\n",
                Some((
                    "Parsing failed:  Parse error on line 1, column 14: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    1,
                    14,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, 0.5] #\n",
                Some((
                    "Parsing failed: Lexer error on line 2, column 24: unexpected character: ->#<- at offset: 36, skipped 1 characters. ",
                    2,
                    24,
                )),
            ),
            (
                "wardley-beta\nnote \"é\" [0.5, 0.5] ~~\n",
                Some((
                    "Parsing failed: Lexer error on line 2, column 21: unexpected character: ->~<- at offset: 33, skipped 2 characters. ",
                    2,
                    21,
                )),
            ),
            (
                "wardley-beta\nA -.-> B\nA - B\n",
                Some((
                    "Parsing failed:  Parse error on line 3, column 3: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '-'\nParse error on line 3, column 6: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    3,
                    3,
                )),
            ),
            (
                "wardley-beta\nA ; label\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 3: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '; label'",
                    2,
                    3,
                )),
            ),
            (
                "wardley-beta\nA +'x' B\n",
                Some((
                    "Parsing failed: Lexer error on line 2, column 3: unexpected character: ->+<- at offset: 15, skipped 1 characters. Parse error on line 2, column 8: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: 'B'\nParse error on line 2, column 9: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    2,
                    3,
                )),
            ),
            (
                "wardley-beta\nevolve A 0.5 0.6\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 14: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '0.6'",
                    2,
                    14,
                )),
            ),
            ("wardley-beta\ntitle x\ntitle\n", None),
            ("wardley-beta\naccDescr {x}\n", None),
            (
                "wardley-beta\naccDescr {x} component A [0.5, 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 1: Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[ACC_DESCR] ,[ACC_TITLE] ,[TITLE]>\nbut found: 'accDescr {x}'\nParse error on line 2, column 1: Expecting end of file but found `accDescr {x}`.",
                    2,
                    1,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, 0.5] inertia (inertia)\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 32: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '('",
                    2,
                    32,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, 0.5] (buy) (build)\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 31: Expecting token of type 'KW_INERTIA' but found `build`.",
                    2,
                    31,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, 0.5] label [1, 2] label [3, 4]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 37: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: 'label'",
                    2,
                    37,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, 0.5] label [1 2]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 33: Expecting token of type ',' but found `2`.",
                    2,
                    33,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, 0.5] (\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 25: Expecting token of type 'KW_INERTIA' but found `\n`.",
                    2,
                    25,
                )),
            ),
            (
                "wardley-beta\nannotation 1 [0.5, 0.5] \"x\"\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 14: Expecting token of type ',' but found `[`.",
                    2,
                    14,
                )),
            ),
            (
                "wardley-beta\nannotation 1, [0.5 0.5] \"x\"\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 20: Expecting token of type ',' but found `0.5`.",
                    2,
                    20,
                )),
            ),
            (
                "wardley-beta\nannotations [0.5, 0.5\ncomponent B [0.5, 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 22: Expecting token of type ']' but found `\n`.",
                    2,
                    22,
                )),
            ),
            (
                "wardley-beta\naccelerator A [0.5, 0.5] x\ndeaccelerator [0.5, 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 26: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: 'x'\nParse error on line 2, column 27: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'\nParse error on line 3, column 15: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '['",
                    2,
                    26,
                )),
            ),
            (
                "wardley-beta\nsize 1, 2\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 6: Expecting token of type '[' but found `1`.\nParse error on line 2, column 10: Expecting token of type ']' but found `\n`.",
                    2,
                    6,
                )),
            ),
            (
                "wardley-beta\nevolution -> A\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 11: Expecting: one of these possible Token sequences:\n  1. [ID]\n  2. [STRING]\n  3. [NAME_WITH_SPACES]\nbut found: '->'",
                    2,
                    11,
                )),
            ),
            (
                "wardley-beta\nevolution A -> B -> C@0.5@0.6\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 26: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '@'",
                    2,
                    26,
                )),
            ),
            (
                "wardley-beta\nA -> -> B\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 6: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '->'\nParse error on line 2, column 10: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    2,
                    6,
                )),
            ),
            (
                "wardley-beta\nA +<> +<> B\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 7: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '+<>'\nParse error on line 2, column 12: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
                    2,
                    7,
                )),
            ),
            (
                "wardley-beta\npipeline A {\n}\ncomponent C [0.5, 0.5]\n",
                Some((
                    "Parsing failed:  Parse error on line 3, column 1: Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[KW_COMPONENT]>\nbut found: '}'",
                    3,
                    1,
                )),
            ),
            (
                "wardley-beta\npipeline {\n component B [0.5]\n}\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 10: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '{'\nParse error on line 3, column 18: Expecting token of type ',' but found `]`.\nParse error on line 4, column 1: Expecting token of type 'EOF' but found `}`.",
                    2,
                    10,
                )),
            ),
            (
                "wardley-beta\n\n\n\ncomponent A [0.5]\n\n\ncomponent B [0.1, 0.1]\n",
                Some((
                    "Parsing failed:  Parse error on line 5, column 17: Expecting token of type ',' but found `]`.",
                    5,
                    17,
                )),
            ),
            (
                "%%{init: {}}%%\nwardley-beta\n---\n",
                Some((
                    "Parsing failed:  Parse error on line 3, column 1: Expecting token of type 'EOF' but found `-`.",
                    3,
                    1,
                )),
            ),
            (
                "wardley-beta\ncomponent A [0.5, 0.5]\ncomponent A [0.5, 0.5\n] \n",
                Some((
                    "Parsing failed:  Parse error on line 3, column 22: Expecting token of type ']' but found `\n`.\nParse error on line 4, column 1: Expecting token of type 'EOF' but found `]`.",
                    3,
                    22,
                )),
            ),
            ("wardley-beta\naccTitle x\n", None),
            ("wardley-beta\ntitle t %% c\naccTitle: a %% b\n", None),
            (
                "wardley-beta\ncomponent A [0.50, 0.25]\r\ncomponent B [0.1, 0.2]\r\n",
                None,
            ),
        ];
        let mut failures = Vec::new();
        for (input, expected) in cases {
            let got = parse(input).err().map(|f| (f.message, f.line, f.column));
            let want = expected.map(|(m, l, c)| (m.to_string(), l, c));
            if got != want {
                failures.push(format!("{input:?}\n  want {want:?}\n  got  {got:?}"));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
