//! CoreText font substitution and face traits, which Chrome's font matching
//! on macOS reads.
//!
//! When no family of the CSS `font-family` list covers a character, Blink's
//! `FontCache::PlatformFallbackFontForCharacter` (font_cache_mac.mm) asks
//! CoreText for a substitute with the primary font as the base, then for
//! the face of that family with the requested traits and weight
//! ([`substitute_face`]). Inside a CSS family Blink matches faces on their
//! CoreText traits too ([`face_traits`]), not on the OS/2 table.

use std::ffi::{c_char, c_void};

type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CTFontRef = *const c_void;
type CFIndex = isize;
type CFDictionaryRef = *const c_void;
type CFNumberRef = *const c_void;

#[repr(C)]
struct CFRange {
    location: CFIndex,
    length: CFIndex,
}

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_CF_NUMBER_FLOAT64_TYPE: CFIndex = 6;
const K_CF_NUMBER_SINT64_TYPE: CFIndex = 4;

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
    fn CFDictionaryGetValue(dict: CFDictionaryRef, key: *const c_void) -> *const c_void;
    fn CFDictionaryCreate(
        alloc: *const c_void,
        keys: *const CFTypeRef,
        values: *const CFTypeRef,
        count: CFIndex,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> CFDictionaryRef;
    fn CFNumberGetValue(number: CFNumberRef, kind: CFIndex, value: *mut c_void) -> u8;
    fn CFNumberCreate(alloc: *const c_void, kind: CFIndex, value: *const c_void) -> CFNumberRef;
    fn CFCharacterSetIsLongCharacterMember(set: *const c_void, ch: u32) -> u8;
    // Only their addresses are passed; the layout stays opaque.
    static kCFTypeDictionaryKeyCallBacks: u8;
    static kCFTypeDictionaryValueCallBacks: u8;
}

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CTFontCreateWithName(name: CFStringRef, size: f64, matrix: *const c_void) -> CTFontRef;
    fn CTFontCreateForString(current: CTFontRef, string: CFStringRef, range: CFRange) -> CTFontRef;
    fn CTFontCopyFamilyName(font: CTFontRef) -> CFStringRef;
    fn CTFontCopyTraits(font: CTFontRef) -> CFDictionaryRef;
    fn CTFontCopyPostScriptName(font: CTFontRef) -> CFStringRef;
    fn CTFontCopyCharacterSet(font: CTFontRef) -> *const c_void;
    fn CTFontDescriptorCreateWithAttributes(attributes: CFDictionaryRef) -> *const c_void;
    fn CTFontCreateWithFontDescriptor(
        descriptor: *const c_void,
        size: f64,
        matrix: *const c_void,
    ) -> CTFontRef;
    static kCTFontWeightTrait: CFStringRef;
    static kCTFontSymbolicTrait: CFStringRef;
    static kCTFontFamilyNameAttribute: CFStringRef;
    static kCTFontTraitsAttribute: CFStringRef;
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

/// The traits CoreText gives a face, which Blink's font matching on macOS
/// (font_matcher_mac.mm, font_cache_mac.mm) reads instead of the OS/2
/// table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceTraits {
    /// `kCTFontWeightTrait`, from -1 to 1 with 0 for a regular face.
    pub weight: f64,
    /// `kCTFontSymbolicTrait` bits (`kCTFontTraitItalic` and so on).
    pub symbolic: u32,
}

pub const TRAIT_ITALIC: u32 = 1 << 0;
pub const TRAIT_BOLD: u32 = 1 << 1;
pub const TRAIT_EXPANDED: u32 = 1 << 5;
pub const TRAIT_CONDENSED: u32 = 1 << 6;
/// Blink's `TraitsMask`: the bits a fallback face is compared on.
const TRAITS_MASK: u32 = TRAIT_ITALIC | TRAIT_BOLD | TRAIT_CONDENSED | TRAIT_EXPANDED;

/// The face CoreText substitutes for a character, by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Substitute {
    pub family: String,
    pub postscript: String,
}

