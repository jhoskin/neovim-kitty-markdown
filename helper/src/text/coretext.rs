//! CoreText font substitution, the platform fallback Chrome uses on macOS.
//!
//! When no family of the CSS `font-family` list covers a character, Blink's
//! `FontCache::PlatformFallbackFontForCharacter` (font_cache_mac.mm) asks
//! CoreText for a substitute with the primary font as the base, through
//! `CTFontCreateForString`. CoreText answers from its cascade list, which
//! is compiled into the framework and has no file to read: `中` goes to
//! PingFang SC, `न` to Kohinoor Devanagari, `₹` to Helvetica, and Arabic
//! goes to Geeza Pro when the base face (Arial Italic) lacks it. Blink then
//! keeps only the family name and matches the requested weight and style
//! in that family, so the helper does the same with the name returned here.

use std::ffi::{c_char, c_void};

type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CTFontRef = *const c_void;
type CFIndex = isize;

#[repr(C)]
struct CFRange {
    location: CFIndex,
    length: CFIndex,
}

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithBytes(
        alloc: *const c_void,
        bytes: *const u8,
        num_bytes: CFIndex,
        encoding: u32,
        is_external_representation: u8,
    ) -> CFStringRef;
    fn CFStringGetLength(string: CFStringRef) -> CFIndex;
    fn CFStringGetCString(
        string: CFStringRef,
        buffer: *mut c_char,
        buffer_size: CFIndex,
        encoding: u32,
    ) -> u8;
    fn CFRelease(cf: CFTypeRef);
}

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CTFontCreateWithName(name: CFStringRef, size: f64, matrix: *const c_void) -> CTFontRef;
    fn CTFontCreateForString(current: CTFontRef, string: CFStringRef, range: CFRange) -> CTFontRef;
    fn CTFontCopyFamilyName(font: CTFontRef) -> CFStringRef;
    fn CTFontCreatePathForGlyph(
        font: CTFontRef,
        glyph: u16,
        matrix: *const CGAffineTransform,
    ) -> CGPathRef;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPathGetPathBoundingBox(path: CGPathRef) -> CGRect;
    fn CGPathRelease(path: CGPathRef);
}

type CGPathRef = *const c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[repr(C)]
struct CGAffineTransform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx: f64,
    ty: f64,
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
        // Create or Copy function, which the caller owns once.
        unsafe { CFRelease(self.0) }
    }
}

fn cf_string(text: &str) -> Option<Owned> {
    let len = CFIndex::try_from(text.len()).ok()?;
    // SAFETY: the pointer and length describe `text`, which outlives the
    // call; CoreFoundation copies the bytes.
    Owned::new(unsafe {
        CFStringCreateWithBytes(
            std::ptr::null(),
            text.as_ptr(),
            len,
            K_CF_STRING_ENCODING_UTF8,
            0,
        )
    })
}

fn rust_string(string: &Owned) -> Option<String> {
    // A family name is short; 4 bytes per UTF-16 unit is the UTF-8 bound.
    // SAFETY: `string` is a live CFString.
    let units = unsafe { CFStringGetLength(string.0) };
    let size = units.checked_mul(4)?.checked_add(1)?;
    let mut buffer = vec![0u8; usize::try_from(size).ok()?];
    // SAFETY: the buffer holds `size` bytes, as passed.
    let ok = unsafe {
        CFStringGetCString(
            string.0,
            buffer.as_mut_ptr().cast(),
            size,
            K_CF_STRING_ENCODING_UTF8,
        )
    };
    if ok == 0 {
        return None;
    }
    let end = buffer.iter().position(|&b| b == 0)?;
    buffer.truncate(end);
    String::from_utf8(buffer).ok()
}

/// The family CoreText substitutes for `text` on the base font named by
/// its PostScript name. CoreText resolves an unknown base name to its
/// default font, whose cascade list is the default one, which is also what
/// Blink gets for a web font such as the page's Open Sans.
pub fn substitute_family(base_postscript_name: &str, text: &str) -> Option<String> {
    let name = cf_string(base_postscript_name)?;
    let string = cf_string(text)?;
    // SAFETY: `name` is a live CFString; a null matrix means identity.
    let base = Owned::new(unsafe { CTFontCreateWithName(name.0, 16.0, std::ptr::null()) })?;
    // SAFETY: `string` is a live CFString.
    let length = unsafe { CFStringGetLength(string.0) };
    // SAFETY: `base` and `string` are live and the range covers `string`.
    let font = Owned::new(unsafe {
        CTFontCreateForString(
            base.0,
            string.0,
            CFRange {
                location: 0,
                length,
            },
        )
    })?;
    // SAFETY: `font` is a live CTFont.
    let family = Owned::new(unsafe { CTFontCopyFamilyName(font.0) })?;
    rust_string(&family)
}

