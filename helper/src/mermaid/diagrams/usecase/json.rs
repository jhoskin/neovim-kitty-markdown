//! Port of `parser/usecaseJson.ts` (`parseOrderedJsonObject`) and of the
//! row flattening in `usecaseDb.ts` (`flattenJsonRows`).
//!
//! mermaid runs `JSON.parse` and walks the text again for the property
//! order. This parser does both in one pass: objects are `IndexMap`s, so
//! a repeated key keeps its first position and takes the last value, as
//! the walker's `propertyOrder` does. Errors carry V8's `JSON.parse`
//! message text and the line and column `locationAtOffset` computes.

use indexmap::IndexMap;

use crate::mermaid::theme::khroma::js_number;

/// A parsed JSON value with object keys in source order.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(IndexMap<String, JsonValue>),
}

/// `UsecaseJsonError`: `<message> (line <l>, column <c>)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl JsonError {
    pub fn full_message(&self) -> String {
        format!(
            "{} (line {}, column {})",
            self.message, self.line, self.column
        )
    }
}

/// A V8 failure: the message, and whether it names its position (the
/// `Unexpected token` form does not, so mermaid's walker locates it).
struct Failure {
    message: String,
    /// UTF-16 offset of the error.
    offset: usize,
}

struct Reader<'a> {
    chars: Vec<char>,
    /// UTF-16 offset of each char, plus the total at the end.
    offsets: Vec<usize>,
    source: &'a str,
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(source: &'a str) -> Reader<'a> {
        let chars: Vec<char> = source.chars().collect();
        let mut offsets = Vec::with_capacity(chars.len() + 1);
        let mut at = 0;
        for c in &chars {
            offsets.push(at);
            at += c.len_utf16();
        }
        offsets.push(at);
        Reader {
            chars,
            offsets,
            source,
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn utf16(&self, pos: usize) -> usize {
        self.offsets[pos.min(self.chars.len())]
    }

    /// `Expected ... in JSON at position N (line L column C)`.
    fn positioned(&self, what: &str, pos: usize) -> Failure {
        let (line, column) = local_line_column(&self.chars[..pos.min(self.chars.len())]);
        let offset = self.utf16(pos);
        Failure {
            message: format!("{what} in JSON at position {offset} (line {line} column {column})"),
            offset,
        }
    }

    /// `Unexpected token 'x', "<source>" is not valid JSON`, with V8's
    /// ten-character context around a longer source.
    fn unexpected_token(&self, pos: usize) -> Failure {
        let Some(c) = self.chars.get(pos) else {
            return Failure {
                message: "Unexpected end of JSON input".to_string(),
                offset: self.utf16(pos),
            };
        };
        let offset = self.utf16(pos);
        let length = self.utf16(self.chars.len());
        let message = if length <= 21 {
            format!(
                "Unexpected token '{c}', \"{}\" is not valid JSON",
                self.source
            )
        } else {
            let start = offset.saturating_sub(10);
            let end = (offset + 10).min(length);
            let from = self.offsets.iter().position(|o| *o >= start).unwrap_or(0);
            let to = self
                .offsets
                .iter()
                .position(|o| *o >= end)
                .unwrap_or(self.chars.len());
            let context: String = self.chars[from..to].iter().collect();
            let before = if start == 0 { "" } else { "..." };
            let after = if end == length { "" } else { "..." };
            format!("Unexpected token '{c}', {before}\"{context}\"{after} is not valid JSON")
        };
        Failure { message, offset }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.pos += 1;
        }
    }

    fn value(&mut self) -> Result<JsonValue, Failure> {
        self.skip_whitespace();
        match self.peek() {
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => self.string().map(JsonValue::String),
            Some('t') => self.literal("true", JsonValue::Bool(true)),
            Some('f') => self.literal("false", JsonValue::Bool(false)),
            Some('n') => self.literal("null", JsonValue::Null),
            Some('-' | '0'..='9') => self.number(),
            _ => Err(self.unexpected_token(self.pos)),
        }
    }

    fn object(&mut self) -> Result<JsonValue, Failure> {
        self.pos += 1;
        let mut map = IndexMap::new();
        self.skip_whitespace();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(JsonValue::Object(map));
        }
        let mut first = true;
        loop {
            if self.peek() != Some('"') {
                return Err(if first {
                    self.positioned("Expected property name or '}'", self.pos)
                } else {
                    self.positioned("Expected double-quoted property name", self.pos)
                });
            }
            first = false;
            let key = self.string()?;
            self.skip_whitespace();
            if self.peek() != Some(':') {
                return Err(self.positioned("Expected ':' after property name", self.pos));
            }
            self.pos += 1;
            let value = self.value()?;
            map.insert(key, value);
            self.skip_whitespace();
            match self.peek() {
                Some('}') => {
                    self.pos += 1;
                    return Ok(JsonValue::Object(map));
                }
                Some(',') => {
                    self.pos += 1;
                    self.skip_whitespace();
                }
                _ => {
                    return Err(
                        self.positioned("Expected ',' or '}' after property value", self.pos)
                    );
                }
            }
        }
    }

    fn array(&mut self) -> Result<JsonValue, Failure> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_whitespace();
            match self.peek() {
                Some(']') => {
                    self.pos += 1;
                    return Ok(JsonValue::Array(items));
                }
                Some(',') => {
                    self.pos += 1;
                }
                _ => {
                    return Err(
                        self.positioned("Expected ',' or ']' after array element", self.pos)
                    );
                }
            }
        }
    }

    fn string(&mut self) -> Result<String, Failure> {
        self.pos += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.positioned("Unterminated string", self.chars.len()));
            };
            let at = self.pos;
            self.pos += 1;
            match c {
                '"' => return Ok(out),
                c if (c as u32) < 0x20 => {
                    return Err(self.positioned("Bad control character in string literal", at));
                }
                '\\' => {
                    let Some(e) = self.peek() else {
                        return Err(self.positioned("Unterminated string", self.chars.len()));
                    };
                    self.pos += 1;
                    match e {
                        '"' | '\\' | '/' => out.push(e),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let hex: String = self.chars[self.pos..]
                                .iter()
                                .take(4)
                                .take_while(|c| c.is_ascii_hexdigit())
                                .collect();
                            if hex.len() != 4 {
                                return Err(
                                    self.positioned("Bad Unicode escape", self.pos + hex.len())
                                );
                            }
                            self.pos += 4;
                            let unit = u32::from_str_radix(&hex, 16).unwrap_or(0xfffd);
                            out.push(char::from_u32(unit).unwrap_or('\u{fffd}'));
                        }
                        _ => return Err(self.positioned("Bad escaped character", at)),
                    }
                }
                _ => out.push(c),
            }
        }
    }

    fn literal(&mut self, word: &str, value: JsonValue) -> Result<JsonValue, Failure> {
        for (i, w) in word.chars().enumerate() {
            if self.chars.get(self.pos + i) != Some(&w) {
                return Err(self.unexpected_token(self.pos + i));
            }
        }
        self.pos += word.chars().count();
        Ok(value)
    }

    fn digits(&mut self) -> usize {
        let start = self.pos;
        while matches!(self.peek(), Some('0'..='9')) {
            self.pos += 1;
        }
        self.pos - start
    }

    fn number(&mut self) -> Result<JsonValue, Failure> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.pos += 1;
        }
        match self.peek() {
            Some('0') => {
                self.pos += 1;
                if matches!(self.peek(), Some('0'..='9')) {
                    return Err(self.positioned("Unexpected number", self.pos));
                }
            }
            Some('1'..='9') => {
                self.digits();
            }
            _ => return Err(self.positioned("No number after minus sign", self.pos)),
        }
        if self.peek() == Some('.') {
            self.pos += 1;
            if self.digits() == 0 {
                return Err(self.positioned("Unterminated fractional number", self.pos));
            }
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some('+' | '-')) {
                self.pos += 1;
            }
            if self.digits() == 0 {
                return Err(self.positioned("Exponent part is missing a number", self.pos));
            }
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        Ok(JsonValue::Number(text.parse().unwrap_or(0.0)))
    }
}