fn traits_of(font: CTFontRef) -> FaceTraits {
    let mut weight = 0f64;
    let mut symbolic = 0i64;
    // SAFETY: `font` is a live CTFont and the dictionary is owned here.
    if let Some(traits) = Owned::new(unsafe { CTFontCopyTraits(font) }) {
        // SAFETY: the keys are CoreText constants, the values (when
        // present) are CFNumbers owned by the dictionary, and each out
        // pointer matches the number type asked for.
        unsafe {
            let w = CFDictionaryGetValue(traits.0, kCTFontWeightTrait);
            if !w.is_null() {
                CFNumberGetValue(w, K_CF_NUMBER_FLOAT64_TYPE, (&raw mut weight).cast());
            }
            let s = CFDictionaryGetValue(traits.0, kCTFontSymbolicTrait);
            if !s.is_null() {
                CFNumberGetValue(s, K_CF_NUMBER_SINT64_TYPE, (&raw mut symbolic).cast());
            }
        }
    }
    FaceTraits {
        weight,
        // The symbolic traits are a 32-bit mask stored in a CFNumber.
        symbolic: symbolic as u32,
    }
}

fn postscript_of(font: CTFontRef) -> Option<String> {
    // SAFETY: `font` is a live CTFont.
    rust_string(&Owned::new(unsafe { CTFontCopyPostScriptName(font) })?)
}

fn family_of(font: CTFontRef) -> Option<String> {
    // SAFETY: `font` is a live CTFont.
    rust_string(&Owned::new(unsafe { CTFontCopyFamilyName(font) })?)
}

fn font_named(postscript_name: &str) -> Option<Owned> {
    let name = cf_string(postscript_name)?;
    // SAFETY: `name` is a live CFString; a null matrix means identity.
    Owned::new(unsafe { CTFontCreateWithName(name.0, 16.0, std::ptr::null()) })
}

/// The CoreText traits of the face named by its PostScript name. `None`
/// when CoreText does not know the name: it would answer with its default
/// font, whose traits say nothing about this face.
pub fn face_traits(postscript_name: &str) -> Option<FaceTraits> {
    let font = font_named(postscript_name)?;
    if postscript_of(font.0)? != postscript_name {
        return None;
    }
    Some(traits_of(font.0))
}

/// Blink's `ToCTFontWeight` (font_matcher_mac.mm): the CoreText weight a
/// CSS weight asks for.
pub fn ct_weight(css_weight: u16) -> f64 {
    const WEIGHTS: [f64; 9] = [-0.80, -0.60, -0.40, 0.0, 0.23, 0.30, 0.40, 0.56, 0.62];
    if css_weight <= 50 || css_weight >= 950 {
        return 0.0;
    }
    WEIGHTS[usize::from((css_weight - 50) / 100)]
}

fn cf_number<T>(kind: CFIndex, value: &T) -> Option<Owned> {
    // SAFETY: `value` is a live number of the type `kind` names, which
    // CoreFoundation copies.
    Owned::new(unsafe { CFNumberCreate(std::ptr::null(), kind, (value as *const T).cast()) })
}

fn cf_dictionary(pairs: &[(CFTypeRef, CFTypeRef)]) -> Option<Owned> {
    let keys: Vec<CFTypeRef> = pairs.iter().map(|p| p.0).collect();
    let values: Vec<CFTypeRef> = pairs.iter().map(|p| p.1).collect();
    let count = CFIndex::try_from(pairs.len()).ok()?;
    // SAFETY: both arrays hold `count` live CF objects, which the
    // dictionary retains through the standard CFType callbacks.
    Owned::new(unsafe {
        CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            count,
            (&raw const kCFTypeDictionaryKeyCallBacks).cast(),
            (&raw const kCFTypeDictionaryValueCallBacks).cast(),
        )
    })
}

