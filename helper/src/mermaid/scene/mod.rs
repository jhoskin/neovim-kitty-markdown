//! Scene model, deterministic SVG serializer, d3-shape curves, markers.
//!
//! The scene is renderer independent: diagram modules build it, `svg`
//! serializes it, and `raster` hands the SVG text to resvg. There is no CSS
//! anywhere in the model. The styles package resolves mermaid's stylesheet
//! rules to the attributes in [`Style`] before an element reaches the scene,
//! so the serializer never has to know a class name's meaning.

pub mod curves;
pub mod filters;
pub mod markers;
pub mod path;
pub mod svg;

pub use path::{PathData, Point};

/// An sRGB color with a straight (not premultiplied) alpha in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f64,
}

impl Color {
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const BLACK: Color = Color::rgb(0, 0, 0);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: f64) -> Color {
        Color { r, g, b, a }
    }

    /// Parses `#rgb`, `#rrggbb`, and `#rrggbbaa`. Named colors are the theme
    /// package's business; it hands the scene resolved hex values.
    pub fn from_hex(hex: &str) -> Option<Color> {
        let digits = hex.strip_prefix('#')?;
        let byte = |s: &str| u8::from_str_radix(s, 16).ok();
        let nibble = |s: &str| byte(s).map(|v| v * 17);
        match digits.len() {
            3 => Some(Color::rgb(
                nibble(&digits[0..1])?,
                nibble(&digits[1..2])?,
                nibble(&digits[2..3])?,
            )),
            6 => Some(Color::rgb(
                byte(&digits[0..2])?,
                byte(&digits[2..4])?,
                byte(&digits[4..6])?,
            )),
            8 => Some(Color::rgba(
                byte(&digits[0..2])?,
                byte(&digits[2..4])?,
                byte(&digits[4..6])?,
                f64::from(byte(&digits[6..8])?) / 255.0,
            )),
            _ => None,
        }
    }

    pub fn is_opaque(self) -> bool {
        self.a >= 1.0
    }
}

/// What a fill or stroke paints with.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// `fill="none"` or `stroke="none"`.
    None,
    Color(Color),
    /// A reference to a gradient or pattern in `defs`, by id.
    Reference(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAnchor {
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Baseline {
    Auto,
    Middle,
    Central,
    Hanging,
    TextBeforeEdge,
    TextAfterEdge,
    Alphabetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Normal,
    Bold,
    /// A numeric weight such as 600.
    Number(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

/// Font attributes. Every field is optional so a run or a text can leave a
/// value to its group.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FontSpec {
    pub family: Option<String>,
    pub size: Option<f64>,
    pub weight: Option<FontWeight>,
    pub style: Option<FontStyle>,
}

impl FontSpec {
    /// Returns `self` with every unset field taken from `base`.
    pub fn merged_over(&self, base: &FontSpec) -> FontSpec {
        FontSpec {
            family: self.family.clone().or_else(|| base.family.clone()),
            size: self.size.or(base.size),
            weight: self.weight.or(base.weight),
            style: self.style.or(base.style),
        }
    }
}

/// Presentation attributes. `None` means "not written", which lets the
/// value inherit from the enclosing group exactly as it would in a browser.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Style {
    pub fill: Option<Paint>,
    pub fill_opacity: Option<f64>,
    pub stroke: Option<Paint>,
    pub stroke_width: Option<f64>,
    pub stroke_dasharray: Option<Vec<f64>>,
    pub stroke_linecap: Option<LineCap>,
    pub stroke_linejoin: Option<LineJoin>,
    pub stroke_opacity: Option<f64>,
    pub opacity: Option<f64>,
    pub font: FontSpec,
    pub text_anchor: Option<TextAnchor>,
    pub marker_start: Option<String>,
    pub marker_end: Option<String>,
}

impl Style {
    pub fn fill(color: Color) -> Style {
        Style {
            fill: Some(Paint::Color(color)),
            ..Style::default()
        }
    }

    pub fn stroke(color: Color, width: f64) -> Style {
        Style {
            stroke: Some(Paint::Color(color)),
            stroke_width: Some(width),
            ..Style::default()
        }
    }

    /// Fills every unset field of `self` from `base`. Markers use it to take
    /// the diagram's line color without overriding their own explicit fills.
    pub fn fill_unset_from(&mut self, base: &Style) {
        if self.fill.is_none() {
            self.fill = base.fill.clone();
        }
        if self.fill_opacity.is_none() {
            self.fill_opacity = base.fill_opacity;
        }
        if self.stroke.is_none() {
            self.stroke = base.stroke.clone();
        }
        if self.stroke_width.is_none() {
            self.stroke_width = base.stroke_width;
        }
        if self.stroke_dasharray.is_none() {
            self.stroke_dasharray = base.stroke_dasharray.clone();
        }
        if self.stroke_linecap.is_none() {
            self.stroke_linecap = base.stroke_linecap;
        }
        if self.stroke_linejoin.is_none() {
            self.stroke_linejoin = base.stroke_linejoin;
        }
        if self.stroke_opacity.is_none() {
            self.stroke_opacity = base.stroke_opacity;
        }
        if self.opacity.is_none() {
            self.opacity = base.opacity;
        }
        self.font = self.font.merged_over(&base.font);
        if self.text_anchor.is_none() {
            self.text_anchor = base.text_anchor;
        }
    }
}

/// One `transform` list entry, applied in list order like SVG does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformOp {
    Translate(f64, f64),
    Scale(f64, f64),
    /// Angle in degrees around `(cx, cy)`.
    Rotate(f64, f64, f64),
    Matrix([f64; 6]),
}

/// Attributes every drawable element carries.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Common {
    pub id: Option<String>,
    /// Kept for debugging and snapshots; nothing resolves it.
    pub class: Option<String>,
    pub transform: Vec<TransformOp>,
    pub style: Style,
}

