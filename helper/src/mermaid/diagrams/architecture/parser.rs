//! A hand-written lexer and parser for the language of
//! `architecture.langium` plus `common.langium` and `arch.langium`
//! (`@mermaid-js/parser` 2.0.1, mermaid 12.2.0).
//!
//! Lexer. `ArchitectureTokenBuilder` (`AbstractMermaidTokenBuilder` with
//! the keyword list `['architecture']`, which matches no keyword, so no
//! boundary suffix is added) gives Langium's token order: the keywords
//! longest first, `architecture-beta`, `junction`, `service`, `column`,
//! `group`, `align`, `row`, `--`, `in`, `:`, `-`, where the word keywords
//! have `ID` and the dashes `YAML` as their `LONGER_ALT`; then the
//! terminals `ARROW_DIRECTION`, `ARROW_GROUP`, `ARROW_INTO`, the reachable
//! common ones (`ACC_DESCR`, `ACC_TITLE`, `TITLE`, `STRING`, `ID`,
//! `NEWLINE`, the skipped `WHITESPACE`, the hidden `YAML`, `DIRECTIVE`,
//! `SINGLE_LINE_COMMENT`), and last `ARCH_ICON` and `ARCH_TITLE`.
//! `ARROW_DIRECTION` has no `LONGER_ALT`, so an id that starts with `L`,
//! `R`, `T`, or `B` lexes as a direction followed by an id, as in the
//! oracle. At each position the first token type that matches wins; a
//! position where none matches is skipped and reported as one lexer error
//! per run of skipped characters.
//!
//! Parser. An LL walk of `Architecture` with the error recovery of
//! Chevrotain's recovering parser. Langium turns every parser rule into a
//! Chevrotain rule, fragments included, so `Statement`, `Arrow`,
//! `LeftPort`, and `RightPort` each push their own re-sync set:
//!
//! - `CONSUME` records a mismatch, then inserts the expected token when the
//!   current token is one the rule can continue with after it, or deletes
//!   the current token when the next one is the expected one. Otherwise
//!   the rule throws.
//! - A rule that throws re-syncs: it finds the first token from the current
//!   one that can follow any rule on the stack. When that token can follow
//!   the current rule in its caller, the rule skips to it and returns;
//!   otherwise it rethrows.
//! - The `NEWLINE*` before `architecture-beta` and the statement loop at
//!   the end of the entry rule have Chevrotain's in-repetition recovery.
//!
//! `MermaidParseError` joins both lists as `Parsing failed: <lexer errors>
//! <parser errors>`.

use std::fmt::Write;

use crate::mermaid::theme::khroma::js_is_whitespace;

/// The `Architecture` AST after Langium's value converters.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ast {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub groups: Vec<Group>,
    pub services: Vec<Service>,
    pub junctions: Vec<Junction>,
    pub edges: Vec<Edge>,
    pub alignments: Vec<Alignment>,
}

/// `Group: 'group' id=ID icon=ARCH_ICON? title=ARCH_TITLE? ('in' in=ID)?`.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub id: String,
    pub icon: Option<String>,
    pub title: Option<String>,
    pub parent: Option<String>,
    pub line: u32,
}

/// `Service: 'service' id=ID (iconText=STRING | icon=ARCH_ICON)?
/// title=ARCH_TITLE? ('in' in=ID)?`.
#[derive(Debug, Clone, PartialEq)]
pub struct Service {
    pub id: String,
    pub icon: Option<String>,
    pub icon_text: Option<String>,
    pub title: Option<String>,
    pub parent: Option<String>,
    pub line: u32,
}

/// `Junction: 'junction' id=ID ('in' in=ID)?`.
#[derive(Debug, Clone, PartialEq)]
pub struct Junction {
    pub id: String,
    pub parent: Option<String>,
    pub line: u32,
}

/// `Edge`: `lhsId{group}?:lhsDir <? (-- | -[title]-) >? rhsDir:rhsId{group}?`.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub lhs_id: String,
    pub lhs_dir: char,
    pub lhs_into: bool,
    pub lhs_group: bool,
    pub rhs_id: String,
    pub rhs_dir: char,
    pub rhs_into: bool,
    pub rhs_group: bool,
    pub title: Option<String>,
    pub line: u32,
}

/// `Alignment: 'align' direction=('row' | 'column') members+=ID (members+=ID)+`.
#[derive(Debug, Clone, PartialEq)]
pub struct Alignment {
    /// `row` or `column`.
    pub direction: String,
    pub members: Vec<String>,
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
    Newline,
    ArchitectureBeta,
    Junction,
    Service,
    Column,
    Group,
    Align,
    Row,
    DashDash,
    In,
    Colon,
    Dash,
    ArrowDirection,
    ArrowGroup,
    ArrowInto,
    AccDescr,
    AccTitle,
    Title,
    Str,
    Id,
    ArchIcon,
    ArchTitle,
    Eof,
}

