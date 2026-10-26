//! CoreText glyph drawing for the faces resvg cannot draw as Chrome does.
//!
//! Chrome on macOS draws every glyph through Skia's CoreText scaler
//! (`SkScalerContext_Mac`): outlines come from `CTFontCreatePathForGlyph`,
//! and color glyphs are drawn with `CTFontDrawGlyphs` into a bitmap
//! context at the device size. This module does the same for one glyph.

use std::ffi::c_void;

type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CTFontRef = *const c_void;
type CGPathRef = *const c_void;
type CGContextRef = *mut c_void;
type CGColorSpaceRef = *const c_void;
type CFIndex = isize;

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
/// `kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big`: RGBA bytes.
const RGBA_PREMULTIPLIED: u32 = 1 | (4 << 12);

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
struct CGRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Declared as `text::coretext` declares it, so the two declarations of
/// `CTFontCreatePathForGlyph` agree.
#[repr(C)]
struct CGAffineTransform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx: f64,
    ty: f64,
}

#[repr(C)]
struct CGPathElement {
    kind: i32,
    points: *const CGPoint,
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithBytes(
        alloc: *const c_void,
        bytes: *const u8,
        num_bytes: CFIndex,
        encoding: u32,
        is_external_representation: u8,
    ) -> CFStringRef;
    fn CFRelease(cf: CFTypeRef);
}

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CTFontCreateWithName(name: CFStringRef, size: f64, matrix: *const c_void) -> CTFontRef;
    fn CTFontCreatePathForGlyph(
        font: CTFontRef,
        glyph: u16,
        matrix: *const CGAffineTransform,
    ) -> CGPathRef;
    fn CTFontGetBoundingRectsForGlyphs(
        font: CTFontRef,
        orientation: u32,
        glyphs: *const u16,
        rects: *mut CGRect,
        count: CFIndex,
    ) -> CGRect;
    fn CTFontDrawGlyphs(
        font: CTFontRef,
        glyphs: *const u16,
        positions: *const CGPoint,
        count: usize,
        context: CGContextRef,
    );
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPathApply(
        path: CGPathRef,
        info: *mut c_void,
        function: unsafe extern "C" fn(*mut c_void, *const CGPathElement),
    );
    fn CGPathRelease(path: CGPathRef);
    fn CGColorSpaceCreateDeviceRGB() -> CGColorSpaceRef;
    fn CGColorSpaceRelease(space: CGColorSpaceRef);
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits_per_component: usize,
        bytes_per_row: usize,
        space: CGColorSpaceRef,
        bitmap_info: u32,
    ) -> CGContextRef;
    fn CGContextRelease(context: CGContextRef);
}

/// An owned Core Foundation object, released on drop.
struct Owned(CFTypeRef);

impl Owned {
    fn new(ptr: CFTypeRef) -> Option<Owned> {
        (!ptr.is_null()).then_some(Owned(ptr))
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: `Owned` only holds a non-null object returned by a
        // Create function, which the caller owns once.
        unsafe { CFRelease(self.0) }
    }
}

fn ct_font(postscript_name: &str, size: f64) -> Option<Owned> {
    let len = CFIndex::try_from(postscript_name.len()).ok()?;
    // SAFETY: the pointer and length describe the name, which outlives the
    // call; CoreFoundation copies the bytes.
    let name = Owned::new(unsafe {
        CFStringCreateWithBytes(
            std::ptr::null(),
            postscript_name.as_ptr(),
            len,
            K_CF_STRING_ENCODING_UTF8,
            0,
        )
    })?;
    // SAFETY: `name` is a live CFString; a null matrix means identity.
    Owned::new(unsafe { CTFontCreateWithName(name.0, size, std::ptr::null()) })
}

/// One outline segment in font units, y up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    Move(f64, f64),
    Line(f64, f64),
    Quad(f64, f64, f64, f64),
    Cubic(f64, f64, f64, f64, f64, f64),
    Close,
}

unsafe extern "C" fn collect_segment(info: *mut c_void, element: *const CGPathElement) {
    // SAFETY: `info` is the `Vec<Segment>` that `glyph_outline` passes, and
    // CoreGraphics hands a valid element whose point count follows its
    // type.
    unsafe {
        let out = &mut *info.cast::<Vec<Segment>>();
        let e = &*element;
        let p = |i: usize| *e.points.add(i);
        out.push(match e.kind {
            0 => Segment::Move(p(0).x, p(0).y),
            1 => Segment::Line(p(0).x, p(0).y),
            2 => Segment::Quad(p(0).x, p(0).y, p(1).x, p(1).y),
            3 => Segment::Cubic(p(0).x, p(0).y, p(1).x, p(1).y, p(2).x, p(2).y),
            _ => Segment::Close,
        });
    }
}

