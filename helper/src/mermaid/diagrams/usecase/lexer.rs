//! Port of `usecase.tokens.ts` and the Chevrotain 13 lexer that runs it
//! (`new Lexer(usecaseLexerModes)` in `usecase.lexer.ts`).
//!
//! Chevrotain tries the token types of the current mode in order and takes
//! the first that matches at the offset; it does not look for the longest
//! match. A token type with a `longer_alt` is replaced by that type when
//! the alternative matches more text (one level only). `HWS` is skipped.
//! `JSON_DECLARATION_START` pushes the `jsonBody` mode, `<<` pushes the
//! `stereotype` mode, and their closing tokens pop it. Where no type
//! matches, the lexer records an error and skips characters until one
//! matches again; `runChevrotainParse` reports the first such error.

use std::sync::LazyLock;

use regex::Regex;

/// The token types, named as `createToken` names them in the messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    MarkdownString,
    UnclosedMarkdownString,
    Comment,
    Newline,
    AccDescrBlock,
    AccTitleLine,
    AccDescrLine,
    JsonDeclarationStart,
    Usecase,
    Actor,
    SystemBoundary,
    End,
    Direction,
    Td,
    Tb,
    Bt,
    Lr,
    Rl,
    Note,
    For,
    Json,
    ClassDef,
    Class,
    Style,
    Include,
    Extend,
    True,
    False,
    Generalization,
    DependencyArrow,
    StereotypeStart,
    ClassSeparator,
    ForwardSolid,
    BackwardSolid,
    ForwardCircle,
    BackwardCircle,
    ForwardCross,
    BackwardCross,
    MarkerlessSolid,
    MetadataStart,
    At,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    LeftParen,
    RightParen,
    Comma,
    Colon,
    HashColor,
    PlainString,
    CssIdentifier,
    Identifier,
    Number,
    CssEscapedComma,
    Dash,
    Dot,
    Percent,
    CssPunctuation,
    LabelPunctuation,
    LabelSymbol,
    JsonObjectLiteral,
    UnclosedJsonObjectLiteral,
    StereotypeEnd,
    StereotypeText,
    UnclosedStereotypeText,
    Eof,
}

