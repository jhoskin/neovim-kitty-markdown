//! Port of `diagrams/treeView/boxDrawingPreprocessor.ts` (mermaid 12.2.0):
//! box-drawing input (`├──`, `└──`, `│` and the heavy `┣━━`, `┗━━`, `┃`)
//! becomes indentation before the Langium parse, with a map from output
//! lines back to the source lines for the error text.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::mermaid::theme::khroma::js_is_whitespace;

/// `INDENT_UNIT`: four spaces per depth level in the output.
const INDENT_UNIT: &str = "    ";

/// `PreprocessResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preprocessed {
    pub text: String,
    /// Output line (1-based) to source line (1-based); empty when the
    /// input is returned unchanged.
    pub line_map: HashMap<u32, u32>,
}

/// An `Error` the preprocessor throws, before any parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessError {
    pub line: u32,
    pub message: String,
}

/// `ALL_BOX_CHARS`.
fn is_box_char(c: char) -> bool {
    matches!(c, '─' | '━' | '│' | '┃' | '└' | '┗' | '├' | '┣')
}

/// `BRANCH_CHAR`.
fn is_branch_char(c: char) -> bool {
    matches!(c, '└' | '┗' | '├' | '┣')
}

/// `DASH_CHAR`.
fn is_dash_char(c: char) -> bool {
    matches!(c, '─' | '━')
}

fn js_trim(s: &str) -> &str {
    s.trim_matches(js_is_whitespace)
}

/// `DECORATION_ONLY`: `/^[\s│┃]+$/`.
fn is_decoration_only(line: &str) -> bool {
    !line.is_empty()
        && line
            .chars()
            .all(|c| js_is_whitespace(c) || c == '│' || c == '┃')
}

/// `METADATA_LINE`: `/^\s*(title[\t ]|accTitle[\t ]*:|accDescr[\t ]*[:{])/`.
fn is_metadata_line(line: &str) -> bool {
    let rest = line.trim_start_matches(js_is_whitespace);
    let blanks_then = |s: &str, accept: &[char]| {
        s.trim_start_matches([' ', '\t'])
            .starts_with(|c: char| accept.contains(&c))
    };
    if let Some(after) = rest.strip_prefix("title") {
        return after.starts_with([' ', '\t']);
    }
    if let Some(after) = rest.strip_prefix("accTitle") {
        return blanks_then(after, &[':']);
    }
    if let Some(after) = rest.strip_prefix("accDescr") {
        return blanks_then(after, &[':', '{']);
    }
    false
}

/// `COMMENT_LINE`: `/^\s*%%/`.
fn is_comment_line(line: &str) -> bool {
    line.trim_start_matches(js_is_whitespace).starts_with("%%")
}

/// The UTF-16 index of the first branch character, as `RegExp.exec` gives
/// `match.index`, and its character index.
fn find_branch(line: &str) -> Option<(usize, usize)> {
    let mut utf16 = 0;
    for (i, c) in line.chars().enumerate() {
        if is_branch_char(c) {
            return Some((utf16, i));
        }
        utf16 += c.len_utf16();
    }
    None
}

/// `inferSegmentWidth`: the column of the first branch character past
/// column 0, else 4.
fn infer_segment_width(content_lines: &[String]) -> usize {
    content_lines
        .iter()
        .filter_map(|line| find_branch(line))
        .map(|(index, _)| index)
        .find(|index| *index > 0)
        .unwrap_or(4)
}

/// `remapErrorLines`: `message.replace(/\bline\s+(\d+)\b/gi, ...)`.
pub fn remap_error_lines(message: &str, line_map: &HashMap<u32, u32>) -> String {
    static LINE: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::new(r"(?i)\bline\s+([0-9]+)\b").ok());
    let Some(re) = LINE.as_ref() else {
        return message.to_string();
    };
    re.replace_all(message, |caps: &regex::Captures<'_>| {
        let original = caps[1]
            .parse::<u32>()
            .ok()
            .and_then(|line| line_map.get(&line));
        match original {
            Some(line) => format!("line {line}"),
            None => caps[0].to_string(),
        }
    })
    .into_owned()
}

