//! fontdb database with the bundled Open Sans first, family chain resolution.
//!
//! mermaid-cli 12.0.0 loads Open Sans 400 and 400 italic into its page from
//! `@fontsource/open-sans`, which shadows any system Open Sans. The helper
//! bundles the same two faces (R-179) and routes every request for the
//! family to them, so a system Open Sans Bold cannot change a measurement
//! that Chrome made with a synthetic bold.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use usvg::fontdb;

pub use fontdb::{ID as FontId, Style, Weight};

const OPEN_SANS_REGULAR: &[u8] = include_bytes!("../../assets/fonts/OpenSans-Regular.ttf");
const OPEN_SANS_ITALIC: &[u8] = include_bytes!("../../assets/fonts/OpenSans-Italic.ttf");

/// How far Chrome's synthetic bold pushes a glyph outline out on each side,
/// as a fraction of the font size. Chrome emboldens a face lighter than 600
/// when the text asks for 600 or more; on the oracle's Open Sans 700 cases
/// the ink box grows 0.280, 0.327, and 0.374 px at 12, 14, and 16 px.
pub const SYNTHETIC_BOLD_GROWTH: f64 = 0.02334;

/// The families whose presence the `probe` result reports.
pub const REPORTED_FAMILIES: [&str; 5] =
    ["trebuchet ms", "verdana", "arial", "open sans", "helvetica"];

/// Chrome's default font for each CSS generic family on the platforms the
/// helper ships for. The oracle on macOS measures `sans-serif` as Arial
/// metrics, so Arial leads the macOS chain and Helvetica follows for
/// machines without it.
fn generic_candidates(generic: &str) -> &'static [&'static str] {
    match generic {
        "sans-serif" | "system-ui" | "ui-sans-serif" => {
            if cfg!(target_os = "macos") {
                &["Arial", "Helvetica", "Helvetica Neue"]
            } else {
                &["DejaVu Sans", "Liberation Sans", "Arimo", "Noto Sans"]
            }
        }
        "serif" | "ui-serif" => {
            if cfg!(target_os = "macos") {
                &["Times New Roman", "Times"]
            } else {
                &["DejaVu Serif", "Liberation Serif", "Tinos", "Noto Serif"]
            }
        }
        "monospace" | "ui-monospace" => {
            if cfg!(target_os = "macos") {
                &["Menlo", "Courier New", "Courier"]
            } else {
                &[
                    "DejaVu Sans Mono",
                    "Liberation Mono",
                    "Cousine",
                    "Noto Sans Mono",
                ]
            }
        }
        "cursive" => &["Comic Sans MS", "Apple Chancery"],
        "fantasy" => &["Papyrus", "Impact"],
        _ => &[],
    }
}

/// Fonts Chrome falls back to on Linux for characters no family of the
/// CSS list covers, in the order fontconfig picks them on a stock install.
/// macOS asks CoreText instead ([`super::coretext`]).
#[cfg(not(target_os = "macos"))]
const FALLBACK_CANDIDATES: [&str; 5] = [
    "Noto Sans CJK SC",
    "Noto Sans CJK JP",
    "Noto Color Emoji",
    "DejaVu Sans",
    "Symbola",
];

/// The color emoji face Blink's `FontFallbackPriority::kEmojiEmoji` tries
/// first for a grapheme with emoji presentation.
const EMOJI_FAMILY: &str = if cfg!(target_os = "macos") {
    "Apple Color Emoji"
} else {
    "Noto Color Emoji"
};

/// Where macOS keeps PingFang, the CoreText substitute for Han text. The
/// directory is outside the folders fontdb scans, so it is loaded apart.
#[cfg(target_os = "macos")]
const MACOS_RESERVED_FONTS: &str =
    "/System/Library/PrivateFrameworks/FontServices.framework/Resources/Reserved";