/// Blink's `CreateCopyWithTraitsAndWeightFromFont`: the face of `font`'s
/// family that CoreText matches to the symbolic traits and weight.
fn variation(font: CTFontRef, symbolic: u32, weight: f64) -> Option<Owned> {
    // SAFETY: `font` is a live CTFont.
    let family = Owned::new(unsafe { CTFontCopyFamilyName(font) })?;
    let symbolic = cf_number(K_CF_NUMBER_SINT64_TYPE, &i64::from(symbolic))?;
    let weight = cf_number(K_CF_NUMBER_FLOAT64_TYPE, &weight)?;
    // SAFETY: the keys are CoreText constants that live for the process.
    let (symbolic_key, weight_key, family_key, traits_key) = unsafe {
        (
            kCTFontSymbolicTrait,
            kCTFontWeightTrait,
            kCTFontFamilyNameAttribute,
            kCTFontTraitsAttribute,
        )
    };
    let traits = cf_dictionary(&[(symbolic_key, symbolic.0), (weight_key, weight.0)])?;
    let attributes = cf_dictionary(&[(family_key, family.0), (traits_key, traits.0)])?;
    // SAFETY: `attributes` is a live dictionary.
    let descriptor = Owned::new(unsafe { CTFontDescriptorCreateWithAttributes(attributes.0) })?;
    // SAFETY: `descriptor` is live; a null matrix means identity.
    Owned::new(unsafe { CTFontCreateWithFontDescriptor(descriptor.0, 16.0, std::ptr::null()) })
}

fn covers(font: CTFontRef, ch: char) -> bool {
    // SAFETY: `font` is a live CTFont; the set is owned here.
    let Some(set) = Owned::new(unsafe { CTFontCopyCharacterSet(font) }) else {
        return false;
    };
    // SAFETY: `set` is a live CFCharacterSet.
    unsafe { CFCharacterSetIsLongCharacterMember(set.0, u32::from(ch)) != 0 }
}

/// The face Chrome falls back to for `text` (whose first character is
/// `ch`) under the base font named by its PostScript name, found the way
/// Blink's `FontCache::PlatformFallbackFontForCharacter`
/// (font_cache_mac.mm) finds it.
///
/// `CTFontCreateForString` answers from the cascade list, which is
/// compiled into CoreText and has no file to read: `中` goes to PingFang
/// SC, `न` to Kohinoor Devanagari, `₹` to Helvetica, and Arabic to Geeza
/// Pro when the base face (Arial Italic) lacks it. When that face's traits
/// or weight differ from the ones asked for (`symbolic`: the base font's
/// traits plus Chrome's synthetic bold and italic; the CSS weight through
/// `ToCTFontWeight`), Blink asks CoreText for the face of the family with
/// those traits and weight, and takes it only when it differs and covers
/// `ch`. So 400 kana go to Hiragino Sans W3 and 500 kana to W4, a 500 `₹`
/// goes to Helvetica Bold, and a 300 `₹` keeps Helvetica because
/// Helvetica Light lacks it. CoreText resolves an unknown base name to its
/// default font, which is also what Blink gets for a web font such as the
/// page's Open Sans.
pub fn substitute_face(
    base_postscript_name: &str,
    text: &str,
    ch: char,
    symbolic: u32,
    css_weight: u16,
) -> Option<Substitute> {
    let base = font_named(base_postscript_name)?;
    let string = cf_string(text)?;
    // SAFETY: `string` is a live CFString.
    let length = unsafe { CFStringGetLength(string.0) };
    // SAFETY: `base` and `string` are live and the range covers `string`.
    let mut font = Owned::new(unsafe {
        CTFontCreateForString(
            base.0,
            string.0,
            CFRange {
                location: 0,
                length,
            },
        )
    })?;
    let found = traits_of(font.0);
    let weight = ct_weight(css_weight);
    let differs = |t: FaceTraits| {
        (t.symbolic ^ found.symbolic) & TRAITS_MASK != 0 || t.weight != found.weight
    };
    let wanted = FaceTraits { weight, symbolic };
    if differs(wanted)
        && let Some(better) = variation(font.0, symbolic, weight)
        && differs(traits_of(better.0))
        && covers(better.0, ch)
    {
        font = better;
    }
    Some(Substitute {
        family: family_of(font.0)?,
        postscript: postscript_of(font.0)?,
    })
}

