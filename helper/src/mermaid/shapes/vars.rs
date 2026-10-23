//! [`NodeStyleVars`] from the resolved theme: the variables the flowchart,
//! state, and ER stylesheets read (`diagrams/flowchart/styles.ts`,
//! `diagrams/state/styles.js`, `diagrams/er/styles.js` in mermaid 12.2.0),
//! parsed to colors once so the shapes never see CSS text.
//!
//! Every built-in theme derives all of these in `getThemeVariables`
//! (`themes/theme-default.js` and its siblings), so the fallbacks below
//! only matter for a `themeVariables` override that removes a value or a
//! future theme that lacks one; each fallback is the derivation the
//! default theme uses (`nodeBorder = border1`, `titleColor = textColor`,
//! `edgeLabelBackground = labelBackground`, `compositeBackground =
//! background`, `innerEndBackground = nodeBorder`, `specialStateColor =
//! lineColor`, `altBackground = '#f0f0f0'`, `noteTextColor =
//! actorTextColor`).

use serde_json::Value;

use crate::mermaid::config::defaults::MermaidConfig;
use crate::mermaid::scene::{Color, Paint};
use crate::mermaid::shapes::NodeStyleVars;
use crate::mermaid::shapes::util::parse_css_color;
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::khroma::{self, Channel};

/// The `strokeWidth` the flowchart stylesheet falls back to (`options
/// .strokeWidth ?? 1`).
const DEFAULT_STROKE_WIDTH: f64 = 1.0;
/// The `radius` theme variable of every built-in theme.
const DEFAULT_RADIUS: f64 = 5.0;
/// The `fontSize` of every built-in theme.
const DEFAULT_FONT_SIZE: f64 = 16.0;

/// A CSS color the way the browser resolves a stylesheet value: hex,
/// `rgb()`/`rgba()`, and the named colors through the shapes' own parser,
/// `hsl()` and the rest of the keywords through the khroma port. `none`
/// and `transparent` become a fully transparent black. Anything else is
/// `None`, so the caller can fall back.
pub fn parse_color(value: &str) -> Option<Color> {
    match parse_css_color(value) {
        Some(Paint::Color(c)) => return Some(c),
        Some(Paint::None) => return Some(Color::rgba(0, 0, 0, 0.0)),
        Some(Paint::Reference(_)) => return None,
        None => {}
    }
    let mut channels = khroma::parse(value).ok()?;
    let mut byte = |ch: Channel| channels.get(ch).round().clamp(0.0, 255.0) as u8;
    let (r, g, b) = (byte(Channel::R), byte(Channel::G), byte(Channel::B));
    Some(Color::rgba(r, g, b, channels.get(Channel::A)))
}

/// A number variable, written as a JSON number or as a string with an
/// optional `px` unit (`fontSize: '16px'`).
fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().trim_end_matches("px").trim().parse().ok(),
        _ => None,
    }
}

