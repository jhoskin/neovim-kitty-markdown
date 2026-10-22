//! The `conf` object of `sequenceRenderer.ts` and the fonts it measures
//! and draws with.
//!
//! `draw` sets `conf = getConfig().sequence`, then `bounds.init()` calls
//! `setConf(getConfig())`, which merges the whole config into that same
//! object and copies the top-level `fontFamily`, `fontSize`, and
//! `fontWeight` over `actorFont*`, `noteFont*`, and `messageFont*`
//! whenever they are set. The defaults set `fontFamily` and `fontSize`,
//! so the sequence-specific font keys never reach the page in practice.

use serde_json::Value;

use crate::mermaid::config::Resolved;
use crate::mermaid::config::defaults::Font;
use crate::mermaid::scene::{FontSpec as SceneFont, FontWeight};
use crate::text::FontSpec as MeasureSpec;
use crate::text::FontSystem;

/// What headless Chrome measures with when an element has no usable
/// `font-family`: the page's default standard font. mermaid-cli's
/// `index.html` sets none, so `getComputedStyle(body).fontFamily` is
/// `"Times New Roman"` (probed through the pinned puppeteer: the pangram
/// at 16 px measures 292.375 px wide there and in the helper).
///
/// Temporary: the central rule lands in
/// `TextMetrics::calculate_text_dimensions`, which will apply the page
/// default to any chain Chrome's CSS parser rejects; `FontPair::new` and
/// `css_family_valid` go with it.
pub const BODY_DEFAULT_FAMILY: &str = "\"Times New Roman\", serif";

/// The family name resvg has to see for `chain`: fontdb matches names
/// exactly while CSS matches them case-insensitively, so `"trebuchet ms"`
/// has to reach the SVG as the installed `Trebuchet MS`. A chain nothing
/// matches is written as it is and resvg falls back on its own.
///
/// Chrome's CSS parser rejects a `font-family` value with a stray `;`
/// (mermaid's default `fontFamily` ends in one), and `style.setProperty`
/// then leaves the property unset, so the element inherits instead.
pub fn css_family_valid(family: &str) -> bool {
    !family.trim().is_empty() && !family.contains(';')
}

/// One font role (message, note, actor): what the browser measured with,
/// what it rendered with, and the inline attribute it kept.
#[derive(Debug, Clone, PartialEq)]
pub struct FontPair {
    /// `calculateTextDimensions` measures in a detached `<svg>` under
    /// `<body>`, so an invalid family falls back to the page default.
    pub measure: MeasureSpec,
    /// `getBBox()` on the drawn element and the glyphs resvg paints: an
    /// invalid family inherits the diagram root's family.
    pub render: MeasureSpec,
    /// The `font-family` attribute to write, or `None` when Chrome dropped
    /// it and the root family applies.
    pub css_family: Option<String>,
    pub size: f64,
    pub weight: u16,
}

impl FontPair {
    fn new(font: &Font, root_family: &str, _fonts: &FontSystem) -> FontPair {
        let family = font.font_family.clone();
        let size = *font.font_size;
        let weight = css_weight(&font.font_weight);
        let valid = css_family_valid(&family);
        let measure_family = if valid {
            family.clone()
        } else {
            BODY_DEFAULT_FAMILY.to_string()
        };
        let render_family = if valid {
            family.clone()
        } else {
            root_family.to_string()
        };
        FontPair {
            measure: MeasureSpec::new(measure_family, size as f32).weight(weight),
            render: MeasureSpec::new(render_family, size as f32).weight(weight),
            css_family: valid.then(|| family.clone()),
            size,
            weight,
        }
    }

    /// The font attributes `drawText` writes inline (`font-size` as `px`,
    /// `font-weight` as given, `font-family` when valid).
    pub fn scene_font(&self) -> SceneFont {
        SceneFont {
            family: self.css_family.clone(),
            size: Some(self.size),
            weight: Some(FontWeight::Number(self.weight)),
            style: None,
        }
    }
}

