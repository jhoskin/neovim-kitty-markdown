//! Port of `diagrams/packet/db.ts` (`PacketDB`), the `populate` and
//! `getNextFittingBlock` functions of `parser.ts`, and the parts of
//! `commonDb.ts` the packet diagram reads.

use serde_json::Value;

use super::parser::Ast;
use crate::mermaid::config::Resolved;
use crate::mermaid::theme::khroma::js_number;

/// `PacketDB.getConfig()`: the effective `packet` section with `paddingY`
/// raised by 10 when `showBits` is on. Numbers are JS numbers, so a
/// fractional or out-of-range value flows into the layout as mermaid lets
/// it; a value that is not a number falls back to the schema default, where
/// mermaid would compute `NaN`.
#[derive(Debug, Clone, PartialEq)]
pub struct PacketConfig {
    pub row_height: f64,
    pub bit_width: f64,
    pub bits_per_row: f64,
    pub show_bits: bool,
    /// `bitOrder === 'descending'`.
    pub descending: bool,
    pub padding_x: f64,
    pub padding_y: f64,
    pub use_max_width: bool,
}

impl PacketConfig {
    /// `cleanAndMerge({ ...DEFAULT_PACKET_CONFIG, ...getConfig().packet })`
    /// followed by the `showBits` padding bump.
    pub fn from_resolved(resolved: &Resolved) -> PacketConfig {
        let section = &resolved.config.packet;
        let num =
            |key: &str, default: f64| section.get(key).and_then(Value::as_f64).unwrap_or(default);
        let truthy = |key: &str, default: bool| match section.get(key) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_f64().is_some_and(|v| v != 0.0 && !v.is_nan()),
            Some(Value::String(s)) => !s.is_empty(),
            Some(Value::Null) | None => default,
            Some(_) => true,
        };
        let show_bits = truthy("showBits", true);
        let mut padding_y = num("paddingY", 5.0);
        if show_bits {
            padding_y += 10.0;
        }
        PacketConfig {
            row_height: num("rowHeight", 32.0),
            bit_width: num("bitWidth", 32.0),
            bits_per_row: num("bitsPerRow", 32.0),
            show_bits,
            descending: section.get("bitOrder").and_then(Value::as_str) == Some("descending"),
            padding_x: num("paddingX", 5.0),
            padding_y,
            use_max_width: truthy("useMaxWidth", true),
        }
    }
}

/// A `Required<PacketBlock>`: one piece of a field as it sits in a row.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub start: f64,
    pub end: f64,
    pub bits: f64,
    pub label: String,
}

/// A `PacketWord`: the blocks of one row.
pub type Word = Vec<Block>;

/// The error `populate` throws, with the line of the block so the plugin
/// can point at it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopulateError {
    pub message: String,
    pub line: u32,
}

/// `PacketDB`: the rows in order plus the common db fields.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PacketDb {
    packet: Vec<Word>,
    title: String,
    acc_title: String,
    acc_descr: String,
}

/// `maxPacketSize` in `parser.ts`: the row count that stops the fill loop.
const MAX_PACKET_SIZE: usize = 10_000;

impl PacketDb {
    pub fn new() -> PacketDb {
        PacketDb::default()
    }

    /// `setDiagramTitle`: `sanitizeText` leaves plain text as it is.
    pub fn set_diagram_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn diagram_title(&self) -> &str {
        &self.title
    }

    pub fn packet(&self) -> &[Word] {
        &self.packet
    }

    /// `pushWord`: an empty word is dropped.
    fn push_word(&mut self, word: Word) {
        if !word.is_empty() {
            self.packet.push(word);
        }
    }

    /// `populate(ast, db)` in `parser.ts`: `populateCommonDb` (a falsy
    /// value sets nothing), then every block is checked for order and
    /// contiguity, completed from `bits` or `start`, and cut at the row
    /// boundaries.
    pub fn populate(&mut self, ast: &Ast, bits_per_row: f64) -> Result<(), PopulateError> {
        if let Some(d) = ast.acc_descr.as_deref().filter(|s| !s.is_empty()) {
            self.acc_descr = d.to_string();
        }
        if let Some(t) = ast.acc_title.as_deref().filter(|s| !s.is_empty()) {
            self.acc_title = t.to_string();
        }
        if let Some(t) = ast.title.as_deref().filter(|s| !s.is_empty()) {
            self.set_diagram_title(t);
        }
        let mut last_bit = -1.0;
        let mut word: Word = Vec::new();
        let mut row = 1.0;
        for ast_block in &ast.blocks {
            let fail = |message: String| PopulateError {
                message,
                line: ast_block.line,
            };
            let (start, end, bits) = (ast_block.start, ast_block.end, ast_block.bits);
            if let (Some(s), Some(e)) = (start, end)
                && e < s
            {
                return Err(fail(format!(
                    "Packet block {} - {} is invalid. End must be greater than start.",
                    js_number(s),
                    js_number(e)
                )));
            }
            let s = start.unwrap_or(last_bit + 1.0);
            if s != last_bit + 1.0 {
                return Err(fail(format!(
                    "Packet block {} - {} is not contiguous. It should start from {}.",
                    js_number(s),
                    js_number(end.unwrap_or(s)),
                    js_number(last_bit + 1.0)
                )));
            }
            if bits == Some(0.0) {
                return Err(fail(format!(
                    "Packet block {} is invalid. Cannot have a zero bit field.",
                    js_number(s)
                )));
            }
            let e = end.unwrap_or(s + bits.unwrap_or(1.0) - 1.0);
            let b = bits.unwrap_or(e - s + 1.0);
            last_bit = e;
            let mut current = Block {
                start: s,
                end: e,
                bits: b,
                label: ast_block.label.clone(),
            };
            while word.len() as f64 <= bits_per_row + 1.0 && self.packet.len() < MAX_PACKET_SIZE {
                let (block, next) = next_fitting_block(current, row, bits_per_row)?;
                let block_end = block.end;
                word.push(block);
                if block_end + 1.0 == row * bits_per_row {
                    self.push_word(std::mem::take(&mut word));
                    row += 1.0;
                }
                match next {
                    Some(n) => current = n,
                    None => break,
                }
            }
        }
        self.push_word(word);
        Ok(())
    }
}

