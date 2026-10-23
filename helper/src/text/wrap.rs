//! Ports of mermaid's `wrapLabel` and `splitLineToFitWidth`, the text
//! segmentation they rely on (`Intl.Segmenter` in Chrome), and the line
//! breaking Chrome applies to an HTML label with `white-space: break-spaces`.
//!
//! mermaid wraps twice: `wrapLabel` pre-wraps with its own measurer before
//! the browser sees the text, and `createText` splits SVG label lines with
//! `splitLineToFitWidth`. HTML labels are wrapped by the browser itself.

use super::markdown::{Line, Word};

mod emoji_presentation;

// --- Segmentation -----------------------------------------------------------
//
// `Intl.Segmenter` follows UAX #29. These tables carry the classes that
// label text reaches in practice; a label is one line of a diagram, not
// running prose, so the dictionary-based scripts are treated as letters.

fn is_extend(ch: char) -> bool {
    matches!(ch as u32,
        0x0300..=0x036F | 0x0483..=0x0489 | 0x0591..=0x05BD | 0x0610..=0x061A
        | 0x064B..=0x065F | 0x0670 | 0x06D6..=0x06DC | 0x06DF..=0x06E4
        | 0x0900..=0x0903 | 0x093A..=0x094F | 0x0E31 | 0x0E34..=0x0E3A | 0x0E47..=0x0E4E
        | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x200C | 0x20D0..=0x20FF | 0x302A..=0x302F
        | 0x3099..=0x309A | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F | 0x1F3FB..=0x1F3FF
        | 0xE0020..=0xE007F | 0xE0100..=0xE01EF)
}

fn is_format(ch: char) -> bool {
    matches!(ch as u32, 0x00AD | 0x061C | 0x180E | 0x200B | 0x200E | 0x200F | 0x202A..=0x202E
        | 0x2060..=0x2064 | 0x2066..=0x206F | 0xFEFF)
}

const ZWJ: char = '\u{200D}';

fn is_regional_indicator(ch: char) -> bool {
    matches!(ch as u32, 0x1F1E6..=0x1F1FF)
}

/// Extended_Pictographic, approximated by the emoji blocks and the common
/// symbols with emoji presentation.
pub fn is_extended_pictographic(ch: char) -> bool {
    matches!(ch as u32,
        0x00A9 | 0x00AE | 0x203C | 0x2049 | 0x2122 | 0x2139 | 0x2194..=0x2199
        | 0x21A9..=0x21AA | 0x231A..=0x231B | 0x2328 | 0x2388 | 0x23CF | 0x23E9..=0x23F3
        | 0x23F8..=0x23FA | 0x24C2 | 0x25AA..=0x25AB | 0x25B6 | 0x25C0 | 0x25FB..=0x25FE
        | 0x2600..=0x27BF | 0x2934..=0x2935 | 0x2B05..=0x2B07 | 0x2B1B..=0x2B1C | 0x2B50
        | 0x2B55 | 0x3030 | 0x303D | 0x3297 | 0x3299 | 0x1F000..=0x1F0FF | 0x1F10D..=0x1F10F
        | 0x1F12F | 0x1F16C..=0x1F171 | 0x1F17E..=0x1F17F | 0x1F18E | 0x1F191..=0x1F19A
        | 0x1F1AD..=0x1F1E5 | 0x1F201..=0x1F20F | 0x1F21A | 0x1F22F | 0x1F232..=0x1F23A
        | 0x1F23C..=0x1F23F | 0x1F249..=0x1F3FA | 0x1F400..=0x1F53D | 0x1F546..=0x1F64F
        | 0x1F680..=0x1F6FF | 0x1F774..=0x1F77F | 0x1F7D5..=0x1F7FF | 0x1F80C..=0x1F80F
        | 0x1F848..=0x1F84F | 0x1F85A..=0x1F85F | 0x1F888..=0x1F88F | 0x1F8AE..=0x1F8FF
        | 0x1F90C..=0x1F93A | 0x1F93C..=0x1F945 | 0x1F947..=0x1FAFF | 0x1FC00..=0x1FFFD)
}

