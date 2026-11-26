//! A condition and a value in words (§FS-021-monitors.3.4).
//!
//! The page, the command and a matter's title say a condition the same way,
//! so it is said here once: `healthy` as the values it accepts, `warn` as the
//! values it marks, and a KPI as `KPI`.

use crate::monitors::condition::{meets, Judge};
use crate::monitors::model::{Condition, Kind, Value};

/// What a monitor's values are measured in, which decides how a value and a
/// bound are said.
#[derive(Debug, Clone, Copy, Default)]
pub struct Terms<'a> {
    pub kind: Option<Kind>,
    pub unit: Option<&'a str>,
    /// The whole a number is a fraction of, said as `12/15`
    /// (§FS-021-monitors.2.2).
    pub of: Option<f64>,
    pub levels: Option<&'a [String]>,
}

impl Terms<'_> {
    /// The least value the measure can take: a count and a fraction are
    /// never below zero (§FS-021-monitors.3.4).
    fn floor(&self) -> Option<f64> {
        (self.unit == Some("count") || self.of.is_some()).then_some(0.0)
    }

    /// The greatest: a fraction is never above its whole.
    fn ceiling(&self) -> Option<f64> {
        self.of
    }

    /// A bound said in the measure's terms: `80%`, `5`, `4/4`, `40 ms`,
    /// `19.1 MiB`.
    fn bound(&self, number: f64) -> String {
        if let Some(of) = self.of {
            return format!("{}/{}", self::number(number), self::number(of));
        }
        match self.unit {
            Some("bytes") => bytes(number),
            unit => with_unit(&self::number(number), unit),
        }
    }

    /// Two bounds as a range, the unit said once: `40–60 ms`, `40–60%`; bytes
    /// each in their own binary unit, `512 bytes–2.0 MiB`.
    fn range(&self, low: f64, high: f64) -> String {
        if let Some(of) = self.of {
            let of = number(of);
            return format!("{}/{of}–{}/{of}", number(low), number(high));
        }
        match self.unit {
            Some("bytes") => format!("{}–{}", bytes(low), bytes(high)),
            unit => with_unit(&format!("{}–{}", number(low), number(high)), unit),
        }
    }
}

/// A number as a sentence says it: `80`, `82.4`, never `80.0` or `-0`.
pub fn number(number: f64) -> String {
    if number == 0.0 {
        return "0".to_string();
    }
    format!("{number}")
}

/// A measure with its unit: `%` and `$` stay with the number, `count` is not
/// said, and any other unit follows after a space (§FS-021-monitors.2.2).
pub fn with_unit(said: &str, unit: Option<&str>) -> String {
    match unit {
        None | Some("count") | Some("") => said.to_string(),
        Some("%") => format!("{said}%"),
        Some("$") => format!("${said}"),
        Some(unit) => format!("{said} {unit}"),
    }
}

/// A size in binary units to one decimal, `17.4 MiB`, and in bytes below
/// one KiB, `512 bytes`, so a size reads at a glance and keeps its column
/// narrow (§FS-021-monitors.2.2).
pub fn bytes(size: f64) -> String {
    const UNITS: [&str; 5] = ["KiB", "MiB", "GiB", "TiB", "PiB"];
    if size.abs() < 1024.0 {
        return format!("{} bytes", number(size.round()));
    }
    let mut scaled = size / 1024.0;
    let mut unit = UNITS[0];
    for next in &UNITS[1..] {
        if scaled.abs() < 1024.0 {
            break;
        }
        scaled /= 1024.0;
        unit = next;
    }
    format!("{scaled:.1} {unit}")
}

/// A measure as a value or a change says it: a size in binary units, money
/// to the cent, `$41.20`, and any other with its unit, `38.1 ms`.
pub fn measure(size: f64, unit: Option<&str>) -> String {
    match unit {
        Some("bytes") => bytes(size),
        Some("$") => format!("${size:.2}"),
        unit => with_unit(&number(size), unit),
    }
}

/// A value as the page and a matter's title say it (§FS-021-monitors.2,
/// §FS-021-monitors.12.1): `yes` or `no`, a level's name, a fraction as
/// `12/15`, a percentage to one decimal, `79.2%`, and a size and money as
/// [`measure`] says them.
pub fn value(value: &Value, unit: Option<&str>, of: Option<f64>) -> String {
    match value {
        Value::Boolean(true) => "yes".to_string(),
        Value::Boolean(false) => "no".to_string(),
        Value::Level(level) => level.clone(),
        Value::Number(measured) => match (of, unit) {
            (Some(of), _) => format!("{}/{}", number(*measured), number(of)),
            (None, Some("%")) => format!("{measured:.1}%"),
            (None, unit) => measure(*measured, unit),
        },
    }
}

/// A list of names as a sentence says them: `a`, `a or b`, `a, b or c`.
fn either(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} or {last}", init.join(", ")),
    }
}

