//! The icon pack `architectureIcons.ts` registers as
//! `mermaid-architecture`, plus iconify's `unknownIcon` that `getIconSVG`
//! falls back on. The bodies are mermaid's markup verbatim; a small reader
//! turns them into scene elements in the icon's 80 by 80 box.
//!
//! An icon name resolves as `getRegisteredIconData` does: a name without a
//! prefix takes the fallback prefix `mermaid-architecture`, a name in that
//! pack draws its body, and any other name in it draws the question mark.
//! A name with another prefix needs an icon pack, which the helper does
//! not support (R-112).

use crate::error::HelperError;
use crate::mermaid::scene::{
    Circle, Color, Common, Element, Ellipse, FontSpec, Group, Line, Paint, Path, PathData, Rect,
    Style, Text, TextRun, TransformOp,
};

/// `architectureIcons.prefix`.
pub const PREFIX: &str = "mermaid-architecture";
/// The icons' `width` and `height`.
pub const ICON_BOX: f64 = 80.0;

/// `wrapIcon`: the blue square behind every body.
const WRAP_RECT: &str =
    r#"<rect width="80" height="80" style="fill: #087ebf; stroke-width: 0px;"/>"#;

const DATABASE: &str = r#"<path id="b" data-name="4" d="m20,57.86c0,3.94,8.95,7.14,20,7.14s20-3.2,20-7.14" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><path id="c" data-name="3" d="m20,45.95c0,3.94,8.95,7.14,20,7.14s20-3.2,20-7.14" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><path id="d" data-name="2" d="m20,34.05c0,3.94,8.95,7.14,20,7.14s20-3.2,20-7.14" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><ellipse id="e" data-name="1" cx="40" cy="22.14" rx="20" ry="7.14" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><line x1="20" y1="57.86" x2="20" y2="22.14" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><line x1="60" y1="57.86" x2="60" y2="22.14" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/>"#;

const SERVER: &str = r#"<rect x="17.5" y="17.5" width="45" height="45" rx="2" ry="2" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><line x1="17.5" y1="32.5" x2="62.5" y2="32.5" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><line x1="17.5" y1="47.5" x2="62.5" y2="47.5" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><g><path d="m56.25,25c0,.27-.45.5-1,.5h-10.5c-.55,0-1-.23-1-.5s.45-.5,1-.5h10.5c.55,0,1,.23,1,.5Z" style="fill: #fff; stroke-width: 0px;"/><path d="m56.25,25c0,.27-.45.5-1,.5h-10.5c-.55,0-1-.23-1-.5s.45-.5,1-.5h10.5c.55,0,1,.23,1,.5Z" style="fill: none; stroke: #fff; stroke-miterlimit: 10;"/></g><g><path d="m56.25,40c0,.27-.45.5-1,.5h-10.5c-.55,0-1-.23-1-.5s.45-.5,1-.5h10.5c.55,0,1,.23,1,.5Z" style="fill: #fff; stroke-width: 0px;"/><path d="m56.25,40c0,.27-.45.5-1,.5h-10.5c-.55,0-1-.23-1-.5s.45-.5,1-.5h10.5c.55,0,1,.23,1,.5Z" style="fill: none; stroke: #fff; stroke-miterlimit: 10;"/></g><g><path d="m56.25,55c0,.27-.45.5-1,.5h-10.5c-.55,0-1-.23-1-.5s.45-.5,1-.5h10.5c.55,0,1,.23,1,.5Z" style="fill: #fff; stroke-width: 0px;"/><path d="m56.25,55c0,.27-.45.5-1,.5h-10.5c-.55,0-1-.23-1-.5s.45-.5,1-.5h10.5c.55,0,1,.23,1,.5Z" style="fill: none; stroke: #fff; stroke-miterlimit: 10;"/></g><g><circle cx="32.5" cy="25" r=".75" style="fill: #fff; stroke: #fff; stroke-miterlimit: 10;"/><circle cx="27.5" cy="25" r=".75" style="fill: #fff; stroke: #fff; stroke-miterlimit: 10;"/><circle cx="22.5" cy="25" r=".75" style="fill: #fff; stroke: #fff; stroke-miterlimit: 10;"/></g><g><circle cx="32.5" cy="40" r=".75" style="fill: #fff; stroke: #fff; stroke-miterlimit: 10;"/><circle cx="27.5" cy="40" r=".75" style="fill: #fff; stroke: #fff; stroke-miterlimit: 10;"/><circle cx="22.5" cy="40" r=".75" style="fill: #fff; stroke: #fff; stroke-miterlimit: 10;"/></g><g><circle cx="32.5" cy="55" r=".75" style="fill: #fff; stroke: #fff; stroke-miterlimit: 10;"/><circle cx="27.5" cy="55" r=".75" style="fill: #fff; stroke: #fff; stroke-miterlimit: 10;"/><circle cx="22.5" cy="55" r=".75" style="fill: #fff; stroke: #fff; stroke-miterlimit: 10;"/></g>"#;

