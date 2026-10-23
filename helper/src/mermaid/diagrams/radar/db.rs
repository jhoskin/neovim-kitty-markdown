//! Port of the radar `db.ts` and the parts of `commonDb.ts` it uses.

use super::parser::{Ast, Entry, RadarOption};

/// `MAX_TICKS`: more rings than this make the graticule unreadable.
const MAX_TICKS: f64 = 32.0;

/// An error `setCurves` throws, with the line of the curve so the plugin
/// can point at it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurveError {
    pub message: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadarAxis {
    pub name: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadarCurve {
    pub name: String,
    pub label: String,
    pub entries: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Graticule {
    Circle,
    Polygon,
}

/// `RadarOptions`. `ticks` stays a JS number: the renderer loops while
/// `i < ticks`, so a fraction adds one more ring.
#[derive(Debug, Clone, PartialEq)]
pub struct RadarOptions {
    pub show_legend: bool,
    pub ticks: f64,
    pub max: Option<f64>,
    pub min: f64,
    pub graticule: Graticule,
}

impl Default for RadarOptions {
    /// `defaultOptions`.
    fn default() -> RadarOptions {
        RadarOptions {
            show_legend: true,
            ticks: 5.0,
            max: None,
            min: 0.0,
            graticule: Graticule::Circle,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RadarDb {
    axes: Vec<RadarAxis>,
    curves: Vec<RadarCurve>,
    options: RadarOptions,
    title: String,
    acc_title: String,
    acc_descr: String,
}

impl RadarDb {
    pub fn new() -> RadarDb {
        RadarDb::default()
    }

    /// `setDiagramTitle`: `sanitizeText` with DOMPurify, which leaves plain
    /// text as it is; markup in a title is out of scope here.
    pub fn set_diagram_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn diagram_title(&self) -> &str {
        &self.title
    }

    pub fn acc_title(&self) -> &str {
        &self.acc_title
    }

    pub fn acc_descr(&self) -> &str {
        &self.acc_descr
    }

    pub fn axes(&self) -> &[RadarAxis] {
        &self.axes
    }

    pub fn curves(&self) -> &[RadarCurve] {
        &self.curves
    }

    pub fn options(&self) -> &RadarOptions {
        &self.options
    }

    /// `setAxes`: the label defaults to the name.
    fn set_axes(&mut self, ast: &Ast) {
        self.axes = ast
            .axes
            .iter()
            .map(|a| RadarAxis {
                name: a.name.clone(),
                label: a.label.clone().unwrap_or_else(|| a.name.clone()),
            })
            .collect();
    }

    /// `computeCurveEntries`: plain values keep their order; entries with
    /// an axis reference are reordered to the axes, the first entry per
    /// axis name wins, and entries for unknown axes are ignored.
    fn curve_entries(&self, entries: &[Entry], line: u32) -> Result<Vec<f64>, CurveError> {
        if entries.first().is_none_or(|e| e.axis.is_none()) {
            return Ok(entries.iter().map(|e| e.value).collect());
        }
        if self.axes.is_empty() {
            return Err(CurveError {
                message: "Axes must be populated before curves for reference entries".to_string(),
                line,
            });
        }
        self.axes
            .iter()
            .map(|axis| {
                entries
                    .iter()
                    .find(|e| e.axis.as_deref() == Some(axis.name.as_str()))
                    .map(|e| e.value)
                    .ok_or_else(|| CurveError {
                        message: format!("Missing entry for axis {}", axis.label),
                        line,
                    })
            })
            .collect()
    }

    /// `setCurves`.
    fn set_curves(&mut self, ast: &Ast) -> Result<(), CurveError> {
        let mut curves = Vec::with_capacity(ast.curves.len());
        for c in &ast.curves {
            curves.push(RadarCurve {
                name: c.name.clone(),
                label: c.label.clone().unwrap_or_else(|| c.name.clone()),
                entries: self.curve_entries(&c.entries, c.line)?,
            });
        }
        self.curves = curves;
        Ok(())
    }

    /// `setOptions`: the last option of each name wins, an absent one keeps
    /// its default, and `ticks` is capped at `MAX_TICKS`.
    fn set_options(&mut self, options: &[RadarOption]) {
        let mut o = RadarOptions::default();
        for option in options {
            match option {
                RadarOption::ShowLegend(v) => o.show_legend = *v,
                RadarOption::Ticks(v) => o.ticks = *v,
                RadarOption::Max(v) => o.max = Some(*v),
                RadarOption::Min(v) => o.min = *v,
                RadarOption::Graticule(v) => {
                    o.graticule = if v == "polygon" {
                        Graticule::Polygon
                    } else {
                        Graticule::Circle
                    }
                }
            }
        }
        if o.ticks > MAX_TICKS {
            log::warn!(
                "Radar diagram ticks ({}) exceeds maximum allowed ({MAX_TICKS}). Using {MAX_TICKS} instead.",
                o.ticks
            );
            o.ticks = MAX_TICKS;
        }
        self.options = o;
    }

    /// `parser.populate`: `populateCommonDb` (a falsy value, that is an
    /// empty one, sets nothing), then the axes, the curves, and the
    /// options, in that order whatever order the text uses.
    pub fn populate(&mut self, ast: &Ast) -> Result<(), CurveError> {
        if let Some(d) = ast.acc_descr.as_deref().filter(|s| !s.is_empty()) {
            self.acc_descr = d.to_string();
        }
        if let Some(t) = ast.acc_title.as_deref().filter(|s| !s.is_empty()) {
            self.acc_title = t.trim_start().to_string();
        }
        if let Some(t) = ast.title.as_deref().filter(|s| !s.is_empty()) {
            self.set_diagram_title(t);
        }
        self.set_axes(ast);
        self.set_curves(ast)?;
        self.set_options(&ast.options);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::diagrams::radar::parser::parse;

    fn db(text: &str) -> Result<RadarDb, CurveError> {
        let mut db = RadarDb::new();
        db.populate(&parse(text).unwrap())?;
        Ok(db)
    }

    #[test]
    fn labels_default_to_names_and_reference_entries_follow_the_axes() {
        let db =
            db("radar-beta\ncurve c[\"C\"]{b: 2, a: 1, z: 9, a: 5}\naxis a[\"A\"], b\n").unwrap();
        assert_eq!(
            db.axes()
                .iter()
                .map(|a| a.label.as_str())
                .collect::<Vec<_>>(),
            vec!["A", "b"]
        );
        assert_eq!(db.curves()[0].label, "C");
        assert_eq!(db.curves()[0].entries, vec![1.0, 2.0]);
    }

    #[test]
    fn reference_errors_carry_the_curve_line() {
        let err = db("radar-beta\naxis a[\"Alpha\"], b\n\ncurve c{b: 1}\n").unwrap_err();
        assert_eq!(err.message, "Missing entry for axis Alpha");
        assert_eq!(err.line, 4);
        let err = db("radar-beta\ncurve c{b: 1}\n").unwrap_err();
        assert_eq!(
            err.message,
            "Axes must be populated before curves for reference entries"
        );
    }

    #[test]
    fn options_take_the_last_value_and_cap_ticks() {
        let db = db(
            "radar-beta\nticks 3, ticks 40\nmax 9, min 2\ngraticule polygon\nshowLegend false\n",
        )
        .unwrap();
        assert_eq!(
            db.options(),
            &RadarOptions {
                show_legend: false,
                ticks: 32.0,
                max: Some(9.0),
                min: 2.0,
                graticule: Graticule::Polygon,
            }
        );
        let plain = super::RadarDb::new();
        assert_eq!(plain.options(), &RadarOptions::default());
    }

    #[test]
    fn front_matter_title_loses_to_a_title_in_the_diagram() {
        let mut db = RadarDb::new();
        db.set_diagram_title("From front matter");
        db.populate(&parse("radar-beta\n title\n").unwrap())
            .unwrap();
        assert_eq!(db.diagram_title(), "From front matter");
        db.populate(&parse("radar-beta\n title In text\n").unwrap())
            .unwrap();
        assert_eq!(db.diagram_title(), "In text");
    }
}
