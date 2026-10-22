//! `diagrams/git/styles.js` (`getStyles`, `normalTheme`) resolved to
//! attribute values.
//!
//! With the base config (`look` `classic`, one of the five built-in themes)
//! `getStyles` takes the `normalTheme` branch: `.commit<i>`,
//! `.commit-highlight<i>`, `.label<i>`, `.arrow<i>`, and `.branch-label<i>`
//! read `git<i>`, `gitInv<i>`, and `gitBranchLabel<i>` for `i` in `0..8`.
//! The `neo` and `redux` branches are unreachable (R-175). The scene
//! carries no CSS, so the renderer sets the resolved values on each element.
//! A declaration the browser would reject falls back the way the browser
//! does: an invalid fill paints black, an invalid stroke is no stroke.

use serde_json::Value;

use crate::mermaid::scene::Color;
use crate::mermaid::styles::pie::{css_color, css_length};
use crate::mermaid::theme::ThemeVariables;
use crate::mermaid::theme::khroma;

/// `THEME_COLOR_LIMIT` in `gitGraphRenderer.ts`; the renderer wraps every
/// branch index into `0..8`.
pub const COLOR_LIMIT: usize = 8;

/// The root `font-size` of a mermaid SVG, inherited by text without a rule.
const ROOT_FONT_SIZE: f64 = 16.0;
/// `.gitTitleText { font-size: 18px }`.
pub const TITLE_FONT_SIZE: f64 = 18.0;

#[derive(Debug, Clone, PartialEq)]
pub struct GitGraphStyles {
    /// The theme `fontFamily`, which the root style and the label rules
    /// both resolve to.
    pub font_family: String,
    /// `git0..git7`: commit bullet fill and stroke, branch label
    /// background, arrow stroke.
    pub git: [Option<Color>; COLOR_LIMIT],
    /// `gitInv0..gitInv7`: the outer square of a highlight commit.
    pub git_inv: [Option<Color>; COLOR_LIMIT],
    /// `gitBranchLabel0..gitBranchLabel7`: the branch label text.
    pub branch_label: [Option<Color>; COLOR_LIMIT],
    /// `.branch` stroke: `commitLineColor ?? lineColor`.
    pub branch_line: Option<Color>,
    /// `.branch` stroke width: the theme `strokeWidth`.
    pub branch_stroke_width: f64,
    pub commit_label_color: Option<Color>,
    pub commit_label_background: Option<Color>,
    pub commit_label_font_size: f64,
    pub tag_label_color: Option<Color>,
    pub tag_label_background: Option<Color>,
    pub tag_label_border: Option<Color>,
    pub tag_label_font_size: f64,
    /// `.tag-hole` fill and `.gitTitleText` fill.
    pub text_color: Option<Color>,
    /// `.commit-merge`, `.commit-reverse`, `.commit-highlight-inner`.
    pub primary_color: Option<Color>,
}

impl GitGraphStyles {
    /// `getStyles(themeVariables)` for the classic look. `fallback_font` is
    /// the config `fontFamily` for a theme without a `fontFamily` variable.
    pub fn resolve(tv: &ThemeVariables, fallback_font: &str) -> GitGraphStyles {
        let color = |name: &str| css_value(tv, name).and_then(|v| css_color(&v));
        let size = |name: &str| {
            css_value(tv, name)
                .and_then(|v| css_length(&v))
                .unwrap_or(ROOT_FONT_SIZE)
        };
        let mut git = [None; COLOR_LIMIT];
        let mut git_inv = [None; COLOR_LIMIT];
        let mut branch_label = [None; COLOR_LIMIT];
        for i in 0..COLOR_LIMIT {
            git[i] = color(&format!("git{i}"));
            git_inv[i] = color(&format!("gitInv{i}"));
            branch_label[i] = color(&format!("gitBranchLabel{i}"));
        }
        GitGraphStyles {
            font_family: tv
                .str("fontFamily")
                .filter(|f| !f.trim().is_empty())
                .unwrap_or(fallback_font)
                .to_string(),
            git,
            git_inv,
            branch_label,
            branch_line: color("commitLineColor").or_else(|| color("lineColor")),
            branch_stroke_width: css_value(tv, "strokeWidth")
                .and_then(|v| css_length(&v))
                .unwrap_or(1.0),
            commit_label_color: color("commitLabelColor"),
            commit_label_background: color("commitLabelBackground"),
            commit_label_font_size: size("commitLabelFontSize"),
            tag_label_color: color("tagLabelColor"),
            tag_label_background: color("tagLabelBackground"),
            tag_label_border: color("tagLabelBorder"),
            tag_label_font_size: size("tagLabelFontSize"),
            text_color: color("textColor"),
            primary_color: color("primaryColor"),
        }
    }

    /// The color class index for a branch index: `calcColorIndex` without
    /// the redux-color exception.
    pub fn index(branch_index: usize) -> usize {
        branch_index % COLOR_LIMIT
    }
}

/// The text a theme variable interpolates into the stylesheet: a string as
/// it is, a number the way JS prints it, nothing for anything else.
fn css_value(tv: &ThemeVariables, name: &str) -> Option<String> {
    match tv.get(name) {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => n.as_f64().map(khroma::js_number),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::theme;

    #[test]
    fn default_theme_resolves_to_the_oracle_stylesheet() {
        let tv = theme::resolve("default", None).unwrap();
        let s = GitGraphStyles::resolve(&tv, "serif");
        assert_eq!(
            s.font_family,
            "\"trebuchet ms\", verdana, arial, sans-serif"
        );
        // `git0` is `primaryColor`, `#ECECFF`.
        assert!(s.git[0].is_some());
        assert!(s.git.iter().all(Option::is_some));
        assert!(s.git_inv.iter().all(Option::is_some));
        assert!(s.branch_label.iter().all(Option::is_some));
        assert_eq!(s.branch_line, Some(Color::rgb(0x33, 0x33, 0x33)));
        assert_eq!(s.branch_stroke_width, 1.0);
        assert_eq!(s.commit_label_font_size, 10.0);
        assert_eq!(s.tag_label_font_size, 10.0);
        assert_eq!(s.primary_color, Some(Color::rgb(0xEC, 0xEC, 0xFF)));
        assert_eq!(s.text_color, Some(Color::rgb(0x33, 0x33, 0x33)));
        assert_eq!(GitGraphStyles::index(9), 1);
    }
}
