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

/// Fonts Chrome falls back to for characters the resolved face lacks. The
/// order follows what CoreText and fontconfig pick on a stock install.
fn fallback_candidates() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &[
            "PingFang SC",
            "Hiragino Sans",
            "Hiragino Kaku Gothic ProN",
            "Apple SD Gothic Neo",
            "Apple Color Emoji",
            "Apple Symbols",
            "Arial Unicode MS",
        ]
    } else {
        &[
            "Noto Sans CJK SC",
            "Noto Sans CJK JP",
            "Noto Color Emoji",
            "DejaVu Sans",
            "Symbola",
        ]
    }
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
        FontSystem {
            db,
            open_sans_regular,
            open_sans_italic,
            data: Mutex::new(HashMap::new()),
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
    fn query_named(&self, family: &str, weight: Weight, style: Style) -> Option<FontId> {
        let known = self
            .db
            .faces()
            .flat_map(|face| face.families.iter())
            .map(|(name, _)| name.as_str())
            .find(|name| name.eq_ignore_ascii_case(family))?;
        self.db.query(&fontdb::Query {
            families: &[fontdb::Family::Name(known)],
            weight,
            stretch: fontdb::Stretch::Normal,
            style,
        })
    }

    /// A face that has a glyph for `ch`, from the platform fallback list,
    /// then from any installed face. `None` when no face covers it. Chrome
    /// gives emoji presentation characters to the color emoji font before
    /// any other fallback, so pictographs try it first.
    pub fn fallback_for(&self, ch: char, weight: Weight, style: Style) -> Option<FontId> {
        let emoji_first = super::wrap::is_extended_pictographic(ch);
        let candidates = fallback_candidates()
            .iter()
            .filter(|name| emoji_first == name.contains("Emoji"))
            .chain(
                fallback_candidates()
                    .iter()
                    .filter(|name| emoji_first != name.contains("Emoji")),
            );
        for name in candidates {
            match self.query_named(name, weight, style) {
                Some(id) if self.has_glyph(id, ch) => return Some(id),
                _ => {}
            }
        }
        let ids: Vec<FontId> = self.db.faces().map(|face| face.id).collect();
        ids.into_iter().find(|&id| self.has_glyph(id, ch))
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

    /// The weight the face itself carries. A request for 700 answered by a
    /// 400 face means Chrome emboldens it.
    pub fn face_weight(&self, id: FontId) -> Option<Weight> {
        self.db.face(id).map(|face| face.weight)
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
    fn face_metrics_are_readable() {
        let fonts = FontSystem::new();
        let upem = fonts
            .with_face(fonts.open_sans_regular, |face| face.units_per_em())
            .expect("face");
        assert_eq!(upem, 2048);
        assert!(fonts.has_glyph(fonts.open_sans_regular, 'A'));
        assert!(!fonts.has_glyph(fonts.open_sans_regular, '中'));
    }
}