impl NodeStyleVars {
    /// The style variables for `tv`, the theme the config resolved.
    /// `config` supplies the font family and size when the theme carries
    /// none; its own `fontFamily` default ends in a `;`, which the measurer
    /// treats as the page default exactly as Chrome does.
    pub fn from_theme(tv: &ThemeVariables, config: &MermaidConfig) -> NodeStyleVars {
        let base = NodeStyleVars::default();
        let color = |names: &[&str], fallback: Color| -> Color {
            names
                .iter()
                .find_map(|name| tv.str(name).and_then(parse_color))
                .unwrap_or(fallback)
        };
        let line_color = color(&["lineColor"], base.line_color);
        let text_color = color(&["textColor"], base.text_color);
        let main_bkg = color(&["mainBkg"], base.main_bkg);
        let node_border = color(&["nodeBorder", "border1"], base.node_border);
        let background = color(&["background"], base.background);
        NodeStyleVars {
            main_bkg,
            node_border,
            line_color,
            text_color,
            // `.label text, span { fill: nodeTextColor || textColor }`.
            node_text_color: color(&["nodeTextColor"], text_color),
            title_color: color(&["titleColor"], text_color),
            cluster_bkg: color(&["clusterBkg", "secondBkg"], base.cluster_bkg),
            cluster_border: color(&["clusterBorder", "border2"], base.cluster_border),
            edge_label_background: color(
                &["edgeLabelBackground", "labelBackground"],
                base.edge_label_background,
            ),
            note_bkg_color: color(&["noteBkgColor"], base.note_bkg_color),
            note_border_color: color(&["noteBorderColor", "border2"], base.note_border_color),
            note_text_color: color(&["noteTextColor", "actorTextColor"], base.note_text_color),
            background,
            composite_background: color(&["compositeBackground"], background),
            composite_title_background: color(&["compositeTitleBackground"], main_bkg),
            alt_background: color(&["altBackground"], base.alt_background),
            state_border: tv.str("stateBorder").and_then(parse_color),
            special_state_color: color(&["specialStateColor"], line_color),
            inner_end_background: color(&["innerEndBackground"], node_border),
            row_even: color(&["rowEven"], base.row_even),
            row_odd: color(&["rowOdd"], base.row_odd),
            radius: number(tv.get("radius")).unwrap_or(DEFAULT_RADIUS),
            stroke_width: number(tv.get("strokeWidth")).unwrap_or(DEFAULT_STROKE_WIDTH),
            font_family: tv
                .str("fontFamily")
                .filter(|f| !f.trim().is_empty())
                .unwrap_or(&config.font_family)
                .to_string(),
            font_size: number(tv.get("fontSize"))
                .or_else(|| Some(*config.font_size))
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or(DEFAULT_FONT_SIZE),
            config_font_family: config.font_family.clone(),
            config_font_size: *config.font_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::theme;
    use serde_json::Map;
    use std::path::PathBuf;

    fn golden(name: &str) -> Map<String, Value> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/goldens/themes")
            .join(name);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        match serde_json::from_str::<Value>(&text).expect("golden is JSON") {
            Value::Object(map) => map,
            other => panic!("{name}: expected an object, got {other}"),
        }
    }

    #[test]
    fn default_theme_matches_the_oracle_dump() {
        let dump = golden("default.json");
        let tv = ThemeVariables::from_json(&dump);
        let vars = NodeStyleVars::from_theme(&tv, &MermaidConfig::default());
        let hex = |s: &str| Color::from_hex(s).expect("hex");
        assert_eq!(vars.main_bkg, hex("#ECECFF"));
        assert_eq!(vars.node_border, hex("#9370DB"));
        assert_eq!(vars.line_color, hex("#333333"));
        assert_eq!(vars.text_color, hex("#333"));
        // `nodeTextColor` is not a theme variable; the stylesheet falls
        // back to `textColor`.
        assert_eq!(dump.get("nodeTextColor"), None);
        assert_eq!(vars.node_text_color, hex("#333"));
        assert_eq!(vars.title_color, hex("#333"));
        assert_eq!(vars.cluster_bkg, hex("#ffffde"));
        assert_eq!(vars.cluster_border, hex("#aaaa33"));
        assert_eq!(vars.edge_label_background, Color::rgba(232, 232, 232, 0.8));
        assert_eq!(vars.note_bkg_color, hex("#fff5ad"));
        assert_eq!(vars.note_border_color, hex("#aaaa33"));
        assert_eq!(vars.note_text_color, Color::BLACK);
        assert_eq!(vars.background, Color::WHITE);
        assert_eq!(vars.composite_background, Color::WHITE);
        assert_eq!(vars.composite_title_background, hex("#ECECFF"));
        assert_eq!(vars.alt_background, hex("#f0f0f0"));
        assert_eq!(dump.get("stateBorder"), None);
        assert_eq!(vars.state_border, None);
        assert_eq!(vars.special_state_color, hex("#333333"));
        assert_eq!(vars.inner_end_background, hex("#9370DB"));
        // `hsl(240, 100%, 97.2745098039%)` and `hsl(240, 100%, 100%)`.
        assert_eq!(vars.row_even, Color::rgb(241, 241, 255));
        assert_eq!(vars.row_odd, Color::WHITE);
        assert_eq!(vars.radius, 5.0);
        assert_eq!(vars.stroke_width, 1.0);
        assert_eq!(
            vars.font_family,
            "\"trebuchet ms\", verdana, arial, sans-serif"
        );
        assert_eq!(vars.font_size, 16.0);
    }

    #[test]
    fn every_color_the_shapes_read_parses_in_every_builtin_theme() {
        let names = [
            "mainBkg",
            "nodeBorder",
            "lineColor",
            "textColor",
            "titleColor",
            "clusterBkg",
            "clusterBorder",
            "edgeLabelBackground",
            "noteBkgColor",
            "noteBorderColor",
            "noteTextColor",
            "background",
            "compositeBackground",
            "compositeTitleBackground",
            "altBackground",
            "specialStateColor",
            "innerEndBackground",
            "rowEven",
            "rowOdd",
        ];
        for name in theme::BUILTIN_THEMES {
            let tv = theme::resolve(name, None).expect("theme resolves");
            for var in names {
                let value = tv
                    .str(var)
                    .unwrap_or_else(|| panic!("{name}: {var} missing"));
                assert!(
                    parse_color(value).is_some(),
                    "{name}: {var} = {value:?} does not parse"
                );
            }
        }
    }

    #[test]
    fn dark_theme_changes_the_node_colors() {
        let tv = theme::resolve("dark", None).expect("dark");
        let vars = NodeStyleVars::from_theme(&tv, &MermaidConfig::default());
        assert_ne!(vars.main_bkg, NodeStyleVars::default().main_bkg);
        assert_eq!(
            vars.main_bkg,
            parse_color(tv.str("mainBkg").expect("mainBkg")).expect("color")
        );
        assert_eq!(vars.font_size, 16.0);
    }

    #[test]
    fn parse_color_handles_hsl_transparent_and_keywords() {
        assert_eq!(parse_color("hsl(240, 100%, 100%)"), Some(Color::WHITE));
        assert_eq!(parse_color("transparent"), Some(Color::rgba(0, 0, 0, 0.0)));
        assert_eq!(parse_color("rebeccapurple"), Some(Color::rgb(102, 51, 153)));
        assert_eq!(parse_color("calculated"), None);
    }
}
