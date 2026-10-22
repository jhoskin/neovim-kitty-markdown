//! Port of `diagrams/packet/renderer.ts` (`draw` and `drawWord`).
//!
//! The layout is arithmetic on the config: every row is `rowHeight +
//! paddingY` tall, every bit `bitWidth` wide, and the SVG is `bitWidth *
//! bitsPerRow + 2` by `(rows + 1) * (rowHeight + paddingY)` less one
//! `rowHeight` when there is no title. No text is measured.
//! `configureSvgSize(svg, height, width, useMaxWidth)` gives the SVG the
//! viewBox `0 0 width height`, so the natural size is that box.

use super::db::{PacketConfig, PacketDb};
use crate::mermaid::scene::{
    Baseline, Color, Common, Element, FontSpec, Group, Paint, Rect, Scene, Style, Text, TextAnchor,
    TextRun,
};
use crate::mermaid::styles::packet::PacketStyles;
use crate::mermaid::theme::khroma::js_number;

fn text_style(fill: Option<Color>, size: f64) -> Style {
    Style {
        // A dropped `fill` declaration inherits the root `fill`.
        fill: fill.map(Paint::Color),
        font: FontSpec {
            size: Some(size),
            ..FontSpec::default()
        },
        ..Style::default()
    }
}

fn text(
    class: &str,
    x: f64,
    y: f64,
    anchor: TextAnchor,
    baseline: Baseline,
    style: Style,
    content: &str,
) -> Element {
    Element::Text(Text {
        common: Common {
            class: Some(class.to_string()),
            style,
            ..Common::default()
        },
        x,
        y,
        anchor: Some(anchor),
        baseline: Some(baseline),
        font: FontSpec::default(),
        runs: vec![TextRun::new(content)],
    })
}

/// `drawWord(svg, word, rowNumber, config)`: one `g` per row with a rect
/// and a centered label per block, and the bit numbers above the block
/// when `showBits` is on.
fn draw_word(
    word: &[super::db::Block],
    row_number: f64,
    cfg: &PacketConfig,
    styles: &PacketStyles,
) -> Element {
    let mut group = Group::default();
    let word_y = row_number * (cfg.row_height + cfg.padding_y) + cfg.padding_y;
    for block in word {
        let bits = block.end - block.start + 1.0;
        // The column the block starts at within its row; `descending`
        // mirrors the row so it reads from its highest bit down.
        let first_column = block.start % cfg.bits_per_row;
        let column = if cfg.descending {
            cfg.bits_per_row - first_column - bits
        } else {
            first_column
        };
        let block_x = column * cfg.bit_width + 1.0;
        let width = bits * cfg.bit_width - cfg.padding_x;
        group.children.push(Element::Rect(Rect {
            common: Common {
                class: Some("packetBlock".to_string()),
                style: Style {
                    fill: styles.block_fill_color.map(Paint::Color),
                    stroke: styles.block_stroke_color.map(Paint::Color),
                    stroke_width: Some(styles.block_stroke_width),
                    ..Style::default()
                },
                ..Common::default()
            },
            x: block_x,
            y: word_y,
            width,
            height: cfg.row_height,
            rx: None,
            ry: None,
        }));
        group.children.push(text(
            "packetLabel",
            block_x + width / 2.0,
            word_y + cfg.row_height / 2.0,
            TextAnchor::Middle,
            Baseline::Middle,
            text_style(styles.label_color, styles.label_font_size),
            &block.label,
        ));
        if !cfg.show_bits {
            continue;
        }
        let (leading_bit, trailing_bit) = if cfg.descending {
            (block.end, block.start)
        } else {
            (block.start, block.end)
        };
        let is_single_block = bits == 1.0;
        let bit_number_y = word_y - 2.0;
        group.children.push(text(
            "packetByte start",
            block_x + if is_single_block { width / 2.0 } else { 0.0 },
            bit_number_y,
            if is_single_block {
                TextAnchor::Middle
            } else {
                TextAnchor::Start
            },
            Baseline::Auto,
            text_style(styles.start_byte_color, styles.byte_font_size),
            &js_number(leading_bit),
        ));
        if !is_single_block {
            group.children.push(text(
                "packetByte end",
                block_x + width,
                bit_number_y,
                TextAnchor::End,
                Baseline::Auto,
                text_style(styles.end_byte_color, styles.byte_font_size),
                &js_number(trailing_bit),
            ));
        }
    }
    Element::Group(group)
}

