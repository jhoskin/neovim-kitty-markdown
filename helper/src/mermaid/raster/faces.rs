//! The face that draws each stretch of label text.
//!
//! Chrome draws each grapheme in the face its font fallback picked, which
//! is the face the measurer measured it in (`TextMetrics::grapheme_faces`).
//! usvg has a fallback of its own, and it redraws a whole run in a
//! fallback face when that face covers every character of the run. So the
//! raster SVG names the face of every fallback stretch in a `tspan` of its
//! own, with a family token that only [`face_for_family`] knows, and
//! resvg never falls back.
//!
//! Two kinds of face resvg cannot draw as Chrome does, and the raster step
//! draws their glyphs itself on macOS (`own.rs`): a face whose outline
//! table usvg cannot read (PingFang keeps its outlines in a table that
//! skrifa does not parse) and a bitmap emoji face (`sbix`, Apple Color
//! Emoji), which Chrome draws through CoreText. Such a grapheme becomes an
//! own piece: a placeholder in the primary face, spaced to the measured
//! advance, at whose place the raster step draws the grapheme. Keeping the
//! real text out of the SVG also keeps it out of usvg's span merging,
//! which panics when two faces shape a ZWJ sequence or a flag into
//! clusters of different lengths.

use std::cell::RefCell;

use rustybuzz::ttf_parser::Tag;

use crate::mermaid::scene::FontStyle;
use crate::mermaid::scene::svg::{FacePiece, OWN_PLACEHOLDER, RunFont};
use crate::text::fonts::{FontId, Style, Weight};
use crate::text::{FontSpec, FontSystem, TextMetrics, measure};

/// The `font-family` prefix of a face token. The digits after it index
/// the table.
const TOKEN_PREFIX: &str = "kitty-markdown-face-";

/// A letter-spacing below this many pixels is left out.
const SPACING_EPSILON: f64 = 1e-4;

/// The face a token family names in `faces`, the table's faces in token
/// order.
pub fn face_for_family(faces: &[FontId], family: &str) -> Option<FontId> {
    let index: usize = family.strip_prefix(TOKEN_PREFIX)?.parse().ok()?;
    faces.get(index).copied()
}

/// How resvg can draw a face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceKind {
    /// usvg reads the outlines; resvg draws them.
    Outline,
    /// The outlines are in a table usvg cannot read.
    UnreadableOutline,
    /// A named instance of a variable font whose outlines usvg reads
    /// (Kefa Bold, Noto Sans Syriac Bold). usvg draws a face at the axis
    /// values of `font-variation-settings` only, and the scene SVG does
    /// not write them, so it would draw the default instance.
    OutlineInstance,
    /// Color bitmaps in an `sbix` table.
    Sbix,
}

/// A grapheme the raster step draws itself. Only macOS makes them.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub struct OwnPiece {
    pub text: String,
    pub face: FontId,
    /// The font size in CSS pixels.
    pub size: f32,
}

/// The faces the raster SVG names by token, in token order, and the own
/// pieces, in the order they were found.
pub struct FaceTable<'f> {
    fonts: &'f FontSystem,
    metrics: TextMetrics<'f>,
    faces: RefCell<Vec<FontId>>,
    own: RefCell<Vec<OwnPiece>>,
}

impl<'f> FaceTable<'f> {
    pub fn new(fonts: &'f FontSystem) -> FaceTable<'f> {
        FaceTable {
            fonts,
            metrics: TextMetrics::new(fonts),
            faces: RefCell::new(Vec::new()),
            own: RefCell::new(Vec::new()),
        }
    }

    /// The faces named so far, in token order.
    pub fn faces(&self) -> Vec<FontId> {
        self.faces.borrow().clone()
    }

    /// The own piece a placeholder's index names.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn own_piece(&self, index: usize) -> Option<OwnPiece> {
        self.own.borrow().get(index).cloned()
    }

    fn token(&self, face: FontId) -> String {
        let mut faces = self.faces.borrow_mut();
        let index = match faces.iter().position(|&f| f == face) {
            Some(i) => i,
            None => {
                faces.push(face);
                faces.len() - 1
            }
        };
        format!("{TOKEN_PREFIX}{index}")
    }

    /// How resvg can draw `face`.
    pub fn kind(&self, face: FontId) -> FaceKind {
        self.fonts
            .with_face(face, |f| {
                let tables = f.tables();
                if f.raw_face().table(Tag::from_bytes(b"sbix")).is_some() {
                    FaceKind::Sbix
                } else if tables.glyf.is_some() || tables.cff.is_some() || tables.cff2.is_some() {
                    if self.fonts.is_named_instance(face) {
                        FaceKind::OutlineInstance
                    } else {
                        FaceKind::Outline
                    }
                } else {
                    FaceKind::UnreadableOutline
                }
            })
            .unwrap_or(FaceKind::Outline)
    }