/// True when Chrome draws `grapheme` with emoji presentation: a following
/// `U+FE0F`, or `Emoji_Presentation=Yes` with no `U+FE0E`, as Blink's
/// `FontFallbackPriority` decides. Such a grapheme goes to the color emoji
/// face even when the primary face has a text glyph for it (`☺️` under
/// Arial).
pub fn emoji_presentation(grapheme: &str) -> bool {
    let Some(ch) = grapheme.chars().next() else {
        return false;
    };
    grapheme.contains('\u{FE0F}')
        || (super::wrap::is_emoji_presentation(ch) && !grapheme.contains('\u{FE0E}'))
}

/// Splits a CSS `font-family` list into family names, unquoted and trimmed.
/// The names keep their case; matching is case-insensitive.
pub fn parse_families(chain: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in chain.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch == ',' => {
                let name = current.trim();
                if !name.is_empty() {
                    out.push(name.to_string());
                }
                current.clear();
            }
            None => current.push(ch),
        }
    }
    let name = current.trim();
    if !name.is_empty() {
        out.push(name.to_string());
    }
    out
}

/// A platform fallback question: the primary face, the grapheme, and the
/// requested weight and style.
#[cfg(target_os = "macos")]
type SubstituteKey = (Option<FontId>, String, u16, Style);

/// Font bytes for one face, held so a `rustybuzz::Face` can borrow them.
struct FaceData {
    bytes: Arc<dyn AsRef<[u8]> + Send + Sync>,
    index: u32,
}

pub struct FontSystem {
    db: fontdb::Database,
    open_sans_regular: FontId,
    open_sans_italic: FontId,
    data: Mutex<HashMap<FontId, Arc<FaceData>>>,
    /// Family matches by lower-case family name, weight, and style.
    matches: Mutex<HashMap<(String, u16, Style), Option<FontId>>>,
    /// CoreText fallback faces by primary face, grapheme, weight, and style.
    #[cfg(target_os = "macos")]
    substitutes: Mutex<HashMap<SubstituteKey, Option<super::coretext::Substitute>>>,
    /// CoreText traits by face, read on first use.
    #[cfg(target_os = "macos")]
    traits: Mutex<HashMap<FontId, Option<super::coretext::FaceTraits>>>,
}