impl Kind {
    /// The `name` of the token type, which Chevrotain prints in messages.
    pub fn name(self) -> &'static str {
        match self {
            Kind::MarkdownString => "MARKDOWN_STRING",
            Kind::UnclosedMarkdownString => "UNCLOSED_MARKDOWN_STRING",
            Kind::Comment => "COMMENT",
            Kind::Newline => "NEWLINE",
            Kind::AccDescrBlock => "ACC_DESCR_BLOCK",
            Kind::AccTitleLine => "ACC_TITLE_LINE",
            Kind::AccDescrLine => "ACC_DESCR_LINE",
            Kind::JsonDeclarationStart => "JSON_DECLARATION_START",
            Kind::Usecase => "USECASE",
            Kind::Actor => "ACTOR",
            Kind::SystemBoundary => "SYSTEM_BOUNDARY",
            Kind::End => "END",
            Kind::Direction => "DIRECTION",
            Kind::Td => "TD",
            Kind::Tb => "TB",
            Kind::Bt => "BT",
            Kind::Lr => "LR",
            Kind::Rl => "RL",
            Kind::Note => "NOTE",
            Kind::For => "FOR",
            Kind::Json => "JSON",
            Kind::ClassDef => "CLASS_DEF",
            Kind::Class => "CLASS",
            Kind::Style => "STYLE",
            Kind::Include => "INCLUDE",
            Kind::Extend => "EXTEND",
            Kind::True => "TRUE",
            Kind::False => "FALSE",
            Kind::Generalization => "GENERALIZATION",
            Kind::DependencyArrow => "DEPENDENCY_ARROW",
            Kind::StereotypeStart => "STEREOTYPE_START",
            Kind::ClassSeparator => "CLASS_SEPARATOR",
            Kind::ForwardSolid => "FORWARD_SOLID",
            Kind::BackwardSolid => "BACKWARD_SOLID",
            Kind::ForwardCircle => "FORWARD_CIRCLE",
            Kind::BackwardCircle => "BACKWARD_CIRCLE",
            Kind::ForwardCross => "FORWARD_CROSS",
            Kind::BackwardCross => "BACKWARD_CROSS",
            Kind::MarkerlessSolid => "MARKERLESS_SOLID",
            Kind::MetadataStart => "METADATA_START",
            Kind::At => "AT",
            Kind::LeftBrace => "LBRACE",
            Kind::RightBrace => "RBRACE",
            Kind::LeftBracket => "LBRACKET",
            Kind::RightBracket => "RBRACKET",
            Kind::LeftParen => "LPAREN",
            Kind::RightParen => "RPAREN",
            Kind::Comma => "COMMA",
            Kind::Colon => "COLON",
            Kind::HashColor => "HASH_COLOR",
            Kind::PlainString => "PLAIN_STRING",
            Kind::CssIdentifier => "CSS_IDENTIFIER",
            Kind::Identifier => "IDENTIFIER",
            Kind::Number => "NUMBER",
            Kind::CssEscapedComma => "CSS_ESCAPED_COMMA",
            Kind::Dash => "DASH",
            Kind::Dot => "DOT",
            Kind::Percent => "PERCENT",
            Kind::CssPunctuation => "CSS_PUNCTUATION",
            Kind::LabelPunctuation => "LABEL_PUNCTUATION",
            Kind::LabelSymbol => "LABEL_SYMBOL",
            Kind::JsonObjectLiteral => "JSON_OBJECT_LITERAL",
            Kind::UnclosedJsonObjectLiteral => "UNCLOSED_JSON_OBJECT_LITERAL",
            Kind::StereotypeEnd => "STEREOTYPE_END",
            Kind::StereotypeText => "STEREOTYPE_TEXT",
            Kind::UnclosedStereotypeText => "UNCLOSED_STEREOTYPE_TEXT",
            Kind::Eof => "EOF",
        }
    }

    /// Membership in the `WORD` category: `IDENTIFIER` and every keyword.
    pub fn is_word(self) -> bool {
        matches!(
            self,
            Kind::Identifier
                | Kind::Usecase
                | Kind::Actor
                | Kind::SystemBoundary
                | Kind::End
                | Kind::Direction
                | Kind::Td
                | Kind::Tb
                | Kind::Bt
                | Kind::Lr
                | Kind::Rl
                | Kind::Note
                | Kind::For
                | Kind::Json
                | Kind::ClassDef
                | Kind::Class
                | Kind::Style
                | Kind::Include
                | Kind::Extend
                | Kind::True
                | Kind::False
        )
    }

    /// Membership in the `LABEL_TEXT` category (`WORD` is a member).
    pub fn is_label_text(self) -> bool {
        self.is_word()
            || matches!(
                self,
                Kind::Number
                    | Kind::At
                    | Kind::Comma
                    | Kind::Colon
                    | Kind::HashColor
                    | Kind::CssIdentifier
                    | Kind::CssEscapedComma
                    | Kind::Dash
                    | Kind::Dot
                    | Kind::Percent
                    | Kind::CssPunctuation
                    | Kind::LabelPunctuation
                    | Kind::LabelSymbol
            )
    }
}

/// One token: its type and its byte range in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    pub start: usize,
    pub end: usize,
}

/// A lexer error as `runChevrotainParse` formats it, positions resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    /// Byte offset of the first skipped character.
    pub start: usize,
    /// Byte offset after the last skipped character.
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Default,
    JsonBody,
    Stereotype,
}

fn re(pattern: &str) -> Regex {
    // Every pattern is anchored at the offset, as Chevrotain's sticky
    // regexes are.
    Regex::new(&format!("^(?:{pattern})")).expect("static regex")
}