    /// True when the raster step draws the glyphs of `face` itself. Only
    /// macOS can: CoreText reads every face Chrome uses there.
    pub fn own_drawing(&self, face: FontId) -> bool {
        cfg!(target_os = "macos") && self.kind(face) != FaceKind::Outline
    }

    /// The stretches of `text` that Chrome draws in a face other than the
    /// primary face of `font`. Whitespace stays with the primary face, as
    /// the measurer assigns it. An own-drawing face gets one own piece per
    /// grapheme, so each grapheme's advance is corrected on its own; it
    /// needs a primary face with the placeholder, else it stays a plain
    /// piece that resvg draws as well as it can.
    pub fn pieces(&self, text: &str, font: &RunFont<'_>) -> Vec<FacePiece> {
        let weight = Weight(font.weight);
        let style = match font.style {
            FontStyle::Normal => Style::Normal,
            FontStyle::Italic => Style::Italic,
            FontStyle::Oblique => Style::Oblique,
        };
        let Some(primary) = self.fonts.resolve(font.family, weight, style) else {
            return Vec::new();
        };
        let size = font.size as f32;
        let spec = FontSpec::new(font.family, size)
            .weight(font.weight)
            .style(match font.style {
                FontStyle::Normal => measure::FontStyle::Normal,
                FontStyle::Italic | FontStyle::Oblique => measure::FontStyle::Italic,
            });
        let placeholder = OWN_PLACEHOLDER
            .chars()
            .all(|c| self.fonts.has_glyph(primary, c));
        let own = |face: FontId| placeholder && self.own_drawing(face);
        let mut stretches: Vec<(std::ops::Range<usize>, FontId)> = Vec::new();
        let mut at = 0;
        for (grapheme, face) in self.metrics.grapheme_faces(text, &spec) {
            let range = at..at + grapheme.len();
            at = range.end;
            if face == primary {
                continue;
            }
            match stretches.last_mut() {
                Some((last, f)) if *f == face && last.end == range.start && !own(face) => {
                    last.end = range.end;
                }
                _ => stretches.push((range, face)),
            }
        }
        // The measurer's graphemes concatenate to the text; anything else
        // would cut a character in half.
        if at != text.len() {
            return Vec::new();
        }
        stretches
            .into_iter()
            .map(|(range, face)| {
                let synthetic_bold =
                    font.weight >= 600 && self.fonts.face_weight(face).is_some_and(|w| w.0 < 600);
                let piece = &text[range.clone()];
                let measured = self.metrics.run_advance(piece, face, size);
                if own(face) {
                    let (shaped, _) = self.plain_advance(OWN_PLACEHOLDER, primary, size);
                    let mut list = self.own.borrow_mut();
                    list.push(OwnPiece {
                        text: piece.to_string(),
                        face,
                        size,
                    });
                    FacePiece {
                        range,
                        family: None,
                        synthetic_bold,
                        letter_spacing: spacing(measured - shaped, 1),
                        own: Some(list.len() - 1),
                    }
                } else {
                    let (shaped, clusters) = self.plain_advance(piece, face, size);
                    FacePiece {
                        family: Some(self.token(face)),
                        synthetic_bold,
                        letter_spacing: spacing(measured - shaped, clusters),
                        own: None,
                        range,
                    }
                }
            })
            .collect()
    }

    /// The advance usvg gives `text` in `face` and the number of clusters
    /// it adds letter-spacing to: usvg shapes with HarfBuzz rules and
    /// nothing else, while Chrome adds PingFang's `trak` tracking and draws
    /// Apple Color Emoji 1.25 times wider than its `hmtx` advance, rounded
    /// to whole pixels.
    fn plain_advance(&self, text: &str, face: FontId, size: f32) -> (f32, usize) {
        self.fonts
            .with_face(face, |f| {
                let mut buffer = rustybuzz::UnicodeBuffer::new();
                buffer.push_str(text);
                let glyphs = rustybuzz::shape(f, &[], buffer);
                let scale = size / f.units_per_em() as f32;
                let advance: f32 = glyphs
                    .glyph_positions()
                    .iter()
                    .map(|p| p.x_advance as f32 * scale)
                    .sum();
                let mut clusters: Vec<u32> =
                    glyphs.glyph_infos().iter().map(|i| i.cluster).collect();
                clusters.dedup();
                (advance, clusters.len())
            })
            .unwrap_or((0.0, 0))
    }
}