/// `draw`: the rows, then the title text (appended even when empty) in the
/// last row slot.
pub fn draw(
    db: &PacketDb,
    cfg: &PacketConfig,
    styles: &PacketStyles,
    text_color: Option<Color>,
) -> Scene {
    let words = db.packet();
    let title = db.diagram_title();
    let total_row_height = cfg.row_height + cfg.padding_y;
    let svg_height = total_row_height * (words.len() as f64 + 1.0)
        - if title.is_empty() {
            cfg.row_height
        } else {
            0.0
        };
    let svg_width = cfg.bit_width * cfg.bits_per_row + 2.0;

    let mut scene = Scene::new(svg_width, svg_height);
    scene.font_family = styles.font_family.clone();
    // `#id { fill: textColor }` from the global stylesheet.
    scene.root.common.style = Style {
        fill: text_color.map(Paint::Color),
        ..Style::default()
    };
    for (row, word) in words.iter().enumerate() {
        scene
            .root
            .children
            .push(draw_word(word, row as f64, cfg, styles));
    }
    scene.root.children.push(text(
        "packetTitle",
        svg_width / 2.0,
        svg_height - total_row_height / 2.0,
        TextAnchor::Middle,
        Baseline::Middle,
        text_style(styles.title_color, styles.title_font_size),
        title,
    ));
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::config::{self, BaseConfig};
    use crate::mermaid::diagrams::packet::parser::parse;

    fn scene_for(source: &str) -> Scene {
        let resolved = config::resolve(source, &BaseConfig::new("default")).unwrap();
        let cfg = PacketConfig::from_resolved(&resolved);
        let mut db = PacketDb::new();
        db.populate(
            &parse(&format!("{}\n", resolved.code())).unwrap(),
            cfg.bits_per_row,
        )
        .unwrap();
        let styles = PacketStyles::resolve(&resolved.theme_variables, "serif");
        draw(&db, &cfg, &styles, None)
    }

    #[test]
    fn default_geometry_matches_the_oracle_svg() {
        // Two rows, no title: `showBits` lifts `paddingY` to 15, so a row
        // is 47 tall and the height is `47 * 3 - 32`.
        let scene = scene_for("packet-beta\n0-15: \"A\"\n16-31: \"B\"\n32-63: \"C\"\n");
        assert_eq!((scene.width, scene.height), (1026.0, 109.0));
        let Element::Group(row) = &scene.root.children[0] else {
            panic!("row group");
        };
        let Element::Rect(rect) = &row.children[0] else {
            panic!("block rect");
        };
        assert_eq!(
            (rect.x, rect.y, rect.width, rect.height),
            (1.0, 15.0, 507.0, 32.0)
        );
        let Element::Text(bit) = &row.children[2] else {
            panic!("start bit");
        };
        assert_eq!((bit.x, bit.y), (1.0, 13.0));
        assert_eq!(bit.runs[0].text, "0");
        let Element::Text(end_bit) = &row.children[3] else {
            panic!("end bit");
        };
        assert_eq!((end_bit.x, end_bit.anchor), (508.0, Some(TextAnchor::End)));
    }

    #[test]
    fn a_title_adds_a_row_and_show_bits_off_drops_the_numbers() {
        let scene = scene_for(
            "---\nconfig:\n  packet:\n    showBits: false\n---\npacket\ntitle T\n0-31: \"A\"\n",
        );
        assert_eq!(scene.height, 74.0);
        let Element::Group(row) = &scene.root.children[0] else {
            panic!("row group");
        };
        assert_eq!(row.children.len(), 2);
        let Element::Text(title) = &scene.root.children[1] else {
            panic!("title");
        };
        assert_eq!((title.x, title.y), (513.0, 55.5));
    }

    #[test]
    fn descending_order_mirrors_each_row() {
        let scene = scene_for(
            "---\nconfig:\n  packet:\n    bitOrder: descending\n    bitsPerRow: 16\n---\npacket\n0-7: \"DATA\"\n8-11: \"TYPE\"\n12: \"EN\"\n13-15: \"RESERVED\"\n",
        );
        let Element::Group(row) = &scene.root.children[0] else {
            panic!("row group");
        };
        let Element::Rect(data) = &row.children[0] else {
            panic!("rect");
        };
        assert_eq!(data.x, 8.0 * 32.0 + 1.0);
        let Element::Text(leading) = &row.children[2] else {
            panic!("leading bit");
        };
        assert_eq!(leading.runs[0].text, "7");
    }
}