/// The regex token types of the default mode, in order. The custom
/// matchers sit between them in [`DEFAULT_ORDER`].
static PATTERNS: LazyLock<Vec<(Kind, Regex)>> = LazyLock::new(|| {
    vec![
        (Kind::UnclosedMarkdownString, re(r#""`(?s:.*)"#)),
        (Kind::Newline, re(r"\r\n|\n|\r")),
        (
            Kind::JsonDeclarationStart,
            re(r"json[\t ]+[A-Za-z0-9_]+[\t ]*@[\t ]*"),
        ),
        (Kind::Usecase, re("usecase-beta")),
        (Kind::Actor, re("actor")),
        (Kind::SystemBoundary, re("systemBoundary")),
        (Kind::End, re("end")),
        (Kind::Direction, re("direction")),
        (Kind::Td, re("TD")),
        (Kind::Tb, re("TB")),
        (Kind::Bt, re("BT")),
        (Kind::Lr, re("LR")),
        (Kind::Rl, re("RL")),
        (Kind::Note, re("note")),
        (Kind::For, re("for")),
        (Kind::Json, re("json")),
        (Kind::ClassDef, re("classDef")),
        (Kind::Class, re("class")),
        (Kind::Style, re("style")),
        (Kind::Include, re("(?i:include)")),
        (Kind::Extend, re("(?i:extend)")),
        (Kind::True, re("true")),
        (Kind::False, re("false")),
        (Kind::Generalization, re(r"--\|>")),
        (Kind::DependencyArrow, re(r"\.\.>")),
        (Kind::StereotypeStart, re("<<")),
        (Kind::ClassSeparator, re(":::")),
        (Kind::ForwardSolid, re("--+>")),
        (Kind::BackwardSolid, re("<--+")),
        (Kind::ForwardCircle, re("--o")),
        (Kind::BackwardCircle, re("o--")),
        (Kind::ForwardCross, re("--x")),
        (Kind::BackwardCross, re("x--")),
        (Kind::MarkerlessSolid, re("--+")),
        (Kind::MetadataStart, re("@\\{")),
        (Kind::At, re("@")),
        (Kind::LeftBrace, re("\\{")),
        (Kind::RightBrace, re("\\}")),
        (Kind::LeftBracket, re("\\[")),
        (Kind::RightBracket, re("\\]")),
        (Kind::LeftParen, re("\\(")),
        (Kind::RightParen, re("\\)")),
        (Kind::Comma, re(",")),
        (Kind::Colon, re(":")),
        (Kind::HashColor, re("#[0-9A-Fa-f]+")),
        (Kind::PlainString, re(r#""[^\n\r"]*"|'[^\n\r']*'"#)),
        (
            Kind::CssIdentifier,
            re(r"--[A-Z_a-z][A-Za-z0-9_-]*|[A-Z_a-z][A-Za-z0-9_]*(?:-[A-Za-z0-9_]+)+"),
        ),
        (Kind::Identifier, re("[A-Za-z0-9_]+")),
        (
            Kind::Number,
            re(r"(?:[0-9]+\.[0-9]+|[0-9]+|\.[0-9]+)(?:[A-Za-z]+)?"),
        ),
        (Kind::CssEscapedComma, re(r"\\,")),
        (Kind::Dash, re("-")),
        (Kind::Dot, re(r"\.")),
        (Kind::Percent, re("%")),
        (Kind::CssPunctuation, re(r"[!#$&*+/=?^_|~]")),
        (Kind::LabelPunctuation, re(r"[;<>\\`]")),
        // `[^\t\n\r !-~]+`: anything outside printable ASCII.
        (Kind::LabelSymbol, re(r"[^\t\n\r\x20-\x7e]+")),
        (Kind::UnclosedJsonObjectLiteral, re(r"\{(?s:.*)")),
        (Kind::StereotypeEnd, re(">>")),
        (Kind::UnclosedStereotypeText, re(r"[^\n\r]+")),
    ]
});

/// One entry of a mode's token list: the skipped whitespace, a custom
/// matcher, or a regex type from [`PATTERNS`].
#[derive(Debug, Clone, Copy)]
enum Matcher {
    Whitespace,
    Custom(Kind),
    Regex(Kind),
}

const DEFAULT_ORDER: &[Matcher] = &[
    Matcher::Whitespace,
    Matcher::Custom(Kind::MarkdownString),
    Matcher::Regex(Kind::UnclosedMarkdownString),
    Matcher::Custom(Kind::Comment),
    Matcher::Regex(Kind::Newline),
    Matcher::Custom(Kind::AccDescrBlock),
    Matcher::Custom(Kind::AccTitleLine),
    Matcher::Custom(Kind::AccDescrLine),
    Matcher::Custom(Kind::JsonDeclarationStart),
    Matcher::Regex(Kind::Usecase),
    Matcher::Regex(Kind::Actor),
    Matcher::Regex(Kind::SystemBoundary),
    Matcher::Regex(Kind::End),
    Matcher::Regex(Kind::Direction),
    Matcher::Regex(Kind::Td),
    Matcher::Regex(Kind::Tb),
    Matcher::Regex(Kind::Bt),
    Matcher::Regex(Kind::Lr),
    Matcher::Regex(Kind::Rl),
    Matcher::Regex(Kind::Note),
    Matcher::Regex(Kind::For),
    Matcher::Regex(Kind::Json),
    Matcher::Regex(Kind::ClassDef),
    Matcher::Regex(Kind::Class),
    Matcher::Regex(Kind::Style),
    Matcher::Regex(Kind::Include),
    Matcher::Regex(Kind::Extend),
    Matcher::Regex(Kind::True),
    Matcher::Regex(Kind::False),
    Matcher::Regex(Kind::Generalization),
    Matcher::Regex(Kind::DependencyArrow),
    Matcher::Regex(Kind::StereotypeStart),
    Matcher::Regex(Kind::ClassSeparator),
    Matcher::Regex(Kind::ForwardSolid),
    Matcher::Regex(Kind::BackwardSolid),
    Matcher::Regex(Kind::ForwardCircle),
    Matcher::Regex(Kind::BackwardCircle),
    Matcher::Regex(Kind::ForwardCross),
    Matcher::Regex(Kind::BackwardCross),
    Matcher::Regex(Kind::MarkerlessSolid),
    Matcher::Regex(Kind::MetadataStart),
    Matcher::Regex(Kind::At),
    Matcher::Regex(Kind::LeftBrace),
    Matcher::Regex(Kind::RightBrace),
    Matcher::Regex(Kind::LeftBracket),
    Matcher::Regex(Kind::RightBracket),
    Matcher::Regex(Kind::LeftParen),
    Matcher::Regex(Kind::RightParen),
    Matcher::Regex(Kind::Comma),
    Matcher::Regex(Kind::Colon),
    Matcher::Regex(Kind::HashColor),
    Matcher::Regex(Kind::PlainString),
    Matcher::Regex(Kind::CssIdentifier),
    Matcher::Regex(Kind::Identifier),
    Matcher::Regex(Kind::Number),
    Matcher::Regex(Kind::CssEscapedComma),
    Matcher::Regex(Kind::Dash),
    Matcher::Regex(Kind::Dot),
    Matcher::Regex(Kind::Percent),
    Matcher::Regex(Kind::CssPunctuation),
    Matcher::Regex(Kind::LabelPunctuation),
    Matcher::Regex(Kind::LabelSymbol),
];

const JSON_ORDER: &[Matcher] = &[
    Matcher::Custom(Kind::JsonObjectLiteral),
    Matcher::Regex(Kind::UnclosedJsonObjectLiteral),
];

const STEREOTYPE_ORDER: &[Matcher] = &[
    Matcher::Regex(Kind::StereotypeEnd),
    Matcher::Custom(Kind::StereotypeText),
    Matcher::Regex(Kind::UnclosedStereotypeText),
];

/// The `longer_alt` of a token type: `IDENTIFIER` for the keywords,
/// `NUMBER` for `IDENTIFIER`.
fn longer_alt(kind: Kind) -> Option<Kind> {
    if kind == Kind::Identifier {
        Some(Kind::Number)
    } else if kind.is_word() {
        Some(Kind::Identifier)
    } else {
        None
    }
}

fn regex_for(kind: Kind) -> Option<&'static Regex> {
    PATTERNS.iter().find(|(k, _)| *k == kind).map(|(_, r)| r)
}

fn regex_len(kind: Kind, text: &str, at: usize) -> Option<usize> {
    let m = regex_for(kind)?.find(&text[at..])?;
    Some(m.end())
}

/// `isIndentedLineStart`: only spaces and tabs between the line start and
/// `at`.
fn is_indented_line_start(text: &str, at: usize) -> bool {
    for c in text[..at].chars().rev() {
        if c == '\n' || c == '\r' {
            return true;
        }
        if c != ' ' && c != '\t' {
            return false;
        }
    }
    true
}

static ACC_TITLE_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^accTitle[\t ]*:[^\n\r]*").expect("static regex"));
static ACC_DESCR_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^accDescr[\t ]*:[^\n\r]*").expect("static regex"));
static ACC_DESCR_OPEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^accDescr[\t ]*\{").expect("static regex"));

/// The custom matchers, returning the matched byte length.
fn custom_len(kind: Kind, text: &str, at: usize) -> Option<usize> {
    let rest = &text[at..];
    let bytes = text.as_bytes();
    match kind {
        // `matchMarkdownString`: `"\`` up to the first `` `" ``.
        Kind::MarkdownString => {
            if !rest.starts_with("\"`") {
                return None;
            }
            let mut i = at + 2;
            while i + 1 < bytes.len() {
                if bytes[i] == b'`' && bytes[i + 1] == b'"' {
                    return Some(i + 2 - at);
                }
                i += 1;
            }
            None
        }
        // `matchComment`: `%%` after indentation only, to the line end.
        Kind::Comment => {
            if !rest.starts_with("%%") || !is_indented_line_start(text, at) {
                return None;
            }
            let end = rest.find(['\n', '\r']).unwrap_or(rest.len());
            Some(end)
        }
        Kind::AccDescrBlock => {
            if !is_indented_line_start(text, at) {
                return None;
            }
            let open = ACC_DESCR_OPEN.find(rest)?;
            let close = rest[open.end()..].find('}')?;
            Some(open.end() + close + 1)
        }
        Kind::AccTitleLine | Kind::AccDescrLine => {
            if !is_indented_line_start(text, at) {
                return None;
            }
            let pattern = if kind == Kind::AccTitleLine {
                &ACC_TITLE_LINE
            } else {
                &ACC_DESCR_LINE
            };
            pattern.find(rest).map(|m| m.end())
        }
        // `/json[\t ]+\w+[\t ]*@[\t ]*(?={)/`: the lookahead is checked by
        // hand because the regex crate has none.
        Kind::JsonDeclarationStart => {
            let len = regex_len(Kind::JsonDeclarationStart, text, at)?;
            rest[len..].starts_with('{').then_some(len)
        }
        // `matchJsonObject`: a balanced object, strings skipped.
        Kind::JsonObjectLiteral => {
            if !rest.starts_with('{') {
                return None;
            }
            let mut depth = 0i32;
            let mut quoted = false;
            let mut escaped = false;
            for (i, c) in rest.char_indices() {
                if quoted {
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        quoted = false;
                    }
                    continue;
                }
                if c == '"' {
                    quoted = true;
                } else if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i + 1);
                    }
                }
            }
            None
        }
        // `matchStereotypeText`: up to `>>` on the same line, not blank.
        Kind::StereotypeText => {
            let end = rest.find(">>")?;
            let image = &rest[..end];
            if image.contains(['\n', '\r']) || image.trim().is_empty() {
                return None;
            }
            Some(end)
        }
        _ => None,
    }
}

