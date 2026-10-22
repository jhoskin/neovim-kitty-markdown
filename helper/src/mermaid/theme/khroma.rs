//! Port of the khroma color library (2.1.0), the dependency mermaid's themes
//! use for every derived color.
//!
//! The port keeps khroma's quirks because mermaid's theme output depends on
//! them: the lazy RGB/HSL channel conversion, the `changed` flag that makes
//! `stringify` return the parsed input verbatim, the `h % 360` clamp that
//! keeps negative hues, the keyword table with `cyanaqua` instead of `cyan`,
//! and the `toLinear` comparison against the 0..255 value. Every function
//! that khroma exports returns a string, so the theme code can store the
//! result as a theme variable as mermaid does.

use std::fmt;

use regex::Regex;
use std::sync::LazyLock;

/// `src/utils/channel.ts`: the maximum of a channel, used by `clamp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    R,
    G,
    B,
    H,
    S,
    L,
    A,
}

impl Channel {
    pub fn from_key(key: &str) -> Option<Channel> {
        Some(match key {
            "r" => Channel::R,
            "g" => Channel::G,
            "b" => Channel::B,
            "h" => Channel::H,
            "s" => Channel::S,
            "l" => Channel::L,
            "a" => Channel::A,
            _ => return None,
        })
    }

    fn is_rgb(self) -> bool {
        matches!(self, Channel::R | Channel::G | Channel::B)
    }

    fn is_hsl(self) -> bool {
        matches!(self, Channel::H | Channel::S | Channel::L)
    }

    /// `Channel.clamp` in `src/utils/channel.ts`. The hue uses the JS `%`,
    /// which keeps the sign, so `adjust(c, { h: -120 })` can yield a negative
    /// hue in the output string.
    pub fn clamp(self, v: f64) -> f64 {
        // khroma writes `v >= max ? max : (v < 0 ? 0 : v)`, which hands a NaN
        // through unchanged; `f64::clamp` does the same.
        match self {
            Channel::R | Channel::G | Channel::B => v.clamp(0.0, 255.0),
            Channel::H => v % 360.0,
            Channel::S | Channel::L => v.clamp(0.0, 100.0),
            Channel::A => v.clamp(0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KhromaError {
    /// `Color.parse` on a string no format recognizes.
    UnsupportedFormat(String),
    /// `Type.set` when a color changes RGB and HSL channels in one call.
    MixedChannels,
    /// khroma received something that is not a string. In JS this ends in a
    /// `TypeError` deep inside the library; the message names the value.
    NotAColor(String),
}

impl fmt::Display for KhromaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KhromaError::UnsupportedFormat(c) => write!(f, "Unsupported color format: \"{c}\""),
            KhromaError::MixedChannels => {
                write!(
                    f,
                    "Cannot change both RGB and HSL channels at the same time"
                )
            }
            KhromaError::NotAColor(v) => write!(f, "Cannot read properties of {v} (not a color)"),
        }
    }
}

impl std::error::Error for KhromaError {}

pub type Result<T> = std::result::Result<T, KhromaError>;

/// `TYPE` in `src/constants.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChannelType {
    All,
    Rgb,
    Hsl,
}

/// `Channels` in `src/channels/index.ts`: the parsed color. Channels that
/// were not parsed stay `None` until a getter derives them from the others.
#[derive(Debug, Clone, PartialEq)]
pub struct Channels {
    r: Option<f64>,
    g: Option<f64>,
    b: Option<f64>,
    h: Option<f64>,
    s: Option<f64>,
    l: Option<f64>,
    a: f64,
    ty: ChannelType,
    changed: bool,
    /// The input string, returned by `stringify` while nothing changed.
    color: Option<String>,
}

impl Channels {
    fn rgba(r: f64, g: f64, b: f64, a: f64, color: Option<String>) -> Channels {
        Channels {
            r: Some(r),
            g: Some(g),
            b: Some(b),
            h: None,
            s: None,
            l: None,
            a,
            ty: ChannelType::All,
            changed: false,
            color,
        }
    }

    fn hsla(h: f64, s: f64, l: f64, a: f64, color: Option<String>) -> Channels {
        Channels {
            r: None,
            g: None,
            b: None,
            h: Some(h),
            s: Some(s),
            l: Some(l),
            a,
            ty: ChannelType::All,
            changed: false,
            color,
        }
    }