/// Line and column inside the JSON text, as V8 prints them.
fn local_line_column(chars: &[char]) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for c in chars {
        if *c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += c.len_utf16();
        }
    }
    (line, column)
}

/// `locationAtOffset(text, offset, startLine, startColumn)`.
fn location_at_offset(
    text: &str,
    offset: usize,
    start_line: usize,
    start_column: usize,
) -> (usize, usize) {
    let mut line = start_line;
    let mut column = start_column;
    let mut index = 0;
    let mut prev = None;
    for c in text.chars() {
        if index >= offset {
            break;
        }
        match c {
            '\r' => {
                line += 1;
                column = 1;
            }
            '\n' => {
                if prev != Some('\r') {
                    line += 1;
                    column = 1;
                }
            }
            _ => column += c.len_utf16(),
        }
        index += c.len_utf16();
        prev = Some(c);
    }
    (line, column)
}

/// `parseOrderedJsonObject(jsonText, startLine, startColumn)`.
pub fn parse_ordered_object(
    text: &str,
    start_line: usize,
    start_column: usize,
) -> Result<IndexMap<String, JsonValue>, JsonError> {
    let mut reader = Reader::new(text);
    let parsed = reader.value().and_then(|v| {
        reader.skip_whitespace();
        if reader.pos < reader.chars.len() {
            Err(reader.positioned("Unexpected non-whitespace character after JSON", reader.pos))
        } else {
            Ok(v)
        }
    });
    match parsed {
        Ok(JsonValue::Object(map)) => Ok(map),
        Ok(_) => Err(JsonError {
            message: "JSON value must have an object root".to_string(),
            line: start_line,
            column: start_column,
        }),
        Err(f) => {
            let (line, column) = location_at_offset(text, f.offset, start_line, start_column);
            Err(JsonError {
                message: format!("Invalid JSON: {}", f.message),
                line,
                column,
            })
        }
    }
}