fn whitespace_len(text: &str, at: usize) -> Option<usize> {
    let n = text[at..]
        .bytes()
        .take_while(|b| *b == b' ' || *b == b'\t')
        .count();
    (n > 0).then_some(n)
}

fn order(mode: Mode) -> &'static [Matcher] {
    match mode {
        Mode::Default => DEFAULT_ORDER,
        Mode::JsonBody => JSON_ORDER,
        Mode::Stereotype => STEREOTYPE_ORDER,
    }
}

/// The first matching token type of `mode` at `at`, after `longer_alt`.
/// `None` for the skipped whitespace means a skip of that length.
fn match_at(mode: Mode, text: &str, at: usize) -> Option<(Option<Kind>, usize)> {
    for m in order(mode) {
        let (kind, len) = match *m {
            Matcher::Whitespace => match whitespace_len(text, at) {
                Some(len) => return Some((None, len)),
                None => continue,
            },
            Matcher::Custom(k) => match custom_len(k, text, at) {
                Some(len) => (k, len),
                None => continue,
            },
            Matcher::Regex(k) => match regex_len(k, text, at) {
                Some(len) => (k, len),
                None => continue,
            },
        };
        if let Some(alt) = longer_alt(kind)
            && let Some(alt_len) = regex_len(alt, text, at)
            && alt_len > len
        {
            return Some((Some(alt), alt_len));
        }
        return Some((Some(kind), len));
    }
    None
}

