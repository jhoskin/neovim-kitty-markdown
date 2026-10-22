//! The label markdown subset mermaid's `createText` accepts.
//!
//! Ports of `markdownToLines`, `markdownToHTML`, `nonMarkdownToLines`, and
//! `nonMarkdownToHTML` from `rendering-util/handle-markdown-text.ts`
//! (mermaid 12.2.0), over a small lexer that behaves like `marked` for the
//! token kinds those functions look at: paragraph, text, strong, em, html,
//! escape, codespan, link, br, and space. Tokens mermaid does not handle
//! are dropped or passed raw exactly as the originals do, so a measurement
//! and a render see the same runs.

use regex::Regex;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WordType {
    Normal,
    Strong,
    Em,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Word {
    pub content: String,
    pub kind: WordType,
}

impl Word {
    pub fn new(content: impl Into<String>, kind: WordType) -> Self {
        Word {
            content: content.into(),
            kind,
        }
    }

    pub fn normal(content: impl Into<String>) -> Self {
        Word::new(content, WordType::Normal)
    }
}

pub type Line = Vec<Word>;

/// Collects words into lines without ever holding an empty line list.
#[derive(Default)]
struct LineBuilder {
    done: Vec<Line>,
    current: Line,
}

impl LineBuilder {
    fn push(&mut self, word: Word) {
        self.current.push(word);
    }

    fn newline(&mut self) {
        self.done.push(std::mem::take(&mut self.current));
    }

    fn current_is_empty(&self) -> bool {
        self.current.is_empty()
    }

    fn finish(mut self) -> Vec<Line> {
        self.done.push(self.current);
        self.done
    }
}

/// `common.lineBreakRegex`: `<br>`, `<br/>`, `<br />`, `</br>`, any case.
static LINE_BREAK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)</?br\s*/?>").expect("static regex"));
static BR_ANY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<br\s*/?>").expect("static regex"));
static NON_MD_SPLIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\\n|\n|<br\s*/?>").expect("static regex"));
static NON_MD_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[^>]+>|[^\s<>]+").expect("static regex"));
static MULTI_NEWLINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\n{2,}").expect("static regex"));
static NEWLINE_SPACES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\n *").expect("static regex"));
static ESCAPED_NEWLINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\n|\n").expect("static regex"));

pub fn has_breaks(text: &str) -> bool {
    LINE_BREAK.is_match(text)
}

/// `common.splitBreaks`: the lines between `<br>` tags.
pub fn split_breaks(text: &str) -> Vec<&str> {
    LINE_BREAK.split(text).collect()
}

/// `createText`'s normalization before `markdownToLines`: every `<br>` form
/// becomes `<br/>`, then `preprocessMarkdown` turns `<br/>` into a newline.
pub fn normalize_breaks(text: &str) -> String {
    BR_ANY.replace_all(text, "<br/>").into_owned()
}

/// `markdownToLines`: lines of words with their style, spaces dropped.
pub fn markdown_to_lines(markdown: &str) -> Vec<Line> {
    let preprocessed = preprocess_markdown(markdown);
    let mut lines = LineBuilder::default();
    for block in lex_blocks(&preprocessed) {
        match block {
            Block::Paragraph(text) => {
                for token in lex_inline(&text) {
                    process_inline(&token, WordType::Normal, &mut lines);
                }
            }
            Block::Html(text) | Block::Other(text) | Block::Space(text) => {
                lines.push(Word::normal(text))
            }
        }
    }
    lines.finish()
}

fn process_inline(token: &Inline, parent: WordType, lines: &mut LineBuilder) {
    match token {
        Inline::Text(text) => {
            for (index, text_line) in text.split('\n').enumerate() {
                if index != 0 {
                    lines.newline();
                }
                for word in text_line.split(' ') {
                    let word = word.replace("&#39;", "'");
                    if !word.is_empty() {
                        lines.push(Word::new(word, parent));
                    }
                }
            }
        }
        Inline::Strong(children) => {
            for child in children {
                process_inline(child, WordType::Strong, lines);
            }
        }
        Inline::Em(children) => {
            for child in children {
                process_inline(child, WordType::Em, lines);
            }
        }
        Inline::Html(text) => lines.push(Word::normal(text.clone())),
        // mermaid's processNode has no branch for these, so they vanish.
        Inline::Escape(_) | Inline::Codespan(_) | Inline::Link(_) | Inline::Br(_) => {}
    }
}