    /// `_ensureHSL`: fills the missing HSL channels from the RGB data.
    fn ensure_hsl(&mut self) {
        let (r, g, b) = (
            self.r.unwrap_or(f64::NAN),
            self.g.unwrap_or(f64::NAN),
            self.b.unwrap_or(f64::NAN),
        );
        if self.h.is_none() {
            self.h = Some(rgb2hsl(r, g, b, Channel::H));
        }
        if self.s.is_none() {
            self.s = Some(rgb2hsl(r, g, b, Channel::S));
        }
        if self.l.is_none() {
            self.l = Some(rgb2hsl(r, g, b, Channel::L));
        }
    }

    /// `_ensureRGB`: fills the missing RGB channels from the HSL data.
    fn ensure_rgb(&mut self) {
        let (h, s, l) = (
            self.h.unwrap_or(f64::NAN),
            self.s.unwrap_or(f64::NAN),
            self.l.unwrap_or(f64::NAN),
        );
        if self.r.is_none() {
            self.r = Some(hsl2rgb(h, s, l, Channel::R));
        }
        if self.g.is_none() {
            self.g = Some(hsl2rgb(h, s, l, Channel::G));
        }
        if self.b.is_none() {
            self.b = Some(hsl2rgb(h, s, l, Channel::B));
        }
    }

    /// The getters of `Channels`. An RGB channel is trusted unless the color
    /// was changed through HSL, and the other way round; the derived value is
    /// computed on every read and never stored, as in khroma.
    pub fn get(&mut self, ch: Channel) -> f64 {
        match ch {
            Channel::A => self.a,
            Channel::R | Channel::G | Channel::B => {
                let stored = match ch {
                    Channel::R => self.r,
                    Channel::G => self.g,
                    _ => self.b,
                };
                if self.ty != ChannelType::Hsl
                    && let Some(v) = stored
                {
                    return v;
                }
                self.ensure_hsl();
                hsl2rgb(
                    self.h.unwrap_or(f64::NAN),
                    self.s.unwrap_or(f64::NAN),
                    self.l.unwrap_or(f64::NAN),
                    ch,
                )
            }
            Channel::H | Channel::S | Channel::L => {
                let stored = match ch {
                    Channel::H => self.h,
                    Channel::S => self.s,
                    _ => self.l,
                };
                if self.ty != ChannelType::Rgb
                    && let Some(v) = stored
                {
                    return v;
                }
                self.ensure_rgb();
                rgb2hsl(
                    self.r.unwrap_or(f64::NAN),
                    self.g.unwrap_or(f64::NAN),
                    self.b.unwrap_or(f64::NAN),
                    ch,
                )
            }
        }
    }

    /// The setters of `Channels`, with `Type.set`'s guard against mixing
    /// RGB and HSL changes.
    pub fn set(&mut self, ch: Channel, v: f64) -> Result<()> {
        if ch.is_rgb() {
            self.set_type(ChannelType::Rgb)?;
        } else if ch.is_hsl() {
            self.set_type(ChannelType::Hsl)?;
        }
        self.changed = true;
        match ch {
            Channel::R => self.r = Some(v),
            Channel::G => self.g = Some(v),
            Channel::B => self.b = Some(v),
            Channel::H => self.h = Some(v),
            Channel::S => self.s = Some(v),
            Channel::L => self.l = Some(v),
            Channel::A => self.a = v,
        }
        Ok(())
    }

    fn set_type(&mut self, ty: ChannelType) -> Result<()> {
        if self.ty != ChannelType::All && self.ty != ty {
            return Err(KhromaError::MixedChannels);
        }
        self.ty = ty;
        Ok(())
    }
}

/// `Channel.hue2rgb` in `src/utils/channel.ts`.
fn hue2rgb(p: f64, q: f64, t: f64) -> f64 {
    let mut t = t;
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

/// `Channel.hsl2rgb`. `!s` in JS is true for 0 and NaN.
fn hsl2rgb(h: f64, s: f64, l: f64, ch: Channel) -> f64 {
    if s == 0.0 || s.is_nan() {
        return l * 2.55;
    }
    let h = h / 360.0;
    let s = s / 100.0;
    let l = l / 100.0;
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        (l + s) - (l * s)
    };
    let p = 2.0 * l - q;
    match ch {
        Channel::R => hue2rgb(p, q, h + 1.0 / 3.0) * 255.0,
        Channel::G => hue2rgb(p, q, h) * 255.0,
        _ => hue2rgb(p, q, h - 1.0 / 3.0) * 255.0,
    }
}