const DISK: &str = r#"<rect x="20" y="15" width="40" height="50" rx="1" ry="1" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><ellipse cx="24" cy="19.17" rx=".8" ry=".83" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><ellipse cx="56" cy="19.17" rx=".8" ry=".83" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><ellipse cx="24" cy="60.83" rx=".8" ry=".83" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><ellipse cx="56" cy="60.83" rx=".8" ry=".83" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><ellipse cx="40" cy="33.75" rx="14" ry="14.58" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><ellipse cx="40" cy="33.75" rx="4" ry="4.17" style="fill: #fff; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><path d="m37.51,42.52l-4.83,13.22c-.26.71-1.1,1.02-1.76.64l-4.18-2.42c-.66-.38-.81-1.26-.33-1.84l9.01-10.8c.88-1.05,2.56-.08,2.09,1.2Z" style="fill: #fff; stroke-width: 0px;"/>"#;

const INTERNET: &str = r#"<circle cx="40" cy="40" r="22.5" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><line x1="40" y1="17.5" x2="40" y2="62.5" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><line x1="17.5" y1="40" x2="62.5" y2="40" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><path d="m39.99,17.51c-15.28,11.1-15.28,33.88,0,44.98" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><path d="m40.01,17.51c15.28,11.1,15.28,33.88,0,44.98" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><line x1="19.75" y1="30.1" x2="60.25" y2="30.1" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/><line x1="19.75" y1="49.9" x2="60.25" y2="49.9" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/>"#;

const CLOUD: &str = r#"<path d="m65,47.5c0,2.76-2.24,5-5,5H20c-2.76,0-5-2.24-5-5,0-1.87,1.03-3.51,2.56-4.36-.04-.21-.06-.42-.06-.64,0-2.6,2.48-4.74,5.65-4.97,1.65-4.51,6.34-7.76,11.85-7.76.86,0,1.69.08,2.5.23,2.09-1.57,4.69-2.5,7.5-2.5,6.1,0,11.19,4.38,12.28,10.17,2.14.56,3.72,2.51,3.72,4.83,0,.03,0,.07-.01.1,2.29.46,4.01,2.48,4.01,4.9Z" style="fill: none; stroke: #fff; stroke-miterlimit: 10; stroke-width: 2px;"/>"#;

/// The body of iconify's `unknownIcon`, a question mark on the same square.
const UNKNOWN: &str = r#"<g><rect width="80" height="80" style="fill: #087ebf; stroke-width: 0px;"/><text transform="translate(21.16 64.67)" style="fill: #fff; font-family: ArialMT, Arial; font-size: 67.75px;"><tspan x="0" y="0">?</tspan></text></g>"#;

/// The family and size of the `unknownIcon` question mark.
const UNKNOWN_FONT: &str = "ArialMT, Arial";
const UNKNOWN_FONT_SIZE: f64 = 67.75;

/// What an icon reference draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Database,
    Server,
    Disk,
    Internet,
    Cloud,
    Blank,
    /// `unknownIcon`: a name the pack does not have.
    Unknown,
}

/// `stringToIcon(name, true, true)` with iconify's name validation: an
/// optional `prefix:`, then a name of lowercase letters, digits, and
/// single hyphens.
fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Resolves an icon reference, or reports the icon pack it needs.
pub fn resolve(reference: &str) -> Result<Icon, HelperError> {
    let unsupported = || HelperError::UnsupportedFeature {
        feature: "icon pack".to_string(),
    };
    let (prefix, name) = match reference.rsplit_once(':') {
        Some((prefix, name)) => (prefix.trim_start_matches('@'), name),
        None => (PREFIX, reference),
    };
    // `@provider:prefix:name` keeps the last two parts.
    let prefix = prefix.rsplit(':').next().unwrap_or(prefix);
    if prefix != PREFIX {
        return Err(unsupported());
    }
    if !valid_name(name) {
        return Ok(Icon::Unknown);
    }
    Ok(match name {
        "database" => Icon::Database,
        "server" => Icon::Server,
        "disk" => Icon::Disk,
        "internet" => Icon::Internet,
        "cloud" => Icon::Cloud,
        "blank" => Icon::Blank,
        _ => Icon::Unknown,
    })
}