/// The symbolic traits of the face named by its PostScript name, or of the
/// default font CoreText puts in its place for a name it does not know.
pub fn base_symbolic_traits(postscript_name: &str) -> Option<u32> {
    Some(traits_of(font_named(postscript_name)?.0).symbolic)
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
    fn traits_come_from_coretext_not_os2() {
        // Hiragino Sans W3 has usWeightClass 300, but CoreText calls it
        // regular (0.0); W5 is a CoreText semibold with the bold bit.
        let w3 = face_traits("HiraginoSans-W3").expect("Hiragino Sans W3");
        assert_eq!(w3.weight, 0.0);
        assert_eq!(w3.symbolic & TRAIT_BOLD, 0);
        let w5 = face_traits("HiraginoSans-W5").expect("Hiragino Sans W5");
        assert!((w5.weight - 0.3).abs() < 1e-6);
        assert_ne!(w5.symbolic & TRAIT_BOLD, 0);
        let italic = face_traits("TrebuchetMS-Italic").expect("Trebuchet MS Italic");
        assert_ne!(italic.symbolic & TRAIT_ITALIC, 0);
    }

    #[test]
    fn a_covered_character_keeps_the_base_family() {
        let Some(found) = substitute_face("Helvetica", "A", 'A', 0, 400) else {
            return;
        };
        assert_eq!(found.postscript, "Helvetica");
    }

    /// The face for `text` under a regular base, as `substitute_face` is
    /// asked with Trebuchet MS at `css_weight`.
    fn substitute(text: &str, css_weight: u16) -> Option<String> {
        let ch = text.chars().next()?;
        let bold = if css_weight >= 600 { TRAIT_BOLD } else { 0 };
        let base = face_traits("TrebuchetMS").map_or(0, |t| t.symbolic);
        substitute_face("TrebuchetMS", text, ch, base | bold, css_weight).map(|s| s.postscript)
    }

    #[test]
    fn scripts_go_to_their_cascade_faces() {
        // The faces a stock macOS install substitutes on a Latin base, as
        // Chrome 156 reports them through `CSS.getPlatformFontsForNode`.
        for (text, face) in [
            ("\u{0928}", "KohinoorDevanagari-Regular"),
            ("\u{0E2A}", "Thonburi"),
            ("\u{4E2D}", "PingFangSC-Regular"),
            ("\u{20B9}", "Helvetica"),
            ("\u{1F600}", "AppleColorEmoji"),
        ] {
            assert_eq!(substitute(text, 400).as_deref(), Some(face), "{text:?}");
        }
    }

    #[test]
    fn the_weight_picks_the_face_inside_the_substitute_family() {
        // Chrome 156 on macOS, `CSS.getPlatformFontsForNode` per weight:
        // the CoreText weight decides, so 400 kana are in W3 (OS/2 class
        // 300) and 500 kana in W4, and a weight whose face lacks the
        // character (Helvetica Light has no `₹`) keeps the substitute.
        for (text, weight, face) in [
            ("\u{30AB}", 100, "HiraginoSans-W0"),
            ("\u{30AB}", 300, "HiraginoSans-W2"),
            ("\u{30AB}", 400, "HiraginoSans-W3"),
            ("\u{30AB}", 500, "HiraginoSans-W4"),
            ("\u{30AB}", 600, "HiraginoSans-W5"),
            ("\u{30AB}", 700, "HiraginoSans-W6"),
            ("\u{30AB}", 900, "HiraginoSans-W8"),
            ("\u{4E2D}", 700, "PingFangSC-Semibold"),
            ("\u{D55C}", 400, "AppleSDGothicNeo-Regular"),
            ("\u{D55C}", 700, "AppleSDGothicNeo-Bold"),
            ("\u{20B9}", 300, "Helvetica"),
            ("\u{20B9}", 500, "Helvetica-Bold"),
        ] {
            assert_eq!(
                substitute(text, weight).as_deref(),
                Some(face),
                "{text:?} at {weight}"
            );
        }
    }

    #[test]
    fn css_weights_map_to_coretext_weights() {
        assert_eq!(ct_weight(400), 0.0);
        assert_eq!(ct_weight(500), 0.23);
        assert_eq!(ct_weight(700), 0.40);
        assert_eq!(ct_weight(900), 0.62);
        assert_eq!(ct_weight(1000), 0.0);
    }
}
