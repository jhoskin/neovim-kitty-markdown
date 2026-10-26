//! The glyphs the raster step draws itself, as SVG elements.
//!
//! The first parse of the raster SVG lays the text out with a placeholder
//! for each own piece (`faces.rs`), so usvg says where each piece starts
//! and which span (fill, synthetic bold stroke) it belongs to. For each
//! glyph of the piece this module writes the element Chrome's drawing
//! amounts to: the CoreText outline as a path in the span's paint, or the
//! CoreText color bitmap as an image on whole device pixels. The second
//! parse hides the placeholders and draws these elements in their place.

use std::collections::HashMap;
use std::fmt::Write;

use usvg::{Node, Transform};

use super::coregraphics::{self, Segment};
use super::faces::{FaceKind, FaceTable, OwnPiece};
use crate::mermaid::scene::svg::OWN_AXIS;
use crate::text::FontSystem;

/// Skia rounds a subpixel glyph position to a quarter pixel on the
/// horizontal axis (`SkPackedGlyphID`, four subpixel steps, rounding by
/// half a step) and to a whole pixel on the vertical axis for horizontal
/// text.
const SUBPIXEL_STEPS: f32 = 4.0;

/// The elements to draw after each text with an own piece, keyed by the
/// text's id. `scale` is the transform from CSS pixels to output pixels
/// that the tree is rendered with.
pub fn own_glyphs(
    tree: &usvg::Tree,
    scale: f32,
    table: &FaceTable<'_>,
    fonts: &FontSystem,
) -> HashMap<String, String> {
    let axis = {
        let mut tag = [0u8; 4];
        tag.copy_from_slice(&OWN_AXIS.as_bytes()[..4]);
        tag
    };
    let mut texts = Vec::new();
    collect_texts(tree.root(), &mut texts);
    let mut out = HashMap::new();
    for text in texts {
        let device = Transform::from_scale(scale, scale).pre_concat(text.abs_transform());
        let mut elements = String::new();
        for span in text.layouted() {
            let Some(index) = span.variations.iter().find(|v| v.tag == axis) else {
                continue;
            };
            let (Some(piece), Some(placeholder)) = (
                table.own_piece(index.value as usize),
                span.positioned_glyphs.first(),
            ) else {
                continue;
            };
            // The placeholder's transform without its font scaling: CSS
            // pixels from the piece's origin on the baseline, y down.
            let upem = fonts
                .with_face(placeholder.font, |f| f.units_per_em() as f32)
                .unwrap_or(1000.0);
            let unscale = upem / placeholder.font_size();
            let origin = placeholder
                .transform()
                .pre_concat(Transform::from_scale(unscale, unscale));
            draw_piece(&mut elements, &piece, table, fonts, span, origin, device);
        }
        if !elements.is_empty() {
            out.insert(text.id().to_string(), elements);
        }
    }
    out
}

fn collect_texts<'t>(group: &'t usvg::Group, out: &mut Vec<&'t usvg::Text>) {
    for node in group.children() {
        match node {
            Node::Group(g) => collect_texts(g, out),
            Node::Text(t) if t.id().starts_with("kmt-") => out.push(t),
            _ => {}
        }
    }
}

/// One glyph of a piece: its id and its pen position in CSS pixels, y up.
struct PlacedGlyph {
    id: u16,
    x: f32,
    y: f32,
}

/// The glyphs of the piece shaped in its face. The piece is one
/// grapheme, and its advance is the placeholder's business, so only the
/// offsets inside the grapheme matter here.
fn shape(piece: &OwnPiece, fonts: &FontSystem) -> (Vec<PlacedGlyph>, f32) {
    fonts
        .with_face(piece.face, |f| {
            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(&piece.text);
            let glyphs = rustybuzz::shape(f, &[], buffer);
            let scale = piece.size / f.units_per_em() as f32;
            let mut pen = 0.0;
            let placed = glyphs
                .glyph_infos()
                .iter()
                .zip(glyphs.glyph_positions())
                .filter_map(|(info, pos)| {
                    let glyph = PlacedGlyph {
                        id: u16::try_from(info.glyph_id).ok()?,
                        x: pen + pos.x_offset as f32 * scale,
                        y: pos.y_offset as f32 * scale,
                    };
                    pen += pos.x_advance as f32 * scale;
                    Some(glyph)
                })
                .collect();
            (placed, f.units_per_em() as f32)
        })
        .unwrap_or((Vec::new(), 1000.0))
}

fn draw_piece(
    out: &mut String,
    piece: &OwnPiece,
    table: &FaceTable<'_>,
    fonts: &FontSystem,
    span: &usvg::layout::Span,
    origin: Transform,
    device: Transform,
) {
    let Some(face) = fonts.db().face(piece.face) else {
        return;
    };
    let name = face.post_script_name.clone();
    let kind = table.kind(piece.face);
    let (glyphs, upem) = shape(piece, fonts);
    for glyph in glyphs {
        let at = origin.pre_translate(glyph.x, -glyph.y);
        match kind {
            FaceKind::Sbix => emoji_image(
                out,
                &name,
                glyph.id,
                piece.size,
                span,
                device.pre_concat(at),
                device,
            ),
            FaceKind::UnreadableOutline => {
                let k = piece.size / upem;
                outline_path(out, &name, upem, glyph.id, at.pre_scale(k, -k), span);
            }
            FaceKind::Outline => {}
        }
    }
}