fn body(icon: Icon) -> String {
    let wrap = |inner: &str| format!("<g>{WRAP_RECT}{inner}</g>");
    match icon {
        Icon::Database => wrap(DATABASE),
        Icon::Server => wrap(SERVER),
        Icon::Disk => wrap(DISK),
        Icon::Internet => wrap(INTERNET),
        Icon::Cloud => wrap(CLOUD),
        Icon::Blank => wrap(""),
        Icon::Unknown => UNKNOWN.to_string(),
    }
}

/// The icon at `size` pixels, its top-left corner at the origin:
/// `getIconSVG(name, { width: size, height: size })`, a nested `<svg>`
/// with `viewBox="0 0 80 80"`, drawn as a scaled group.
pub fn elements(icon: Icon, size: f64) -> Element {
    let scale = size / ICON_BOX;
    let children = parse_markup(&body(icon));
    Element::Group(Group {
        common: Common {
            transform: vec![TransformOp::Scale(scale, scale)],
            ..Common::default()
        },
        children,
    })
}

// ---------------------------------------------------------------------------
// A reader for the icon markup above: elements, attributes, `style`.

struct Tag<'a> {
    name: &'a str,
    attrs: Vec<(&'a str, &'a str)>,
    self_closing: bool,
    closing: bool,
}

fn tags(markup: &str) -> Vec<(Tag<'_>, &str)> {
    let mut out = Vec::new();
    let mut rest = markup;
    while let Some(start) = rest.find('<') {
        let Some(end) = rest[start..].find('>') else {
            break;
        };
        let inner = &rest[start + 1..start + end];
        let after = &rest[start + end + 1..];
        let text = after.split('<').next().unwrap_or("");
        let closing = inner.starts_with('/');
        let self_closing = inner.ends_with('/');
        let inner = inner.trim_start_matches('/').trim_end_matches('/');
        let (name, attr_text) = inner.split_once(' ').unwrap_or((inner, ""));
        let mut attrs = Vec::new();
        let mut a = attr_text;
        while let Some(eq) = a.find("=\"") {
            let key = a[..eq].trim();
            let value_start = eq + 2;
            let Some(len) = a[value_start..].find('"') else {
                break;
            };
            attrs.push((key, &a[value_start..value_start + len]));
            a = &a[value_start + len + 1..];
        }
        out.push((
            Tag {
                name,
                attrs,
                self_closing,
                closing,
            },
            text,
        ));
        rest = after;
    }
    out
}

fn attr<'a>(tag: &Tag<'a>, key: &str) -> Option<&'a str> {
    tag.attrs.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

fn num(tag: &Tag<'_>, key: &str) -> f64 {
    attr(tag, key).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

fn paint(value: &str) -> Option<Paint> {
    let v = value.trim();
    if v == "none" {
        Some(Paint::None)
    } else {
        Color::from_hex(v).map(Paint::Color)
    }
}

fn style_of(tag: &Tag<'_>) -> Style {
    let mut style = Style::default();
    for decl in attr(tag, "style").unwrap_or("").split(';') {
        let Some((key, value)) = decl.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "fill" => style.fill = paint(value),
            "stroke" => style.stroke = paint(value),
            "stroke-width" => {
                style.stroke_width = value.trim_end_matches("px").parse().ok();
            }
            "font-size" => style.font.size = value.trim_end_matches("px").parse().ok(),
            "font-family" => style.font.family = Some(value.to_string()),
            _ => {}
        }
    }
    style
}