/// One table row: the shown key, the accessible key, the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonRow {
    pub key: String,
    pub value: String,
}

/// `displayJsonScalar`.
fn display_scalar(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => "null".to_string(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Number(n) => js_number(*n),
        JsonValue::String(s) => s.clone(),
        JsonValue::Array(_) | JsonValue::Object(_) => String::new(),
    }
}

/// `flattenJsonRows(value, propertyOrder, sanitize)`.
pub fn flatten_rows(root: &IndexMap<String, JsonValue>) -> Vec<JsonRow> {
    fn visit(current: &JsonValue, path: &str, rows: &mut Vec<JsonRow>) {
        let mut push = |key: &str, value: String| {
            rows.push(JsonRow {
                key: key.to_string(),
                value,
            });
        };
        match current {
            JsonValue::Array(items) => {
                if items.is_empty() {
                    push(path, "[]".to_string());
                    return;
                }
                let scalar = items
                    .iter()
                    .all(|i| !matches!(i, JsonValue::Array(_) | JsonValue::Object(_)));
                if scalar {
                    for (index, item) in items.iter().enumerate() {
                        push(if index == 0 { path } else { "" }, display_scalar(item));
                    }
                    return;
                }
                for (index, item) in items.iter().enumerate() {
                    visit(item, &format!("{path}[{index}]"), rows);
                }
            }
            JsonValue::Object(map) => {
                if map.is_empty() {
                    push(path, "{}".to_string());
                    return;
                }
                for (key, value) in map {
                    let child = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    visit(value, &child, rows);
                }
            }
            scalar => push(path, display_scalar(scalar)),
        }
    }
    let mut rows = Vec::new();
    visit(&JsonValue::Object(root.clone()), "", &mut rows);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(text: &str) -> String {
        parse_ordered_object(text, 1, 1).unwrap_err().message
    }

    #[test]
    fn v8_messages() {
        assert_eq!(
            message("{ a: 1 }"),
            "Invalid JSON: Expected property name or '}' in JSON at position 2 (line 1 column 3)"
        );
        assert_eq!(
            message("{ \"a\": }"),
            "Invalid JSON: Unexpected token '}', \"{ \"a\": }\" is not valid JSON"
        );
        assert_eq!(
            message("{\n \"a\": 01 }"),
            "Invalid JSON: Unexpected number in JSON at position 9 (line 2 column 8)"
        );
    }

    #[test]
    fn rows_follow_source_order() {
        let map = parse_ordered_object(
            r#"{"2": "b", "1": "a", "x": [1, 2], "o": {"c": null}, "e": [], "l": [{"n": true}], "2": "last"}"#,
            1,
            1,
        )
        .unwrap();
        let rows: Vec<(String, String)> = flatten_rows(&map)
            .into_iter()
            .map(|r| (r.key, r.value))
            .collect();
        let expect = [
            ("2", "last"),
            ("1", "a"),
            ("x", "1"),
            ("", "2"),
            ("o.c", "null"),
            ("e", "[]"),
            ("l[0].n", "true"),
        ];
        assert_eq!(
            rows,
            expect
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
        );
    }
}