/// `preprocessBoxDrawing`.
pub fn preprocess(input: &str) -> Result<Preprocessed, PreprocessError> {
    let lines: Vec<&str> = input.split('\n').collect();
    let mut line_map = HashMap::new();
    let unchanged = |line_map: HashMap<u32, u32>| Preprocessed {
        text: input.to_string(),
        line_map,
    };

    let Some(keyword_idx) = lines.iter().position(|l| js_trim(l) == "treeView-beta") else {
        return Ok(unchanged(line_map));
    };

    let mut content_lines: Vec<String> = Vec::new();
    for line in &lines[keyword_idx + 1..] {
        if js_trim(line).is_empty() || is_comment_line(line) || is_metadata_line(line) {
            continue;
        }
        if is_decoration_only(line) {
            continue;
        }
        content_lines.push(line.replace('\t', INDENT_UNIT));
    }

    if !content_lines.iter().any(|l| l.chars().any(is_box_char)) {
        return Ok(unchanged(line_map));
    }

    let segment_width = infer_segment_width(&content_lines);
    let mut output: Vec<String> = Vec::new();
    let mut push = |output: &mut Vec<String>, line: String, original: usize| {
        output.push(line);
        line_map.insert(output.len() as u32, original as u32);
    };

    for (i, line) in lines.iter().enumerate().take(keyword_idx + 1) {
        push(&mut output, (*line).to_string(), i + 1);
    }

    for (i, line) in lines.iter().enumerate().skip(keyword_idx + 1) {
        let original = i + 1;
        if js_trim(line).is_empty() || is_comment_line(line) || is_metadata_line(line) {
            push(&mut output, (*line).to_string(), original);
            continue;
        }
        if is_decoration_only(line) {
            continue;
        }
        let normalized = line.replace('\t', INDENT_UNIT);
        if let Some((branch_col, branch_char)) = find_branch(&normalized) {
            // `Math.round(branchCol / segmentWidth) + 1`.
            let depth = (branch_col as f64 / segment_width as f64 + 0.5).floor() as usize + 1;
            let chars: Vec<char> = normalized.chars().collect();
            let mut pos = branch_char + 1;
            while pos < chars.len() && is_dash_char(chars[pos]) {
                pos += 1;
            }
            while pos < chars.len() && chars[pos] == ' ' {
                pos += 1;
            }
            let content: String = chars[pos..].iter().collect();
            let content = content.trim_end_matches(js_is_whitespace);
            if content.is_empty() {
                return Err(PreprocessError {
                    line: original as u32,
                    message: format!(
                        "Line {original}: Empty node — expected a filename or directory name after the box-drawing prefix"
                    ),
                });
            }
            push(
                &mut output,
                format!("{}{content}", INDENT_UNIT.repeat(depth)),
                original,
            );
        } else if normalized
            .chars()
            .all(|c| js_is_whitespace(c) || is_box_char(c))
        {
            // Box-drawing decoration and white space only.
            continue;
        } else if normalized.chars().any(is_box_char) {
            // A box character inside a name, no branch: a root-level item.
            push(&mut output, (*line).to_string(), original);
        } else if normalized.starts_with(js_is_whitespace) {
            return Err(PreprocessError {
                line: original as u32,
                message: format!(
                    "Line {original}: Unexpected indentation without box-drawing characters. In box-drawing format, use ├── or └── prefixes for indented nodes."
                ),
            });
        } else {
            push(&mut output, (*line).to_string(), original);
        }
    }

    Ok(Preprocessed {
        text: output.join("\n"),
        line_map,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(input: &str) -> String {
        preprocess(input).expect("preprocesses").text
    }

    #[test]
    fn indented_input_is_returned_unchanged() {
        let input = "treeView-beta\n    src/\n        a.ts\n";
        let out = preprocess(input).unwrap();
        assert_eq!(out.text, input);
        assert!(out.line_map.is_empty());
    }

    #[test]
    fn branches_become_four_spaces_per_level() {
        assert_eq!(
            text("treeView-beta\n├── src/\n│   ├── index.ts\n│   └── utils.ts\n└── README.md\n"),
            "treeView-beta\n    src/\n        index.ts\n        utils.ts\n    README.md\n"
        );
        assert_eq!(
            text("treeView-beta\n┣━━ a/\n┃   ┗━━ b\n"),
            "treeView-beta\n    a/\n        b\n"
        );
    }

    #[test]
    fn decoration_lines_are_dropped_and_the_map_skips_them() {
        let out = preprocess("treeView-beta\n├── a/\n│\n│   └── \"b\n").unwrap();
        assert_eq!(out.text, "treeView-beta\n    a/\n        \"b\n");
        assert_eq!(out.line_map.get(&3), Some(&4));
        assert_eq!(
            remap_error_lines("Lexer error on line 3, column 9: x", &out.line_map),
            "Lexer error on line 4, column 9: x"
        );
    }

    #[test]
    fn the_segment_width_follows_the_first_indented_branch() {
        assert_eq!(
            text("treeView-beta\n├─ a/\n│  └─ b\n└─ c\n"),
            "treeView-beta\n    a/\n        b\n    c\n"
        );
    }

    #[test]
    fn errors_name_the_source_line() {
        let err = preprocess("treeView-beta\n├── a\n├──\n").unwrap_err();
        assert_eq!(err.line, 3);
        assert!(err.message.starts_with("Line 3: Empty node"));
        let err = preprocess("treeView-beta\n├── a\n  b\n").unwrap_err();
        assert_eq!(err.line, 3);
        assert!(
            err.message
                .starts_with("Line 3: Unexpected indentation without box-drawing characters.")
        );
    }
}