/// `preprocessMarkdown`: `<br/>` to newline, collapse blank lines, dedent.
fn preprocess_markdown(markdown: &str) -> String {
    let without_br = markdown.replace("<br/>", "\n");
    let single = MULTI_NEWLINE.replace_all(&without_br, "\n");
    dedent(&single)
}

/// `ts-dedent`: strips the common leading indentation of the non-empty
/// lines and trims the leading and trailing blank lines.
fn dedent(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    let mut out: Vec<&str> = lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                ""
            } else {
                &line[indent..]
            }
        })
        .collect();
    while out.first().is_some_and(|line| line.is_empty()) {
        out.remove(0);
    }
    while out.last().is_some_and(|line| line.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// `markdownToHTML`: paragraphs as `<p>`, newlines as `<br/>`, strong and
/// em as tags, everything else raw.
pub fn markdown_to_html(markdown: &str) -> String {
    let mut out = String::new();
    for block in lex_blocks(markdown) {
        match block {
            Block::Paragraph(text) => {
                out.push_str("<p>");
                for token in lex_inline(&text) {
                    out.push_str(&inline_to_html(&token));
                }
                out.push_str("</p>");
            }
            Block::Html(text) | Block::Other(text) => out.push_str(&text),
            Block::Space(_) => {}
        }
    }
    out
}

fn inline_to_html(token: &Inline) -> String {
    match token {
        Inline::Text(text) => NEWLINE_SPACES.replace_all(text, "<br/>").into_owned(),
        Inline::Strong(children) => format!(
            "<strong>{}</strong>",
            children.iter().map(inline_to_html).collect::<String>()
        ),
        Inline::Em(children) => format!(
            "<em>{}</em>",
            children.iter().map(inline_to_html).collect::<String>()
        ),
        Inline::Html(text) | Inline::Escape(text) => text.clone(),
        Inline::Codespan(raw) | Inline::Link(raw) | Inline::Br(raw) => raw.clone(),
    }
}

/// `nonMarkdownToLines`: split on `\n`, literal `\n`, and `<br>`; keep tags
/// as words of their own.
pub fn non_markdown_to_lines(text: &str) -> Vec<Line> {
    NON_MD_SPLIT
        .split(text)
        .map(|line| {
            NON_MD_WORD
                .find_iter(line.trim())
                .map(|word| Word::normal(word.as_str()))
                .collect()
        })
        .collect()
}

/// `nonMarkdownToHTML`: one `<p>` with `<br />` for each newline.
pub fn non_markdown_to_html(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    format!("<p>{}</p>", ESCAPED_NEWLINE.replace_all(text, "<br />"))
}

/// The lines of styled runs a browser lays out for a label's HTML: `<p>`
/// and `<br>` break lines, `<strong>`/`<b>` and `<em>`/`<i>` set the style,
/// other tags are dropped, and entities are decoded. Spaces stay in the
/// run text so the HTML measurer can collapse or preserve them.
pub fn html_to_lines(html: &str) -> Vec<Line> {
    let mut lines = LineBuilder::default();
    let mut strong = 0usize;
    let mut em = 0usize;
    let mut text = String::new();
    let flush = |text: &mut String, lines: &mut LineBuilder, strong: usize, em: usize| {
        if text.is_empty() {
            return;
        }
        let kind = if strong > 0 {
            WordType::Strong
        } else if em > 0 {
            WordType::Em
        } else {
            WordType::Normal
        };
        lines.push(Word::new(decode_entities(text), kind));
        text.clear();
    };
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        // The HTML parser only opens a tag for `<` followed by a letter,
        // `/`, `!`, or `?`; any other `<` is text.
        let opens_tag = rest[start + 1..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '/' | '!' | '?'));
        let Some(end) = rest[start..].find('>') else {
            break;
        };
        if !opens_tag {
            text.push_str(&rest[..=start]);
            rest = &rest[start + 1..];
            continue;
        }
        text.push_str(&rest[..start]);
        let tag = &rest[start + 1..start + end];
        rest = &rest[start + end + 1..];
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let closing = tag.starts_with('/');
        match name.as_str() {
            "br" => {
                flush(&mut text, &mut lines, strong, em);
                lines.newline();
            }
            "p" | "div" => {
                flush(&mut text, &mut lines, strong, em);
                // A block after content starts a new line; the first <p>
                // and the closing tag of the last one do not.
                if !closing && !lines.current_is_empty() {
                    lines.newline();
                }
            }
            "strong" | "b" => {
                flush(&mut text, &mut lines, strong, em);
                strong = if closing {
                    strong.saturating_sub(1)
                } else {
                    strong + 1
                };
            }
            "em" | "i" => {
                flush(&mut text, &mut lines, strong, em);
                em = if closing {
                    em.saturating_sub(1)
                } else {
                    em + 1
                };
            }
            _ => {}
        }
    }
    text.push_str(rest);
    flush(&mut text, &mut lines, strong, em);
    lines.finish()
}