impl Default for FontSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FontSystem {
    /// Loads the bundled Open Sans faces, then the system fonts (R-179).
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        let open_sans_regular = Self::load_bundled(&mut db, OPEN_SANS_REGULAR);
        let open_sans_italic = Self::load_bundled(&mut db, OPEN_SANS_ITALIC);
        db.load_system_fonts();
        #[cfg(target_os = "macos")]
        db.load_fonts_dir(MACOS_RESERVED_FONTS);
        FontSystem {
            db,
            open_sans_regular,
            open_sans_italic,
            data: Mutex::new(HashMap::new()),
            matches: Mutex::new(HashMap::new()),
            #[cfg(target_os = "macos")]
            substitutes: Mutex::new(HashMap::new()),
            #[cfg(target_os = "macos")]
            traits: Mutex::new(HashMap::new()),
        }
    }

    fn load_bundled(db: &mut fontdb::Database, bytes: &'static [u8]) -> FontId {
        let ids = db.load_font_source(fontdb::Source::Binary(Arc::new(bytes)));
        // A bundled TTF holds one face, and the build fails before this
        // runs if the asset is missing, so the only way to get no id is a
        // corrupt asset. That is a packaging bug worth a loud failure.
        ids.first()
            .copied()
            .expect("bundled Open Sans face must parse")
    }

    pub fn db(&self) -> &fontdb::Database {
        &self.db
    }

    /// The first family in `chain` that has a face, matched with CSS
    /// weight and style rules (R-115). Generic families map to Chrome's
    /// platform default. `None` when nothing in the chain exists.
    pub fn resolve(&self, chain: &str, weight: Weight, style: Style) -> Option<FontId> {
        for family in parse_families(chain) {
            if let Some(id) = self.resolve_family(&family, weight, style) {
                return Some(id);
            }
        }
        None
    }

    /// One family name, generic or concrete.
    pub fn resolve_family(&self, family: &str, weight: Weight, style: Style) -> Option<FontId> {
        let lower = family.to_ascii_lowercase();
        if lower == "open sans" {
            return Some(match style {
                Style::Normal => self.open_sans_regular,
                Style::Italic | Style::Oblique => self.open_sans_italic,
            });
        }
        let candidates = generic_candidates(&lower);
        if candidates.is_empty() {
            return self.query_named(family, weight, style);
        }
        candidates
            .iter()
            .find_map(|name| self.query_named(name, weight, style))
    }

    /// CSS matching is case-insensitive while fontdb compares names
    /// exactly, so find the spelling the database knows first.
    /// Every grapheme of a fallback string asks this, and each answer
    /// scans the whole database, so answers are cached.
    fn query_named(&self, family: &str, weight: Weight, style: Style) -> Option<FontId> {
        let key = (family.to_ascii_lowercase(), weight.0, style);
        if let Some(found) = self.matches.lock().ok().and_then(|m| m.get(&key).copied()) {
            return found;
        }
        let found = self.match_named(family, weight, style);
        if let Ok(mut matches) = self.matches.lock() {
            matches.insert(key, found);
        }
        found
    }

    fn match_named(&self, family: &str, weight: Weight, style: Style) -> Option<FontId> {
        let known = self
            .db
            .faces()
            .flat_map(|face| face.families.iter())
            .map(|(name, _)| name.as_str())
            .find(|name| name.eq_ignore_ascii_case(family))?;
        #[cfg(target_os = "macos")]
        if let Some(id) = self.best_style_match(known, weight, style) {
            return Some(id);
        }
        self.db.query(&fontdb::Query {
            families: &[fontdb::Family::Name(known)],
            weight,
            stretch: fontdb::Stretch::Normal,
            style,
        })
    }

    /// The CoreText traits of a face, cached.
    #[cfg(target_os = "macos")]
    fn face_traits(&self, id: FontId) -> Option<super::coretext::FaceTraits> {
        if let Some(found) = self.traits.lock().ok()?.get(&id) {
            return *found;
        }
        let traits = self
            .db
            .face(id)
            .and_then(|face| super::coretext::face_traits(&face.post_script_name));
        self.traits.lock().ok()?.insert(id, traits);
        traits
    }

    /// The CSS weight Chrome on macOS gives a face: Blink's
    /// `ToCssFontWeight` (font_matcher_mac.mm) buckets the CoreText weight
    /// trait, not the OS/2 `usWeightClass`. The two disagree for Hiragino
    /// Sans, whose W3 is class 300 but a CoreText regular (0.0), so Chrome
    /// draws 400 kana in W3 and 500 in W4, as `getPlatformFontsForNode`
    /// reports in Chrome 156.
    #[cfg(target_os = "macos")]
    fn css_weight(&self, id: FontId) -> Option<u16> {
        const BUCKETS: [(f64, f64, u16); 9] = [
            (-1.0, -0.70, 100),
            (-0.70, -0.45, 200),
            (-0.45, -0.10, 300),
            (-0.10, 0.10, 400),
            (0.10, 0.27, 500),
            (0.27, 0.35, 600),
            (0.35, 0.50, 700),
            (0.50, 0.60, 800),
            (0.60, 1.0, 900),
        ];
        let weight = self.face_traits(id)?.weight;
        Some(
            BUCKETS
                .iter()
                .find(|(low, high, _)| (*low..=*high).contains(&weight))
                .map_or(400, |b| b.2),
        )
    }

    /// Blink's `BestStyleMatchForFamily` with `BetterChoiceCT`
    /// (font_matcher_mac.mm, the path Chrome 156 takes: the oracle's
    /// choice of Hiragino Sans W3 for 400 rules out the OS/2 weight path).
    /// Faces are compared on the condensed, expanded, italic, and bold
    /// symbolic traits in that order, an exact weight winning at the bold
    /// step, then on the weight distance, a tie going to the weight farther
    /// from 500. `None` when CoreText knows none of the family's faces.
    #[cfg(target_os = "macos")]
    fn best_style_match(&self, family: &str, weight: Weight, style: Style) -> Option<FontId> {
        use super::coretext::{TRAIT_BOLD, TRAIT_CONDENSED, TRAIT_EXPANDED, TRAIT_ITALIC};
        const MASKS: [u32; 4] = [TRAIT_CONDENSED, TRAIT_EXPANDED, TRAIT_ITALIC, TRAIT_BOLD];
        const IMPORTANT: u32 = TRAIT_CONDENSED | TRAIT_EXPANDED | TRAIT_ITALIC | TRAIT_BOLD;
        // Blink's `ComputeDesiredTraits`: italic for any slant, bold from
        // the 600 threshold.
        let mut desired_traits = 0;
        if style != Style::Normal {
            desired_traits |= TRAIT_ITALIC;
        }
        if weight.0 >= 600 {
            desired_traits |= TRAIT_BOLD;
        }
        let desired_weight = i32::from(weight.0);
        let better = |chosen: (u32, i32), candidate: (u32, i32)| -> bool {
            for mask in MASKS {
                if mask == TRAIT_BOLD && candidate.1 == desired_weight && chosen.1 != desired_weight
                {
                    return true;
                }
                let desired = desired_traits & mask != 0;
                let chosen_unwanted = desired != (chosen.0 & mask != 0);
                let candidate_unwanted = desired != (candidate.0 & mask != 0);
                if !candidate_unwanted && chosen_unwanted {
                    return true;
                }
                if !chosen_unwanted && candidate_unwanted {
                    return false;
                }
            }
            let chosen_delta = (chosen.1 - desired_weight).abs();
            let candidate_delta = (candidate.1 - desired_weight).abs();
            if chosen_delta == candidate_delta {
                return (candidate.1 - 500).abs() > (chosen.1 - 500).abs();
            }
            candidate_delta < chosen_delta
        };
        let candidates: Vec<FontId> = self
            .db
            .faces()
            .filter(|face| face.families.iter().any(|(name, _)| name == family))
            .map(|face| face.id)
            .collect();
        let mut chosen: Option<(FontId, (u32, i32))> = None;
        for id in candidates {
            let (Some(traits), Some(css)) = (self.face_traits(id), self.css_weight(id)) else {
                continue;
            };
            let candidate = (traits.symbolic, i32::from(css));
            if chosen.is_none_or(|(_, c)| better(c, candidate)) {
                chosen = Some((id, candidate));
            }
            if let Some((id, (traits, css))) = chosen
                && css == desired_weight
                && traits & IMPORTANT == desired_traits & IMPORTANT
            {
                return Some(id);
            }
        }
        chosen.map(|(id, _)| id)
    }

    /// A face that has a glyph for `ch` when no primary face exists.
    pub fn fallback_for(&self, ch: char, weight: Weight, style: Style) -> Option<FontId> {
        let grapheme = ch.to_string();
        self.platform_fallback(
            None,
            &grapheme,
            emoji_presentation(&grapheme),
            weight,
            style,
        )
    }

    /// The face the platform gives `grapheme` once the CSS list is
    /// exhausted. An emoji-presentation grapheme tries the color emoji face
    /// first. On macOS CoreText picks the face with the primary face as the
    /// base, as Blink asks it ([`super::coretext::substitute_face`]);
    /// elsewhere the family comes from the fontconfig order and the
    /// requested weight and style are matched inside it. A bold or italic
    /// request the face lacks gets a synthetic bold or oblique.
    fn platform_fallback(
        &self,
        primary: Option<FontId>,
        grapheme: &str,
        emoji_first: bool,
        weight: Weight,
        style: Style,
    ) -> Option<FontId> {
        let ch = grapheme.chars().next()?;
        let covered = |name: &str| {
            self.query_named(name, weight, style)
                .filter(|&id| self.has_glyph(id, ch))
        };
        if emoji_first && let Some(id) = covered(EMOJI_FAMILY) {
            return Some(id);
        }
        self.platform_face(primary, grapheme, weight, style)
            .or_else(|| {
                let ids: Vec<FontId> = self.db.faces().map(|face| face.id).collect();
                ids.into_iter().find(|&id| self.has_glyph(id, ch))
            })
    }

    /// The CoreText substitute for `grapheme` as a face of this database:
    /// the face with the substitute's PostScript name, or, for a named
    /// instance of a variable font that fontdb does not list (PingFang SC
    /// Regular), the family's face matched to the weight and style.
    #[cfg(target_os = "macos")]
    fn platform_face(
        &self,
        primary: Option<FontId>,
        grapheme: &str,
        weight: Weight,
        style: Style,
    ) -> Option<FontId> {
        use super::coretext::{TRAIT_BOLD, TRAIT_ITALIC};
        let ch = grapheme.chars().next()?;
        let key = (primary, grapheme.to_string(), weight.0, style);
        let cached = self.substitutes.lock().ok()?.get(&key).cloned();
        let substitute = match cached {
            Some(found) => found,
            None => {
                let face = primary.and_then(|id| self.db.face(id));
                let base = face.map_or("Helvetica", |face| face.post_script_name.as_str());
                // Blink's desired traits: the primary font's own, plus the
                // bold and italic it synthesizes. CoreText does not know
                // the bundled Open Sans, so its traits come from fontdb,
                // as a web font's come from its own tables in Blink.
                let own = match primary.and_then(|id| self.face_traits(id)) {
                    Some(traits) => traits.symbolic,
                    None => {
                        let italic = face.is_some_and(|f| f.style != Style::Normal);
                        let bold = face.is_some_and(|f| f.weight.0 >= 600);
                        let base_traits = super::coretext::base_symbolic_traits(base).unwrap_or(0)
                            & !(TRAIT_BOLD | TRAIT_ITALIC);
                        base_traits
                            | if italic { TRAIT_ITALIC } else { 0 }
                            | if bold { TRAIT_BOLD } else { 0 }
                    }
                };
                let mut symbolic = own;
                if weight.0 >= 600 {
                    symbolic |= TRAIT_BOLD;
                }
                if style != Style::Normal {
                    symbolic |= TRAIT_ITALIC;
                }
                let found =
                    super::coretext::substitute_face(base, grapheme, ch, symbolic, weight.0);
                self.substitutes.lock().ok()?.insert(key, found.clone());
                found
            }
        }?;
        self.db
            .faces()
            .find(|face| face.post_script_name == substitute.postscript)
            .map(|face| face.id)
            .filter(|&id| self.has_glyph(id, ch))
            .or_else(|| {
                self.query_named(&substitute.family, weight, style)
                    .filter(|&id| self.has_glyph(id, ch))
            })
    }

    #[cfg(not(target_os = "macos"))]
    fn platform_face(
        &self,
        _primary: Option<FontId>,
        grapheme: &str,
        weight: Weight,
        style: Style,
    ) -> Option<FontId> {
        let ch = grapheme.chars().next()?;
        FALLBACK_CANDIDATES
            .iter()
            .find(|name| {
                self.query_named(name, Weight::NORMAL, Style::Normal)
                    .is_some_and(|id| self.has_glyph(id, ch))
            })
            .and_then(|name| self.query_named(name, weight, style))
            .filter(|&id| self.has_glyph(id, ch))
    }

    /// The face Chrome draws `grapheme` in when the primary face lacks it
    /// or the grapheme has emoji presentation, with the grapheme as its
    /// own fallback hint: [`Self::listed_face`], else
    /// [`Self::platform_face_in_chain`].
    pub fn fallback_in_chain(
        &self,
        chain: &str,
        grapheme: &str,
        weight: Weight,
        style: Style,
    ) -> Option<FontId> {
        self.listed_face(chain, grapheme, weight, style)
            .or_else(|| self.platform_face_in_chain(chain, grapheme, weight, style))
    }

    /// The first family of `chain` that covers `grapheme`.
    ///
    /// Blink's `FontFallbackList` tries every family of the `font-family`
    /// list in order before it asks the platform, so under mermaid's
    /// `"trebuchet ms", verdana, arial, sans-serif` an `→` is drawn in
    /// Arial and a `├` in Arial while `─` stays in Verdana. A grapheme with
    /// emoji presentation ([`emoji_presentation`]) skips the list: Chrome
    /// draws `❤️` in Apple Color Emoji even when a listed family has a text
    /// `❤`. A lone `❤` takes the text order and Chrome draws it in Zapf
    /// Dingbats. All of these read off `CSS.getPlatformFontsForNode` in
    /// the oracle's Chrome 156 on macOS.
    pub fn listed_face(
        &self,
        chain: &str,
        grapheme: &str,
        weight: Weight,
        style: Style,
    ) -> Option<FontId> {
        let ch = grapheme.chars().next()?;
        if emoji_presentation(grapheme) {
            return None;
        }
        parse_families(chain)
            .into_iter()
            .filter_map(|family| self.resolve_family(&family, weight, style))
            .find(|&id| self.has_glyph(id, ch))
    }

    /// The face the platform gives `hint` under the primary face of
    /// `chain` ([`Self::platform_fallback`]). Blink asks this once per
    /// stretch of text the list cannot cover, with one character of the
    /// stretch as the hint, and draws every character of the stretch that
    /// the answer covers in it.
    pub fn platform_face_in_chain(
        &self,
        chain: &str,
        hint: &str,
        weight: Weight,
        style: Style,
    ) -> Option<FontId> {
        let primary = self.resolve(chain, weight, style);
        self.platform_fallback(primary, hint, emoji_presentation(hint), weight, style)
    }

    /// Glyph boxes from the platform for a face whose outline tables
    /// ttf-parser does not read (no `glyf`, `CFF `, or `CFF2`), in font
    /// units; one entry per glyph, or an empty list where the platform
    /// cannot tell. See [`super::coretext::glyph_bounds`].
    /// `skew` is the synthetic oblique, applied to the outline before the
    /// box is taken.
    pub fn platform_glyph_bounds(
        &self,
        id: FontId,
        glyphs: &[u16],
        skew: f32,
    ) -> Vec<Option<[f64; 4]>> {
        #[cfg(target_os = "macos")]
        {
            let units = self.with_face(id, |face| face.units_per_em());
            if let (Some(face), Some(units)) = (self.db.face(id), units)
                && let Some(bounds) = super::coretext::glyph_bounds(
                    &face.post_script_name,
                    units.into(),
                    glyphs,
                    skew.into(),
                )
            {
                return bounds;
            }
        }
        let _ = (id, glyphs, skew);
        Vec::new()
    }

    /// True when the face maps `ch` to a glyph other than `.notdef`.
    pub fn has_glyph(&self, id: FontId, ch: char) -> bool {
        self.with_face(id, |face| face.glyph_index(ch).is_some())
            .unwrap_or(false)
    }

    /// Runs `f` with a parsed `rustybuzz::Face`. The bytes are cached per
    /// face; the parse itself is a table-directory read and is cheap enough
    /// to repeat, and it keeps the face from borrowing across the cache lock.
    pub fn with_face<T>(&self, id: FontId, f: impl FnOnce(&rustybuzz::Face<'_>) -> T) -> Option<T> {
        let data = self.face_data(id)?;
        let face = rustybuzz::Face::from_slice((*data.bytes).as_ref(), data.index)?;
        Some(f(&face))
    }

    fn face_data(&self, id: FontId) -> Option<Arc<FaceData>> {
        if let Some(found) = self.data.lock().ok()?.get(&id) {
            return Some(found.clone());
        }
        let (source, index) = self.db.face_source(id)?;
        let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = match source {
            fontdb::Source::Binary(bytes) => bytes,
            fontdb::Source::SharedFile(_, bytes) => bytes,
            fontdb::Source::File(path) => Arc::new(std::fs::read(path).ok()?),
        };
        let data = Arc::new(FaceData { bytes, index });
        self.data.lock().ok()?.insert(id, data.clone());
        Some(data)
    }

    /// Which of the families mermaid and its diagrams ask for resolved on
    /// this machine; the `probe` result carries it as `fonts`.
    pub fn report(&self) -> BTreeMap<String, bool> {
        REPORTED_FAMILIES
            .iter()
            .map(|family| {
                let found = self
                    .resolve_family(family, Weight::NORMAL, Style::Normal)
                    .is_some();
                (family.to_string(), found)
            })
            .collect()
    }

    /// The style the face itself carries. A request for italic answered by
    /// a `Normal` face means Chrome synthesizes an oblique.
    pub fn face_style(&self, id: FontId) -> Option<Style> {
        self.db.face(id).map(|face| face.style)
    }

    /// The weight the face itself carries, as CSS weight: on macOS the
    /// bucketed CoreText weight ([`Self::css_weight`]), elsewhere the OS/2
    /// class. A request for 700 answered by a 400 face means Chrome
    /// emboldens it.
    pub fn face_weight(&self, id: FontId) -> Option<Weight> {
        #[cfg(target_os = "macos")]
        if let Some(css) = self.css_weight(id) {
            return Some(Weight(css));
        }
        self.db.face(id).map(|face| face.weight)
    }

    /// True when Chrome draws `chain` at `weight` with a synthetic bold:
    /// the request is 600 or more and the face it resolves to is lighter
    /// (Blink's `FontDescription::SyntheticBold` rule). The bundled Open
    /// Sans has no bold face, so every bold Open Sans text qualifies.
    pub fn synthesizes_bold(&self, chain: &str, weight: Weight, style: Style) -> bool {
        weight.0 >= 600
            && self
                .resolve(chain, weight, style)
                .and_then(|id| self.face_weight(id))
                .is_some_and(|w| w.0 < 600)
    }

    /// Every family name the face carries, localized ones included
    /// (PingFang SC also names itself `蘋方-簡`, the name Chrome reports).
    pub fn family_names(&self, id: FontId) -> Vec<String> {
        self.db
            .face(id)
            .map(|face| face.families.iter().map(|(name, _)| name.clone()).collect())
            .unwrap_or_default()
    }

    /// Family name of a face as the database lists it, for diagnostics.
    pub fn family_name(&self, id: FontId) -> Option<String> {
        self.db
            .face(id)
            .and_then(|face| face.families.first())
            .map(|(name, _)| name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_css_family_lists() {
        assert_eq!(
            parse_families("\"trebuchet ms\", verdana, arial, sans-serif"),
            vec!["trebuchet ms", "verdana", "arial", "sans-serif"]
        );
        assert_eq!(
            parse_families("'Open Sans',sans-serif"),
            vec!["Open Sans", "sans-serif"]
        );
        assert_eq!(parse_families("  Arial  "), vec!["Arial"]);
        assert_eq!(parse_families("\"A, B\", C"), vec!["A, B", "C"]);
        assert!(parse_families("").is_empty());
        assert!(parse_families(" , ,").is_empty());
    }

    #[test]
    fn generic_families_have_platform_defaults() {
        assert!(!generic_candidates("sans-serif").is_empty());
        assert!(!generic_candidates("serif").is_empty());
        assert!(!generic_candidates("monospace").is_empty());
        assert!(generic_candidates("verdana").is_empty());
    }

    #[test]
    fn bundled_open_sans_resolves_without_system_fonts() {
        let fonts = FontSystem::new();
        let regular = fonts
            .resolve("\"Open Sans\", sans-serif", Weight::NORMAL, Style::Normal)
            .expect("open sans");
        let bold = fonts
            .resolve("Open Sans", Weight::BOLD, Style::Normal)
            .expect("open sans bold");
        let italic = fonts
            .resolve("open sans", Weight::NORMAL, Style::Italic)
            .expect("open sans italic");
        assert_eq!(regular, fonts.open_sans_regular);
        assert_eq!(bold, fonts.open_sans_regular, "Chrome synthesizes the bold");
        assert_eq!(italic, fonts.open_sans_italic);
        assert_eq!(fonts.family_name(regular).as_deref(), Some("Open Sans"));
        assert!(fonts.report()["open sans"]);
    }

    #[test]
    fn chain_falls_through_to_a_present_family() {
        let fonts = FontSystem::new();
        let id = fonts
            .resolve(
                "\"No Such Font\", \"Open Sans\"",
                Weight::NORMAL,
                Style::Normal,
            )
            .expect("second family");
        assert_eq!(id, fonts.open_sans_regular);
        assert!(
            fonts
                .resolve("\"No Such Font\"", Weight::NORMAL, Style::Normal)
                .is_none()
        );
    }

    #[test]
    fn a_missing_glyph_comes_from_the_next_listed_family() {
        let fonts = FontSystem::new();
        let (w, s) = (Weight::NORMAL, Style::Normal);
        let has = |family: &str| {
            fonts
                .resolve_family(family, w, s)
                .map(|id| (id, fonts.has_glyph(id, '\u{2192}')))
        };
        // The oracle's fonts: Trebuchet MS and Verdana lack `→`, Arial has
        // it. Elsewhere there is nothing to check.
        let (Some((_, false)), Some((_, false)), Some((arial, true))) =
            (has("trebuchet ms"), has("verdana"), has("arial"))
        else {
            return;
        };
        let chain = "\"trebuchet ms\", verdana, arial, sans-serif";
        assert_eq!(
            fonts.fallback_in_chain(chain, "\u{2192}", w, s),
            Some(arial)
        );
    }

    #[test]
    fn bold_open_sans_is_synthesized() {
        let fonts = FontSystem::new();
        assert!(fonts.synthesizes_bold("\"Open Sans\", sans-serif", Weight::BOLD, Style::Normal));
        assert!(fonts.synthesizes_bold("Open Sans", Weight(600), Style::Italic));
        assert!(!fonts.synthesizes_bold("Open Sans", Weight(500), Style::Normal));
        assert!(!fonts.synthesizes_bold("Open Sans", Weight::NORMAL, Style::Normal));
        assert!(!fonts.synthesizes_bold("No Such Font", Weight::BOLD, Style::Normal));
    }

    #[test]
    fn face_metrics_are_readable() {
        let fonts = FontSystem::new();
        let upem = fonts
            .with_face(fonts.open_sans_regular, |face| face.units_per_em())
            .expect("face");
        assert_eq!(upem, 2048);
        assert!(fonts.has_glyph(fonts.open_sans_regular, 'A'));
        assert!(!fonts.has_glyph(fonts.open_sans_regular, '中'));
    }

    /// The faces Chrome 156 on macOS reports through
    /// `CSS.getPlatformFontsForNode` under mermaid's chain for `❤`, `❤️`,
    /// `★`, and `✓`; `⚡` (`Emoji_Presentation=Yes`) follows the emoji
    /// priority rule. Elsewhere, or on a Mac without one of these faces,
    /// there is nothing to check.
    #[test]
    fn only_emoji_presentation_goes_to_the_emoji_font_first() {
        let fonts = FontSystem::new();
        let (w, s) = (Weight::NORMAL, Style::Normal);
        let expected = [
            ("\u{2764}", "Zapf Dingbats"),
            ("\u{2764}\u{FE0F}", "Apple Color Emoji"),
            ("\u{2605}", "Hiragino Sans"),
            ("\u{2713}", "Lucida Grande"),
            ("\u{26A1}", "Apple Color Emoji"),
        ];
        let present = |family: &str| fonts.resolve_family(family, w, s).is_some();
        if !cfg!(target_os = "macos") || !expected.iter().all(|(_, family)| present(family)) {
            return;
        }
        let chain = "\"trebuchet ms\", verdana, arial, sans-serif";
        for (grapheme, family) in expected {
            let face = fonts.fallback_in_chain(chain, grapheme, w, s);
            assert_eq!(
                face.and_then(|id| fonts.family_name(id)).as_deref(),
                Some(family),
                "{grapheme:?}"
            );
        }
    }
}