/// Turns the markup into elements; `g` nests, `text` holds one `tspan`.
fn parse_markup(markup: &str) -> Vec<Element> {
    let mut stack: Vec<Vec<Element>> = vec![Vec::new()];
    let mut pending_text: Option<Text> = None;
    for (tag, text_after) in tags(markup) {
        if tag.closing {
            match tag.name {
                "g" => {
                    let children = stack.pop().unwrap_or_default();
                    if let Some(parent) = stack.last_mut() {
                        parent.push(Element::Group(Group {
                            common: Common::default(),
                            children,
                        }));
                    }
                }
                "text" => {
                    if let (Some(text), Some(parent)) = (pending_text.take(), stack.last_mut()) {
                        parent.push(Element::Text(text));
                    }
                }
                _ => {}
            }
            continue;
        }
        let common = Common::with_style(style_of(&tag));
        let element = match tag.name {
            "g" if !tag.self_closing => {
                stack.push(Vec::new());
                None
            }
            "rect" => Some(Element::Rect(Rect {
                common,
                x: num(&tag, "x"),
                y: num(&tag, "y"),
                width: num(&tag, "width"),
                height: num(&tag, "height"),
                rx: attr(&tag, "rx").and_then(|v| v.parse().ok()),
                ry: attr(&tag, "ry").and_then(|v| v.parse().ok()),
            })),
            "line" => Some(Element::Line(Line {
                common,
                x1: num(&tag, "x1"),
                y1: num(&tag, "y1"),
                x2: num(&tag, "x2"),
                y2: num(&tag, "y2"),
            })),
            "circle" => Some(Element::Circle(Circle {
                common,
                cx: num(&tag, "cx"),
                cy: num(&tag, "cy"),
                r: num(&tag, "r"),
            })),
            "ellipse" => Some(Element::Ellipse(Ellipse {
                common,
                cx: num(&tag, "cx"),
                cy: num(&tag, "cy"),
                rx: num(&tag, "rx"),
                ry: num(&tag, "ry"),
            })),
            "path" => Some(Element::Path(Path {
                common,
                d: parse_path(attr(&tag, "d").unwrap_or("")),
            })),
            "text" => {
                let (tx, ty) = attr(&tag, "transform")
                    .and_then(|t| t.strip_prefix("translate("))
                    .and_then(|t| t.strip_suffix(')'))
                    .and_then(|t| {
                        let mut it = t.split([' ', ',']).filter(|s| !s.is_empty());
                        Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
                    })
                    .unwrap_or((0.0, 0.0));
                let mut text_common = common;
                text_common.transform = vec![TransformOp::Translate(tx, ty)];
                pending_text = Some(Text {
                    common: text_common,
                    x: 0.0,
                    y: 0.0,
                    anchor: None,
                    baseline: None,
                    font: FontSpec {
                        family: Some(UNKNOWN_FONT.to_string()),
                        size: Some(UNKNOWN_FONT_SIZE),
                        ..FontSpec::default()
                    },
                    runs: Vec::new(),
                });
                None
            }
            "tspan" => {
                if let Some(text) = pending_text.as_mut() {
                    text.runs.push(TextRun::new(text_after));
                }
                None
            }
            _ => None,
        };
        if let (Some(element), Some(parent)) = (element, stack.last_mut()) {
            parent.push(element);
        }
    }
    stack.pop().unwrap_or_default()
}