/// The entities `sanitizeText` and marked produce.
pub fn decode_entities(text: &str) -> String {
    text.replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", "\u{a0}")
        .replace("&amp;", "&")
}

/// marked's `escape(text, false)`: `<>"'` and bare `&` become entities.
fn marked_escape(text: &str) -> String {
    static ENTITY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(#\d{1,7}|#[Xx][a-fA-F0-9]{1,6}|\w+);").expect("static regex")
    });
    let mut out = String::with_capacity(text.len());
    for (index, ch) in text.char_indices() {
        match ch {
            '&' if !ENTITY.is_match(&text[index + 1..]) => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

enum Block {
    Paragraph(String),
    Html(String),
    Space(String),
    Other(String),
}

static INTERRUPT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?: {0,3}#{1,6}(?: |$)| {0,3}(?:```|~~~)| {0,3}(?:[-*+]|\d{1,9}[.)])(?: |$)| {0,3}>| {0,3}(?:(?:-[ \t]*){3,}|(?:\*[ \t]*){3,}|(?:_[ \t]*){3,})$| {0,3}<(?:/?(?:address|article|aside|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[1-6]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|meta|nav|noframes|ol|optgroup|option|p|param|section|source|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)(?:\s|/?>|$)|!--|\?|![A-Za-z]))",
    )
    .expect("static regex")
});

/// A line that ends a paragraph in marked: blank, heading, fence, list
/// item, blockquote, thematic break, or block html.
fn interrupts_paragraph(line: &str) -> bool {
    line.trim().is_empty() || INTERRUPT.is_match(line)
}

fn starts_block_html(line: &str) -> bool {
    line.trim_start().starts_with('<') && INTERRUPT.is_match(line)
}

/// marked's block lexer, reduced to what the label functions distinguish:
/// paragraphs, blank-line runs, block html, and "other" blocks that pass
/// through raw.
fn lex_blocks(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let start = i;
        if line.trim().is_empty() {
            while i < lines.len() && lines[i].trim().is_empty() {
                i += 1;
            }
            if i < lines.len() || start > 0 {
                blocks.push(Block::Space(join_raw(&lines[start..i], i < lines.len())));
            }
        } else if starts_block_html(line) {
            while i < lines.len() && !lines[i].trim().is_empty() {
                i += 1;
            }
            blocks.push(Block::Html(join_raw(&lines[start..i], i < lines.len())));
        } else if interrupts_paragraph(line) {
            // Headings, lists, fences, quotes, rules: marked emits a token
            // mermaid passes through raw, so keep the source text.
            i += 1;
            while i < lines.len() && !lines[i].trim().is_empty() {
                i += 1;
            }
            blocks.push(Block::Other(join_raw(&lines[start..i], i < lines.len())));
        } else {
            i += 1;
            while i < lines.len() && !interrupts_paragraph(lines[i]) {
                i += 1;
            }
            blocks.push(Block::Paragraph(lines[start..i].join("\n")));
        }
    }
    blocks
}

fn join_raw(lines: &[&str], trailing_newline: bool) -> String {
    let mut raw = lines.join("\n");
    if trailing_newline {
        raw.push('\n');
    }
    raw
}

