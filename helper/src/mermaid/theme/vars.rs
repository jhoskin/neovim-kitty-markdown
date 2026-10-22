//! The theme property bag: mermaid's `Theme` classes are plain JS objects
//! whose constructor and `updateColors` assign `this.<name>` with `||` and
//! `??` chains over whatever the user put in `themeVariables`. Porting each
//! theme as a struct would lose that dynamism (an override can be a number,
//! an object, or an empty string, and `||` sees each differently), so the
//! port keeps the bag and gives it the few operations the theme files use.

use std::fmt;

use indexmap::IndexMap;
use serde_json::{Map, Value};

use super::khroma::{self, Channel, KhromaError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeError(pub String);

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ThemeError {}

impl From<KhromaError> for ThemeError {
    fn from(err: KhromaError) -> Self {
        ThemeError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ThemeError>;

/// JS truthiness of a JSON value. `undefined` is modelled as `Null`.
pub fn js_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// `Value::String` shorthand for the theme files.
pub fn s(text: &str) -> Value {
    Value::String(text.to_string())
}

/// The string a khroma call receives. Anything else makes khroma throw in
/// JS (`Color.parse` hands a non-string back and the next property access
/// fails), so it is an error here too.
fn color_str(v: &Value) -> Result<&str> {
    match v {
        Value::String(c) => Ok(c),
        other => Err(ThemeError(
            KhromaError::NotAColor(other.to_string()).to_string(),
        )),
    }
}

pub fn adjust(color: &Value, channels: &[(Channel, f64)]) -> Result<Value> {
    Ok(s(&khroma::adjust(color_str(color)?, channels)?))
}

pub fn lighten(color: &Value, amount: f64) -> Result<Value> {
    Ok(s(&khroma::lighten(color_str(color)?, amount)?))
}

pub fn darken(color: &Value, amount: f64) -> Result<Value> {
    Ok(s(&khroma::darken(color_str(color)?, amount)?))
}

pub fn invert(color: &Value) -> Result<Value> {
    Ok(s(&khroma::invert(color_str(color)?, 100.0)?))
}

pub fn is_dark(color: &Value) -> Result<bool> {
    Ok(khroma::is_dark(color_str(color)?)?)
}

/// `rgba(r, g, b, a)` from `khroma`, with numbers.
pub fn rgba(r: f64, g: f64, b: f64, a: f64) -> Value {
    s(&khroma::rgba(r, g, b, a))
}

/// `mkBorder` in `theme-helpers.js`.
pub fn mk_border(color: &Value, dark_mode: bool) -> Result<Value> {
    if dark_mode {
        adjust(color, &[(Channel::S, -40.0), (Channel::L, 10.0)])
    } else {
        adjust(color, &[(Channel::S, -40.0), (Channel::L, -10.0)])
    }
}

/// `applyOverride` in `theme-helpers.js`: an object override keeps the
/// generated keys it does not name.
pub fn apply_override(generated: &Value, override_: &Value) -> Value {
    match (generated, override_) {
        (Value::Object(g), Value::Object(o)) => {
            let mut merged = g.clone();
            for (k, v) in o {
                merged.insert(k.clone(), v.clone());
            }
            Value::Object(merged)
        }
        _ => override_.clone(),
    }
}

/// `this` of a mermaid `Theme`. Keys keep insertion order, so the output
/// lists variables the way mermaid's object does.
#[derive(Debug, Clone, Default)]
pub struct Vars {
    map: IndexMap<String, Value>,
}

impl Vars {
    pub fn new() -> Vars {
        Vars::default()
    }

    /// `this.k`; `Null` stands for `undefined`.
    pub fn get(&self, k: &str) -> &Value {
        self.map.get(k).unwrap_or(&Value::Null)
    }

    /// `this.k?.sub`: a key of an object-valued variable.
    pub fn get_in(&self, k: &str, sub: &str) -> &Value {
        match self.get(k) {
            Value::Object(o) => o.get(sub).unwrap_or(&Value::Null),
            _ => &Value::Null,
        }
    }

    pub fn truthy(&self, k: &str) -> bool {
        js_truthy(self.get(k))
    }

    /// `this.k = v`.
    pub fn set(&mut self, k: &str, v: impl Into<Value>) {
        self.map.insert(k.to_string(), v.into());
    }

    /// `this.k = this.from`.
    pub fn copy(&mut self, k: &str, from: &str) {
        let v = self.get(from).clone();
        self.set(k, v);
    }

    /// `this.k = this.k || <f()>`; `f` runs only when `this.k` is falsy.
    pub fn or(&mut self, k: &str, f: impl FnOnce(&Vars) -> Result<Value>) -> Result<()> {
        if !self.truthy(k) {
            let v = f(self)?;
            self.set(k, v);
        }
        Ok(())
    }

    /// `this.k = this.k || <literal>`.
    pub fn or_val(&mut self, k: &str, v: impl Into<Value>) {
        if !self.truthy(k) {
            self.set(k, v);
        }
    }

    /// `this.k = this.k || this.from`.
    pub fn or_var(&mut self, k: &str, from: &str) {
        if !self.truthy(k) {
            self.copy(k, from);
        }
    }

    /// `this.k = this.k ?? <f()>`.
    pub fn or_nullish(&mut self, k: &str, f: impl FnOnce(&Vars) -> Result<Value>) -> Result<()> {
        if self.get(k).is_null() {
            let v = f(self)?;
            self.set(k, v);
        }
        Ok(())
    }

    /// `this.THEME_COLOR_LIMIT` as a loop bound: `i < limit` with a
    /// non-number is `false` in JS, so the loop does not run.
    pub fn limit(&self) -> usize {
        match self.get("THEME_COLOR_LIMIT") {
            Value::Number(n) => n.as_f64().map_or(0, |f| f.max(0.0) as usize),
            _ => 0,
        }
    }

    pub fn dark_mode(&self) -> bool {
        self.truthy("darkMode")
    }

    /// `Object.keys(this).forEach(k => { if (this[k] === 'calculated') this[k] = undefined; })`
    pub fn reset_calculated(&mut self) {
        for v in self.map.values_mut() {
            if v.as_str() == Some("calculated") {
                *v = Value::Null;
            }
        }
    }

    /// `this.k = { sub: this.k?.sub || <fallback>, ... }` for the
    /// object-valued variables (`cynefin`, `radar`, `wardley`, `xyChart`).
    pub fn set_object(&mut self, k: &str, entries: Vec<(&str, Value)>) {
        let mut obj = Map::new();
        for (sub, v) in entries {
            obj.insert(sub.to_string(), v);
        }
        self.set(k, Value::Object(obj));
    }

    /// `this.k?.sub || fallback` for building an object-valued variable.
    pub fn sub_or(&self, k: &str, sub: &str, fallback: impl Into<Value>) -> Value {
        let v = self.get_in(k, sub);
        if js_truthy(v) {
            v.clone()
        } else {
            fallback.into()
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.map.iter()
    }

    pub fn into_map(self) -> IndexMap<String, Value> {
        self.map
    }
}

/// `calculate(overrides)` as every theme writes it: copy the overrides in,
/// derive, copy them in again so an override of a derived value wins. The
/// theme-specific steps come in through `update_colors` and the two flags.
pub fn calculate(
    vars: &mut Vars,
    overrides: Option<&Map<String, Value>>,
    update_colors: fn(&mut Vars) -> Result<()>,
) -> Result<()> {
    let Some(overrides) = overrides else {
        update_colors(vars)?;
        return Ok(());
    };
    for (k, v) in overrides {
        vars.set(k, v.clone());
    }
    update_colors(vars)?;
    for (k, v) in overrides {
        let merged = apply_override(vars.get(k), v);
        vars.set(k, merged);
    }
    Ok(())
}