/// Splits SVG path data into command letters and numbers, where a sign or
/// a second decimal point starts a new number (`.27-.45.5-1`).
fn path_tokens(d: &str) -> Vec<PathToken> {
    let chars: Vec<char> = d.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphabetic() && c != 'e' && c != 'E' {
            out.push(PathToken::Command(c));
            i += 1;
        } else if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' {
            let start = i;
            if c == '-' || c == '+' {
                i += 1;
            }
            let mut seen_dot = false;
            while i < chars.len() {
                let ch = chars[i];
                if ch.is_ascii_digit() {
                    i += 1;
                } else if ch == '.' && !seen_dot {
                    seen_dot = true;
                    i += 1;
                } else if (ch == 'e' || ch == 'E') && i > start {
                    i += 1;
                    if i < chars.len() && (chars[i] == '-' || chars[i] == '+') {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            let s: String = chars[start..i].iter().collect();
            out.push(PathToken::Number(s.parse().unwrap_or(0.0)));
        } else {
            i += 1;
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PathToken {
    Command(char),
    Number(f64),
}

/// Path data with relative and absolute `M L H V C S Q Z` commands, made
/// absolute.
pub fn parse_path(d: &str) -> PathData {
    let tokens = path_tokens(d);
    let mut path = PathData::new();
    let mut i = 0;
    let mut cmd = 'M';
    let (mut cx, mut cy) = (0.0, 0.0);
    let (mut sx, mut sy) = (0.0, 0.0);
    let mut last_control: Option<(f64, f64)> = None;
    let nums = |i: &mut usize, n: usize| -> Option<Vec<f64>> {
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            match tokens.get(*i) {
                Some(PathToken::Number(v)) => {
                    out.push(*v);
                    *i += 1;
                }
                _ => return None,
            }
        }
        Some(out)
    };
    while i < tokens.len() {
        if let PathToken::Command(c) = tokens[i] {
            cmd = c;
            i += 1;
            if c == 'Z' || c == 'z' {
                path.close();
                cx = sx;
                cy = sy;
                last_control = None;
                continue;
            }
        }
        let relative = cmd.is_ascii_lowercase();
        let (ox, oy) = if relative { (cx, cy) } else { (0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            'M' => {
                let Some(v) = nums(&mut i, 2) else { break };
                cx = ox + v[0];
                cy = oy + v[1];
                sx = cx;
                sy = cy;
                path.move_to(cx, cy);
                // Further pairs are line-tos.
                cmd = if relative { 'l' } else { 'L' };
                last_control = None;
            }
            'L' => {
                let Some(v) = nums(&mut i, 2) else { break };
                cx = ox + v[0];
                cy = oy + v[1];
                path.line_to(cx, cy);
                last_control = None;
            }
            'H' => {
                let Some(v) = nums(&mut i, 1) else { break };
                cx = ox + v[0];
                path.line_to(cx, cy);
                last_control = None;
            }
            'V' => {
                let Some(v) = nums(&mut i, 1) else { break };
                cy = oy + v[0];
                path.line_to(cx, cy);
                last_control = None;
            }
            'C' => {
                let Some(v) = nums(&mut i, 6) else { break };
                let (x1, y1, x2, y2) = (ox + v[0], oy + v[1], ox + v[2], oy + v[3]);
                cx = ox + v[4];
                cy = oy + v[5];
                path.cubic_to(x1, y1, x2, y2, cx, cy);
                last_control = Some((x2, y2));
            }
            'S' => {
                let Some(v) = nums(&mut i, 4) else { break };
                let (x1, y1) = match last_control {
                    Some((px, py)) => (2.0 * cx - px, 2.0 * cy - py),
                    None => (cx, cy),
                };
                let (x2, y2) = (ox + v[0], oy + v[1]);
                cx = ox + v[2];
                cy = oy + v[3];
                path.cubic_to(x1, y1, x2, y2, cx, cy);
                last_control = Some((x2, y2));
            }
            'Q' => {
                let Some(v) = nums(&mut i, 4) else { break };
                let (x1, y1) = (ox + v[0], oy + v[1]);
                cx = ox + v[2];
                cy = oy + v[3];
                path.quad_to(x1, y1, cx, cy);
                last_control = None;
            }
            _ => break,
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_numbers_split_on_signs_and_second_dots() {
        let t = path_tokens("c0,.27-.45.5-1,.5");
        assert_eq!(
            t,
            vec![
                PathToken::Command('c'),
                PathToken::Number(0.0),
                PathToken::Number(0.27),
                PathToken::Number(-0.45),
                PathToken::Number(0.5),
                PathToken::Number(-1.0),
                PathToken::Number(0.5),
            ]
        );
    }

    #[test]
    fn names_resolve_like_get_registered_icon_data() {
        assert_eq!(resolve("database").unwrap(), Icon::Database);
        assert_eq!(resolve("mermaid-architecture:cloud").unwrap(), Icon::Cloud);
        assert_eq!(resolve("nope").unwrap(), Icon::Unknown);
        assert_eq!(resolve("Server").unwrap(), Icon::Unknown);
        assert!(matches!(
            resolve("logos:aws-s3"),
            Err(HelperError::UnsupportedFeature { .. })
        ));
    }

    #[test]
    fn every_icon_parses() {
        for icon in [
            Icon::Database,
            Icon::Server,
            Icon::Disk,
            Icon::Internet,
            Icon::Cloud,
            Icon::Blank,
            Icon::Unknown,
        ] {
            let Element::Group(g) = elements(icon, 80.0) else {
                panic!("not a group");
            };
            assert!(!g.children.is_empty(), "{icon:?}");
        }
    }
}