/// Tokenizes `text`. On the first unmatched character the error carries
/// the skipped range, as Chevrotain's resync loop computes it.
pub fn tokenize(text: &str) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let mut stack = vec![Mode::Default];
    let mut at = 0;
    while at < text.len() {
        let mode = *stack.last().unwrap_or(&Mode::Default);
        let Some((kind, len)) = match_at(mode, text, at) else {
            // Skip characters until some type of the mode matches again.
            let start = at;
            let mut next = at;
            loop {
                next += text[next..].chars().next().map_or(1, char::len_utf8);
                if next >= text.len() || match_at(mode, text, next).is_some() {
                    break;
                }
            }
            return Err(LexError { start, end: next });
        };
        if let Some(kind) = kind {
            tokens.push(Token {
                kind,
                start: at,
                end: at + len,
            });
            match kind {
                Kind::JsonDeclarationStart => stack.push(Mode::JsonBody),
                Kind::StereotypeStart => stack.push(Mode::Stereotype),
                Kind::JsonObjectLiteral
                | Kind::UnclosedJsonObjectLiteral
                | Kind::StereotypeEnd
                | Kind::UnclosedStereotypeText
                    if stack.len() > 1 =>
                {
                    stack.pop();
                }
                _ => {}
            }
        }
        at += len;
    }
    Ok(tokens)
}

