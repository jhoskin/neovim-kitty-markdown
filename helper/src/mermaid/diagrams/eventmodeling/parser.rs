//! A hand-written lexer and parser for the language of
//! `event-modeling.langium` (`@mermaid-js/parser` 2.0.1, mermaid 12.2.0).
//! The grammar imports nothing from `common.langium`; it has its own
//! `EM_*` terminals.
//!
//! Langium builds a Chevrotain lexer from the grammar. The token order,
//! read from `TokenBuilder.buildTokens` of the oracle package, is the
//! keywords longest first with ties in grammar order (`eventmodeling`,
//! `resetframe`, `readmodel`, ... `tf`, `[[`, `]]`, `rf`, `` ` ``, `.`),
//! then the terminals in grammar order: `EM_ID`, `EM_FID`,
//! `EM_DATA_INLINE`, `EM_DATA_BLOCK`, `EM_ACC_DESCR`, `EM_ACC_TITLE`,
//! `EM_TITLE`, and the hidden `EM_WS`, `EM_YAML`, `EM_DIRECTIVE`,
//! `EM_SINGLE_LINE_COMMENT`, `EM_ML_COMMENT`, `EM_SL_COMMENT`. Every word
//! keyword carries `EM_ID` as its `LONGER_ALT`, so `tfx` is an `EM_ID`.
//! `EventModelingTokenBuilder` adds the `(?:(?=%%)|(?!\S))` suffix to
//! `eventmodeling`. `EM_ID` comes before `EM_TITLE`, so a `title` at the
//! start of a word is an `EM_ID`; only a `title` after a blank on the same
//! line, as in `eventmodeling title X`, is a title token. At each position
//! the first token type that matches wins; a position where none matches
//! is skipped and reported as one lexer error per run of skipped
//! characters.
//!
//! The parser is an LL walk of the `EventModel` rule with the error texts
//! of Langium's parser error provider. It reports the first parser error,
//! where Langium's recovering parser can report several. `MermaidParseError`
//! joins both lists as `Parsing failed: <lexer errors> <parser errors>`.

use std::fmt::Write;

use crate::mermaid::theme::khroma::js_is_whitespace;

/// The `EventModel` AST. Langium's default value converter returns the
/// image of every `EM_*` terminal unchanged.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ast {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub model_entities: Vec<String>,
    pub frames: Vec<Frame>,
    pub data_entities: Vec<DataEntity>,
    pub note_entities: Vec<NoteEntity>,
    pub gwt_entities: Vec<Gwt>,
}

/// `EmTimeFrame` or `EmResetFrame`.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub reset: bool,
    /// The `EM_FID` image, for example `01`.
    pub name: String,
    pub model_entity_type: String,
    /// `QualifiedName`: the `EM_ID` images joined with `.`.
    pub entity_identifier: String,
    /// The `$refText` of each `->>` source.
    pub source_frames: Vec<String>,
    /// The `$refText` of `[[...]]`.
    pub data_reference: Option<String>,
    pub data_type: Option<String>,
    pub data_inline_value: Option<String>,
    /// 0-based line of the frame keyword, `$cstNode.range.start.line`.
    pub line: u32,
}

/// `EmDataEntity: 'data' name=EM_EID EmDataBlock`.
#[derive(Debug, Clone, PartialEq)]
pub struct DataEntity {
    pub name: String,
    pub data_type: Option<String>,
    pub data_block_value: String,
}

/// `EmNoteEntity: 'note' sourceFrame=[EmFrame:EM_FI] EmDataBlock`.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteEntity {
    pub source_frame: String,
    pub data_type: Option<String>,
    pub data_block_value: String,
}

/// `EmGwt`; the renderer draws none of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Gwt {
    pub source_frame: String,
    pub given: Vec<GwtStatement>,
    pub when: Vec<GwtStatement>,
    pub then: Vec<GwtStatement>,
}

/// `EmGwtStatement: EmModelEntityType entityIdentifier=[EmModelEntity:EM_EID]`.
#[derive(Debug, Clone, PartialEq)]
pub struct GwtStatement {
    pub model_entity_type: String,
    pub entity_identifier: String,
}

