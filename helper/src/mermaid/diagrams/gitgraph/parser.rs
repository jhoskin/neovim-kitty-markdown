//! A hand-written lexer and parser for the language of `gitGraph.langium`
//! plus `common.langium` and `reference.langium` (`@mermaid-js/parser`
//! 2.0.1, mermaid 12.2.0).
//!
//! Langium builds a Chevrotain lexer from the grammar. The token types are
//! the whitespace-like terminals, then the keywords longest first, then the
//! terminals a parser rule reaches, then the remaining hidden terminals;
//! the first type that matches at a position wins. A keyword that a
//! terminal also matches in full (`commit` is a `REFERENCE`) carries that
//! terminal as its `LONGER_ALT`, so `commits` lexes as one `REFERENCE`.
//! `GitGraphTokenBuilder` appends `(?:(?=%%)|(?!\S))` to the `gitGraph`
//! keyword only. A position no type matches is skipped and reported as one
//! lexer error per run of skipped characters.
//!
//! The parser is an LL(1) walk of the `GitGraph` rule with the error texts
//! of Chevrotain's default error provider; it reports the first parser
//! error, where Langium's recovering parser can report several.
//! `MermaidParseError` joins both lists as
//! `Parsing failed: <lexer errors> <parser errors>`.

use std::fmt::Write;

use crate::mermaid::theme::khroma::js_is_whitespace;

/// The `GitGraph` AST after Langium's value converters.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ast {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub dir: Option<Direction>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    LR,
    TB,
    BT,
}

/// `'NORMAL' | 'REVERSE' | 'HIGHLIGHT'`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitKind {
    Normal,
    Reverse,
    Highlight,
}

/// One `Statement`, with the 1-based source line of its keyword for the
/// errors the db throws.
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub line: u32,
    pub kind: StatementKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    Commit {
        id: Option<String>,
        message: Option<String>,
        tags: Vec<String>,
        kind: Option<CommitKind>,
    },
    Branch {
        name: String,
        order: Option<f64>,
    },
    Merge {
        branch: String,
        id: Option<String>,
        tags: Vec<String>,
        kind: Option<CommitKind>,
    },
    Checkout {
        branch: String,
    },
    CherryPick {
        id: Option<String>,
        parent: Option<String>,
        tags: Vec<String>,
    },
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
    Newline,
    /// A keyword; the index points into [`KEYWORDS`].
    Keyword(usize),
    AccDescr,
    AccTitle,
    Title,
    Str,
    Int,
    Reference,
    Eof,
}

/// The keywords of `gitGraph.langium` as Langium orders them: longest
/// first, grammar order among equals.
const KEYWORDS: &[&str] = &[
    "cherry-pick",
    "HIGHLIGHT",
    "gitGraph:",
    "checkout",
    "gitGraph",
    "REVERSE",
    "parent:",
    "commit",
    "branch",
    "switch",
    "NORMAL",
    "order:",
    "merge",
    "type:",
    "msg:",
    "tag:",
    "id:",
    "LR",
    "TB",
    "BT",
    ":",
];

impl Kind {
    /// Chevrotain's `tokenType.name`: the keyword text for keywords, the
    /// terminal rule name otherwise.
    fn name(self) -> &'static str {
        match self {
            Kind::Newline => "NEWLINE",
            Kind::Keyword(i) => KEYWORDS.get(i).copied().unwrap_or("?"),
            Kind::AccDescr => "ACC_DESCR",
            Kind::AccTitle => "ACC_TITLE",
            Kind::Title => "TITLE",
            Kind::Str => "STRING",
            Kind::Int => "INT",
            Kind::Reference => "REFERENCE",
            Kind::Eof => "EOF",
        }
    }

    fn is_keyword(self, word: &str) -> bool {
        matches!(self, Kind::Keyword(i) if KEYWORDS.get(i) == Some(&word))
    }
}

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