/// Glyph ink boxes in font units, as `[x_min, y_min, x_max, y_max]`, for
/// a face whose outlines ttf-parser cannot read. The PingFang faces of
/// macOS 26 keep their outlines in a table only CoreText reads, while
/// Chrome's `getBBox` needs their ink. CoreText sizes the font at
/// `units_per_em`, so the paths come out in font units. A `skew` other
/// than 0 draws each path with x' = x + skew * y, Chrome's synthetic
/// oblique, and the box is the tight box of the skewed path
/// (`CGPathGetPathBoundingBox` leaves out control points). A glyph with no
/// ink (a space) is `None`.
pub fn glyph_bounds(
    postscript_name: &str,
    units_per_em: f64,
    glyphs: &[u16],
    skew: f64,
) -> Option<Vec<Option<[f64; 4]>>> {
    let name = cf_string(postscript_name)?;
    // SAFETY: `name` is a live CFString; a null matrix means identity.
    let font = Owned::new(unsafe { CTFontCreateWithName(name.0, units_per_em, std::ptr::null()) })?;
    let matrix = CGAffineTransform {
        a: 1.0,
        b: 0.0,
        c: skew,
        d: 1.0,
        tx: 0.0,
        ty: 0.0,
    };
    let boxes = glyphs
        .iter()
        .map(|&glyph| {
            // SAFETY: `font` is live and `matrix` outlives the call.
            let path = unsafe { CTFontCreatePathForGlyph(font.0, glyph, &matrix) };
            if path.is_null() {
                return None;
            }
            // SAFETY: `path` is a live CGPath that this code owns once.
            let r = unsafe {
                let r = CGPathGetPathBoundingBox(path);
                CGPathRelease(path);
                r
            };
            (r.width > 0.0 || r.height > 0.0).then_some([r.x, r.y, r.x + r.width, r.y + r.height])
        })
        .collect();
    Some(boxes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skew_moves_the_top_right_of_the_box() {
        let upright =
            glyph_bounds("Helvetica", 1000.0, &[43], 0.0).expect("Helvetica")[0].expect("ink");
        let skewed =
            glyph_bounds("Helvetica", 1000.0, &[43], 0.25).expect("Helvetica")[0].expect("ink");
        // `H` has a stem at the top right corner of its box.
        assert!((skewed[2] - (upright[2] + 0.25 * upright[3])).abs() < 1.0);
        assert_eq!(skewed[1], upright[1]);
    }

    #[test]
    fn glyph_bounds_come_in_font_units() {
        // Helvetica `H` (glyph 43 in the stock face) stands on the
        // baseline and is about 0.72 em tall at 1000 units per em; glyph
        // 3 is the space, which has no ink.
        let bounds = glyph_bounds("Helvetica", 1000.0, &[43, 3], 0.0).expect("Helvetica");
        let h = bounds[0].expect("ink");
        assert!(h[1].abs() < 1.0 && (600.0..800.0).contains(&h[3]), "{h:?}");
        assert!(bounds[1].is_none());
    }

    #[test]
    fn a_covered_character_keeps_the_base_family() {
        let Some(family) = substitute_family("Helvetica", "A") else {
            return;
        };
        assert_eq!(family, "Helvetica");
    }

    #[test]
    fn scripts_go_to_their_cascade_faces() {
        // The faces a stock macOS install substitutes on a Latin base, as
        // Chrome 156 reports them through `CSS.getPlatformFontsForNode`.
        for (text, family) in [
            ("\u{0928}", "Kohinoor Devanagari"),
            ("\u{0E2A}", "Thonburi"),
            ("\u{4E2D}", "PingFang SC"),
            ("\u{20B9}", "Helvetica"),
            ("\u{1F600}", "Apple Color Emoji"),
        ] {
            assert_eq!(
                substitute_family("TrebuchetMS", text).as_deref(),
                Some(family),
                "{text:?}"
            );
        }
    }
}