/// What `MermaidParseError` carries: the joined message and the position of
/// the first error (lexer errors first, as mermaid prints them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFailure {
    pub message: String,
    pub line: u32,
    /// 1-based; 0 when Chevrotain has no position (an inserted EOF token).
    pub column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A keyword; the text is its Chevrotain name.
    Kw(&'static str),
    Id,
    Fid,
    DataInline,
    DataBlock,
    AccDescr,
    AccTitle,
    Title,
    Eof,
}

impl Kind {
    /// Chevrotain's `tokenType.name`: the keyword text for keywords, the
    /// terminal rule name otherwise.
    fn name(self) -> &'static str {
        match self {
            Kind::Kw(k) => k,
            Kind::Id => "EM_ID",
            Kind::Fid => "EM_FID",
            Kind::DataInline => "EM_DATA_INLINE",
            Kind::DataBlock => "EM_DATA_BLOCK",
            Kind::AccDescr => "EM_ACC_DESCR",
            Kind::AccTitle => "EM_ACC_TITLE",
            Kind::Title => "EM_TITLE",
            Kind::Eof => "EOF",
        }
    }
}

/// The keywords in Chevrotain's order; `true` marks the word keywords,
/// which have `EM_ID` as their `LONGER_ALT`. `eventmodeling` is matched
/// first, apart, for its boundary suffix.
const KEYWORDS: [(&str, bool); 32] = [
    ("resetframe", true),
    ("readmodel", true),
    ("processor", true),
    ("timeframe", true),
    ("command", true),
    ("entity", true),
    ("event", true),
    ("jsobj", true),
    ("figma", true),
    ("given", true),
    ("json", true),
    ("salt", true),
    ("html", true),
    ("text", true),
    ("data", true),
    ("note", true),
    ("when", true),
    ("then", true),
    ("rmo", true),
    ("cmd", true),
    ("evt", true),
    ("pcr", true),
    ("uri", true),
    ("->>", false),
    ("gwt", true),
    ("ui", true),
    ("md", true),
    ("tf", true),
    ("[[", false),
    ("]]", false),
    ("rf", true),
    ("`", false),
];

/// `EmModelEntityType` in grammar order.
const MODEL_ENTITY_TYPES: [&str; 9] = [
    "rmo",
    "readmodel",
    "ui",
    "cmd",
    "command",
    "evt",
    "event",
    "pcr",
    "processor",
];

/// `EmDataType` in grammar order.
const DATA_TYPES: [&str; 8] = [
    "json", "jsobj", "figma", "salt", "uri", "md", "html", "text",
];

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

/// A token matcher: the end of the match at a position, if any.
type Matcher<'a> = fn(&Lexer<'a>, usize) -> Option<usize>;

struct Lexer<'a> {
    chars: &'a [char],
    /// UTF-16 offset of each character, plus one past the end.
    utf16: Vec<usize>,
}