impl Kind {
    /// Chevrotain's `tokenType.name`: the keyword text for keywords, the
    /// terminal rule name otherwise.
    fn name(self) -> &'static str {
        match self {
            Kind::Newline => "NEWLINE",
            Kind::ArchitectureBeta => "architecture-beta",
            Kind::Junction => "junction",
            Kind::Service => "service",
            Kind::Column => "column",
            Kind::Group => "group",
            Kind::Align => "align",
            Kind::Row => "row",
            Kind::DashDash => "--",
            Kind::In => "in",
            Kind::Colon => ":",
            Kind::Dash => "-",
            Kind::ArrowDirection => "ARROW_DIRECTION",
            Kind::ArrowGroup => "ARROW_GROUP",
            Kind::ArrowInto => "ARROW_INTO",
            Kind::AccDescr => "ACC_DESCR",
            Kind::AccTitle => "ACC_TITLE",
            Kind::Title => "TITLE",
            Kind::Str => "STRING",
            Kind::Id => "ID",
            Kind::ArchIcon => "ARCH_ICON",
            Kind::ArchTitle => "ARCH_TITLE",
            Kind::Eof => "EOF",
        }
    }
}

/// A set of token kinds, one bit per [`Kind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Set(u32);

impl Set {
    const EMPTY: Set = Set(0);

    const fn of(kinds: &[Kind]) -> Set {
        let mut bits = 0;
        let mut i = 0;
        while i < kinds.len() {
            bits |= 1 << kinds[i] as u32;
            i += 1;
        }
        Set(bits)
    }

    const fn with(self, other: Set) -> Set {
        Set(self.0 | other.0)
    }

    fn has(self, kind: Kind) -> bool {
        self.0 & (1 << kind as u32) != 0
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
    Kind::Group,
    Kind::Service,
    Kind::Junction,
    Kind::Id,
    Kind::Align,
]);
/// The first tokens of an iteration of the `Architecture` loop, which is
/// also the follow set of `TitleAndAccessibilities` and `Statement`.
const LOOP: Set = Set::of(&[Kind::Newline]).with(TITLES).with(STATEMENTS);
/// What `Arrow` can continue with after `LeftPort`.
const AFTER_LEFT_PORT: Set = Set::of(&[Kind::ArrowInto, Kind::DashDash, Kind::Dash]);
/// What follows `Arrow` in `Edge`.
const AFTER_ARROW: Set = Set::of(&[Kind::Id]);

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