/// `getNextFittingBlock`: the part of `block` that fits in `row`, and the
/// rest for the next row when the block runs past the row end. The `bits`
/// of the two parts are off by one from their size, as mermaid computes
/// them; the renderer measures blocks by `end - start + 1`.
fn next_fitting_block(
    block: Block,
    row: f64,
    bits_per_row: f64,
) -> Result<(Block, Option<Block>), PopulateError> {
    if block.start > block.end {
        return Err(PopulateError {
            message: format!(
                "Block start {} is greater than block end {}.",
                js_number(block.start),
                js_number(block.end)
            ),
            line: 0,
        });
    }
    if block.end + 1.0 <= row * bits_per_row {
        return Ok((block, None));
    }
    let row_end = row * bits_per_row - 1.0;
    let row_start = row * bits_per_row;
    Ok((
        Block {
            start: block.start,
            end: row_end,
            bits: row_end - block.start,
            label: block.label.clone(),
        },
        Some(Block {
            start: row_start,
            end: block.end,
            bits: block.end - row_start,
            label: block.label,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::diagrams::packet::parser::parse;

    fn rows(text: &str, bits_per_row: f64) -> Vec<Vec<(f64, f64, String)>> {
        let mut db = PacketDb::new();
        db.populate(&parse(text).unwrap(), bits_per_row).unwrap();
        db.packet()
            .iter()
            .map(|w| {
                w.iter()
                    .map(|b| (b.start, b.end, b.label.clone()))
                    .collect()
            })
            .collect()
    }

    fn error(text: &str) -> PopulateError {
        let mut db = PacketDb::new();
        db.populate(&parse(text).unwrap(), 32.0).unwrap_err()
    }

    #[test]
    fn rows_fill_to_bits_per_row_and_a_block_splits_at_the_row_end() {
        let got = rows(
            "packet-beta\n0-15: \"A\"\n16-31: \"B\"\n32-63: \"C\"\n64-71: \"D\"\n",
            32.0,
        );
        assert_eq!(
            got,
            vec![
                vec![(0.0, 15.0, "A".to_string()), (16.0, 31.0, "B".to_string())],
                vec![(32.0, 63.0, "C".to_string())],
                vec![(64.0, 71.0, "D".to_string())],
            ]
        );
        let wide = rows(
            "packet\n0-7: \"Head\"\n8-71: \"Payload\"\n72-79: \"Tail\"\n",
            32.0,
        );
        assert_eq!(
            wide,
            vec![
                vec![
                    (0.0, 7.0, "Head".to_string()),
                    (8.0, 31.0, "Payload".to_string())
                ],
                vec![(32.0, 63.0, "Payload".to_string())],
                vec![
                    (64.0, 71.0, "Payload".to_string()),
                    (72.0, 79.0, "Tail".to_string())
                ],
            ]
        );
    }

    #[test]
    fn bits_and_single_bit_forms_follow_the_last_bit() {
        let got = rows(
            "packet\n+16: \"A\"\n+16: \"B\"\n32: \"C\"\n33-40: \"D\"\n",
            32.0,
        );
        assert_eq!(
            got,
            vec![
                vec![(0.0, 15.0, "A".to_string()), (16.0, 31.0, "B".to_string())],
                vec![(32.0, 32.0, "C".to_string()), (33.0, 40.0, "D".to_string())],
            ]
        );
    }

    #[test]
    fn validation_errors_use_the_oracle_texts() {
        assert_eq!(
            error("packet\n0-7: \"A\"\n9-15: \"B\"\n"),
            PopulateError {
                message: "Packet block 9 - 15 is not contiguous. It should start from 8."
                    .to_string(),
                line: 3,
            }
        );
        assert_eq!(
            error("packet\n7-0: \"A\"\n").message,
            "Packet block 7 - 0 is invalid. End must be greater than start."
        );
        assert_eq!(
            error("packet\n+0: \"A\"\n").message,
            "Packet block 0 is invalid. Cannot have a zero bit field."
        );
        assert_eq!(
            error("packet\n0-7: \"A\"\n5: \"B\"\n").message,
            "Packet block 5 - 5 is not contiguous. It should start from 8."
        );
    }

    #[test]
    fn front_matter_title_loses_to_a_title_in_the_diagram() {
        let mut db = PacketDb::new();
        db.set_diagram_title("From front matter");
        db.populate(&parse("packet\n title\n 0: \"A\"\n").unwrap(), 32.0)
            .unwrap();
        assert_eq!(db.diagram_title(), "From front matter");
        db.populate(&parse("packet\ntitle In text\n").unwrap(), 32.0)
            .unwrap();
        assert_eq!(db.diagram_title(), "In text");
    }
}