#[derive(Debug, Clone, PartialEq)]
enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Em(Vec<Inline>),
    Html(String),
    Escape(String),
    Codespan(String),
    Link(String),
    Br(String),
}

static TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:<!--[\s\S]*?-->|<[A-Za-z][\w-]*(?:\s+[A-Za-z_:][\w.:-]*(?:\s*=\s*(?:\x22[^\x22]*\x22|'[^']*'|[^\s\x22'=<>`]+))?)*\s*/?>|</[A-Za-z][\w-]*\s*>|<\?[\s\S]*?\?>)")
        .expect("static regex")
});
static AUTOLINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^<(?:[A-Za-z][A-Za-z0-9+.-]{1,31}:[^\s\x00-\x1f<>]*|[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?(?:\.[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?)+)>")
        .expect("static regex")
});
static URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:https?://|www\.)[^\s<]+[^<?!.,:*_~\s]").expect("static regex")
});
static LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^!?\[(?:[^\[\]]|\\.)*\](?:\([^)]*\)|\[[^\]]*\])").expect("static regex")
});
static ESCAPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r##"^\\([!"#$%&'()*+,\-./:;<=>?@\[\]\\^_`{|}~])"##).expect("static regex")
});
static BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^( {2,}|\\)\n").expect("static regex"));

/// Inline tokens the way marked produces them for a paragraph, with
/// adjacent text tokens merged.
fn lex_inline(src: &str) -> Vec<Inline> {
    let mut tokens: Vec<Inline> = Vec::new();
    let mut rest = src;
    let mut prev_char: Option<char> = None;
    while !rest.is_empty() {
        let (token, consumed) = next_inline(rest, prev_char);
        match (token, tokens.last_mut()) {
            (Inline::Text(text), Some(Inline::Text(last))) => last.push_str(&text),
            (token, _) => tokens.push(token),
        }
        prev_char = rest[..consumed].chars().last();
        rest = &rest[consumed..];
    }
    tokens
}

fn next_inline(rest: &str, prev_char: Option<char>) -> (Inline, usize) {
    if let Some(m) = ESCAPE.find(rest) {
        return (Inline::Escape(rest[1..m.end()].to_string()), m.end());
    }
    if let Some(m) = AUTOLINK.find(rest) {
        return (Inline::Link(m.as_str().to_string()), m.end());
    }
    if let Some(m) = TAG.find(rest) {
        return (Inline::Html(m.as_str().to_string()), m.end());
    }
    if let Some(m) = LINK.find(rest) {
        return (Inline::Link(m.as_str().to_string()), m.end());
    }
    if let Some((token, len)) = em_strong(rest, prev_char) {
        return (token, len);
    }
    if let Some(len) = codespan_len(rest) {
        return (Inline::Codespan(rest[..len].to_string()), len);
    }
    if let Some(m) = BR.find(rest) {
        // marked's br needs text after the newline; a trailing one is text.
        if !rest[m.end()..].trim().is_empty() {
            return (Inline::Br(m.as_str().to_string()), m.end());
        }
    }
    let at_word_start = prev_char.is_none_or(|c| !c.is_alphanumeric());
    if at_word_start && let Some(m) = URL.find(rest) {
        return (Inline::Link(m.as_str().to_string()), m.end());
    }
    // Plain text up to the next character that can start another token.
    let mut end = rest.len();
    for (index, ch) in rest.char_indices().skip(1) {
        if matches!(
            ch,
            '\\' | '<' | '[' | '`' | '*' | '_' | '~' | 'h' | 'w' | ' '
        ) {
            end = index;
            break;
        }
    }
    (Inline::Text(marked_escape(&rest[..end])), end)
}

fn codespan_len(rest: &str) -> Option<usize> {
    let ticks = rest.chars().take_while(|&c| c == '`').count();
    if ticks == 0 {
        return None;
    }
    let body = &rest[ticks..];
    let fence: String = "`".repeat(ticks);
    let mut search = 0;
    while let Some(pos) = body[search..].find(&fence) {
        let at = search + pos;
        let run = body[at..].chars().take_while(|&c| c == '`').count();
        if run == ticks && at > 0 {
            return Some(ticks + at + ticks);
        }
        search = at + run;
    }
    None
}