/// The word keywords in Chevrotain's order; each has `ID` as its
/// `LONGER_ALT`.
const WORD_KEYWORDS: [(Kind, &str); 7] = [
    (Kind::ArchitectureBeta, "architecture-beta"),
    (Kind::Junction, "junction"),
    (Kind::Service, "service"),
    (Kind::Column, "column"),
    (Kind::Group, "group"),
    (Kind::Align, "align"),
    (Kind::Row, "row"),
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

    /// `[\t ]+`.
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

    /// One quoted alternative, `"([^"\\]|\\.)*"` or `'([^'\\]|\\.)*'`;
    /// `.` excludes line terminators.
    fn quoted(&self, i: usize) -> Option<usize> {
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

    /// `STRING`.
    fn string(&self, i: usize) -> Option<usize> {
        self.quoted(i)
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

    /// A word keyword, unless its `LONGER_ALT` `ID` matches a longer image.
    fn word_keyword(&self, i: usize, word: &str) -> Option<usize> {
        let end = self.literal(i, word)?;
        match self.id(i) {
            Some(id_end) if id_end > end => None,
            _ => Some(end),
        }
    }

    /// A dash keyword, unless its `LONGER_ALT` `YAML` matches a longer
    /// image (then the YAML block is hidden).
    fn dash_keyword(&self, i: usize, word: &str, kind: Kind) -> Option<(Option<Kind>, usize)> {
        let end = self.literal(i, word)?;
        match self.yaml(i) {
            Some(yaml_end) if yaml_end > end => Some((HIDDEN, yaml_end)),
            _ => Some((Some(kind), end)),
        }
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

    /// `[\t ]*%%{[\S\s]*?}%%(?:\r?\n|(?!\S))`; `WHITESPACE` comes first in
    /// the token order, so the prefix never matters.
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

    /// `ARROW_DIRECTION: 'L' | 'R' | 'T' | 'B'`.
    fn arrow_direction(&self, i: usize) -> Option<usize> {
        matches!(self.at(i)?, 'L' | 'R' | 'T' | 'B').then_some(i + 1)
    }

    /// `ARROW_INTO: /<|>/`.
    fn arrow_into(&self, i: usize) -> Option<usize> {
        matches!(self.at(i)?, '<' | '>').then_some(i + 1)
    }

    /// `ARCH_ICON: /\([\w-:]+\)/`.
    fn arch_icon(&self, i: usize) -> Option<usize> {
        if self.at(i)? != '(' {
            return None;
        }
        let mut j = i + 1;
        while self
            .at(j)
            .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':'))
        {
            j += 1;
        }
        (j > i + 1 && self.at(j) == Some(')')).then_some(j + 1)
    }

    /// `ARCH_TITLE: /\[(?:"([^"\\]|\\.)*"|'([^'\\]|\\.)*'|[^\[\]\r\n]+)\]/`.
    fn arch_title(&self, i: usize) -> Option<usize> {
        if self.at(i)? != '[' {
            return None;
        }
        if let Some(end) = self.quoted(i + 1)
            && self.at(end) == Some(']')
        {
            return Some(end + 1);
        }
        let mut j = i + 1;
        while self
            .at(j)
            .is_some_and(|c| !matches!(c, '[' | ']' | '\r' | '\n'))
        {
            j += 1;
        }
        (j > i + 1 && self.at(j) == Some(']')).then_some(j + 1)
    }

    /// The first token type that matches at `i`, in Chevrotain's order.
    fn next(&self, i: usize) -> Option<(Option<Kind>, usize)> {
        for (kind, word) in WORD_KEYWORDS {
            if let Some(end) = self.word_keyword(i, word) {
                return Some((Some(kind), end));
            }
        }
        if let Some(found) = self.dash_keyword(i, "--", Kind::DashDash) {
            return Some(found);
        }
        if let Some(end) = self.word_keyword(i, "in") {
            return Some((Some(Kind::In), end));
        }
        if let Some(end) = self.literal(i, ":") {
            return Some((Some(Kind::Colon), end));
        }
        if let Some(found) = self.dash_keyword(i, "-", Kind::Dash) {
            return Some(found);
        }
        let table: [(Option<Kind>, Matcher<'a>); 15] = [
            (Some(Kind::ArrowDirection), Lexer::arrow_direction),
            (Some(Kind::ArrowGroup), |l, i| l.literal(i, "{group}")),
            (Some(Kind::ArrowInto), Lexer::arrow_into),
            (Some(Kind::AccDescr), Lexer::acc_descr),
            (Some(Kind::AccTitle), Lexer::acc_title),
            (Some(Kind::Title), Lexer::title),
            (Some(Kind::Str), Lexer::string),
            (Some(Kind::Id), Lexer::id),
            (Some(Kind::Newline), Lexer::newline),
            (HIDDEN, Lexer::whitespace),
            (HIDDEN, Lexer::yaml),
            (HIDDEN, Lexer::directive),
            (HIDDEN, Lexer::comment),
            (Some(Kind::ArchIcon), Lexer::arch_icon),
            (Some(Kind::ArchTitle), Lexer::arch_title),
        ];
        table
            .iter()
            .find_map(|(kind, matcher)| matcher(self, i).map(|end| (*kind, end)))
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

/// The fields `fragment Arrow` assigns.
struct Arrow {
    lhs_dir: char,
    lhs_into: bool,
    title: Option<String>,
    rhs_into: bool,
    rhs_dir: char,
}

type Parsed<T> = Result<T, Thrown>;

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

    /// `CONSUME` with Chevrotain's in-rule recovery. `follows` is what the
    /// rule can continue with after the token, for the single token
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
    /// loop and what the rule can continue with after that terminal, for
    /// the in-repetition recovery at the loop exit.
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

    /// An `ID` with its image; `None` for an inserted one.
    fn id(&mut self, follows: Set) -> Parsed<Option<String>> {
        Ok(self.consume(Kind::Id, follows)?.map(|t| self.image(&t)))
    }

    /// An optional terminal, converted.
    fn optional(&mut self, kind: Kind, convert: fn(&str) -> String) -> Option<String> {
        (self.kind(1) == kind).then(|| {
            let t = self.advance();
            convert(&self.image(&t))
        })
    }

    /// `('in' in=ID)?` at the end of a declaration.
    fn parent(&mut self) -> Parsed<Option<String>> {
        if self.kind(1) != Kind::In {
            return Ok(None);
        }
        self.advance();
        self.id(EOL)
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

    /// `Group`.
    fn group(&mut self) -> Parsed<Group> {
        let line = self.line();
        self.advance();
        let id = self.id(Set::of(&[Kind::ArchIcon, Kind::ArchTitle, Kind::In]).with(EOL))?;
        let icon = self.optional(Kind::ArchIcon, convert_arch_icon);
        let title = self.optional(Kind::ArchTitle, convert_arch_title);
        let parent = self.parent()?;
        self.eol(Set::EMPTY)?;
        Ok(Group {
            id: id.unwrap_or_default(),
            icon,
            title,
            parent,
            line,
        })
    }

    /// `Service`.
    fn service(&mut self) -> Parsed<Service> {
        let line = self.line();
        self.advance();
        let id =
            self.id(Set::of(&[Kind::Str, Kind::ArchIcon, Kind::ArchTitle, Kind::In]).with(EOL))?;
        let icon_text = self.optional(Kind::Str, convert_string);
        let icon = if icon_text.is_none() {
            self.optional(Kind::ArchIcon, convert_arch_icon)
        } else {
            None
        };
        let title = self.optional(Kind::ArchTitle, convert_arch_title);
        let parent = self.parent()?;
        self.eol(Set::EMPTY)?;
        Ok(Service {
            id: id.unwrap_or_default(),
            icon,
            icon_text,
            title,
            parent,
            line,
        })
    }

    /// `Junction`.
    fn junction(&mut self) -> Parsed<Junction> {
        let line = self.line();
        self.advance();
        let id = self.id(Set::of(&[Kind::In]).with(EOL))?;
        let parent = self.parent()?;
        self.eol(Set::EMPTY)?;
        Ok(Junction {
            id: id.unwrap_or_default(),
            parent,
            line,
        })
    }

    /// `fragment LeftPort: ':' lhsDir=ARROW_DIRECTION`.
    fn left_port(&mut self) -> Parsed<Option<char>> {
        Ok(self
            .rule(AFTER_LEFT_PORT, |p| {
                p.consume(Kind::Colon, Set::of(&[Kind::ArrowDirection]))?;
                let dir = p.consume(Kind::ArrowDirection, Set::EMPTY)?;
                Ok(dir.and_then(|t| p.image(&t).chars().next()))
            })?
            .flatten())
    }

    /// `fragment RightPort: rhsDir=ARROW_DIRECTION ':'`.
    fn right_port(&mut self) -> Parsed<Option<char>> {
        Ok(self
            .rule(Set::EMPTY, |p| {
                let dir = p.consume(Kind::ArrowDirection, Set::of(&[Kind::Colon]))?;
                p.consume(Kind::Colon, Set::EMPTY)?;
                Ok(dir.and_then(|t| p.image(&t).chars().next()))
            })?
            .flatten())
    }

    /// `fragment Arrow: LeftPort lhsInto?=ARROW_INTO? ('--' | '-'
    /// title=ARCH_TITLE '-') rhsInto?=ARROW_INTO? RightPort`.
    fn arrow(&mut self) -> Parsed<Option<Arrow>> {
        self.rule(AFTER_ARROW, |p| {
            let lhs_dir = p.left_port()?;
            let lhs_into = p.kind(1) == Kind::ArrowInto;
            if lhs_into {
                p.advance();
            }
            let mut title = None;
            match p.kind(1) {
                Kind::DashDash => {
                    p.advance();
                }
                Kind::Dash => {
                    p.advance();
                    title = p
                        .consume(Kind::ArchTitle, Set::of(&[Kind::Dash]))?
                        .map(|t| convert_arch_title(&p.image(&t)));
                    p.consume(
                        Kind::Dash,
                        Set::of(&[Kind::ArrowInto, Kind::ArrowDirection]),
                    )?;
                }
                _ => return Err(p.no_viable_alt(&["--", "-"])),
            }
            let rhs_into = p.kind(1) == Kind::ArrowInto;
            if rhs_into {
                p.advance();
            }
            let rhs_dir = p.right_port()?;
            Ok(Arrow {
                lhs_dir: lhs_dir.unwrap_or('?'),
                lhs_into,
                title,
                rhs_into,
                rhs_dir: rhs_dir.unwrap_or('?'),
            })
        })
    }

    /// `Edge: lhsId=ID lhsGroup?=ARROW_GROUP? Arrow rhsId=ID
    /// rhsGroup?=ARROW_GROUP?`.
    fn edge(&mut self) -> Parsed<Edge> {
        let line = self.line();
        let lhs = self.advance();
        let lhs_id = self.image(&lhs);
        let lhs_group = self.kind(1) == Kind::ArrowGroup;
        if lhs_group {
            self.advance();
        }
        let arrow = self.arrow()?;
        let rhs_id = self.id(Set::of(&[Kind::ArrowGroup]).with(EOL))?;
        let rhs_group = self.kind(1) == Kind::ArrowGroup;
        if rhs_group {
            self.advance();
        }
        self.eol(Set::EMPTY)?;
        // A recovered `Arrow` leaves its fields unset; the AST is not used
        // after an error.
        let Arrow {
            lhs_dir,
            lhs_into,
            title,
            rhs_into,
            rhs_dir,
        } = arrow.unwrap_or(Arrow {
            lhs_dir: '?',
            lhs_into: false,
            title: None,
            rhs_into: false,
            rhs_dir: '?',
        });
        Ok(Edge {
            lhs_id,
            lhs_dir,
            lhs_into,
            lhs_group,
            rhs_id: rhs_id.unwrap_or_default(),
            rhs_dir,
            rhs_into,
            rhs_group,
            title,
            line,
        })
    }

    /// `Alignment`.
    fn alignment(&mut self) -> Parsed<Alignment> {
        let line = self.line();
        self.advance();
        let direction = match self.kind(1) {
            Kind::Row | Kind::Column => {
                let t = self.advance();
                self.image(&t)
            }
            _ => return Err(self.no_viable_alt(&["row", "column"])),
        };
        let mut members = Vec::new();
        members.extend(self.id(Set::of(&[Kind::Id]))?);
        self.repetition(Some("[ID]"), |p| p.kind(1) == Kind::Id, None, &mut |p| {
            let t = p.advance();
            members.push(p.image(&t));
            Ok(())
        })?;
        self.eol(Set::EMPTY)?;
        Ok(Alignment {
            direction,
            members,
            line,
        })
    }

    /// `fragment Statement`: one alternative per first token, each a rule
    /// that nothing follows inside `Statement`.
    fn statement(&mut self, ast: &mut Ast) -> Parsed<()> {
        self.rule(LOOP, |p| {
            let e = Set::EMPTY;
            match p.kind(1) {
                Kind::Group => ast.groups.extend(p.rule(e, Self::group)?),
                Kind::Service => ast.services.extend(p.rule(e, Self::service)?),
                Kind::Junction => ast.junctions.extend(p.rule(e, Self::junction)?),
                Kind::Align => ast.alignments.extend(p.rule(e, Self::alignment)?),
                _ => ast.edges.extend(p.rule(e, Self::edge)?),
            }
            Ok(())
        })
        .map(|_| ())
    }

    /// The `Architecture` entry rule: `NEWLINE* 'architecture-beta'
    /// (NEWLINE | TitleAndAccessibilities | Statement)*`, then Chevrotain's
    /// check that all input was parsed.
    fn architecture(&mut self) -> Ast {
        let mut ast = Ast::default();
        let _ = self.rule(Set::of(&[Kind::Eof]), |p| {
            p.repetition(
                None,
                |p| p.kind(1) == Kind::Newline,
                Some((Kind::ArchitectureBeta, LOOP)),
                &mut |p| {
                    p.advance();
                    Ok(())
                },
            )?;
            p.consume(Kind::ArchitectureBeta, LOOP)?;
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

/// `ArchitectureValueConverter` for `ARCH_ICON`:
/// `input.replace(/[()]/g, '').trim()`.
fn convert_arch_icon(image: &str) -> String {
    let stripped: String = image.chars().filter(|c| !matches!(c, '(' | ')')).collect();
    js_trim(&stripped).to_string()
}

/// `ArchitectureValueConverter` for `ARCH_TITLE`: the brackets go, the
/// rest is trimmed, and outer quotes go with their escapes.
fn convert_arch_title(image: &str) -> String {
    let inner = image.strip_prefix('[').unwrap_or(image);
    let inner = inner.strip_suffix(']').unwrap_or(inner);
    let mut result = js_trim(inner).to_string();
    let quoted = result.len() >= 2
        && ((result.starts_with('"') && result.ends_with('"'))
            || (result.starts_with('\'') && result.ends_with('\'')));
    // `startsWith('"') && endsWith('"')` holds for a lone `"` too.
    let lone_quote = result == "\"" || result == "'";
    if quoted || lone_quote {
        let chars: Vec<char> = result.chars().collect();
        let sliced: String = if chars.len() >= 2 {
            chars[1..chars.len() - 1].iter().collect()
        } else {
            String::new()
        };
        result = sliced.replace("\\\"", "\"").replace("\\'", "'");
    }
    js_trim(&result).to_string()
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

fn js_trim(s: &str) -> &str {
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

/// `parse('architecture', text)`: the AST, or the `MermaidParseError` text
/// with the position of its first error.
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
    let ast = parser.architecture();
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
            "architecture-beta\n  title  My   Arch \n",
            "  group api(cloud)[ API ] in outer\n",
            "  service db(database)[\"Data [main]\"] in api\n",
            "  service t \"icon \\\"text\\\"\" ['T']\n",
            "  junction j in api\n",
            "  db{group}:R <-[ flows ]-> L:t{group}\n",
            "  align column db t\n",
        ));
        assert_eq!(ast.title.as_deref(), Some("My Arch"));
        let g = &ast.groups[0];
        assert_eq!(
            (
                g.id.as_str(),
                g.icon.as_deref(),
                g.title.as_deref(),
                g.parent.as_deref()
            ),
            ("api", Some("cloud"), Some("API"), Some("outer"))
        );
        let s = &ast.services[0];
        assert_eq!(
            (s.icon.as_deref(), s.title.as_deref(), s.parent.as_deref()),
            (Some("database"), Some("Data [main]"), Some("api"))
        );
        assert_eq!(ast.services[1].icon_text.as_deref(), Some("icon \"text\""));
        assert_eq!(ast.services[1].title.as_deref(), Some("T"));
        assert_eq!(ast.junctions[0].parent.as_deref(), Some("api"));
        let e = &ast.edges[0];
        assert_eq!(
            (
                e.lhs_dir,
                e.lhs_into,
                e.lhs_group,
                e.rhs_dir,
                e.rhs_into,
                e.rhs_group
            ),
            ('R', true, true, 'L', true, true)
        );
        assert_eq!(e.title.as_deref(), Some("flows"));
        assert_eq!(ast.alignments[0].direction, "column");
        assert_eq!(ast.alignments[0].members, ["db", "t"]);
        assert_eq!(e.line, 7);
    }

    /// An input and the oracle's error message, line, and column.
    type OracleCase = (&'static str, Option<(&'static str, u32, u32)>);

    /// Inputs and `parse('architecture', input)` results from the oracle's
    /// `@mermaid-js/parser`: `None` when it parses, else the
    /// `MermaidParseError` text and the first error position.
    #[test]
    fn errors_match_the_langium_parser() {
        let cases: &[OracleCase] = &[
            (
                "architecture-beta\n    service Lambda(server)[x]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 13: Expecting token of type 'ID' but found `L`.",
                    2,
                    13,
                )),
            ),
            (
                "architecture-beta\n    service lb(server)[Load Balancer]\n",
                None,
            ),
            (
                "architecture-beta\n    group Backend(cloud)[B]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 11: Expecting token of type 'ID' but found `B`.",
                    2,
                    11,
                )),
            ),
            (
                "architecture-beta\n    service a(server)[A]\n    service b(server)[B]\n    a:L -- R:b\n",
                None,
            ),
            (
                "architecture-beta\n    service a(server)[A]\n    service b(server)[B]\n    a:L <--> R:b\n",
                None,
            ),
            (
                "architecture-beta\n    service a(server)[A]\n    service b(server)[B]\n    a{group}:L -[label]- R:b{group}\n",
                None,
            ),
            (
                "architecture-beta\n    service a(server)[A]\n    service b(server)[B]\n    a:L--R:b\n",
                None,
            ),
            (
                "architecture-beta\n    service a(server)[A]\n    service b(server)[B]\n    a :L -- R: b\n",
                None,
            ),
            ("architecture-beta\n    service a \"text\" [A]\n", None),
            ("architecture-beta\n    service a(logos:aws-s3)[A]\n", None),
            ("architecture-beta\n    service a[A]\n", None),
            ("architecture-beta\n    service a(server)\n", None),
            (
                "architecture-beta\n    title Hello world\n    service a(server)[A]\n",
                None,
            ),
            (
                "architecture-beta\n    service a(server)[\"quoted [x]\"]\n",
                None,
            ),
            ("architecture-beta\n    junction j in g\n", None),
            (
                "architecture-beta\n    service a(server)[A]\n    service b(server)[B]\n    align row a b\n",
                None,
            ),
            (
                "architecture-beta\n    service a(server)[A] b\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 26: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: 'b'\nParse error on line 2, column 27: Expecting token of type ':' but found `\n`.",
                    2,
                    26,
                )),
            ),
            (
                "architecture-beta\n    service\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 12: Expecting token of type 'ID' but found `\n`.",
                    2,
                    12,
                )),
            ),
            (
                "architecture-beta\n    foo\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 8: Expecting token of type ':' but found `\n`.",
                    2,
                    8,
                )),
            ),
            (
                "architecture-beta\n    service a(server)[A]\n    a:X -- R:b\n",
                Some((
                    "Parsing failed:  Parse error on line 3, column 7: Expecting token of type 'ARROW_DIRECTION' but found `X`.\nParse error on line 3, column 9: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '--'\nParse error on line 3, column 15: Expecting token of type ':' but found `\n`.",
                    3,
                    7,
                )),
            ),
            (
                "architecture-beta\n    service a(server)[A]\n    service b(server)[B]\n    a:L - R:b\n",
                Some((
                    "Parsing failed:  Parse error on line 4, column 11: Expecting token of type 'ARCH_TITLE' but found `R`.",
                    4,
                    11,
                )),
            ),
            ("architecture-beta service a(server)[A]\n", None),
            (
                "architecture-betax\n",
                Some((
                    "Parsing failed:  Parse error on line 1, column 1: Expecting token of type 'architecture-beta' but found `architecture-betax`.\nParse error on line 1, column 19: Expecting token of type ':' but found `\n`.",
                    1,
                    1,
                )),
            ),
            ("  \n\narchitecture-beta\n", None),
            (
                "architecture-beta\n    service a(server)[A]\n    service b(server)[B]\n    a:L -- R:b %% comment\n",
                None,
            ),
            (
                "architecture-beta\n    service T(server)[A]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 13: Expecting token of type 'ID' but found `T`.",
                    2,
                    13,
                )),
            ),
            (
                "architecture-beta\n    service a(se rver)[A]\n",
                Some((
                    "Parsing failed: Lexer error on line 2, column 14: unexpected character: ->(<- at offset: 31, skipped 1 characters.\nLexer error on line 2, column 22: unexpected character: ->)<- at offset: 39, skipped 1 characters. Parse error on line 2, column 15: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: 'se'\nParse error on line 2, column 18: Expecting token of type ':' but found `rver`.\nParse error on line 2, column 23: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '[A]'",
                    2,
                    14,
                )),
            ),
            (
                "architecture-beta\n    service a(server)[A]\n    service b(server)[B]\n    a:L -- R:b c\n",
                Some((
                    "Parsing failed:  Parse error on line 4, column 16: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: 'c'\nParse error on line 4, column 17: Expecting token of type ':' but found `\n`.",
                    4,
                    16,
                )),
            ),
            (
                "architecture-beta\n    group api(cloud)[API]\n\n    service db(database)[Database] in api\n    service disk1(disk)[Storage] in api\n    db:L -- R:disk1\n",
                None,
            ),
            (
                "architecture-beta\n    service Lambda(server)[x]\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 13: Expecting token of type 'ID' but found `L`.",
                    2,
                    13,
                )),
            ),
            (
                "architecture-beta\n    service a(server)[A]\n    service b(server)[B]\n    a{group}:L <-[label]-> R:b{group}\n",
                None,
            ),
            (
                "architecture-beta\n    service a \"text \\\"quoted\\\"\" ['B']\n",
                None,
            ),
            (
                "architecture-beta\n    service a(server)[A] b\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 26: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: 'b'\nParse error on line 2, column 27: Expecting token of type ':' but found `\n`.",
                    2,
                    26,
                )),
            ),
            (
                "architecture-beta\n    service\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 12: Expecting token of type 'ID' but found `\n`.",
                    2,
                    12,
                )),
            ),
            (
                "architecture-beta\n    foo\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 8: Expecting token of type ':' but found `\n`.",
                    2,
                    8,
                )),
            ),
            (
                "architecture-beta\n    service a(server)[A]\n    a:X -- R:b\n",
                Some((
                    "Parsing failed:  Parse error on line 3, column 7: Expecting token of type 'ARROW_DIRECTION' but found `X`.\nParse error on line 3, column 9: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '--'\nParse error on line 3, column 15: Expecting token of type ':' but found `\n`.",
                    3,
                    7,
                )),
            ),
            (
                "architecture-beta\n    a:L - R:b\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 11: Expecting token of type 'ARCH_TITLE' but found `R`.",
                    2,
                    11,
                )),
            ),
            (
                "architecture-betax\n",
                Some((
                    "Parsing failed:  Parse error on line 1, column 1: Expecting token of type 'architecture-beta' but found `architecture-betax`.\nParse error on line 1, column 19: Expecting token of type ':' but found `\n`.",
                    1,
                    1,
                )),
            ),
            (
                "architecture-beta\n    service a(se rver)[A]\n",
                Some((
                    "Parsing failed: Lexer error on line 2, column 14: unexpected character: ->(<- at offset: 31, skipped 1 characters.\nLexer error on line 2, column 22: unexpected character: ->)<- at offset: 39, skipped 1 characters. Parse error on line 2, column 15: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: 'se'\nParse error on line 2, column 18: Expecting token of type ':' but found `rver`.\nParse error on line 2, column 23: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: '[A]'",
                    2,
                    14,
                )),
            ),
            (
                "architecture-beta\n    group g(cloud)[G] in\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 25: Expecting token of type 'ID' but found `\n`.",
                    2,
                    25,
                )),
            ),
            (
                "architecture-beta\n    align row a\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 16: Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[ID]>\nbut found: '\n'",
                    2,
                    16,
                )),
            ),
            (
                "architecture-beta\n    align diag a b\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 11: Expecting: one of these possible Token sequences:\n  1. [row]\n  2. [column]\nbut found: 'diag'\nParse error on line 2, column 16: Expecting token of type ':' but found `a`.\nParse error on line 2, column 18: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: 'b'\nParse error on line 2, column 19: Expecting token of type ':' but found `\n`.",
                    2,
                    11,
                )),
            ),
            (
                "architecture-beta\n    a:L -[x] R:b\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 14: Expecting token of type '-' but found `R`.",
                    2,
                    14,
                )),
            ),
            (
                "\narchitecture-beta\n  service a(server)[A]\n  accTitle: t\n  accDescr: d\n",
                None,
            ),
            (
                "architecture\n",
                Some((
                    "Parsing failed:  Parse error on line 1, column 1: Expecting token of type 'architecture-beta' but found `architecture`.\nParse error on line 1, column 13: Expecting token of type ':' but found `\n`.",
                    1,
                    1,
                )),
            ),
            (
                "architecture-beta\n  title x\n  service a(server)[A]\n",
                None,
            ),
            (
                "architecture-beta\n  service a(server)[A] @\n",
                Some((
                    "Parsing failed: Lexer error on line 2, column 24: unexpected character: ->@<- at offset: 41, skipped 1 characters. ",
                    2,
                    24,
                )),
            ),
            (
                "architecture-beta\n  service 'x'\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 11: Expecting token of type 'ID' but found `'x'`.",
                    2,
                    11,
                )),
            ),
            (
                "architecture-beta\n  junction j in\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 16: Expecting token of type 'ID' but found `\n`.",
                    2,
                    16,
                )),
            ),
            (
                "architecture-beta\n  service a(server)[ A title ] in g\n  junction j1 in g\n  group g\n",
                None,
            ),
            (
                "architecture-beta\n  a:L -- R\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 11: Expecting token of type ':' but found `\n`.",
                    2,
                    11,
                )),
            ),
            (
                "architecture-beta\n  :L -- R:b\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 3: Expecting token of type 'EOF' but found `:`.\nParse error on line 2, column 12: Expecting token of type ':' but found `\n`.",
                    2,
                    3,
                )),
            ),
            (
                "hello\narchitecture-beta\n",
                Some((
                    "Parsing failed:  Parse error on line 1, column 1: Expecting token of type 'architecture-beta' but found `hello`.\nParse error on line 1, column 6: Expecting token of type ':' but found `\n`.\nParse error on line 2, column 1: Expecting token of type 'EOF' but found `architecture-beta`.",
                    1,
                    1,
                )),
            ),
            (
                "architecture-beta\n  service a(server)[A]\n  [x]\n",
                Some((
                    "Parsing failed:  Parse error on line 3, column 3: Expecting token of type 'EOF' but found `[x]`.",
                    3,
                    3,
                )),
            ),
            (
                "architecture-beta\n  service a(server)[A] in\n  service b\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 26: Expecting token of type 'ID' but found `\n`.",
                    2,
                    26,
                )),
            ),
            (
                "architecture-beta\n  a:LR -- L:b\n",
                Some((
                    "Parsing failed:  Parse error on line 2, column 6: Expecting: one of these possible Token sequences:\n  1. [--]\n  2. [-]\nbut found: 'R'",
                    2,
                    6,
                )),
            ),
        ];
        for (input, want) in cases {
            match (parse(input), want) {
                (Ok(_), None) => {}
                (Ok(_), Some((message, _, _))) => panic!("{input:?} parsed, want {message:?}"),
                (Err(e), None) => panic!("{input:?}: {}", e.message),
                (Err(e), Some((message, line, column))) => {
                    assert_eq!(e.message, *message, "{input:?}");
                    assert_eq!((e.line, e.column), (*line, *column), "{input:?}");
                }
            }
        }
    }
}