/// The letter-spacing that spreads `missing` pixels over `clusters`, as
/// usvg adds letter-spacing after each cluster.
fn spacing(missing: f32, clusters: usize) -> Option<f64> {
    if clusters == 0 {
        return None;
    }
    let spacing = f64::from(missing) / clusters as f64;
    (spacing.abs() > SPACING_EPSILON).then_some(spacing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::scene::DEFAULT_FONT_FAMILY;

    fn font(weight: u16) -> RunFont<'static> {
        RunFont {
            family: DEFAULT_FONT_FAMILY,
            size: 16.0,
            weight,
            style: FontStyle::Normal,
        }
    }

    /// The pieces of `text` as (piece text, face family, own) triples.
    #[cfg(target_os = "macos")]
    fn described(table: &FaceTable<'_>, text: &str, weight: u16) -> Vec<(String, String, bool)> {
        let fonts = crate::text::font_system();
        let pieces = table.pieces(text, &font(weight));
        let faces = table.faces();
        pieces
            .into_iter()
            .map(|p| {
                let face = match (&p.family, p.own) {
                    (Some(family), _) => face_for_family(&faces, family),
                    (None, Some(i)) => table.own_piece(i).map(|o| o.face),
                    (None, None) => None,
                };
                let name = face.and_then(|f| fonts.family_name(f)).unwrap_or_default();
                (text[p.range].to_string(), name, p.own.is_some())
            })
            .collect()
    }

    #[test]
    fn primary_face_text_has_no_pieces() {
        let table = FaceTable::new(crate::text::font_system());
        assert!(table.pieces("Fix the login bug", &font(400)).is_empty());
        assert!(table.pieces("", &font(400)).is_empty());
    }

    #[test]
    fn tokens_name_faces_in_table_order() {
        let table = FaceTable::new(crate::text::font_system());
        let a = table.token(FontId::dummy());
        assert_eq!(a, format!("{TOKEN_PREFIX}0"));
        assert_eq!(table.token(FontId::dummy()), a);
        assert_eq!(face_for_family(&table.faces(), &a), Some(FontId::dummy()));
        assert_eq!(
            face_for_family(&table.faces(), "kitty-markdown-face-9"),
            None
        );
        assert_eq!(face_for_family(&table.faces(), "Arial"), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn each_script_goes_to_the_face_chrome_draws_it_in() {
        // The faces Chrome 156 reports through `CSS.getPlatformFontsForNode`
        // for these labels under mermaid's default chain.
        let table = FaceTable::new(crate::text::font_system());
        let pieces = described(&table, "ログイン 한국어", 400);
        assert_eq!(
            pieces,
            vec![
                ("ログイン".to_string(), "Hiragino Sans".to_string(), false),
                (
                    "한국어".to_string(),
                    "Apple SD Gothic Neo".to_string(),
                    false
                ),
            ]
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn faces_resvg_cannot_draw_become_own_pieces_per_grapheme() {
        let table = FaceTable::new(crate::text::font_system());
        // PingFang's outlines and Apple Color Emoji's bitmaps; the flag and
        // the ZWJ sequence stay one grapheme each.
        let pieces = described(&table, "中文 📋🇰🇷🧑\u{200D}💻", 400);
        let texts: Vec<&str> = pieces.iter().map(|p| p.0.as_str()).collect();
        assert_eq!(texts, ["中", "文", "📋", "🇰🇷", "🧑\u{200D}💻"]);
        assert!(pieces.iter().all(|p| p.2), "{pieces:?}");
        assert!(pieces[0].1.starts_with("PingFang"), "{pieces:?}");
        assert_eq!(pieces[2].1, "Apple Color Emoji");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn spacing_brings_the_placeholder_to_the_measured_advance() {
        let fonts = crate::text::font_system();
        let table = FaceTable::new(fonts);
        let pieces = table.pieces("a📋", &font(400));
        let [piece] = pieces.as_slice() else {
            panic!("{pieces:?}");
        };
        let primary = fonts
            .resolve(DEFAULT_FONT_FAMILY, Weight(400), Style::Normal)
            .expect("Trebuchet MS");
        let (placeholder, _) = table.plain_advance(OWN_PLACEHOLDER, primary, 16.0);
        // Chrome draws a 16 px emoji 20 px wide (1.25 em, whole pixels).
        let spacing = piece.letter_spacing.expect("spacing");
        assert!(
            (f64::from(placeholder) + spacing - 20.0).abs() < 1e-3,
            "{spacing}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_light_fallback_face_under_a_bold_run_is_emboldened() {
        let fonts = crate::text::font_system();
        let table = FaceTable::new(fonts);
        // Hiragino Sans has a W6 for 700, so Chrome needs no synthetic bold;
        // Apple Color Emoji has one weight only.
        let pieces = table.pieces("カ😀", &font(700));
        assert_eq!(pieces.len(), 2, "{pieces:?}");
        assert!(!pieces[0].synthetic_bold);
        assert!(pieces[1].synthetic_bold);
    }
}