/// Positions as Chevrotain reports them: UTF-16 offsets, 1-based lines
/// (terminators `\n`, `\r\n`, `\r`), and 1-based columns.
pub struct Positions<'a> {
    text: &'a str,
}

impl<'a> Positions<'a> {
    pub fn new(text: &'a str) -> Positions<'a> {
        Positions { text }
    }

    /// The UTF-16 offset of byte offset `at`.
    pub fn offset(&self, at: usize) -> usize {
        self.text[..at.min(self.text.len())]
            .chars()
            .map(char::len_utf16)
            .sum()
    }

    /// The UTF-16 length of the source.
    pub fn utf16_len(&self) -> usize {
        self.offset(self.text.len())
    }

    /// Line and column of byte offset `at`.
    pub fn line_column(&self, at: usize) -> (usize, usize) {
        let mut line = 1;
        let mut column = 1;
        let mut prev_cr = false;
        for c in self.text[..at.min(self.text.len())].chars() {
            match c {
                '\r' => {
                    line += 1;
                    column = 1;
                    prev_cr = true;
                    continue;
                }
                '\n' if prev_cr => {}
                '\n' => {
                    line += 1;
                    column = 1;
                }
                _ => column += c.len_utf16(),
            }
            prev_cr = false;
        }
        (line, column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<Kind> {
        tokenize(text).unwrap().iter().map(|t| t.kind).collect()
    }

    #[test]
    fn keywords_yield_to_longer_identifiers() {
        assert_eq!(
            kinds("actor actors className classDef end-user"),
            vec![
                Kind::Actor,
                Kind::Identifier,
                Kind::Identifier,
                Kind::ClassDef,
                Kind::End,
                Kind::Dash,
                Kind::Identifier,
            ]
        );
    }

    #[test]
    fn numbers_and_identifiers() {
        assert_eq!(
            kinds("1mg 1.5px .5 3rd"),
            vec![
                Kind::Identifier,
                Kind::Number,
                Kind::Number,
                Kind::Identifier
            ]
        );
    }

    #[test]
    fn operators() {
        assert_eq!(
            kinds("A --> B <-- C -- D --o E o-- F --x G x-- H --|> I ..> : include"),
            vec![
                Kind::Identifier,
                Kind::ForwardSolid,
                Kind::Identifier,
                Kind::BackwardSolid,
                Kind::Identifier,
                Kind::MarkerlessSolid,
                Kind::Identifier,
                Kind::ForwardCircle,
                Kind::Identifier,
                Kind::BackwardCircle,
                Kind::Identifier,
                Kind::ForwardCross,
                Kind::Identifier,
                Kind::BackwardCross,
                Kind::Identifier,
                Kind::Generalization,
                Kind::Identifier,
                Kind::DependencyArrow,
                Kind::Colon,
                Kind::Include,
            ]
        );
    }

    #[test]
    fn modes() {
        assert_eq!(
            kinds("A <<Core>>:::x"),
            vec![
                Kind::Identifier,
                Kind::StereotypeStart,
                Kind::StereotypeText,
                Kind::StereotypeEnd,
                Kind::ClassSeparator,
                Kind::Identifier,
            ]
        );
        assert_eq!(
            kinds("json P@{ \"a\": \"}\" }:::d"),
            vec![
                Kind::JsonDeclarationStart,
                Kind::JsonObjectLiteral,
                Kind::ClassSeparator,
                Kind::Identifier,
            ]
        );
    }

    #[test]
    fn stray_quote_is_a_lexer_error() {
        assert_eq!(tokenize("A \"x\n"), Err(LexError { start: 2, end: 3 }));
    }

    #[test]
    fn positions_count_utf16_and_crlf() {
        let p = Positions::new("a\r\nb\u{1F600}c");
        assert_eq!(p.line_column(3), (2, 1));
        assert_eq!(p.line_column(8), (2, 4));
        assert_eq!(p.offset(8), 6);
    }
}