/// `font-weight` as mermaid passes it to `.style()`: a number, or
/// `bold`/`normal`; anything else is left to the default of 400.
fn css_weight(value: &Value) -> u16 {
    match value {
        Value::Number(n) => n
            .as_f64()
            .map_or(400, |v| v.round().clamp(1.0, 1000.0) as u16),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "bold" => 700,
            "normal" => 400,
            other => other.parse::<u16>().unwrap_or(400),
        },
        _ => 400,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fonts {
    pub message: FontPair,
    pub note: FontPair,
    pub actor: FontPair,
}

/// The numbers `sequenceRenderer.ts` and `svgDraw.js` read from `conf`.
#[derive(Debug, Clone, PartialEq)]
pub struct Conf {
    pub activation_width: f64,
    pub diagram_margin_x: f64,
    pub diagram_margin_y: f64,
    pub actor_margin: f64,
    pub width: f64,
    /// `calculateActorMargins` overwrites it with the tallest actor.
    pub height: f64,
    pub box_margin: f64,
    pub box_text_margin: f64,
    pub note_margin: f64,
    pub message_margin: f64,
    pub message_align: String,
    pub note_align: String,
    pub mirror_actors: bool,
    pub bottom_margin_adj: f64,
    pub right_angles: bool,
    pub show_sequence_numbers: bool,
    pub wrap_padding: f64,
    pub label_box_width: f64,
    pub label_box_height: f64,
    pub hide_unused_participants: bool,
    pub use_max_width: bool,
    pub fonts: Fonts,
}

impl Conf {
    /// `conf` after `setConf(getConfig())`. `root_family` is the theme's
    /// `fontFamily`, what the stylesheet puts on the svg root.
    pub fn from_resolved(resolved: &Resolved, root_family: &str, fonts: &FontSystem) -> Conf {
        let cfg = &resolved.config;
        let seq = &cfg.sequence;
        let mut message = seq.message_font();
        let mut note = seq.note_font();
        let mut actor = seq.actor_font();
        if !cfg.font_family.is_empty() {
            for f in [&mut message, &mut note, &mut actor] {
                f.font_family = cfg.font_family.clone();
            }
        }
        if *cfg.font_size != 0.0 {
            for f in [&mut message, &mut note, &mut actor] {
                f.font_size = cfg.font_size;
            }
        }
        if let Some(weight) = resolved.raw.get("fontWeight")
            && crate::mermaid::theme::vars::js_truthy(weight)
        {
            for f in [&mut message, &mut note, &mut actor] {
                f.font_weight = weight.clone();
            }
        }
        Conf {
            activation_width: *seq.activation_width,
            diagram_margin_x: *seq.diagram_margin_x,
            diagram_margin_y: *seq.diagram_margin_y,
            actor_margin: *seq.actor_margin,
            width: *seq.width,
            height: *seq.height,
            box_margin: *seq.box_margin,
            box_text_margin: *seq.box_text_margin,
            note_margin: *seq.note_margin,
            message_margin: *seq.message_margin,
            message_align: seq.message_align.clone(),
            note_align: seq.note_align.clone(),
            mirror_actors: seq.mirror_actors,
            bottom_margin_adj: *seq.bottom_margin_adj,
            right_angles: seq.right_angles,
            show_sequence_numbers: seq.show_sequence_numbers,
            wrap_padding: *seq.wrap_padding,
            label_box_width: *seq.label_box_width,
            label_box_height: *seq.label_box_height,
            hide_unused_participants: seq.hide_unused_participants,
            use_max_width: seq.use_max_width,
            fonts: Fonts {
                message: FontPair::new(&message, root_family, fonts),
                note: FontPair::new(&note, root_family, fonts),
                actor: FontPair::new(&actor, root_family, fonts),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_family_measures_in_the_page_default() {
        let font = Font {
            font_family: "\"trebuchet ms\", verdana, arial, sans-serif;".into(),
            font_size: 16.0.into(),
            font_weight: serde_json::json!(400),
        };
        let pair = FontPair::new(
            &font,
            "\"trebuchet ms\", verdana, arial, sans-serif",
            crate::text::font_system(),
        );
        assert_eq!(pair.measure.family, BODY_DEFAULT_FAMILY);
        assert_eq!(
            pair.render.family,
            "\"trebuchet ms\", verdana, arial, sans-serif"
        );
        assert_eq!(pair.css_family, None);
        assert_eq!(pair.weight, 400);
    }

    #[test]
    fn valid_family_is_kept_inline() {
        let font = Font {
            font_family: "\"Open Sans\", sans-serif".into(),
            font_size: 14.0.into(),
            font_weight: serde_json::json!("bold"),
        };
        let pair = FontPair::new(&font, "serif", crate::text::font_system());
        assert_eq!(
            pair.css_family.as_deref(),
            Some("\"Open Sans\", sans-serif")
        );
        assert_eq!(pair.measure.family, "\"Open Sans\", sans-serif");
        assert_eq!(pair.weight, 700);
    }
}