/// JS `.`: anything but a line terminator.
fn is_js_dot(c: char) -> bool {
    !matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
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

    /// The end of `[\t ]*` from `i`.
    fn blanks(&self, i: usize) -> usize {
        let mut j = i;
        while matches!(self.at(j), Some(' ') | Some('\t')) {
            j += 1;
        }
        j
    }

    /// The end of `\s*` with JS whitespace from `i`.
    fn js_spaces(&self, i: usize) -> usize {
        let mut j = i;
        while let Some(c) = self.at(j)
            && js_is_whitespace(c)
        {
            j += 1;
        }
        j
    }

    /// `\s+`.
    fn whitespace(&self, i: usize) -> Option<usize> {
        let j = self.js_spaces(i);
        (j > i).then_some(j)
    }

    /// `EM_ID`: `[_a-zA-Z][\w_]*` with ASCII `\w`.
    fn id(&self, i: usize) -> Option<usize> {
        let first = self.at(i)?;
        if !(first.is_ascii_alphabetic() || first == '_') {
            return None;
        }
        let mut j = i + 1;
        while self
            .at(j)
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            j += 1;
        }
        Some(j)
    }

    /// `EM_FID`: `\d{1,3}`.
    fn fid(&self, i: usize) -> Option<usize> {
        let mut j = i;
        while j - i < 3 && self.at(j).is_some_and(|c| c.is_ascii_digit()) {
            j += 1;
        }
        (j > i).then_some(j)
    }

    /// `EM_DATA_INLINE`: `\{(.*)\}|"(.*)"|'(.*)'`. The greedy `.*` reaches
    /// the last closing character on the line.
    fn data_inline(&self, i: usize) -> Option<usize> {
        let close = match self.at(i)? {
            '{' => '}',
            '"' => '"',
            '\'' => '\'',
            _ => return None,
        };
        let mut j = i + 1;
        let mut last = None;
        while let Some(c) = self.at(j)
            && is_js_dot(c)
        {
            j += 1;
            if c == close {
                last = Some(j);
            }
        }
        last
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

    /// `EM_DATA_BLOCK`:
    /// `\{[\t ]*\r?\n(?:[\S\s]*?\r?\n)?\}(?:\r?\n|(?!\S))`. The optional
    /// group is tried first with the shortest content; only when no
    /// newline-and-brace closes it does the brace right after the first
    /// newline count.
    fn data_block(&self, i: usize) -> Option<usize> {
        if self.at(i) != Some('{') {
            return None;
        }
        let first = self.newline(self.blanks(i + 1))?;
        let mut k = first;
        while k < self.chars.len() {
            if let Some(after) = self.newline(k)
                && self.at(after) == Some('}')
                && let Some(end) = self.block_end(after + 1)
            {
                return Some(end);
            }
            k += 1;
        }
        if self.at(first) == Some('}') {
            return self.block_end(first + 1);
        }
        None
    }

    /// `[\t ]*accDescr(?:[\t ]*:([^\n\r]*?(?=%%)|[^\n\r]*)|\s*{([^}]*)})`.
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

    /// `[\t ]*%%{[\S\s]*?}%%(?:\r?\n|(?!\S))`; `EM_WS` comes first in the
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

    /// `\/\*[\s\S]*?\*\/`.
    fn ml_comment(&self, i: usize) -> Option<usize> {
        if !self.starts_with(i, "/*") {
            return None;
        }
        let mut k = i + 2;
        while k + 2 <= self.chars.len() {
            if self.starts_with(k, "*/") {
                return Some(k + 2);
            }
            k += 1;
        }
        None
    }

    /// `\/\/[^\n\r]*`.
    fn sl_comment(&self, i: usize) -> Option<usize> {
        self.starts_with(i, "//").then(|| self.line_end(i + 2))
    }

    /// A keyword, unless its `LONGER_ALT` `EM_ID` matches a longer image.
    fn keyword(&self, i: usize, word: &str, longer_alt: bool) -> Option<usize> {
        let end = self.literal(i, word)?;
        if longer_alt && self.id(i).is_some_and(|id_end| id_end > end) {
            return None;
        }
        Some(end)
    }

    /// The first token type that matches at `i`, in Chevrotain's order;
    /// `None` as the kind marks a hidden terminal.
    fn next(&self, i: usize) -> Option<(Option<Kind>, usize)> {
        // `eventmodeling(?:(?=%%)|(?!\S))`, then its `LONGER_ALT`.
        if let Some(end) = self.literal(i, "eventmodeling")
            && match self.at(end) {
                None => true,
                Some('%') => self.at(end + 1) == Some('%'),
                Some(c) => js_is_whitespace(c),
            }
            && self.id(i).is_none_or(|id_end| id_end <= end)
        {
            return Some((Some(Kind::Kw("eventmodeling")), end));
        }
        for (word, longer_alt) in KEYWORDS {
            if let Some(end) = self.keyword(i, word, longer_alt) {
                return Some((Some(Kind::Kw(word)), end));
            }
        }
        if let Some(end) = self.literal(i, ".") {
            return Some((Some(Kind::Kw(".")), end));
        }
        let table: [(Option<Kind>, Matcher<'a>); 13] = [
            (Some(Kind::Id), Lexer::id),
            (Some(Kind::Fid), Lexer::fid),
            (Some(Kind::DataInline), Lexer::data_inline),
            (Some(Kind::DataBlock), Lexer::data_block),
            (Some(Kind::AccDescr), Lexer::acc_descr),
            (Some(Kind::AccTitle), Lexer::acc_title),
            (Some(Kind::Title), Lexer::title),
            (None, Lexer::whitespace),
            (None, Lexer::yaml),
            (None, Lexer::directive),
            (None, Lexer::comment),
            (None, Lexer::ml_comment),
            (None, Lexer::sl_comment),
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

struct Parser<'a> {
    chars: &'a [char],
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn is_kw(&self, word: &str) -> bool {
        self.peek().kind == Kind::Kw(keyword_name(word))
    }

    fn is_any_kw(&self, words: &[&str]) -> bool {
        words.iter().any(|w| self.is_kw(w))
    }

    fn image(&self, t: &Token) -> String {
        self.chars[t.start..t.end].iter().collect()
    }

    fn advance(&mut self) -> Token {
        let t = self.peek().clone();
        if t.kind != Kind::Eof {
            self.pos += 1;
        }
        t
    }

    fn error_at_peek(&self, message: String) -> ParseError {
        let t = self.peek();
        ParseError {
            line: t.line,
            column: t.column,
            message,
        }
    }

    /// `MismatchedTokenException` from Langium's parser error provider.
    fn mismatched(&self, expected: Kind) -> ParseError {
        let found = self.image(self.peek());
        self.error_at_peek(format!(
            "Expecting token of type '{}' but found `{found}`.",
            expected.name()
        ))
    }

    /// `NoViableAltException` from Chevrotain's default error provider,
    /// one line per one-token sequence.
    fn no_viable_alt(&self, alternatives: &[&str]) -> ParseError {
        let mut message = "Expecting: one of these possible Token sequences:".to_string();
        for (i, alt) in alternatives.iter().enumerate() {
            let _ = write!(message, "\n  {}. [{alt}]", i + 1);
        }
        let _ = write!(message, "\nbut found: '{}'", self.image(self.peek()));
        self.error_at_peek(message)
    }

    /// `EarlyExitException` of an `AT_LEAST_ONE` whose iteration starts
    /// with one of `alternatives`.
    fn early_exit(&self, alternatives: &[&str]) -> ParseError {
        let list = alternatives
            .iter()
            .map(|a| format!("[{a}]"))
            .collect::<Vec<_>>()
            .join(" ,");
        let found = self.image(self.peek());
        self.error_at_peek(format!(
            "Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <{list}>\nbut found: '{found}'"
        ))
    }

    fn expect(&mut self, kind: Kind) -> Result<Token, ParseError> {
        if self.peek().kind == kind {
            Ok(self.advance())
        } else {
            Err(self.mismatched(kind))
        }
    }

    fn expect_image(&mut self, kind: Kind) -> Result<String, ParseError> {
        let t = self.expect(kind)?;
        Ok(self.image(&t))
    }

    /// One keyword out of `alternatives`, else a `NoViableAltException`.
    fn one_of(&mut self, alternatives: &[&str]) -> Result<String, ParseError> {
        if self.is_any_kw(alternatives) {
            let t = self.advance();
            Ok(self.image(&t))
        } else {
            Err(self.no_viable_alt(alternatives))
        }
    }

    /// `QualifiedName: EM_EID ('.' EM_EID)*`, a data type rule whose value
    /// is the token images joined without the hidden whitespace.
    fn qualified_name(&mut self) -> Result<String, ParseError> {
        let mut name = self.expect_image(Kind::Id)?;
        while self.is_kw(".") {
            self.advance();
            name.push('.');
            name.push_str(&self.expect_image(Kind::Id)?);
        }
        Ok(name)
    }

    /// `` ('`' dataType=EmDataType '`')? ``.
    fn data_type(&mut self) -> Result<Option<String>, ParseError> {
        if !self.is_kw("`") {
            return Ok(None);
        }
        self.advance();
        let data_type = self.one_of(&DATA_TYPES)?;
        self.expect(Kind::Kw("`"))?;
        Ok(Some(data_type))
    }

    /// `EmTimeFrame` and `EmResetFrame` after their keyword.
    fn frame(&mut self, reset: bool, line: u32) -> Result<Frame, ParseError> {
        let name = self.expect_image(Kind::Fid)?;
        let model_entity_type = self.one_of(&MODEL_ENTITY_TYPES)?;
        let entity_identifier = self.qualified_name()?;
        let mut source_frames = Vec::new();
        while self.is_kw("->>") {
            self.advance();
            source_frames.push(self.expect_image(Kind::Fid)?);
        }
        let mut data_reference = None;
        if self.is_kw("[[") {
            self.advance();
            data_reference = Some(self.expect_image(Kind::Id)?);
            self.expect(Kind::Kw("]]"))?;
        }
        let mut data_type = None;
        let mut data_inline_value = None;
        if self.is_kw("`") || self.peek().kind == Kind::DataInline {
            data_type = self.data_type()?;
            data_inline_value = Some(self.expect_image(Kind::DataInline)?);
        }
        Ok(Frame {
            reset,
            name,
            model_entity_type,
            entity_identifier,
            source_frames,
            data_reference,
            data_type,
            data_inline_value,
            line,
        })
    }

    /// `fragment EmDataBlock`.
    fn data_block(&mut self) -> Result<(Option<String>, String), ParseError> {
        let data_type = self.data_type()?;
        let value = self.expect_image(Kind::DataBlock)?;
        Ok((data_type, value))
    }

    /// `EmGwtStatement+`.
    fn gwt_statements(&mut self) -> Result<Vec<GwtStatement>, ParseError> {
        if !self.is_any_kw(&MODEL_ENTITY_TYPES) {
            return Err(self.early_exit(&MODEL_ENTITY_TYPES));
        }
        let mut statements = Vec::new();
        while self.is_any_kw(&MODEL_ENTITY_TYPES) {
            let t = self.advance();
            let model_entity_type = self.image(&t);
            let entity_identifier = self.expect_image(Kind::Id)?;
            statements.push(GwtStatement {
                model_entity_type,
                entity_identifier,
            });
        }
        Ok(statements)
    }

    fn gwt(&mut self) -> Result<Gwt, ParseError> {
        let source_frame = self.expect_image(Kind::Fid)?;
        self.expect(Kind::Kw("given"))?;
        let given = self.gwt_statements()?;
        let mut when = Vec::new();
        if self.is_kw("when") {
            self.advance();
            when = self.gwt_statements()?;
        }
        self.expect(Kind::Kw("then"))?;
        let then = self.gwt_statements()?;
        Ok(Gwt {
            source_frame,
            given,
            when,
            then,
        })
    }

    /// The `EventModel` entry rule, then the end of input.
    fn event_model(&mut self) -> Result<Ast, ParseError> {
        let mut ast = Ast::default();
        self.expect(Kind::Kw("eventmodeling"))?;
        loop {
            let t = self.peek().clone();
            match t.kind {
                Kind::AccDescr => {
                    self.advance();
                    ast.acc_descr = Some(self.image(&t));
                }
                Kind::AccTitle => {
                    self.advance();
                    ast.acc_title = Some(self.image(&t));
                }
                Kind::Title => {
                    self.advance();
                    ast.title = Some(self.image(&t));
                }
                Kind::Kw("entity") => {
                    self.advance();
                    let name = self.qualified_name()?;
                    ast.model_entities.push(name);
                }
                Kind::Kw("tf" | "timeframe" | "rf" | "resetframe") => {
                    self.advance();
                    let reset = matches!(t.kind, Kind::Kw("rf" | "resetframe"));
                    // Chevrotain lines are 1-based; the CST range is 0-based.
                    let frame = self.frame(reset, t.line.saturating_sub(1))?;
                    ast.frames.push(frame);
                }
                Kind::Kw("data") => {
                    self.advance();
                    let name = self.expect_image(Kind::Id)?;
                    let (data_type, data_block_value) = self.data_block()?;
                    ast.data_entities.push(DataEntity {
                        name,
                        data_type,
                        data_block_value,
                    });
                }
                Kind::Kw("note") => {
                    self.advance();
                    let source_frame = self.expect_image(Kind::Fid)?;
                    let (data_type, data_block_value) = self.data_block()?;
                    ast.note_entities.push(NoteEntity {
                        source_frame,
                        data_type,
                        data_block_value,
                    });
                }
                Kind::Kw("gwt") => {
                    self.advance();
                    let gwt = self.gwt()?;
                    ast.gwt_entities.push(gwt);
                }
                _ => break,
            }
        }
        self.expect(Kind::Eof)?;
        Ok(ast)
    }
}

/// The `'static` keyword name for a keyword text.
fn keyword_name(word: &str) -> &'static str {
    if word == "eventmodeling" {
        return "eventmodeling";
    }
    if word == "." {
        return ".";
    }
    KEYWORDS
        .iter()
        .find(|(k, _)| *k == word)
        .map_or("", |(k, _)| k)
}

/// `formatLocation`: Chevrotain marks a missing position (the EOF token)
/// with `NaN` or `-1`, printed as `?`.
fn location(v: u32) -> String {
    if v == 0 {
        "?".to_string()
    } else {
        v.to_string()
    }
}

/// `parse('eventmodeling', text)`: the AST, or the `MermaidParseError`
/// text with the position of its first error.
pub fn parse(text: &str) -> Result<Ast, ParseFailure> {
    let chars: Vec<char> = text.chars().collect();
    let lexer = Lexer::new(&chars);
    let (tokens, lex_errors) = lexer.tokenize();
    let mut parser = Parser {
        chars: &chars,
        tokens,
        pos: 0,
    };
    let parsed = parser.event_model();
    match (&lex_errors[..], parsed) {
        ([], Ok(ast)) => Ok(ast),
        (lex, parsed) => {
            let mut lexer_text = String::new();
            for (i, e) in lex.iter().enumerate() {
                if i > 0 {
                    lexer_text.push('\n');
                }
                let _ = write!(
                    lexer_text,
                    "Lexer error on line {}, column {}: unexpected character: ->{}<- at offset: {}, skipped {} characters.",
                    e.line, e.column, e.first, e.offset, e.skipped
                );
            }
            let parser_text = match &parsed {
                Err(e) => format!(
                    "Parse error on line {}, column {}: {}",
                    location(e.line),
                    location(e.column),
                    e.message
                ),
                Ok(_) => String::new(),
            };
            let (line, column) = match (lex.first(), &parsed) {
                (Some(e), _) => (e.line, e.column),
                (None, Err(e)) => (e.line, e.column),
                (None, Ok(_)) => (0, 0),
            };
            Err(ParseFailure {
                message: format!("Parsing failed: {lexer_text} {parser_text}"),
                line,
                column,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(text: &str) -> Ast {
        parse(text).unwrap_or_else(|e| panic!("{text:?}: {}", e.message))
    }

    /// Frames as the oracle probe prints them: kind, name, entity type,
    /// identifier, sources, data reference, data type, inline value.
    fn frames(ast: &Ast) -> Vec<String> {
        ast.frames
            .iter()
            .map(|f| {
                format!(
                    "{} {} {} {} {:?} {:?} {:?} {:?}",
                    if f.reset { "rf" } else { "tf" },
                    f.name,
                    f.model_entity_type,
                    f.entity_identifier,
                    f.source_frames,
                    f.data_reference,
                    f.data_type,
                    f.data_inline_value
                )
            })
            .collect()
    }

    #[test]
    fn frames_match_the_oracle_ast() {
        let cases: [(&str, &[&str]); 9] = [
            (
                "eventmodeling\ntf 02 cmd A . b\n",
                &["tf 02 cmd A.b [] None None None"],
            ),
            (
                "eventmodeling\ntf 02 cmd A.b.c\n",
                &["tf 02 cmd A.b.c [] None None None"],
            ),
            (
                "eventmodeling\ntf 02 cmd A ->> 01 ->> 03 [[D]] { x }\n",
                &["tf 02 cmd A [\"01\", \"03\"] Some(\"D\") None Some(\"{ x }\")"],
            ),
            (
                "eventmodeling\ntf 02 cmd A `json` { \"a\": 1 }\n",
                &["tf 02 cmd A [] None Some(\"json\") Some(\"{ \\\"a\\\": 1 }\")"],
            ),
            (
                "eventmodeling\ntf 02 cmd A \"quoted\"\n",
                &["tf 02 cmd A [] None None Some(\"\\\"quoted\\\"\")"],
            ),
            (
                "eventmodeling\ntf 01 cmd A { a } b }\n",
                &["tf 01 cmd A [] None None Some(\"{ a } b }\")"],
            ),
            (
                "eventmodeling\ntimeframe 01 command A\nresetframe 02 event B ->> 01\n",
                &[
                    "tf 01 command A [] None None None",
                    "rf 02 event B [\"01\"] None None None",
                ],
            ),
            (
                "eventmodeling\ntf 01 cmd A /* c */ ->> 02 // x\n",
                &["tf 01 cmd A [\"02\"] None None None"],
            ),
            (
                "eventmodeling\ntf 01 cmd tfx\n",
                &["tf 01 cmd tfx [] None None None"],
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(frames(&ok(text)), expected, "{text:?}");
        }
    }

    #[test]
    fn data_notes_gwt_and_headers() {
        let ast = ok(
            "eventmodeling\ntf 02 cmd AddItem [[AddItem01]]\ndata AddItem01 {\n  description: 'john'\n  price: 20.4\n}\n",
        );
        assert_eq!(
            ast.data_entities[0].data_block_value,
            "{\n  description: 'john'\n  price: 20.4\n}\n"
        );
        let ast = ok("eventmodeling\ndata D {\n}\ntf 01 cmd A [[D]]\n");
        assert_eq!(ast.data_entities[0].data_block_value, "{\n}\n");
        let ast = ok("eventmodeling\nnote 01 `md`{\n hi\n}\n");
        assert_eq!(ast.note_entities[0].data_type.as_deref(), Some("md"));
        assert_eq!(ast.note_entities[0].data_block_value, "{\n hi\n}\n");
        let ast = ok(
            "eventmodeling\nnote 01 {\n hi\n}\ngwt 01 given evt E when cmd C then evt F\nentity E\n",
        );
        assert_eq!(ast.model_entities, vec!["E".to_string()]);
        assert_eq!(ast.gwt_entities[0].given.len(), 1);
        assert_eq!(ast.gwt_entities[0].when.len(), 1);
        assert_eq!(ast.gwt_entities[0].then.len(), 1);
        let ast = ok("eventmodeling title Hello\ntf 02 cmd A\n");
        assert_eq!(ast.title.as_deref(), Some(" title Hello"));
        for text in [
            "eventmodeling\n",
            "eventmodeling",
            "\neventmodeling\ntf 01 cmd A\n",
            "eventmodeling%% x\ntf 01 cmd A\n",
            "  eventmodeling\ntf 01 cmd A\n",
            "eventmodeling\ntf 01 cmd A\ntf 01 evt B\n",
        ] {
            ok(text);
        }
    }

    /// The oracle's messages, cut before a second parser error: the helper
    /// reports the first one only.
    #[test]
    fn errors_match_the_oracle() {
        let types = "Expecting: one of these possible Token sequences:\n  1. [rmo]\n  2. [readmodel]\n  3. [ui]\n  4. [cmd]\n  5. [command]\n  6. [evt]\n  7. [event]\n  8. [pcr]\n  9. [processor]\nbut found: ";
        let iteration = "Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[rmo] ,[readmodel] ,[ui] ,[cmd] ,[command] ,[evt] ,[event] ,[pcr] ,[processor]>\nbut found: ";
        let cases: Vec<(&str, String, (u32, u32))> = vec![
            (
                "eventmodeling\ntitle Hello\ntf 02 cmd A\n",
                "Parsing failed:  Parse error on line 2, column 1: Expecting token of type 'EOF' but found `title`.".into(),
                (2, 1),
            ),
            (
                "eventmodeling\n  title Hello\ntf 02 cmd A\n",
                "Parsing failed:  Parse error on line 2, column 3: Expecting token of type 'EOF' but found `title`.".into(),
                (2, 3),
            ),
            (
                "eventmodeling\ntf 02 cmd ui\n",
                "Parsing failed:  Parse error on line 2, column 11: Expecting token of type 'EM_ID' but found `ui`.".into(),
                (2, 11),
            ),
            (
                "eventmodeling\ntf 0123 cmd A\n",
                format!("Parsing failed:  Parse error on line 2, column 7: {types}'3'"),
                (2, 7),
            ),
            (
                "eventmodeling\ntf 02 foo A\n",
                format!("Parsing failed:  Parse error on line 2, column 7: {types}'foo'"),
                (2, 7),
            ),
            (
                "eventmodeling\ntf 01\n",
                format!("Parsing failed:  Parse error on line ?, column ?: {types}''"),
                (0, 0),
            ),
            (
                "eventmodeling\ntf 01 cmd\n",
                "Parsing failed:  Parse error on line ?, column ?: Expecting token of type 'EM_ID' but found ``.".into(),
                (0, 0),
            ),
            (
                "eventmodeling\ntf 01 cmd A ->>\n",
                "Parsing failed:  Parse error on line ?, column ?: Expecting token of type 'EM_FID' but found ``.".into(),
                (0, 0),
            ),
            (
                "eventmodeling\ntf 01 cmd A [[ B\ntf 02 evt C\n",
                "Parsing failed:  Parse error on line 3, column 1: Expecting token of type ']]' but found `tf`.".into(),
                (3, 1),
            ),
            (
                "eventmodelingx\ntf 01 cmd A\n",
                "Parsing failed:  Parse error on line 1, column 1: Expecting token of type 'eventmodeling' but found `eventmodelingx`.".into(),
                (1, 1),
            ),
            (
                "eventmodeling\ntf 01 cmd A #\n",
                "Parsing failed: Lexer error on line 2, column 13: unexpected character: ->#<- at offset: 26, skipped 1 characters. ".into(),
                (2, 13),
            ),
            (
                "eventmodeling\ntf 01 cmd A $$ tf 02 evt B\n",
                "Parsing failed: Lexer error on line 2, column 13: unexpected character: ->$<- at offset: 26, skipped 2 characters. ".into(),
                (2, 13),
            ),
            (
                "eventmodeling\ntf 02 cmd A\n%% comment\n// sl\n/* ml */\naccTitle: hi\n",
                "Parsing failed: Lexer error on line 6, column 9: unexpected character: ->:<- at offset: 60, skipped 1 characters. Parse error on line 6, column 1: Expecting token of type 'EOF' but found `accTitle`.".into(),
                (6, 9),
            ),
            (
                "eventmodeling\n accDescr { multi\n line }\ntf 01 cmd A\n",
                "Parsing failed: Lexer error on line 2, column 11: unexpected character: ->{<- at offset: 24, skipped 1 characters.\nLexer error on line 3, column 7: unexpected character: ->}<- at offset: 38, skipped 1 characters. Parse error on line 2, column 2: Expecting token of type 'EOF' but found `accDescr`.".into(),
                (2, 11),
            ),
            (
                "eventmodeling\ndata D {x}\n",
                "Parsing failed:  Parse error on line 2, column 8: Expecting token of type 'EM_DATA_BLOCK' but found `{x}`.".into(),
                (2, 8),
            ),
            (
                "eventmodeling\ngwt 01 given\n",
                format!("Parsing failed:  Parse error on line ?, column ?: {iteration}''"),
                (0, 0),
            ),
            (
                "eventmodeling\ngwt 01 given evt E\n",
                "Parsing failed:  Parse error on line ?, column ?: Expecting token of type 'then' but found ``.".into(),
                (0, 0),
            ),
            (
                "eventmodeling\ngwt 01 given evt E when then evt F\n",
                format!("Parsing failed:  Parse error on line 2, column 25: {iteration}'then'"),
                (2, 25),
            ),
            (
                "eventmodeling\ntf 01 cmd A `xml` { a }\n",
                "Parsing failed:  Parse error on line 2, column 14: Expecting: one of these possible Token sequences:\n  1. [json]\n  2. [jsobj]\n  3. [figma]\n  4. [salt]\n  5. [uri]\n  6. [md]\n  7. [html]\n  8. [text]\nbut found: 'xml'".into(),
                (2, 14),
            ),
            (
                "eventmodeling\ntf 01 cmd A `json` B\n",
                "Parsing failed:  Parse error on line 2, column 20: Expecting token of type 'EM_DATA_INLINE' but found `B`.".into(),
                (2, 20),
            ),
            (
                "eventmodeling\ntf 01 cmd A.\n",
                "Parsing failed:  Parse error on line ?, column ?: Expecting token of type 'EM_ID' but found ``.".into(),
                (0, 0),
            ),
            (
                "eventmodeling\nentity\n",
                "Parsing failed:  Parse error on line ?, column ?: Expecting token of type 'EM_ID' but found ``.".into(),
                (0, 0),
            ),
            (
                "eventmodeling\ntf a1 cmd A\n",
                "Parsing failed:  Parse error on line 2, column 4: Expecting token of type 'EM_FID' but found `a1`.".into(),
                (2, 4),
            ),
        ];
        for (text, message, position) in cases {
            let err = parse(text).expect_err(text);
            assert_eq!(err.message, message, "{text:?}");
            assert_eq!((err.line, err.column), position, "{text:?}");
        }
    }
}