/// Unicode `Emoji_Presentation=Yes`: a character that shows as a color
/// emoji with no variation selector. Blink gives such a character the
/// emoji fallback priority; any other pictograph (`❤` U+2764, `★`, `✓`)
/// takes the text priority unless U+FE0F follows it.
pub fn is_emoji_presentation(ch: char) -> bool {
    let cp = ch as u32;
    emoji_presentation::RANGES
        .binary_search_by(|&(first, last)| {
            if last < cp {
                std::cmp::Ordering::Less
            } else if first > cp {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// Han, Hiragana, Katakana, and the fullwidth forms: break before and
/// after each, and never part of a word.
pub fn is_ideographic(ch: char) -> bool {
    matches!(ch as u32,
        0x2E80..=0x2FDF | 0x3000..=0x303F | 0x3040..=0x30FF | 0x3100..=0x312F
        | 0x3190..=0x31FF | 0x3200..=0x4DBF | 0x4E00..=0x9FFF | 0xA000..=0xA4CF
        | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6
        | 0x1B000..=0x1B16F | 0x20000..=0x2FA1F | 0x30000..=0x3134F)
}

fn is_katakana(ch: char) -> bool {
    matches!(ch as u32, 0x30A0..=0x30FF | 0x31F0..=0x31FF | 0x32D0..=0x32FE | 0x3300..=0x3357
        | 0xFF66..=0xFF9F | 0x1B000)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WordClass {
    Newline,
    Space,
    Letter,
    Numeric,
    Katakana,
    ExtendNumLet,
    MidLetter,
    MidNumLet,
    MidNum,
    SingleQuote,
    RegionalIndicator,
    Pictographic,
    Other,
}

fn word_class(ch: char) -> WordClass {
    match ch {
        '\r' | '\n' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}' => {
            WordClass::Newline
        }
        '\'' => WordClass::SingleQuote,
        ':' | '\u{B7}' | '\u{387}' | '\u{5F4}' | '\u{2027}' | '\u{FE13}' | '\u{FE55}'
        | '\u{FF1A}' => WordClass::MidLetter,
        '.' | '\u{2018}' | '\u{2019}' | '\u{2024}' | '\u{FE52}' | '\u{FF07}' | '\u{FF0E}' => {
            WordClass::MidNumLet
        }
        ',' | ';' | '\u{37E}' | '\u{589}' | '\u{60C}' | '\u{60D}' | '\u{66C}' | '\u{7F8}'
        | '\u{2044}' | '\u{FE10}' | '\u{FE14}' | '\u{FE50}' | '\u{FE54}' | '\u{FF0C}'
        | '\u{FF1B}' => WordClass::MidNum,
        '_' | '\u{203F}' | '\u{2040}' | '\u{2054}' | '\u{FE33}' | '\u{FE34}' | '\u{FE4D}'
        | '\u{FE4E}' | '\u{FE4F}' | '\u{FF3F}' => WordClass::ExtendNumLet,
        _ if ch.is_whitespace() => WordClass::Space,
        _ if is_regional_indicator(ch) => WordClass::RegionalIndicator,
        _ if is_katakana(ch) => WordClass::Katakana,
        _ if is_ideographic(ch) => WordClass::Other,
        _ if is_extended_pictographic(ch) => WordClass::Pictographic,
        _ if ch.is_ascii_digit() || (ch.is_numeric() && !ch.is_alphabetic()) => WordClass::Numeric,
        _ if ch.is_alphabetic() => WordClass::Letter,
        _ => WordClass::Other,
    }
}

/// One base character with the Extend, Format, and ZWJ characters that
/// attach to it (WB4), plus the pictograph a ZWJ joins (WB3c).
struct Cluster<'a> {
    text: &'a str,
    class: WordClass,
}

fn clusters(text: &str) -> Vec<Cluster<'_>> {
    let mut out: Vec<Cluster<'_>> = Vec::new();
    let mut start: Option<usize> = None;
    let mut class = WordClass::Other;
    let mut after_zwj = false;
    let mut prev_newline = false;
    for (index, ch) in text.char_indices() {
        let attaches = match start {
            None => false,
            Some(_) if prev_newline => false,
            Some(_) if after_zwj && is_extended_pictographic(ch) => true,
            Some(_) => is_extend(ch) || is_format(ch) || ch == ZWJ,
        };
        if !attaches {
            if let Some(s) = start {
                out.push(Cluster {
                    text: &text[s..index],
                    class,
                });
            }
            start = Some(index);
            class = word_class(ch);
            prev_newline = class == WordClass::Newline;
        }
        after_zwj = ch == ZWJ;
    }
    if let Some(s) = start {
        out.push(Cluster {
            text: &text[s..],
            class,
        });
    }
    out
}

fn is_ah_letter(class: WordClass) -> bool {
    class == WordClass::Letter
}

fn is_mid_num_let_q(class: WordClass) -> bool {
    matches!(class, WordClass::MidNumLet | WordClass::SingleQuote)
}

/// UAX #29 word boundary between cluster `i - 1` and cluster `i`.
fn word_break_between(cl: &[Cluster<'_>], i: usize) -> bool {
    use WordClass::*;
    let prev = cl[i - 1].class;
    let next = cl[i].class;
    // WB3: CR × LF.
    if cl[i - 1].text == "\r" && cl[i].text == "\n" {
        return false;
    }
    // WB3a, WB3b.
    if prev == Newline || next == Newline {
        return true;
    }
    // WB3d.
    if prev == Space && next == Space {
        return false;
    }
    // WB5.
    if is_ah_letter(prev) && is_ah_letter(next) {
        return false;
    }
    // WB6: AHLetter × (MidLetter | MidNumLetQ) AHLetter.
    if is_ah_letter(prev)
        && (next == MidLetter || is_mid_num_let_q(next))
        && cl.get(i + 1).is_some_and(|c| is_ah_letter(c.class))
    {
        return false;
    }
    // WB7: AHLetter (MidLetter | MidNumLetQ) × AHLetter.
    if (prev == MidLetter || is_mid_num_let_q(prev))
        && is_ah_letter(next)
        && i >= 2
        && is_ah_letter(cl[i - 2].class)
    {
        return false;
    }
    // WB8, WB9, WB10.
    let letter_or_number = |c: WordClass| is_ah_letter(c) || c == Numeric;
    if (letter_or_number(prev) && next == Numeric) || (prev == Numeric && is_ah_letter(next)) {
        return false;
    }
    // WB11: Numeric (MidNum | MidNumLetQ) × Numeric.
    if (prev == MidNum || is_mid_num_let_q(prev))
        && next == Numeric
        && i >= 2
        && cl[i - 2].class == Numeric
    {
        return false;
    }
    // WB12: Numeric × (MidNum | MidNumLetQ) Numeric.
    if prev == Numeric
        && (next == MidNum || is_mid_num_let_q(next))
        && cl.get(i + 1).is_some_and(|c| c.class == Numeric)
    {
        return false;
    }
    // WB13.
    if prev == Katakana && next == Katakana {
        return false;
    }
    // WB13a, WB13b.
    let word_like = |c: WordClass| matches!(c, Letter | Numeric | Katakana | ExtendNumLet);
    if (word_like(prev) && next == ExtendNumLet) || (prev == ExtendNumLet && word_like(next)) {
        return false;
    }
    // WB15, WB16: regional indicators pair up.
    if prev == RegionalIndicator && next == RegionalIndicator {
        let run = cl[..i]
            .iter()
            .rev()
            .take_while(|c| c.class == RegionalIndicator)
            .count();
        return run % 2 == 0;
    }
    true
}

/// `splitLineToWords`: `Intl.Segmenter` with word granularity. Spaces,
/// punctuation, and each emoji or ideograph come back as segments of their
/// own; runs of spaces stay together.
pub fn split_line_to_words(text: &str) -> Vec<String> {
    let cl = clusters(text);
    let mut out = Vec::new();
    let mut current = String::new();
    for (i, cluster) in cl.iter().enumerate() {
        if i > 0 && word_break_between(&cl, i) {
            out.push(std::mem::take(&mut current));
        }
        current.push_str(cluster.text);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// `splitTextToChars`: `Intl.Segmenter` with grapheme granularity.
pub fn split_text_to_chars(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut after_zwj = false;
    let mut ri_run = 0usize;
    let mut prev: Option<char> = None;
    for ch in text.chars() {
        let attaches = match prev {
            None => false,
            Some('\r') => ch == '\n',
            Some(p) if p == '\n' || p == '\r' => false,
            Some(_) if after_zwj && is_extended_pictographic(ch) => true,
            Some(_) if is_regional_indicator(ch) && ri_run % 2 == 1 => true,
            Some(_) => is_extend(ch) || is_format(ch) || ch == ZWJ,
        };
        if attaches {
            if let Some(last) = out.last_mut() {
                last.push(ch);
            }
        } else {
            out.push(ch.to_string());
        }
        ri_run = if is_regional_indicator(ch) {
            ri_run + 1
        } else {
            0
        };
        after_zwj = ch == ZWJ;
        prev = Some(ch);
    }
    out
}

// --- splitLineToFitWidth -----------------------------------------------------

/// `splitWordToFitWidth`: the longest prefix of the word that fits, and the
/// rest. A first grapheme that does not fit is taken anyway.
pub fn split_word_to_fit_width(check_fit: &dyn Fn(&[Word]) -> bool, word: &Word) -> (Word, Word) {
    let chars = split_text_to_chars(&word.content);
    let mut used = String::new();
    let mut index = 0;
    while index < chars.len() {
        let candidate = format!("{used}{}", chars[index]);
        if check_fit(&[Word::new(candidate.clone(), word.kind)]) {
            used = candidate;
            index += 1;
        } else {
            break;
        }
    }
    if used.is_empty() && index < chars.len() {
        used.push_str(&chars[index]);
        index += 1;
    }
    (
        Word::new(used, word.kind),
        Word::new(chars[index..].concat(), word.kind),
    )
}

/// `splitLineToFitWidth`: greedy fill with the line's words, where a single
/// space word joins the next word onto the line, and a word that does not
/// fit on an empty line is split by grapheme.
pub fn split_line_to_fit_width(line: &[Word], check_fit: &dyn Fn(&[Word]) -> bool) -> Vec<Line> {
    let mut words: std::collections::VecDeque<Word> = line.iter().cloned().collect();
    let mut lines: Vec<Line> = Vec::new();
    let mut new_line: Line = Vec::new();
    loop {
        if words.is_empty() {
            if !new_line.is_empty() {
                lines.push(new_line);
            }
            return lines;
        }
        let mut joiner = "";
        if words.front().is_some_and(|w| w.content == " ") {
            joiner = " ";
            words.pop_front();
        }
        let next_word = words.pop_front().unwrap_or_else(|| Word::normal(" "));
        let mut with_next = new_line.clone();
        if !joiner.is_empty() {
            with_next.push(Word::normal(joiner));
        }
        with_next.push(next_word.clone());
        if check_fit(&with_next) {
            new_line = with_next;
            continue;
        }
        if !new_line.is_empty() {
            lines.push(std::mem::take(&mut new_line));
            words.push_front(next_word);
        } else if !next_word.content.is_empty() {
            let (head, rest) = split_word_to_fit_width(check_fit, &next_word);
            lines.push(vec![head]);
            if !rest.content.is_empty() {
                words.push_front(rest);
            }
        }
    }
}

// --- wrapLabel ---------------------------------------------------------------

/// `wrapLabel`: mermaid's own pre-wrap. `text_width` is
/// `calculateTextWidth` bound to the label's font config.
pub fn wrap_label(
    label: &str,
    max_width: f32,
    join_with: &str,
    text_width: &dyn Fn(&str) -> f32,
) -> String {
    if label.is_empty() || super::markdown::has_breaks(label) {
        return label.to_string();
    }
    let words: Vec<&str> = label.split(' ').filter(|w| !w.is_empty()).collect();
    let mut completed: Vec<String> = Vec::new();
    let mut next_line = String::new();
    let count = words.len();
    for (index, word) in words.iter().enumerate() {
        let word_length = text_width(&format!("{word} "));
        let next_line_length = text_width(&next_line);
        if word_length > max_width {
            let (hyphenated, remaining) = break_string(word, max_width, '-', text_width);
            completed.push(std::mem::take(&mut next_line));
            completed.extend(hyphenated);
            next_line = remaining;
        } else if next_line_length + word_length >= max_width {
            completed.push(std::mem::take(&mut next_line));
            next_line = word.to_string();
        } else {
            next_line = [next_line.as_str(), word]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(" ");
        }
        if index + 1 == count {
            completed.push(next_line.clone());
        }
    }
    completed
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(join_with)
}

/// `breakString`: cuts a word into hyphenated pieces that reach
/// `max_width`, returning the pieces and the unfinished remainder.
pub fn break_string(
    word: &str,
    max_width: f32,
    hyphen: char,
    text_width: &dyn Fn(&str) -> f32,
) -> (Vec<String>, String) {
    let characters: Vec<char> = word.chars().collect();
    let mut lines = Vec::new();
    let mut current = String::new();
    for (index, ch) in characters.iter().enumerate() {
        let next_line = format!("{current}{ch}");
        let line_width = text_width(&next_line);
        if line_width >= max_width {
            let is_last = index + 1 == characters.len();
            lines.push(if is_last {
                next_line
            } else {
                format!("{next_line}{hyphen}")
            });
            current.clear();
        } else {
            current = next_line;
        }
    }
    (lines, current)
}

// --- Browser line breaking ----------------------------------------------------

/// `white-space: nowrap` collapsing: runs of whitespace become one space and
/// the line loses its leading and trailing spaces.
pub fn collapse_whitespace(line: &[Word]) -> Line {
    let mut out: Line = Vec::new();
    let mut in_space = false;
    let mut at_start = true;
    for word in line {
        let mut content = String::new();
        for ch in word.content.chars() {
            if ch.is_whitespace() && ch != '\u{a0}' {
                // The first space of a run survives in the run it starts in.
                if !at_start && !in_space {
                    content.push(' ');
                }
                in_space = true;
            } else {
                content.push(ch);
                at_start = false;
                in_space = false;
            }
        }
        if !content.is_empty() {
            out.push(Word::new(content, word.kind));
        }
    }
    while let Some(last) = out.last_mut() {
        if last.content.ends_with(' ') {
            last.content.pop();
        }
        if last.content.is_empty() {
            out.pop();
        } else {
            break;
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SegmentKind {
    Text,
    Space,
}

/// A piece of a line between two UAX #14 break opportunities, with its
/// style. Under `break-spaces` every space is a segment of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub word: Word,
    pub kind: SegmentKind,
}

/// Splits a line into break-spaces segments: a break opportunity exists
/// after every space, between ideographs, and after a hyphen between
/// letters.
pub fn html_segments(line: &[Word]) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    for word in line {
        let chars: Vec<char> = word.content.chars().collect();
        let mut current = String::new();
        let flush = |current: &mut String, out: &mut Vec<Segment>| {
            if !current.is_empty() {
                out.push(Segment {
                    word: Word::new(std::mem::take(current), word.kind),
                    kind: SegmentKind::Text,
                });
            }
        };
        for (i, &ch) in chars.iter().enumerate() {
            if ch == ' ' || ch == '\t' || ch == '\n' {
                flush(&mut current, &mut out);
                out.push(Segment {
                    word: Word::new(ch.to_string(), word.kind),
                    kind: SegmentKind::Space,
                });
                continue;
            }
            if is_ideographic(ch) {
                flush(&mut current, &mut out);
                current.push(ch);
                flush(&mut current, &mut out);
                continue;
            }
            current.push(ch);
            let next = chars.get(i + 1).copied();
            let hyphen_break = (ch == '-' || ch == '\u{2010}')
                && i > 0
                && chars[i - 1].is_alphabetic()
                && next.is_some_and(|n| n.is_alphabetic());
            if hyphen_break {
                flush(&mut current, &mut out);
            }
        }
        flush(&mut current, &mut out);
    }
    out
}

/// The unbreakable units of a line under `break-spaces`: a word carries
/// the space that follows it, because the break opportunity sits after the
/// space and the space never hangs (the oracle's wrapped cases show both:
/// a table grows to the width of the longest word plus its space, and a
/// word whose trailing space overflows moves to the next line whole).
/// Further spaces in a run are units of their own.
fn break_units(line: &[Word]) -> Vec<Line> {
    let segments = html_segments(line);
    let mut units: Vec<Line> = Vec::new();
    let mut i = 0;
    while i < segments.len() {
        let mut unit: Line = vec![segments[i].word.clone()];
        let is_text = segments[i].kind == SegmentKind::Text;
        i += 1;
        if is_text
            && segments
                .get(i)
                .is_some_and(|s| s.kind == SegmentKind::Space)
        {
            unit.push(segments[i].word.clone());
            i += 1;
        }
        units.push(unit);
    }
    units
}

/// Chrome's `white-space: break-spaces` wrapping at `avail` pixels: greedy
/// over the break units, where a unit that does not fit starts the next
/// line and a unit wider than the line overflows. `width` measures a
/// candidate line of styled runs.
pub fn wrap_break_spaces(line: &[Word], avail: f32, width: &dyn Fn(&[Word]) -> f32) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut current: Line = Vec::new();
    for unit in break_units(line) {
        let mut candidate = current.clone();
        candidate.extend(unit.iter().cloned());
        let fits = width(&candidate) <= avail + 1e-3;
        if fits || current.is_empty() {
            current = candidate;
        } else {
            lines.push(std::mem::take(&mut current));
            current = unit;
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

/// The widest break unit: the min-content width that a table grows to.
pub fn min_content_width(line: &[Word], width: &dyn Fn(&[Word]) -> f32) -> f32 {
    break_units(line)
        .iter()
        .map(|unit| width(unit))
        .fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::markdown::WordType;

    fn line(words: &[&str]) -> Line {
        words.iter().map(|w| Word::normal(*w)).collect()
    }

    #[test]
    fn emoji_presentation_is_a_subset_of_the_pictographs() {
        for ch in ['\u{231A}', '\u{26A1}', '\u{2B50}', '\u{1F600}', '\u{1FAE9}'] {
            assert!(is_emoji_presentation(ch), "{ch:?}");
        }
        for ch in [
            '\u{2764}', '\u{2605}', '\u{2713}', '\u{2192}', '\u{00A9}', 'a',
        ] {
            assert!(!is_emoji_presentation(ch), "{ch:?}");
        }
        let ranges = emoji_presentation::RANGES;
        assert!(ranges.windows(2).all(|w| w[0].1 + 1 < w[1].0));
        for &(first, last) in ranges {
            for cp in first..=last {
                let ch = char::from_u32(cp).expect("scalar value");
                // The skin tone modifiers are Extend, not pictographs.
                assert!(
                    is_extended_pictographic(ch) || is_regional_indicator(ch) || is_extend(ch),
                    "{cp:X}"
                );
            }
        }
    }

    fn words_of(text: &str) -> Line {
        split_line_to_words(text)
            .into_iter()
            .map(Word::normal)
            .collect()
    }

    fn joined(line: &[Word]) -> String {
        line.iter().map(|w| w.content.as_str()).collect()
    }

    fn check_fn(width: usize) -> impl Fn(&[Word]) -> bool {
        move |text: &[Word]| split_text_to_chars(&joined(text)).len() <= width
    }

    #[test]
    fn graphemes_match_intl_segmenter() {
        assert_eq!(split_text_to_chars(""), Vec::<String>::new());
        assert_eq!(split_text_to_chars("🏳️‍⚧️🏳️‍🌈👩🏾‍❤️‍👨🏻"), vec!["🏳️‍⚧️", "🏳️‍🌈", "👩🏾‍❤️‍👨🏻"]);
        assert_eq!(split_text_to_chars("ok"), vec!["o", "k"]);
        assert_eq!(split_text_to_chars("abc"), vec!["a", "b", "c"]);
        assert_eq!(split_text_to_chars("e\u{301}x"), vec!["e\u{301}", "x"]);
        assert_eq!(split_text_to_chars("🇸🇪🇸🇪"), vec!["🇸🇪", "🇸🇪"]);
    }

    #[test]
    fn words_match_intl_segmenter() {
        assert_eq!(
            split_line_to_words("hello world"),
            vec!["hello", " ", "world"]
        );
        assert_eq!(
            split_line_to_words("hello  1 2 world"),
            vec!["hello", "  ", "1", " ", "2", " ", "world"]
        );
        assert_eq!(split_line_to_words("中文中"), vec!["中", "文", "中"]);
        assert_eq!(
            split_line_to_words("Flag 🏳️‍⚧️ this 🏳️‍🌈"),
            vec!["Flag", " ", "🏳️‍⚧️", " ", "this", " ", "🏳️‍🌈"]
        );
        assert_eq!(
            split_line_to_words("hog... it's 3.14 a-b"),
            vec![
                "hog", ".", ".", ".", " ", "it's", " ", "3.14", " ", "a", "-", "b"
            ]
        );
        assert_eq!(split_line_to_words("snake_case"), vec!["snake_case"]);
        assert_eq!(split_line_to_words(""), Vec::<String>::new());
    }

    #[test]
    fn check_fn_counts_graphemes() {
        let check5 = check_fn(5);
        assert!(check5(&line(&["hello"])));
        assert!(!check5(&line(&["hello", "world"])));
        let check1 = check_fn(1);
        assert!(check1(&line(&["A"])));
        assert!(check1(&line(&["🏳️‍⚧️"])));
        assert!(!check1(&line(&["🏳️‍⚧️🏳️‍⚧️"])));
    }

    #[test]
    fn split_line_to_fit_width_matches_mermaid_spec() {
        let cases: &[(&str, usize, &[&str])] = &[
            ("hello world", 7, &["hello", "world"]),
            ("hello world", 20, &["hello world"]),
            ("hello world", 3, &["hel", "lo", "wor", "ld"]),
            ("hello 12 world", 4, &["hell", "o 12", "worl", "d"]),
            ("hello  1 2 world", 4, &["hell", "o  1", "2", "worl", "d"]),
            ("hello  1 2 world", 6, &["hello", "  1 2", "world"]),
            ("🏳️‍⚧️🏳️‍🌈👩🏾‍❤️‍👨🏻", 0, &["🏳️‍⚧️", "🏳️‍🌈", "👩🏾‍❤️‍👨🏻"]),
            ("🏳️‍⚧️🏳️‍🌈👩🏾‍❤️‍👨🏻", 1, &["🏳️‍⚧️", "🏳️‍🌈", "👩🏾‍❤️‍👨🏻"]),
            ("🏳️‍⚧️🏳️‍🌈👩🏾‍❤️‍👨🏻", 2, &["🏳️‍⚧️🏳️‍🌈", "👩🏾‍❤️‍👨🏻"]),
            ("🏳️‍⚧️🏳️‍🌈👩🏾‍❤️‍👨🏻", 3, &["🏳️‍⚧️🏳️‍🌈👩🏾‍❤️‍👨🏻"]),
            ("中文中", 1, &["中", "文", "中"]),
            ("中文中", 2, &["中文", "中"]),
            ("中文中", 3, &["中文中"]),
            ("Flag 🏳️‍⚧️ this 🏳️‍🌈", 6, &["Flag 🏳️‍⚧️", "this 🏳️‍🌈"]),
        ];
        for (text, width, expected) in cases {
            let check = check_fn(*width);
            let got = split_line_to_fit_width(&words_of(text), &check);
            let expected: Vec<Line> = expected.iter().map(|s| words_of(s)).collect();
            assert_eq!(got, expected, "{text} at {width}");
        }
    }

    #[test]
    fn split_word_keeps_style() {
        let check = check_fn(3);
        let (head, rest) = split_word_to_fit_width(&check, &Word::new("hello", WordType::Strong));
        assert_eq!(head, Word::new("hel", WordType::Strong));
        assert_eq!(rest, Word::new("lo", WordType::Strong));
    }

    #[test]
    fn wrap_label_breaks_on_width() {
        // Ten pixels per character, so a 55 px line holds five characters
        // plus the trailing space wrapLabel measures with each word.
        let width = |s: &str| 10.0 * s.chars().count() as f32;
        assert_eq!(wrap_label("", 50.0, "<br/>", &width), "");
        assert_eq!(wrap_label("a<br/>b", 50.0, "<br/>", &width), "a<br/>b");
        assert_eq!(
            wrap_label("one two three", 85.0, "<br/>", &width),
            "one two<br/>three"
        );
        assert_eq!(
            wrap_label("abcdefghij", 55.0, "<br/>", &width),
            "abcdef-<br/>ghij"
        );
        assert_eq!(
            wrap_label("abcdefghijk x", 55.0, "<br/>", &width),
            "abcdef-<br/>ghijk<br/>x"
        );
    }

    #[test]
    fn break_string_hyphenates() {
        let width = |s: &str| 10.0 * s.chars().count() as f32;
        let (lines, rest) = break_string("abcdefg", 30.0, '-', &width);
        assert_eq!(lines, vec!["abc-", "def-"]);
        assert_eq!(rest, "g");
        let (lines, rest) = break_string("abcdef", 30.0, '-', &width);
        assert_eq!(lines, vec!["abc-", "def"]);
        assert_eq!(rest, "");
    }

    #[test]
    fn collapse_whitespace_like_nowrap() {
        assert_eq!(
            collapse_whitespace(&line(&["  Hello  ", "big ", " world  "])),
            line(&["Hello ", "big ", "world"])
        );
        assert_eq!(collapse_whitespace(&line(&["   "])), Line::new());
    }

    #[test]
    fn break_spaces_wraps_greedily_and_keeps_spaces() {
        let width = |l: &[Word]| 10.0 * joined(l).chars().count() as f32;
        let text = line(&["aaa bbb ccc"]);
        let lines = wrap_break_spaces(&text, 80.0, &width);
        let texts: Vec<String> = lines.iter().map(|l| joined(l)).collect();
        assert_eq!(texts, vec!["aaa bbb ", "ccc"]);
        // "bbb " does not fit with its space, so the word moves down whole.
        let lines = wrap_break_spaces(&text, 70.0, &width);
        let texts: Vec<String> = lines.iter().map(|l| joined(l)).collect();
        assert_eq!(texts, vec!["aaa ", "bbb ccc"]);
        let lines = wrap_break_spaces(&line(&["中文中"]), 20.0, &width);
        let texts: Vec<String> = lines.iter().map(|l| joined(l)).collect();
        assert_eq!(texts, vec!["中文", "中"]);
        assert_eq!(min_content_width(&text, &width), 40.0);
        assert_eq!(min_content_width(&line(&["a bbbbb"]), &width), 50.0);
    }
}