/// The outline of `glyph` at `units_per_em`, so the points come out in
/// font units. `None` when the glyph has no outline.
pub fn glyph_outline(postscript_name: &str, units_per_em: f64, glyph: u16) -> Option<Vec<Segment>> {
    let font = ct_font(postscript_name, units_per_em)?;
    let identity = CGAffineTransform {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx: 0.0,
        ty: 0.0,
    };
    // SAFETY: `font` is live and `identity` outlives the call.
    let path = unsafe { CTFontCreatePathForGlyph(font.0, glyph, &identity) };
    if path.is_null() {
        return None;
    }
    let mut segments: Vec<Segment> = Vec::new();
    // SAFETY: `path` is a live CGPath this code owns once, and the callback
    // only touches `segments` while `CGPathApply` runs.
    unsafe {
        CGPathApply(
            path,
            (&mut segments as *mut Vec<Segment>).cast(),
            collect_segment,
        );
        CGPathRelease(path);
    }
    (!segments.is_empty()).then_some(segments)
}

/// A glyph drawn into device pixels: straight-alpha RGBA rows, top row
/// first, and the place of the top left pixel relative to the integer
/// part of the glyph origin, y down.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphBitmap {
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
    pub rgba: Vec<u8>,
}

/// Draws `glyph` of a color face at `size` device pixels per em with its
/// origin `fraction` pixels right of a pixel corner, as Skia's CoreText
/// scaler draws a color glyph image: `CTFontDrawGlyphs` into a
/// premultiplied RGBA context that covers the glyph's bounding rect.
pub fn draw_glyph(
    postscript_name: &str,
    size: f64,
    glyph: u16,
    fraction: f64,
) -> Option<GlyphBitmap> {
    let font = ct_font(postscript_name, size)?;
    let mut rect = CGRect::default();
    // SAFETY: one glyph and one rect, as `count` says.
    unsafe {
        CTFontGetBoundingRectsForGlyphs(font.0, 0, &glyph, &mut rect, 1);
    }
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return None;
    }
    // The rect rounded out around the shifted origin, with a pixel of
    // margin: CoreText draws sbix images a little past their bounds.
    let left = (fraction + rect.x).floor() as i32 - 1;
    let right = (fraction + rect.x + rect.width).ceil() as i32 + 1;
    let bottom = rect.y.floor() as i32 - 1;
    let top = (rect.y + rect.height).ceil() as i32 + 1;
    let width = usize::try_from(right - left).ok()?;
    let height = usize::try_from(top - bottom).ok()?;
    let mut data = vec![0u8; width * height * 4];
    // SAFETY: the buffer holds `height` rows of `width * 4` bytes, the
    // layout the context is created with, and outlives the context.
    unsafe {
        let space = CGColorSpaceCreateDeviceRGB();
        if space.is_null() {
            return None;
        }
        let context = CGBitmapContextCreate(
            data.as_mut_ptr().cast(),
            width,
            height,
            8,
            width * 4,
            space,
            RGBA_PREMULTIPLIED,
        );
        CGColorSpaceRelease(space);
        if context.is_null() {
            return None;
        }
        // CoreGraphics puts y = 0 at the bottom row.
        let origin = CGPoint {
            x: fraction - f64::from(left),
            y: -f64::from(bottom),
        };
        CTFontDrawGlyphs(font.0, &glyph, &origin, 1, context);
        CGContextRelease(context);
    }
    for px in data.chunks_exact_mut(4) {
        let a = px[3];
        if a != 0 && a != 255 {
            for c in &mut px[..3] {
                *c = ((u16::from(*c) * 255 + u16::from(a) / 2) / u16::from(a)).min(255) as u8;
            }
        }
    }
    Some(GlyphBitmap {
        width: width as u32,
        height: height as u32,
        left,
        top: -top,
        rgba: data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helvetica_h_has_a_closed_outline_in_font_units() {
        // Glyph 43 is `H` in the stock Helvetica.
        let segments = glyph_outline("Helvetica", 1000.0, 43).expect("outline");
        assert!(matches!(segments[0], Segment::Move(..)));
        assert!(segments.contains(&Segment::Close));
        let top = segments
            .iter()
            .filter_map(|s| match *s {
                Segment::Move(_, y) | Segment::Line(_, y) => Some(y),
                _ => None,
            })
            .fold(f64::MIN, f64::max);
        assert!((600.0..800.0).contains(&top), "{top}");
    }

    #[test]
    fn pingfang_outlines_come_from_coretext() {
        // usvg reads no outline from PingFang; CoreText does. Glyph ids
        // differ between releases, so ask for a range and expect ink.
        let found = (100..200).any(|g| glyph_outline("PingFangSC-Regular", 1000.0, g).is_some());
        assert!(found);
    }

    #[test]
    fn emoji_bitmaps_have_color_and_sit_above_the_baseline() {
        // Apple Color Emoji glyph ids differ between releases; any glyph
        // with a bitmap will do.
        let bitmap = (1000..1100)
            .find_map(|g| draw_glyph("AppleColorEmoji", 16.0, g, 0.25))
            .expect("an emoji glyph");
        assert!(bitmap.top < -10 && bitmap.height >= 16, "{bitmap:?}");
        let colored = bitmap
            .rgba
            .chunks_exact(4)
            .any(|p| p[3] == 255 && (p[0] != p[1] || p[1] != p[2]));
        assert!(colored);
    }
}