/// Full `f32` precision: these numbers never reach a snapshot, and the
/// inverse device transform must cancel the forward one exactly.
fn num(v: f32) -> String {
    format!("{v}")
}

fn matrix(t: Transform) -> String {
    format!(
        "matrix({} {} {} {} {} {})",
        num(t.sx),
        num(t.ky),
        num(t.kx),
        num(t.sy),
        num(t.tx),
        num(t.ty)
    )
}

/// `fill`, `fill-opacity`, and a stroke when the span has one (the
/// synthetic bold stroke), for a path in the text's user space.
fn paint_attrs(out: &mut String, span: &usvg::layout::Span) {
    let color = |paint: &usvg::Paint| match paint {
        usvg::Paint::Color(c) => Some(format!("#{:02x}{:02x}{:02x}", c.red, c.green, c.blue)),
        _ => None,
    };
    match span
        .fill
        .as_ref()
        .and_then(|f| color(f.paint()).map(|c| (c, f)))
    {
        Some((c, fill)) => {
            let _ = write!(
                out,
                " fill=\"{c}\" fill-opacity=\"{}\"",
                num(fill.opacity().get())
            );
        }
        None => out.push_str(" fill=\"none\""),
    }
    if let Some((c, stroke)) = span
        .stroke
        .as_ref()
        .and_then(|s| color(s.paint()).map(|c| (c, s)))
    {
        let _ = write!(
            out,
            " stroke=\"{c}\" stroke-opacity=\"{}\" stroke-width=\"{}\"",
            num(stroke.opacity().get()),
            num(stroke.width().get())
        );
    }
}

/// A CoreText outline, read in font units and mapped into the text's user
/// space by `t`.
fn outline_path(
    out: &mut String,
    name: &str,
    upem: f32,
    glyph: u16,
    t: Transform,
    span: &usvg::layout::Span,
) {
    let Some(segments) = coregraphics::glyph_outline(name, f64::from(upem), glyph) else {
        return;
    };
    let p = |x: f64, y: f64| {
        let mut pt = usvg::tiny_skia_path::Point::from_xy(x as f32, y as f32);
        t.map_point(&mut pt);
        format!("{} {}", num(pt.x), num(pt.y))
    };
    let mut d = String::new();
    for s in segments {
        let _ = match s {
            Segment::Move(x, y) => write!(d, "M{}", p(x, y)),
            Segment::Line(x, y) => write!(d, "L{}", p(x, y)),
            Segment::Quad(x1, y1, x, y) => write!(d, "Q{} {}", p(x1, y1), p(x, y)),
            Segment::Cubic(x1, y1, x2, y2, x, y) => {
                write!(d, "C{} {} {}", p(x1, y1), p(x2, y2), p(x, y))
            }
            Segment::Close => write!(d, "Z"),
        };
    }
    let _ = write!(out, "<path d=\"{d}\"");
    paint_attrs(out, span);
    out.push_str("/>\n");
}

/// A color glyph drawn by CoreText at the device size and placed on whole
/// device pixels, as Skia draws a glyph image. `at` maps the glyph's pen
/// position to device pixels; the image's transform undoes `device`, so
/// resvg copies its pixels one to one.
fn emoji_image(
    out: &mut String,
    name: &str,
    glyph: u16,
    size: f32,
    span: &usvg::layout::Span,
    at: Transform,
    device: Transform,
) {
    let Some(inverse) = device.invert() else {
        return;
    };
    let x = ((at.tx + 0.5 / SUBPIXEL_STEPS) * SUBPIXEL_STEPS).floor() / SUBPIXEL_STEPS;
    let y = (at.ty + 0.5).floor();
    let whole_x = x.floor();
    let ppem = size * (at.sx * at.sy - at.kx * at.ky).abs().sqrt();
    let Some(bitmap) =
        coregraphics::draw_glyph(name, f64::from(ppem), glyph, f64::from(x - whole_x))
    else {
        return;
    };
    let Some(image) = image::RgbaImage::from_raw(bitmap.width, bitmap.height, bitmap.rgba) else {
        return;
    };
    let mut png = Vec::new();
    if image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .is_err()
    {
        return;
    }
    let opacity = span.fill.as_ref().map_or(1.0, |f| f.opacity().get());
    let _ = write!(
        out,
        "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" transform=\"{}\" image-rendering=\"optimizeSpeed\"",
        num(whole_x + bitmap.left as f32),
        num(y + bitmap.top as f32),
        bitmap.width,
        bitmap.height,
        matrix(inverse)
    );
    if opacity < 1.0 {
        let _ = write!(out, " opacity=\"{}\"", num(opacity));
    }
    let _ = writeln!(out, " href=\"data:image/png;base64,{}\"/>", base64(&png));
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_pads_like_rfc_4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