/// marked's emStrong for `*`, `**`, `_`, `__` with the flanking rules that
/// matter for labels: `_` needs a word boundary, `*` does not; a closer
/// cannot follow whitespace.
fn em_strong(rest: &str, prev_char: Option<char>) -> Option<(Inline, usize)> {
    let delim = rest.chars().next()?;
    if delim != '*' && delim != '_' {
        return None;
    }
    let run = rest.chars().take_while(|&c| c == delim).count();
    if run > 3 {
        return None;
    }
    let after = rest[run..].chars().next()?;
    if after.is_whitespace() {
        return None;
    }
    if delim == '_' && prev_char.is_some_and(|c| c.is_alphanumeric()) {
        return None;
    }
    let body = &rest[run..];
    let fence: String = std::iter::repeat_n(delim, run).collect();
    let mut search = 0;
    while let Some(pos) = body[search..].find(&fence) {
        let at = search + pos;
        let before = body[..at].chars().last();
        let close_run = body[at..].chars().take_while(|&c| c == delim).count();
        let following = body[at + close_run..].chars().next();
        let closes = at > 0
            && before.is_some_and(|c| !c.is_whitespace())
            && close_run == run
            && (delim == '*' || following.is_none_or(|c| !c.is_alphanumeric()));
        if closes {
            let children = lex_inline(&body[..at]);
            let token = match run {
                1 => Inline::Em(children),
                2 => Inline::Strong(children),
                _ => Inline::Em(vec![Inline::Strong(children)]),
            };
            return Some((token, run + at + run));
        }
        search = at + close_run.max(1);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(content: &str) -> Word {
        Word::normal(content)
    }
    fn s(content: &str) -> Word {
        Word::new(content, WordType::Strong)
    }
    fn e(content: &str) -> Word {
        Word::new(content, WordType::Em)
    }

    #[test]
    fn markdown_to_lines_basic() {
        let input = "This is regular text\nHere is a new line\nThere is some words **with a bold** section\nHere is a line *with an italic* section";
        assert_eq!(
            markdown_to_lines(input),
            vec![
                vec![w("This"), w("is"), w("regular"), w("text")],
                vec![w("Here"), w("is"), w("a"), w("new"), w("line")],
                vec![
                    w("There"),
                    w("is"),
                    w("some"),
                    w("words"),
                    s("with"),
                    s("a"),
                    s("bold"),
                    w("section")
                ],
                vec![
                    w("Here"),
                    w("is"),
                    w("a"),
                    w("line"),
                    e("with"),
                    e("an"),
                    e("italic"),
                    w("section")
                ],
            ]
        );
    }

    #[test]
    fn markdown_to_lines_empty_and_plain() {
        assert_eq!(markdown_to_lines(""), vec![Vec::<Word>::new()]);
        assert_eq!(
            markdown_to_lines("This is a simple test\nwith no formatting"),
            vec![
                vec![w("This"), w("is"), w("a"), w("simple"), w("test")],
                vec![w("with"), w("no"), w("formatting")],
            ]
        );
    }

    #[test]
    fn markdown_to_lines_paragraph_with_indent() {
        assert_eq!(
            markdown_to_lines("**Start** with\n    a second line"),
            vec![
                vec![s("Start"), w("with")],
                vec![w("a"), w("second"), w("line")],
            ]
        );
    }

    #[test]
    fn markdown_to_lines_mixed() {
        assert_eq!(
            markdown_to_lines("*Italic* and **bold** formatting"),
            vec![vec![e("Italic"), w("and"), s("bold"), w("formatting")]]
        );
        assert_eq!(
            markdown_to_lines("*Italic with space* and **bold ws** formatting"),
            vec![vec![
                e("Italic"),
                e("with"),
                e("space"),
                w("and"),
                s("bold"),
                s("ws"),
                w("formatting")
            ]]
        );
        assert_eq!(
            markdown_to_lines("The dog in **the** hog... a *very long text* about it\nWord!"),
            vec![
                vec![
                    w("The"),
                    w("dog"),
                    w("in"),
                    s("the"),
                    w("hog..."),
                    w("a"),
                    e("very"),
                    e("long"),
                    e("text"),
                    w("about"),
                    w("it")
                ],
                vec![w("Word!")],
            ]
        );
    }

    #[test]
    fn markdown_to_lines_breaks_and_entities() {
        assert_eq!(
            markdown_to_lines("a<br/>b<br/><br/>c"),
            vec![vec![w("a")], vec![w("b")], vec![w("c")]]
        );
        assert_eq!(markdown_to_lines("it's"), vec![vec![w("it's")]]);
        assert_eq!(
            markdown_to_lines("say \"hi\""),
            vec![vec![w("say"), w("&quot;hi&quot;")]]
        );
        assert_eq!(
            markdown_to_lines("a <b>tag</b> here"),
            vec![vec![w("a"), w("<b>"), w("tag"), w("</b>"), w("here")]]
        );
    }

    #[test]
    fn markdown_to_html_cases() {
        assert_eq!(
            markdown_to_html(
                "This is regular text\nHere is a new line\nThere is some words **with a bold** section\nHere is a line *with an italic* section"
            ),
            "<p>This is regular text<br/>Here is a new line<br/>There is some words <strong>with a bold</strong> section<br/>Here is a line <em>with an italic</em> section</p>"
        );
        assert_eq!(markdown_to_html(""), "");
        assert_eq!(
            markdown_to_html("This is a simple test\nwith no formatting"),
            "<p>This is a simple test<br/>with no formatting</p>"
        );
        assert_eq!(
            markdown_to_html("This is a **bold** test"),
            "<p>This is a <strong>bold</strong> test</p>"
        );
        assert_eq!(
            markdown_to_html("This is an *italic* test"),
            "<p>This is an <em>italic</em> test</p>"
        );
        assert_eq!(
            markdown_to_html("*Italic* and **bold** formatting"),
            "<p><em>Italic</em> and <strong>bold</strong> formatting</p>"
        );
        assert_eq!(
            markdown_to_html("Hello\n  - l1\n  - l2\n  - l3"),
            "<p>Hello</p>  - l1\n  - l2\n  - l3"
        );
        assert_eq!(
            markdown_to_html("Hello, how do\n  you do?"),
            "<p>Hello, how do<br/>you do?</p>"
        );
        assert_eq!(markdown_to_html("a<br/>b"), "<p>a<br/>b</p>");
    }

    #[test]
    fn non_markdown_cases() {
        assert_eq!(
            non_markdown_to_lines("Line 1\\nLine 2<br/>Text with <strong>tag</strong>"),
            vec![
                vec![w("Line"), w("1")],
                vec![w("Line"), w("2")],
                vec![
                    w("Text"),
                    w("with"),
                    w("<strong>"),
                    w("tag"),
                    w("</strong>")
                ],
            ]
        );
        assert_eq!(
            non_markdown_to_lines("**bold** and *italic*"),
            vec![vec![w("**bold**"), w("and"), w("*italic*")]]
        );
        assert_eq!(non_markdown_to_html(""), "");
        assert_eq!(non_markdown_to_html("a\\nb\nc"), "<p>a<br />b<br />c</p>");
    }

    #[test]
    fn html_to_lines_splits_and_styles() {
        assert_eq!(
            html_to_lines("<p>Hello <strong>big</strong> world<br/>next</p>"),
            vec![vec![w("Hello "), s("big"), w(" world")], vec![w("next")],]
        );
        assert_eq!(
            html_to_lines("<p>a</p><p>b &amp; c</p>"),
            vec![vec![w("a")], vec![w("b & c")]]
        );
        assert_eq!(html_to_lines(""), vec![Vec::<Word>::new()]);
        assert_eq!(
            html_to_lines("<span class=\"x\"><i>it</i></span>"),
            vec![vec![e("it")]]
        );
    }

    #[test]
    fn break_regex_matches_every_br_form() {
        assert!(has_breaks("a<br>b"));
        assert!(has_breaks("a<BR />b"));
        assert!(has_breaks("a</br>b"));
        assert!(!has_breaks("a\nb"));
        assert_eq!(split_breaks("a<br/>b<br >c"), vec!["a", "b", "c"]);
        assert_eq!(normalize_breaks("a<br>b<br />c"), "a<br/>b<br/>c");
    }
}