impl Common {
    pub fn with_style(style: Style) -> Common {
        Common {
            style,
            ..Common::default()
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Group {
    pub common: Common,
    pub children: Vec<Element>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rect {
    pub common: Common,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub rx: Option<f64>,
    pub ry: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Circle {
    pub common: Common,
    pub cx: f64,
    pub cy: f64,
    pub r: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ellipse {
    pub common: Common,
    pub cx: f64,
    pub cy: f64,
    pub rx: f64,
    pub ry: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub common: Common,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Polygon {
    pub common: Common,
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Polyline {
    pub common: Common,
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub common: Common,
    pub d: PathData,
}

/// A CSS length as mermaid writes it on `tspan` elements.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    Px(f64),
    Em(f64),
}

/// One `tspan`. A text with a single run and no `dy` or `x` serializes as
/// bare character data; everything else becomes a `tspan` per run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextRun {
    pub text: String,
    pub x: Option<f64>,
    pub dy: Option<Length>,
    pub font: FontSpec,
    pub fill: Option<Paint>,
}

impl TextRun {
    pub fn new(text: impl Into<String>) -> TextRun {
        TextRun {
            text: text.into(),
            ..TextRun::default()
        }
    }

    /// A new line as mermaid emits it: `x` back to the text origin and a
    /// `dy` of one line.
    pub fn line(text: impl Into<String>, x: f64, dy: Length) -> TextRun {
        TextRun {
            text: text.into(),
            x: Some(x),
            dy: Some(dy),
            ..TextRun::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    pub common: Common,
    pub x: f64,
    pub y: f64,
    pub anchor: Option<TextAnchor>,
    pub baseline: Option<Baseline>,
    /// Overrides `common.style.font` where set.
    pub font: FontSpec,
    pub runs: Vec<TextRun>,
}

/// An embedded raster image; reserved for later diagram types.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub common: Common,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub href: String,
}

/// A `<use>` of an element in `defs`.
#[derive(Debug, Clone, PartialEq)]
pub struct Use {
    pub common: Common,
    pub href: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orient {
    Auto,
    AutoStartReverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerUnits {
    StrokeWidth,
    UserSpaceOnUse,
}

/// A `<marker>` definition. Shapes reference it by id through
/// [`Style::marker_start`] and [`Style::marker_end`].
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub id: String,
    pub class: Option<String>,
    pub view_box: Option<ViewBox>,
    pub ref_x: f64,
    pub ref_y: f64,
    pub marker_width: f64,
    pub marker_height: f64,
    pub orient: Orient,
    pub marker_units: MarkerUnits,
    pub style: Style,
    pub children: Vec<Element>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    Group(Group),
    Rect(Rect),
    Circle(Circle),
    Ellipse(Ellipse),
    Line(Line),
    Polygon(Polygon),
    Polyline(Polyline),
    Path(Path),
    Text(Text),
    Image(Image),
    Use(Use),
    Marker(Marker),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ViewBox {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> ViewBox {
        ViewBox {
            x,
            y,
            width,
            height,
        }
    }
}

/// A complete diagram. `width` and `height` are the natural size in CSS
/// pixels (SPEC.md Definitions, "Natural size"); `view_box` is what mermaid
/// wrote in the `viewBox` attribute.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub view_box: ViewBox,
    /// Painted under everything. `None` leaves the canvas transparent.
    pub background: Option<Color>,
    /// The `font-family` of the root element; text without its own family
    /// inherits it. mermaid's default chain is
    /// `"trebuchet ms", verdana, arial, sans-serif`.
    pub font_family: String,
    pub defs: Vec<Element>,
    pub root: Group,
}

impl Scene {
    /// A scene whose viewBox starts at the origin and matches its size.
    pub fn new(width: f64, height: f64) -> Scene {
        Scene {
            width,
            height,
            view_box: ViewBox::new(0.0, 0.0, width, height),
            background: None,
            font_family: DEFAULT_FONT_FAMILY.to_string(),
            defs: Vec::new(),
            root: Group::default(),
        }
    }

    pub fn to_svg(&self) -> String {
        svg::to_svg(self)
    }
}

/// mermaid's `fontFamily` default (config.schema.yaml).
pub const DEFAULT_FONT_FAMILY: &str = "\"trebuchet ms\", verdana, arial, sans-serif";