/// `\w` of a JS regex without the `u` flag.
fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
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

    /// The `(?:(?=%%)|(?!\S))` suffix the token builder appends to
    /// `gitGraph`.
    fn keyword_boundary(&self, i: usize) -> bool {
        match self.at(i) {
            None => true,
            Some('%') => self.at(i + 1) == Some('%'),
            Some(c) => js_is_whitespace(c),
        }
    }

    /// A keyword at `i`, unless its `LONGER_ALT` (`REFERENCE`) matches a
    /// longer text there.
    fn keyword(&self, i: usize, word: &str) -> Option<usize> {
        if !self.starts_with(i, word) {
            return None;
        }
        let end = i + word.chars().count();
        if word == "gitGraph" && !self.keyword_boundary(end) {
            return None;
        }
        if word.chars().all(is_word)
            && let Some(alt) = self.reference(i)
            && alt > end
        {
            return None;
        }
        Some(end)
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

    /// `accDescr(?:[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)|\s*{([^}]*)})`; the
    /// leading `[\t ]*` of the terminal is consumed by `WHITESPACE` first.
    fn acc_descr(&self, i: usize) -> Option<usize> {
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

    /// `accTitle[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)`.
    fn acc_title(&self, i: usize) -> Option<usize> {
        if !self.starts_with(i, "accTitle") {
            return None;
        }
        let colon = self.blanks(i + "accTitle".len());
        (self.at(colon) == Some(':')).then(|| self.rest_of_line_before_comment(colon + 1))
    }

    /// `title(?:[\t ][^\n\r]*?(?=%%)|[\t ][^\n\r]*|)`.
    fn title(&self, i: usize) -> Option<usize> {
        if !self.starts_with(i, "title") {
            return None;
        }
        let j = i + "title".len();
        match self.at(j) {
            Some(' ') | Some('\t') => Some(self.rest_of_line_before_comment(j + 1)),
            _ => Some(j),
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

    fn digits(&self, i: usize) -> usize {
        let mut j = i;
        while self.at(j).is_some_and(|c| c.is_ascii_digit()) {
            j += 1;
        }
        j
    }

    /// `INT`: `0|[1-9][0-9]*(?!\.)`. The first alternative has no
    /// lookahead, so `0.5` lexes `0` as an `INT`.
    fn int(&self, i: usize) -> Option<usize> {
        match self.at(i)? {
            '0' => Some(i + 1),
            '1'..='9' => {
                let end = self.digits(i);
                (self.at(end) != Some('.')).then_some(end)
            }
            _ => None,
        }
    }

    /// `REFERENCE`: `\w([-\./\w]*[-\w])?`. The greedy middle backtracks
    /// until the last character is not a slash or a dot.
    fn reference(&self, i: usize) -> Option<usize> {
        if !self.at(i).is_some_and(is_word) {
            return None;
        }
        let mut j = i + 1;
        while self
            .at(j)
            .is_some_and(|c| is_word(c) || c == '-' || c == '.' || c == '/')
        {
            j += 1;
        }
        while j > i + 1 && matches!(self.at(j - 1), Some('.') | Some('/')) {
            j -= 1;
        }
        Some(j)
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

    /// `%%{[\S\s]*?}%%(?:\r?\n|(?!\S))`.
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

    /// `%%[^\n\r]*`.
    fn comment(&self, i: usize) -> Option<usize> {
        self.starts_with(i, "%%").then(|| self.line_end(i + 2))
    }

    /// The first token type that matches at `i`, in Chevrotain's order.
    /// `None` in the result means hidden.
    fn next(&self, i: usize) -> Option<(Option<Kind>, usize)> {
        if let Some(end) = self.newline(i) {
            return Some((Some(Kind::Newline), end));
        }
        if let Some(end) = self.whitespace(i) {
            return Some((None, end));
        }
        for (k, word) in KEYWORDS.iter().enumerate() {
            if let Some(end) = self.keyword(i, word) {
                return Some((Some(Kind::Keyword(k)), end));
            }
        }
        let terminals: [(Kind, Matcher<'a>); 6] = [
            (Kind::AccDescr, Lexer::acc_descr),
            (Kind::AccTitle, Lexer::acc_title),
            (Kind::Title, Lexer::title),
            (Kind::Str, Lexer::string),
            (Kind::Int, Lexer::int),
            (Kind::Reference, Lexer::reference),
        ];
        for (kind, matcher) in terminals {
            if let Some(end) = matcher(self, i) {
                return Some((Some(kind), end));
            }
        }
        for matcher in [Lexer::yaml, Lexer::directive, Lexer::comment] {
            if let Some(end) = matcher(self, i) {
                return Some((None, end));
            }
        }
        None
    }

    fn tokenize(&self) -> (Vec<Token>, Vec<LexError>) {
        let mut tokens = Vec::new();
        let mut errors = Vec::new();
        let mut i = 0;
        let mut line = 1;
        let mut line_start = 0;
        let mut pending: Option<LexError> = None;
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
                        if self.chars[k] == '\n'
                            || (self.chars[k] == '\r' && self.at(k + 1) != Some('\n'))
                        {
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
                    if self.chars[i] == '\n'
                        || (self.chars[i] == '\r' && self.at(i + 1) != Some('\n'))
                    {
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

    fn at_keyword(&self, word: &str) -> bool {
        self.peek().kind.is_keyword(word)
    }

    /// `MismatchedTokenException` from Chevrotain's default error provider.
    fn mismatched(&self, expected: &str) -> ParseError {
        let t = self.peek();
        ParseError {
            line: t.line,
            column: t.column,
            message: format!(
                "Expecting token of type '{}' but found `{}`.",
                expected,
                self.image(t)
            ),
        }
    }

    /// `NoViableAltException`: the alternatives of an `OR`, one per line.
    fn no_viable(&self, alternatives: &[&str]) -> ParseError {
        let t = self.peek();
        let mut list = String::new();
        for (i, alt) in alternatives.iter().enumerate() {
            let _ = write!(list, "\n  {}. [{}]", i + 1, alt);
        }
        ParseError {
            line: t.line,
            column: t.column,
            message: format!(
                "Expecting: one of these possible Token sequences:{list}\nbut found: '{}'",
                self.image(t)
            ),
        }
    }

    fn expect(&mut self, kind: Kind) -> Result<Token, ParseError> {
        if self.peek().kind == kind {
            Ok(self.advance())
        } else {
            Err(self.mismatched(kind.name()))
        }
    }

    fn expect_keyword(&mut self, word: &str) -> Result<Token, ParseError> {
        if self.at_keyword(word) {
            Ok(self.advance())
        } else {
            Err(self.mismatched(word))
        }
    }

    /// `EOL: NEWLINE+ | EOF`.
    fn eol(&mut self) -> Result<(), ParseError> {
        match self.peek().kind {
            Kind::Newline => {
                while self.peek().kind == Kind::Newline {
                    self.advance();
                }
                Ok(())
            }
            Kind::Eof => Ok(()),
            _ => Err(self.no_viable(&["NEWLINE", "EOF"])),
        }
    }

    /// `STRING` with Langium's `convertString`.
    fn string(&mut self) -> Result<String, ParseError> {
        let t = self.expect(Kind::Str)?;
        Ok(convert_string(&self.image(&t)))
    }

    /// `REFERENCE | STRING`.
    fn name(&mut self) -> Result<String, ParseError> {
        match self.peek().kind {
            Kind::Reference => {
                let t = self.advance();
                Ok(self.image(&t))
            }
            Kind::Str => self.string(),
            _ => Err(self.no_viable(&["REFERENCE", "STRING"])),
        }
    }

    /// `'type:' type=('NORMAL' | 'REVERSE' | 'HIGHLIGHT')`, after `type:`.
    fn commit_kind(&mut self) -> Result<CommitKind, ParseError> {
        let kind = if self.at_keyword("NORMAL") {
            CommitKind::Normal
        } else if self.at_keyword("REVERSE") {
            CommitKind::Reverse
        } else if self.at_keyword("HIGHLIGHT") {
            CommitKind::Highlight
        } else {
            return Err(self.no_viable(&["NORMAL", "REVERSE", "HIGHLIGHT"]));
        };
        self.advance();
        Ok(kind)
    }

    fn commit(&mut self) -> Result<StatementKind, ParseError> {
        let mut id = None;
        let mut message = None;
        let mut tags = Vec::new();
        let mut kind = None;
        loop {
            if self.at_keyword("id:") {
                self.advance();
                id = Some(self.string()?);
            } else if self.at_keyword("msg:") {
                self.advance();
                message = Some(self.string()?);
            } else if self.peek().kind == Kind::Str {
                message = Some(self.string()?);
            } else if self.at_keyword("tag:") {
                self.advance();
                tags.push(self.string()?);
            } else if self.at_keyword("type:") {
                self.advance();
                kind = Some(self.commit_kind()?);
            } else {
                break;
            }
        }
        self.eol()?;
        Ok(StatementKind::Commit {
            id,
            message,
            tags,
            kind,
        })
    }

    fn branch(&mut self) -> Result<StatementKind, ParseError> {
        let name = self.name()?;
        let mut order = None;
        if self.at_keyword("order:") {
            self.advance();
            let t = self.expect(Kind::Int)?;
            order = Some(self.image(&t).parse().unwrap_or(f64::NAN));
        }
        self.eol()?;
        Ok(StatementKind::Branch { name, order })
    }

    fn merge(&mut self) -> Result<StatementKind, ParseError> {
        let branch = self.name()?;
        let mut id = None;
        let mut tags = Vec::new();
        let mut kind = None;
        loop {
            if self.at_keyword("id:") {
                self.advance();
                id = Some(self.string()?);
            } else if self.at_keyword("tag:") {
                self.advance();
                tags.push(self.string()?);
            } else if self.at_keyword("type:") {
                self.advance();
                kind = Some(self.commit_kind()?);
            } else {
                break;
            }
        }
        self.eol()?;
        Ok(StatementKind::Merge {
            branch,
            id,
            tags,
            kind,
        })
    }

    fn checkout(&mut self) -> Result<StatementKind, ParseError> {
        let branch = self.name()?;
        self.eol()?;
        Ok(StatementKind::Checkout { branch })
    }

    fn cherry_pick(&mut self) -> Result<StatementKind, ParseError> {
        let mut id = None;
        let mut parent = None;
        let mut tags = Vec::new();
        loop {
            if self.at_keyword("id:") {
                self.advance();
                id = Some(self.string()?);
            } else if self.at_keyword("tag:") {
                self.advance();
                tags.push(self.string()?);
            } else if self.at_keyword("parent:") {
                self.advance();
                parent = Some(self.string()?);
            } else {
                break;
            }
        }
        self.eol()?;
        Ok(StatementKind::CherryPick { id, parent, tags })
    }

    /// The `GitGraph` entry rule.
    fn git_graph(&mut self) -> Result<Ast, ParseError> {
        let mut ast = Ast::default();
        while self.peek().kind == Kind::Newline {
            self.advance();
        }
        if self.at_keyword("gitGraph:") {
            self.advance();
        } else {
            self.expect_keyword("gitGraph")?;
            if self.at_keyword(":") {
                self.advance();
            } else {
                let dir = if self.at_keyword("LR") {
                    Some(Direction::LR)
                } else if self.at_keyword("TB") {
                    Some(Direction::TB)
                } else if self.at_keyword("BT") {
                    Some(Direction::BT)
                } else {
                    None
                };
                if dir.is_some() {
                    self.advance();
                    ast.dir = dir;
                    self.expect_keyword(":")?;
                }
            }
        }
        loop {
            let t = self.peek().clone();
            match t.kind {
                Kind::Newline => {
                    self.advance();
                }
                Kind::AccDescr => {
                    self.advance();
                    ast.acc_descr = Some(convert_acc_descr(&self.image(&t)));
                    self.eol()?;
                }
                Kind::AccTitle => {
                    self.advance();
                    ast.acc_title = Some(convert_acc_title(&self.image(&t)));
                    self.eol()?;
                }
                Kind::Title => {
                    self.advance();
                    ast.title = Some(convert_title(&self.image(&t)));
                    self.eol()?;
                }
                Kind::Keyword(_) => {
                    let kind = if self.at_keyword("commit") {
                        self.advance();
                        self.commit()?
                    } else if self.at_keyword("branch") {
                        self.advance();
                        self.branch()?
                    } else if self.at_keyword("merge") {
                        self.advance();
                        self.merge()?
                    } else if self.at_keyword("checkout") || self.at_keyword("switch") {
                        self.advance();
                        self.checkout()?
                    } else if self.at_keyword("cherry-pick") {
                        self.advance();
                        self.cherry_pick()?
                    } else {
                        break;
                    };
                    ast.statements.push(Statement { line: t.line, kind });
                }
                _ => break,
            }
        }
        self.expect(Kind::Eof)?;
        Ok(ast)
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

/// `AbstractMermaidValueConverter.runCommonConverter` with `titleRegex`
/// `/title([\t ][^\n\r]*|)/`: group 1 trimmed, blank runs collapsed.
fn convert_title(image: &str) -> String {
    let rest = &image["title".len()..];
    collapse_blanks(js_trim(rest))
}

/// `accessibilityTitleRegex` `/accTitle[\t ]*:([^\n\r]*)/`.
fn convert_acc_title(image: &str) -> String {
    let after = image.split_once(':').map(|(_, r)| r).unwrap_or("");
    collapse_blanks(js_trim(after))
}

/// `accessibilityDescrRegex` `/accDescr(?:[\t ]*:([^\n\r]*)|\s*{([^}]*)})/`:
/// the single-line form like a title, the block form with every line
/// trimmed, blank runs collapsed, and blank lines dropped.
fn convert_acc_descr(image: &str) -> String {
    let after_keyword = &image["accDescr".len()..];
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

/// `parse('gitGraph', text)`: the AST, or the `MermaidParseError` text with
/// the position of its first error.
pub fn parse(text: &str) -> Result<Ast, ParseFailure> {
    let chars: Vec<char> = text.chars().collect();
    let lexer = Lexer::new(&chars);
    let (tokens, lex_errors) = lexer.tokenize();
    let mut parser = Parser {
        chars: &chars,
        tokens,
        pos: 0,
    };
    let parsed = parser.git_graph();
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

/// `formatLocation`: Chevrotain marks a missing position (the EOF token)
/// with `NaN` or `-1`, printed as `?`.
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

    fn err(text: &str) -> ParseFailure {
        parse(text)
            .err()
            .unwrap_or_else(|| panic!("{text:?} parsed"))
    }

    #[test]
    fn docs_example_with_options() {
        let ast = ok(
            "gitGraph\n   commit id: \"Alpha\"\n   commit id:\"Beta\" tag: \"v1\" tag:\"v2\" type: REVERSE\n   branch develop order: 2\n   checkout develop\n   commit \"a message\"\n   checkout main\n   merge develop id: \"M\" type: HIGHLIGHT\n   cherry-pick id:\"Beta\" parent:\"Alpha\"\n   switch \"develop\"\n",
        );
        assert_eq!(ast.dir, None);
        assert_eq!(ast.statements.len(), 9);
        assert_eq!(
            ast.statements[1].kind,
            StatementKind::Commit {
                id: Some("Beta".to_string()),
                message: None,
                tags: vec!["v1".to_string(), "v2".to_string()],
                kind: Some(CommitKind::Reverse),
            }
        );
        assert_eq!(
            ast.statements[2].kind,
            StatementKind::Branch {
                name: "develop".to_string(),
                order: Some(2.0),
            }
        );
        assert_eq!(
            ast.statements[4].kind,
            StatementKind::Commit {
                id: None,
                message: Some("a message".to_string()),
                tags: vec![],
                kind: None,
            }
        );
        assert_eq!(
            ast.statements[6].kind,
            StatementKind::Merge {
                branch: "develop".to_string(),
                id: Some("M".to_string()),
                tags: vec![],
                kind: Some(CommitKind::Highlight),
            }
        );
        assert_eq!(
            ast.statements[8].kind,
            StatementKind::Checkout {
                branch: "develop".to_string(),
            }
        );
        assert_eq!(ast.statements[8].line, 10);
    }

    #[test]
    fn direction_forms_and_header_variants() {
        assert_eq!(ok("gitGraph TB:\n commit\n").dir, Some(Direction::TB));
        assert_eq!(ok("gitGraph BT:\n commit\n").dir, Some(Direction::BT));
        assert_eq!(ok("gitGraph LR:\n commit\n").dir, Some(Direction::LR));
        assert_eq!(ok("gitGraph:\n commit\n").dir, None);
        assert_eq!(ok("gitGraph :\n commit\n").dir, None);
        assert_eq!(ok("\n\ngitGraph\n").statements.len(), 0);
    }

    #[test]
    fn title_comments_and_accessibility() {
        let ast = ok(
            "gitGraph\n  title  My   graph\n  accTitle: acc\n  accDescr { multi\n  line }\n  %% a comment\n  commit %% trailing\n",
        );
        assert_eq!(ast.title.as_deref(), Some("My graph"));
        assert_eq!(ast.acc_title.as_deref(), Some("acc"));
        assert_eq!(ast.acc_descr.as_deref(), Some("multi\nline"));
        assert_eq!(ast.statements.len(), 1);
    }

    #[test]
    fn references_with_dots_slashes_and_keyword_prefixes() {
        let ast = ok("gitGraph\n branch release/1.0\n branch commits\n branch main-2\n");
        let names: Vec<&str> = ast
            .statements
            .iter()
            .map(|s| match &s.kind {
                StatementKind::Branch { name, .. } => name.as_str(),
                _ => "",
            })
            .collect();
        assert_eq!(names, vec!["release/1.0", "commits", "main-2"]);
    }

    #[test]
    fn errors_carry_the_chevrotain_text() {
        let e = err("gitGraph\n commit id: Alpha\n");
        assert_eq!(e.line, 2);
        assert_eq!(e.column, 13);
        assert_eq!(
            e.message,
            "Parsing failed:  Parse error on line 2, column 13: Expecting token of type 'STRING' but found `Alpha`."
        );
        let e = err("gitGraph\n branch\n");
        assert!(e.message.contains("1. [REFERENCE]\n  2. [STRING]"));
        let e = err("gitGraph\n commit\n frobnicate\n");
        assert_eq!(e.line, 3);
        assert!(
            e.message
                .contains("Expecting token of type 'EOF' but found `frobnicate`.")
        );
        let e = err("gitGraph\n commit §\n");
        assert!(e.message.starts_with(
            "Parsing failed: Lexer error on line 2, column 9: unexpected character: ->§<- at offset: 17, skipped 1 characters."
        ));
        let e = err("flowchart\n");
        assert_eq!(
            e.message,
            "Parsing failed:  Parse error on line 1, column 1: Expecting token of type 'gitGraph' but found `flowchart`."
        );
    }
}