/// `Channel.rgb2hsl`.
fn rgb2hsl(r: f64, g: f64, b: f64, ch: Channel) -> f64 {
    let r = r / 255.0;
    let g = g / 255.0;
    let b = b / 255.0;
    let max = js_max(js_max(r, g), b);
    let min = js_min(js_min(r, g), b);
    let l = (max + min) / 2.0;
    if ch == Channel::L {
        return l * 100.0;
    }
    if max == min {
        return 0.0;
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    if ch == Channel::S {
        return s * 100.0;
    }
    if max == r {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) * 60.0
    } else if max == g {
        ((b - r) / d + 2.0) * 60.0
    } else if max == b {
        ((r - g) / d + 4.0) * 60.0
    } else {
        -1.0
    }
}

/// `Math.max` propagates NaN, unlike `f64::max`.
fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

/// `Math.round`: halves round toward positive infinity, so `-2.5` gives
/// `-2` where Rust's `round` gives `-3`.
pub fn js_round(x: f64) -> f64 {
    if x.is_nan() || x.is_infinite() {
        return x;
    }
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

/// `_.lang.round`: rounding to 10 decimal digits.
pub fn lang_round(x: f64) -> f64 {
    js_round(x * 10000000000.0) / 10000000000.0
}

/// JS `Number.prototype.toString()` for the values khroma prints: the
/// shortest round-trip digits, no trailing `.0`, exponent form below 1e-6
/// and from 1e21, and `0` for negative zero.
pub fn js_number(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if x == 0.0 {
        return "0".to_string();
    }
    let abs = x.abs();
    if !(1e-6..1e21).contains(&abs) {
        let s = format!("{x:e}");
        return match s.split_once('e') {
            Some((mantissa, exp)) if !exp.starts_with('-') => format!("{mantissa}e+{exp}"),
            _ => s,
        };
    }
    if x.fract() == 0.0 {
        return format!("{x:.0}");
    }
    format!("{x}")
}

/// JS `Number.isInteger`.
fn is_integer(x: f64) -> bool {
    x.is_finite() && x.fract() == 0.0
}

/// `_.unit.dec2hex`: two lowercase hex digits of the rounded value.
fn dec2hex(dec: f64) -> String {
    let n = js_round(dec);
    // `Math.round(dec).toString(16)` on a value that `clamp` kept in
    // 0..=255. The `as` cast saturates, so no out-of-range value can escape
    // the two-digit format.
    let hex = format!("{:x}", n as i64);
    if hex.len() > 1 {
        hex
    } else {
        format!("0{hex}")
    }
}

/// JS `\s`: ASCII whitespace plus the Unicode space separators, the line
/// terminators, and the BOM.
pub fn js_is_whitespace(c: char) -> bool {
    matches!(
        c,
        ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// The class JS `\s` matches, as a regex fragment.
pub const JS_WHITESPACE: &str = r"[ \t\n\x0B\x0C\r   -     　﻿]";

const NUMBER: &str = r"(-?(?:[0-9]+(?:\.[0-9]+)?|(?:\.[0-9]+))(?:e[0-9]+)?(%?))";
const HSL_NUMBER: &str = r"-?(?:[0-9]+(?:\.[0-9]+)?|(?:\.[0-9]+))(?:e-?[0-9]+)?";

static HEX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^#((?:[a-f0-9]{2}){2,4}|[a-f0-9]{3})$").expect("valid regex")
});

/// `RGB.re` in `src/color/rgb.ts`.
static RGB_RE: LazyLock<Regex> = LazyLock::new(|| {
    let ws = JS_WHITESPACE;
    let n = NUMBER;
    Regex::new(&format!(
        r"(?i)^rgba?\({ws}*?{n}{ws}*?(?:,|{ws}){ws}*?{n}{ws}*?(?:,|{ws}){ws}*?{n}(?:{ws}*?(?:,|/){ws}*?\+?{n})?{ws}*?\)$"
    ))
    .expect("valid regex")
});

/// `HSL.re` in `src/color/hsl.ts`.
static HSL_RE: LazyLock<Regex> = LazyLock::new(|| {
    let ws = JS_WHITESPACE;
    let n = HSL_NUMBER;
    Regex::new(&format!(
        r"(?i)^hsla?\({ws}*?({n}(?:deg|grad|rad|turn)?){ws}*?(?:,|{ws}){ws}*?({n}%){ws}*?(?:,|{ws}){ws}*?({n}%)(?:{ws}*?(?:,|/){ws}*?\+?({n}(%)?))?{ws}*?\)$"
    ))
    .expect("valid regex")
});

static HUE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(.+?)(deg|grad|rad|turn)$").expect("valid regex"));

/// JS `parseFloat` on a string the color regexes already validated as a
/// numeral, so a plain parse is enough; the unit suffix of a hue is cut by
/// the caller.
fn parse_float(s: &str) -> f64 {
    let digits: String = s
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
        .collect();
    digits.parse().unwrap_or(f64::NAN)
}

/// `Hex.parse`.
fn parse_hex(color: &str) -> Option<Channels> {
    if !color.starts_with('#') {
        return None;
    }
    let caps = HEX_RE.captures(color)?;
    let hex = &caps[1];
    let dec = u32::from_str_radix(hex, 16).ok()?;
    let length = hex.len();
    let has_alpha = length % 4 == 0;
    let is_full_length = length > 4;
    let multiplier = if is_full_length { 1.0 } else { 17.0 };
    let bits: i32 = if is_full_length { 8 } else { 4 };
    let bits_offset: i32 = if has_alpha { 0 } else { -1 };
    let mask: u32 = if is_full_length { 255 } else { 15 };
    let shift = |n: i32| -> u32 {
        let by = bits * (bits_offset + n);
        if by <= 0 { dec } else { dec >> by }
    };
    let r = f64::from(shift(3) & mask) * multiplier;
    let g = f64::from(shift(2) & mask) * multiplier;
    let b = f64::from(shift(1) & mask) * multiplier;
    let a = if has_alpha {
        f64::from(dec & mask) * multiplier / 255.0
    } else {
        1.0
    };
    Some(Channels::rgba(r, g, b, a, Some(color.to_string())))
}

/// `RGB.parse`.
fn parse_rgb(color: &str) -> Option<Channels> {
    if !color.starts_with(['r', 'R']) {
        return None;
    }
    let caps = RGB_RE.captures(color)?;
    let chan = |i: usize, ch: Channel| -> f64 {
        let v = parse_float(&caps[i]);
        let pct = !caps[i + 1].is_empty();
        ch.clamp(if pct { v * 2.55 } else { v })
    };
    let a = match caps.get(7) {
        Some(m) => {
            let v = parse_float(m.as_str());
            let pct = caps.get(8).is_some_and(|p| !p.as_str().is_empty());
            Channel::A.clamp(if pct { v / 100.0 } else { v })
        }
        None => 1.0,
    };
    Some(Channels::rgba(
        chan(1, Channel::R),
        chan(3, Channel::G),
        chan(5, Channel::B),
        a,
        Some(color.to_string()),
    ))
}

/// `HSL._hue2deg`.
fn hue2deg(hue: &str) -> f64 {
    if let Some(m) = HUE_RE.captures(hue) {
        let number = parse_float(&m[1]);
        match m[2].to_ascii_lowercase().as_str() {
            "grad" => return Channel::H.clamp(number * 0.9),
            "rad" => return Channel::H.clamp(number * 180.0 / std::f64::consts::PI),
            "turn" => return Channel::H.clamp(number * 360.0),
            _ => {}
        }
    }
    Channel::H.clamp(parse_float(hue))
}

/// `HSL.parse`.
fn parse_hsl(color: &str) -> Option<Channels> {
    if !color.starts_with(['h', 'H']) {
        return None;
    }
    let caps = HSL_RE.captures(color)?;
    let h = hue2deg(&caps[1]);
    let s = Channel::S.clamp(parse_float(&caps[2]));
    let l = Channel::L.clamp(parse_float(&caps[3]));
    let a = match caps.get(4) {
        Some(m) => {
            let v = parse_float(m.as_str());
            let pct = caps.get(5).is_some();
            Channel::A.clamp(if pct { v / 100.0 } else { v })
        }
        None => 1.0,
    };
    Some(Channels::hsla(h, s, l, a, Some(color.to_string())))
}

/// `Keyword.colors` in `src/color/keyword.ts`, verbatim: the table has
/// `cyanaqua` and no `cyan`.
const KEYWORDS: &[(&str, &str)] = &[
    ("aliceblue", "#f0f8ff"),
    ("antiquewhite", "#faebd7"),
    ("aqua", "#00ffff"),
    ("aquamarine", "#7fffd4"),
    ("azure", "#f0ffff"),
    ("beige", "#f5f5dc"),
    ("bisque", "#ffe4c4"),
    ("black", "#000000"),
    ("blanchedalmond", "#ffebcd"),
    ("blue", "#0000ff"),
    ("blueviolet", "#8a2be2"),
    ("brown", "#a52a2a"),
    ("burlywood", "#deb887"),
    ("cadetblue", "#5f9ea0"),
    ("chartreuse", "#7fff00"),
    ("chocolate", "#d2691e"),
    ("coral", "#ff7f50"),
    ("cornflowerblue", "#6495ed"),
    ("cornsilk", "#fff8dc"),
    ("crimson", "#dc143c"),
    ("cyanaqua", "#00ffff"),
    ("darkblue", "#00008b"),
    ("darkcyan", "#008b8b"),
    ("darkgoldenrod", "#b8860b"),
    ("darkgray", "#a9a9a9"),
    ("darkgreen", "#006400"),
    ("darkgrey", "#a9a9a9"),
    ("darkkhaki", "#bdb76b"),
    ("darkmagenta", "#8b008b"),
    ("darkolivegreen", "#556b2f"),
    ("darkorange", "#ff8c00"),
    ("darkorchid", "#9932cc"),
    ("darkred", "#8b0000"),
    ("darksalmon", "#e9967a"),
    ("darkseagreen", "#8fbc8f"),
    ("darkslateblue", "#483d8b"),
    ("darkslategray", "#2f4f4f"),
    ("darkslategrey", "#2f4f4f"),
    ("darkturquoise", "#00ced1"),
    ("darkviolet", "#9400d3"),
    ("deeppink", "#ff1493"),
    ("deepskyblue", "#00bfff"),
    ("dimgray", "#696969"),
    ("dimgrey", "#696969"),
    ("dodgerblue", "#1e90ff"),
    ("firebrick", "#b22222"),
    ("floralwhite", "#fffaf0"),
    ("forestgreen", "#228b22"),
    ("fuchsia", "#ff00ff"),
    ("gainsboro", "#dcdcdc"),
    ("ghostwhite", "#f8f8ff"),
    ("gold", "#ffd700"),
    ("goldenrod", "#daa520"),
    ("gray", "#808080"),
    ("green", "#008000"),
    ("greenyellow", "#adff2f"),
    ("grey", "#808080"),
    ("honeydew", "#f0fff0"),
    ("hotpink", "#ff69b4"),
    ("indianred", "#cd5c5c"),
    ("indigo", "#4b0082"),
    ("ivory", "#fffff0"),
    ("khaki", "#f0e68c"),
    ("lavender", "#e6e6fa"),
    ("lavenderblush", "#fff0f5"),
    ("lawngreen", "#7cfc00"),
    ("lemonchiffon", "#fffacd"),
    ("lightblue", "#add8e6"),
    ("lightcoral", "#f08080"),
    ("lightcyan", "#e0ffff"),
    ("lightgoldenrodyellow", "#fafad2"),
    ("lightgray", "#d3d3d3"),
    ("lightgreen", "#90ee90"),
    ("lightgrey", "#d3d3d3"),
    ("lightpink", "#ffb6c1"),
    ("lightsalmon", "#ffa07a"),
    ("lightseagreen", "#20b2aa"),
    ("lightskyblue", "#87cefa"),
    ("lightslategray", "#778899"),
    ("lightslategrey", "#778899"),
    ("lightsteelblue", "#b0c4de"),
    ("lightyellow", "#ffffe0"),
    ("lime", "#00ff00"),
    ("limegreen", "#32cd32"),
    ("linen", "#faf0e6"),
    ("magenta", "#ff00ff"),
    ("maroon", "#800000"),
    ("mediumaquamarine", "#66cdaa"),
    ("mediumblue", "#0000cd"),
    ("mediumorchid", "#ba55d3"),
    ("mediumpurple", "#9370db"),
    ("mediumseagreen", "#3cb371"),
    ("mediumslateblue", "#7b68ee"),
    ("mediumspringgreen", "#00fa9a"),
    ("mediumturquoise", "#48d1cc"),
    ("mediumvioletred", "#c71585"),
    ("midnightblue", "#191970"),
    ("mintcream", "#f5fffa"),
    ("mistyrose", "#ffe4e1"),
    ("moccasin", "#ffe4b5"),
    ("navajowhite", "#ffdead"),
    ("navy", "#000080"),
    ("oldlace", "#fdf5e6"),
    ("olive", "#808000"),
    ("olivedrab", "#6b8e23"),
    ("orange", "#ffa500"),
    ("orangered", "#ff4500"),
    ("orchid", "#da70d6"),
    ("palegoldenrod", "#eee8aa"),
    ("palegreen", "#98fb98"),
    ("paleturquoise", "#afeeee"),
    ("palevioletred", "#db7093"),
    ("papayawhip", "#ffefd5"),
    ("peachpuff", "#ffdab9"),
    ("peru", "#cd853f"),
    ("pink", "#ffc0cb"),
    ("plum", "#dda0dd"),
    ("powderblue", "#b0e0e6"),
    ("purple", "#800080"),
    ("rebeccapurple", "#663399"),
    ("red", "#ff0000"),
    ("rosybrown", "#bc8f8f"),
    ("royalblue", "#4169e1"),
    ("saddlebrown", "#8b4513"),
    ("salmon", "#fa8072"),
    ("sandybrown", "#f4a460"),
    ("seagreen", "#2e8b57"),
    ("seashell", "#fff5ee"),
    ("sienna", "#a0522d"),
    ("silver", "#c0c0c0"),
    ("skyblue", "#87ceeb"),
    ("slateblue", "#6a5acd"),
    ("slategray", "#708090"),
    ("slategrey", "#708090"),
    ("snow", "#fffafa"),
    ("springgreen", "#00ff7f"),
    ("tan", "#d2b48c"),
    ("teal", "#008080"),
    ("thistle", "#d8bfd8"),
    ("turquoise", "#40e0d0"),
    ("violet", "#ee82ee"),
    ("wheat", "#f5deb3"),
    ("white", "#ffffff"),
    ("whitesmoke", "#f5f5f5"),
    ("yellow", "#ffff00"),
    ("yellowgreen", "#9acd32"),
];

/// `Keyword.parse`: the keyword's hex is parsed, so `stringify` of an
/// unchanged keyword color returns that hex, not the keyword.
fn parse_keyword(color: &str) -> Option<Channels> {
    let lower = color.to_lowercase();
    let hex = KEYWORDS.iter().find(|(k, _)| *k == lower)?.1;
    parse_hex(hex)
}

/// `Color.parse`: hex, then rgb(), then hsl(), then a keyword.
pub fn parse(color: &str) -> Result<Channels> {
    parse_hex(color)
        .or_else(|| parse_rgb(color))
        .or_else(|| parse_hsl(color))
        .or_else(|| parse_keyword(color))
        .ok_or_else(|| KhromaError::UnsupportedFormat(color.to_string()))
}

/// `Hex.stringify`.
fn stringify_hex(ch: &mut Channels) -> String {
    let (r, g, b, a) = (
        ch.get(Channel::R),
        ch.get(Channel::G),
        ch.get(Channel::B),
        ch.get(Channel::A),
    );
    if a < 1.0 {
        format!(
            "#{}{}{}{}",
            dec2hex(r),
            dec2hex(g),
            dec2hex(b),
            dec2hex(a * 255.0)
        )
    } else {
        format!("#{}{}{}", dec2hex(r), dec2hex(g), dec2hex(b))
    }
}

/// `RGB.stringify`.
fn stringify_rgb(ch: &mut Channels) -> String {
    let (r, g, b, a) = (
        ch.get(Channel::R),
        ch.get(Channel::G),
        ch.get(Channel::B),
        ch.get(Channel::A),
    );
    if a < 1.0 {
        format!(
            "rgba({}, {}, {}, {})",
            js_number(lang_round(r)),
            js_number(lang_round(g)),
            js_number(lang_round(b)),
            js_number(lang_round(a))
        )
    } else {
        format!(
            "rgb({}, {}, {})",
            js_number(lang_round(r)),
            js_number(lang_round(g)),
            js_number(lang_round(b))
        )
    }
}

/// `HSL.stringify`. The alpha is printed unrounded, as khroma does.
fn stringify_hsl(ch: &mut Channels) -> String {
    let (h, s, l, a) = (
        ch.get(Channel::H),
        ch.get(Channel::S),
        ch.get(Channel::L),
        ch.get(Channel::A),
    );
    if a < 1.0 {
        format!(
            "hsla({}, {}%, {}%, {})",
            js_number(lang_round(h)),
            js_number(lang_round(s)),
            js_number(lang_round(l)),
            js_number(a)
        )
    } else {
        format!(
            "hsl({}, {}%, {}%)",
            js_number(lang_round(h)),
            js_number(lang_round(s)),
            js_number(lang_round(l))
        )
    }
}

/// `Color.stringify`: the input string while nothing changed; `hsl()` after
/// an HSL change or for a parsed `hsl()`; `rgb()` for fractional channels or
/// alpha; hex otherwise.
pub fn stringify(ch: &mut Channels) -> String {
    if !ch.changed
        && let Some(color) = &ch.color
    {
        return color.clone();
    }
    if ch.ty == ChannelType::Hsl || ch.r.is_none() {
        return stringify_hsl(ch);
    }
    let (r, g, b, a) = (
        ch.get(Channel::R),
        ch.get(Channel::G),
        ch.get(Channel::B),
        ch.get(Channel::A),
    );
    if a < 1.0 || !is_integer(r) || !is_integer(g) || !is_integer(b) {
        stringify_rgb(ch)
    } else {
        stringify_hex(ch)
    }
}

/// `change` in `src/methods/change.ts`: sets channels to absolute values.
pub fn change(color: &str, channels: &[(Channel, f64)]) -> Result<String> {
    let mut ch = parse(color)?;
    for (c, v) in channels {
        ch.set(*c, c.clamp(*v))?;
    }
    Ok(stringify(&mut ch))
}

/// `adjust` in `src/methods/adjust.ts`: adds to channels. A zero amount is
/// skipped (`if (!channels[c]) continue`), so `{ l: 0 }` changes nothing and
/// does not even switch the output to `hsl()`.
pub fn adjust(color: &str, channels: &[(Channel, f64)]) -> Result<String> {
    let mut ch = parse(color)?;
    let mut changes = Vec::with_capacity(channels.len());
    for (c, amount) in channels {
        if *amount == 0.0 || amount.is_nan() {
            continue;
        }
        changes.push((*c, ch.get(*c) + amount));
    }
    change(color, &changes)
}

/// `adjustChannel` in `src/methods/adjust_channel.ts`. The channel is only
/// written when the clamped value differs, so a no-op keeps the input
/// string.
pub fn adjust_channel(color: &str, channel: Channel, amount: f64) -> Result<String> {
    let mut ch = parse(color)?;
    let current = ch.get(channel);
    let next = channel.clamp(current + amount);
    if current != next {
        ch.set(channel, next)?;
    }
    Ok(stringify(&mut ch))
}

/// `lighten`.
pub fn lighten(color: &str, amount: f64) -> Result<String> {
    adjust_channel(color, Channel::L, amount)
}

/// `darken`.
pub fn darken(color: &str, amount: f64) -> Result<String> {
    adjust_channel(color, Channel::L, -amount)
}

/// `rgba(r, g, b, a)` with numbers: the color string for the channels.
pub fn rgba(r: f64, g: f64, b: f64, a: f64) -> String {
    let mut ch = Channels::rgba(
        Channel::R.clamp(r),
        Channel::G.clamp(g),
        Channel::B.clamp(b),
        Channel::A.clamp(a),
        None,
    );
    stringify(&mut ch)
}

/// `rgba(color, opacity)` with a color: `change(color, { a })`.
pub fn with_alpha(color: &str, opacity: f64) -> Result<String> {
    change(color, &[(Channel::A, opacity)])
}

/// `mix` in `src/methods/mix.ts`, the dart-sass formula.
pub fn mix_channels(c1: &mut Channels, c2: &mut Channels, weight: f64) -> String {
    let (r1, g1, b1, a1) = (
        c1.get(Channel::R),
        c1.get(Channel::G),
        c1.get(Channel::B),
        c1.get(Channel::A),
    );
    let (r2, g2, b2, a2) = (
        c2.get(Channel::R),
        c2.get(Channel::G),
        c2.get(Channel::B),
        c2.get(Channel::A),
    );
    let weight_scale = weight / 100.0;
    let weight_normalized = (weight_scale * 2.0) - 1.0;
    let alpha_delta = a1 - a2;
    let weight1_combined = if (weight_normalized * alpha_delta) == -1.0 {
        weight_normalized
    } else {
        (weight_normalized + alpha_delta) / (1.0 + weight_normalized * alpha_delta)
    };
    let weight1 = (weight1_combined + 1.0) / 2.0;
    let weight2 = 1.0 - weight1;
    let r = (r1 * weight1) + (r2 * weight2);
    let g = (g1 * weight1) + (g2 * weight2);
    let b = (b1 * weight1) + (b2 * weight2);
    let a = (a1 * weight_scale) + (a2 * (1.0 - weight_scale));
    rgba(r, g, b, a)
}

/// `mix(color1, color2, weight)`.
pub fn mix(color1: &str, color2: &str, weight: f64) -> Result<String> {
    let mut c1 = parse(color1)?;
    let mut c2 = parse(color2)?;
    Ok(mix_channels(&mut c1, &mut c2, weight))
}

/// `invert` in `src/methods/invert.ts`: the RGB complement mixed with the
/// input at `weight` percent (100 by default in khroma; mermaid never passes
/// a weight).
pub fn invert(color: &str, weight: f64) -> Result<String> {
    let mut inverse = parse(color)?;
    let r = inverse.get(Channel::R);
    let g = inverse.get(Channel::G);
    let b = inverse.get(Channel::B);
    inverse.set(Channel::R, 255.0 - r)?;
    inverse.set(Channel::G, 255.0 - g)?;
    inverse.set(Channel::B, 255.0 - b)?;
    let mut original = parse(color)?;
    Ok(mix_channels(&mut inverse, &mut original, weight))
}

/// `Channel.toLinear`. The threshold compares the 0..255 value, not the
/// normalized one; khroma ships it that way.
fn to_linear(c: f64) -> f64 {
    let n = c / 255.0;
    if c > 0.03928 {
        ((n + 0.055) / 1.055).powf(2.4)
    } else {
        n / 12.92
    }
}

/// `luminance` in `src/methods/luminance.ts`.
pub fn luminance(color: &str) -> Result<f64> {
    let mut ch = parse(color)?;
    let (r, g, b) = (ch.get(Channel::R), ch.get(Channel::G), ch.get(Channel::B));
    Ok(lang_round(
        0.2126 * to_linear(r) + 0.7152 * to_linear(g) + 0.0722 * to_linear(b),
    ))
}

/// `isLight`: luminance of at least 0.5.
pub fn is_light(color: &str) -> Result<bool> {
    Ok(luminance(color)? >= 0.5)
}

/// `isDark`: the complement of `isLight`.
pub fn is_dark(color: &str) -> Result<bool> {
    Ok(!is_light(color)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parse_and_stringify_round_trip() {
        let mut ch = parse("#ECECFF").unwrap();
        assert_eq!(stringify(&mut ch), "#ECECFF");
        assert_eq!(ch.get(Channel::R), 236.0);
        let mut short = parse("#abc").unwrap();
        assert_eq!(short.get(Channel::R), 170.0);
        assert_eq!(short.get(Channel::B), 204.0);
        let mut alpha = parse("#ff000080").unwrap();
        assert_eq!(alpha.get(Channel::A), 128.0 / 255.0);
    }

    #[test]
    fn adjust_hue_prints_hsl_with_negative_hue() {
        assert_eq!(
            adjust("#fff4dd", &[(Channel::H, -120.0)]).unwrap(),
            "hsl(-79.4117647059, 100%, 93.3333333333%)"
        );
    }

    #[test]
    fn lighten_of_noop_returns_the_input() {
        assert_eq!(lighten("#ffffff", 10.0).unwrap(), "#ffffff");
        assert_eq!(lighten("white", 10.0).unwrap(), "#ffffff");
        assert_eq!(darken("white", 30.0).unwrap(), "hsl(0, 0%, 70%)");
    }

    #[test]
    fn invert_mixes_to_hex_or_rgb() {
        assert_eq!(invert("#ECECFF", 100.0).unwrap(), "#131300");
        assert_eq!(invert("black", 100.0).unwrap(), "#ffffff");
        assert_eq!(invert("#333", 100.0).unwrap(), "#cccccc");
    }

    #[test]
    fn rgba_numbers() {
        assert_eq!(rgba(102.0, 102.0, 255.0, 0.49), "rgba(102, 102, 255, 0.49)");
        assert_eq!(rgba(255.0, 255.0, 255.0, 70.0), "#ffffff");
        assert_eq!(rgba(255.0, 255.0, 255.0, 0.25), "rgba(255, 255, 255, 0.25)");
    }

    #[test]
    fn keyword_table_has_khromas_typo() {
        assert!(parse("cyan").is_err());
        assert!(parse("cyanaqua").is_ok());
        assert!(parse("Red").is_ok());
    }

    #[test]
    fn luminance_and_is_dark() {
        assert!(is_dark("#000").unwrap());
        assert!(!is_dark("#fff").unwrap());
        assert_eq!(luminance("#ffffff").unwrap(), 1.0);
    }

    #[test]
    fn js_number_formats() {
        assert_eq!(js_number(96.0), "96");
        assert_eq!(js_number(96.2745098039), "96.2745098039");
        assert_eq!(js_number(1e-10), "1e-10");
        assert_eq!(js_number(-0.0), "0");
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(2.5), 3.0);
    }

    #[test]
    fn mixed_channel_change_is_an_error() {
        assert_eq!(
            change("#fff", &[(Channel::H, 1.0), (Channel::R, 1.0)]),
            Err(KhromaError::MixedChannels)
        );
    }

    #[test]
    fn rgb_and_hsl_parse() {
        let mut ch = parse("rgba(232,232,232, 0.8)").unwrap();
        assert_eq!(ch.get(Channel::A), 0.8);
        assert_eq!(stringify(&mut ch), "rgba(232,232,232, 0.8)");
        let mut h = parse("hsl(210, 66.6666666667%, 95%)").unwrap();
        assert_eq!(h.get(Channel::H), 210.0);
        assert_eq!(
            lighten("hsl(210, 50%, 95%)", 10.0).unwrap(),
            "hsl(210, 50%, 100%)"
        );
    }
}