/// The levels a condition accepts, in the order `levels` lists them, or as
/// it names them where the levels are not known.
fn accepted(condition: &Condition, terms: &Terms) -> Vec<String> {
    match terms.levels {
        Some(levels) => levels
            .iter()
            .filter(|level| meets(condition, &Value::Level((*level).clone()), Some(levels)))
            .cloned()
            .collect(),
        None => condition.within.clone().unwrap_or_default(),
    }
}

/// `healthy` as the values it accepts (§FS-021-monitors.3.4):
/// `healthy ≥ 80%`, `healthy ≤ 5`, `healthy 40–60 ms`, `healthy at 0`,
/// `healthy at 4/4`, `healthy when yes`, `healthy when clean`,
/// `healthy: published or pending`, and `KPI` for a monitor nothing judges.
pub fn healthy(judge: &Judge, terms: &Terms) -> String {
    let condition = match judge {
        Judge::Kpi => return "KPI".to_string(),
        Judge::SampleOk => return "healthy as its sample says".to_string(),
        Judge::Condition(condition) => condition,
    };
    if let Some(is) = condition.is {
        return format!("healthy when {}", if is { "yes" } else { "no" });
    }
    if condition.within.is_some() || condition.at_most.is_some() {
        let levels = accepted(condition, terms);
        return match (levels.as_slice(), terms.levels, &condition.at_most) {
            ([one], _, _) => format!("healthy when {one}"),
            ([], None, Some(at_most)) => format!("healthy at {at_most} or better"),
            (levels, _, _) => format!("healthy: {}", either(levels)),
        };
    }
    let low = match (condition.min, terms.floor()) {
        (Some(min), Some(floor)) => Some(min.max(floor)),
        (min, floor) => min.or(floor),
    };
    let high = match (condition.max, terms.ceiling()) {
        (Some(max), Some(ceiling)) => Some(max.min(ceiling)),
        (max, ceiling) => max.or(ceiling),
    };
    if let (Some(low), Some(high)) = (low, high) {
        if low == high {
            return format!("healthy at {}", terms.bound(low));
        }
    }
    match (condition.min, condition.max) {
        (Some(min), Some(max)) => format!("healthy {}", terms.range(min, max)),
        (Some(min), None) => format!("healthy ≥ {}", terms.bound(min)),
        (None, Some(max)) => format!("healthy ≤ {}", terms.bound(max)),
        (None, None) => "healthy".to_string(),
    }
}

/// `warn` as the values it marks, the negation of what it is written as
/// (§FS-021-monitors.3.4): `warn above 0` for `{"max": 0}`, `warn below 90%`
/// for `{"min": 90}`, and for a level the levels it marks that `healthy`
/// still accepts, `warn: slow`. Where `healthy` accepts none of them, it is
/// said as every level it marks, and as `warn: no level` where it marks none,
/// so the words never end on an empty list.
pub fn warn(warn: &Condition, healthy: Option<&Condition>, terms: &Terms) -> String {
    if let Some(is) = warn.is {
        return format!("warn when {}", if is { "no" } else { "yes" });
    }
    if warn.within.is_some() || warn.at_most.is_some() {
        let Some(levels) = terms.levels else {
            let kept = either(&warn.within.clone().unwrap_or_default());
            return format!("warn: other than {kept}");
        };
        let marks = |accepted: &dyn Fn(&Value) -> bool| -> Vec<String> {
            levels
                .iter()
                .filter(|level| {
                    let value = Value::Level((*level).clone());
                    !meets(warn, &value, Some(levels)) && accepted(&value)
                })
                .cloned()
                .collect()
        };
        let mut marked =
            marks(&|value| healthy.is_none_or(|healthy| meets(healthy, value, Some(levels))));
        if marked.is_empty() {
            marked = marks(&|_| true);
        }
        return match marked.is_empty() {
            true => "warn: no level".to_string(),
            false => format!("warn: {}", either(&marked)),
        };
    }
    let below = warn
        .min
        .filter(|min| terms.floor().is_none_or(|floor| *min > floor));
    let above = warn
        .max
        .filter(|max| terms.ceiling().is_none_or(|ceiling| *max < ceiling));
    match (below, above) {
        (Some(min), Some(max)) => format!("warn outside {}", terms.range(min, max)),
        (Some(min), None) => format!("warn below {}", terms.bound(min)),
        (None, Some(max)) => format!("warn above {}", terms.bound(max)),
        (None, None) => match (warn.min, warn.max) {
            (Some(min), _) => format!("warn below {}", terms.bound(min)),
            (_, Some(max)) => format!("warn above {}", terms.bound(max)),
            _ => "warn".to_string(),
        },
    }
}

/// A monitor's whole condition in words: `healthy ≤ 5 · warn above 0`, or
/// `KPI` (§FS-021-monitors.3.4). A KPI has no band, so its `warn` is not
/// said.
pub fn condition(judge: &Judge, warn_by: Option<&Condition>, terms: &Terms) -> String {
    let said = healthy(judge, terms);
    match (judge, warn_by) {
        (Judge::Kpi, _) | (_, None) => said,
        (judge, Some(by)) => format!("{said} · {}", warn(by, judge.condition(), terms)),
    }
}

#[cfg(test)]
#[path = "words_tests.rs"]
mod tests;
