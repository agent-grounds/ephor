//! What a condition makes of a value (§FS-021-monitors.3.3), and the rules a
//! condition must keep to be one (§FS-021-monitors.3.1,
//! §FS-021-monitors.3.5).

use serde::{Deserialize, Serialize};

use crate::monitors::model::{Band, Condition, Kind, Value};

/// What judges a monitor's readings, once its homes are resolved
/// (§FS-021-monitors.3.6). Kept with an open drop, so a change of it is seen
/// as movement (§FS-021-monitors.13.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Judge {
    /// A `healthy`, from a home or the kind's default.
    Condition(Condition),
    /// The sample's own `ok`: `true` is ok, `false` is down.
    SampleOk,
    /// Nothing: the monitor is a KPI (§FS-021-monitors.1.2).
    Kpi,
}

impl Judge {
    pub fn is_kpi(&self) -> bool {
        matches!(self, Judge::Kpi)
    }

    pub fn condition(&self) -> Option<&Condition> {
        match self {
            Judge::Condition(condition) => Some(condition),
            _ => None,
        }
    }
}

/// The condition a kind has where no home gives one (§FS-021-monitors.3.2):
/// a boolean is healthy when `true`, a level at its first level, and a number
/// or a series has none.
pub fn kind_default(kind: Option<Kind>, levels: Option<&[String]>) -> Option<Condition> {
    match kind? {
        Kind::Boolean => Some(Condition {
            is: Some(true),
            ..Condition::default()
        }),
        Kind::Level => Some(Condition {
            within: Some(vec![levels?.first()?.clone()]),
            ..Condition::default()
        }),
        Kind::Number | Kind::Series => None,
    }
}

/// Whether a value meets a condition: every key it holds
/// (§FS-021-monitors.3.1). A key the value cannot meet, such as `min` on a
/// boolean, is not met.
pub fn meets(condition: &Condition, value: &Value, levels: Option<&[String]>) -> bool {
    if let Some(is) = condition.is {
        if value != &Value::Boolean(is) {
            return false;
        }
    }
    if condition.min.is_some() || condition.max.is_some() {
        let Some(number) = value.as_number() else {
            return false;
        };
        if condition.min.is_some_and(|min| number < min)
            || condition.max.is_some_and(|max| number > max)
        {
            return false;
        }
    }
    if condition.within.is_some() || condition.at_most.is_some() {
        let Value::Level(level) = value else {
            return false;
        };
        if let Some(within) = &condition.within {
            if !within.contains(level) {
                return false;
            }
        }
        if let Some(at_most) = &condition.at_most {
            let rank = |name: &str| levels.and_then(|levels| levels.iter().position(|l| l == name));
            match (rank(level), rank(at_most)) {
                (Some(level), Some(bound)) if level <= bound => {}
                _ => return false,
            }
        }
    }
    true
}

/// A reading's own health (§FS-021-monitors.3.3): `down` when it fails what
/// judges it, `warn` when it passes and fails `warn`, `ok` when it meets
/// both. A KPI's reading has none.
pub fn band(
    judge: &Judge,
    warn: Option<&Condition>,
    value: &Value,
    ok: Option<bool>,
    levels: Option<&[String]>,
) -> Option<Band> {
    let passes = match judge {
        Judge::Kpi => return None,
        Judge::Condition(healthy) => meets(healthy, value, levels),
        Judge::SampleOk => ok?,
    };
    Some(if !passes {
        Band::Down
    } else if warn.is_some_and(|warn| !meets(warn, value, levels)) {
        Band::Warn
    } else {
        Band::Ok
    })
}

/// Which of the language's families a key belongs to: the kind it fits.
fn family(key: &str) -> Option<Family> {
    match key {
        "is" => Some(Family::Boolean),
        "min" | "max" => Some(Family::Numeric),
        "in" | "at_most" => Some(Family::Level),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Boolean,
    Numeric,
    Level,
}

impl Family {
    fn fits(self, kind: Kind) -> bool {
        match self {
            Family::Boolean => kind == Kind::Boolean,
            Family::Numeric => kind.is_numeric(),
            Family::Level => kind == Kind::Level,
        }
    }
}

/// The keys a condition holds, in the language's order.
pub fn keys(condition: &Condition) -> Vec<&'static str> {
    let mut keys = Vec::new();
    if condition.is.is_some() {
        keys.push("is");
    }
    if condition.min.is_some() {
        keys.push("min");
    }
    if condition.max.is_some() {
        keys.push("max");
    }
    if condition.within.is_some() {
        keys.push("in");
    }
    if condition.at_most.is_some() {
        keys.push("at_most");
    }
    keys
}

/// The kind a condition makes a monitor whose kind no home states
/// (§FS-021-monitors.4.4): `is` a boolean, `in` or `at_most` a level.
/// `min` and `max` fit a number and a series alike, so they decide nothing.
pub fn implied_kind(condition: &Condition) -> Option<Kind> {
    if condition.is.is_some() {
        Some(Kind::Boolean)
    } else if condition.within.is_some() || condition.at_most.is_some() {
        Some(Kind::Level)
    } else {
        None
    }
}

/// What keeps a condition from being one, in the sentences the page shows
/// (§FS-021-monitors.3.1): a key the language does not know, a key that does
/// not fit the monitor's kind (or keys that fit no one kind, where the kind
/// is not known yet), a level `levels` does not hold, and a condition no
/// value can meet. `field` is `healthy` or `warn`.
pub fn faults(
    field: &str,
    condition: &Condition,
    kind: Option<Kind>,
    levels: Option<&[String]>,
) -> Vec<String> {
    let mut faults: Vec<String> = condition
        .unknown
        .iter()
        .map(|key| format!("{field}'s {key} is not a condition key: is, min, max, in or at_most"))
        .collect();
    let keys = keys(condition);
    match kind {
        Some(kind) => {
            for key in &keys {
                if family(key).is_some_and(|family| !family.fits(kind)) {
                    faults.push(format!("{field}'s {key} does not fit a {}", kind.name()));
                }
            }
        }
        None => {
            let first = keys.first().and_then(|key| family(key));
            if let Some(other) = keys.iter().find(|key| family(key) != first) {
                faults.push(format!(
                    "{field} mixes {} and {other}, which fit no one kind",
                    keys[0]
                ));
            }
        }
    }
    if !faults.is_empty() {
        return faults;
    }
    if let Some(levels) = levels {
        let named = condition
            .within
            .iter()
            .flatten()
            .map(|name| ("in", name))
            .chain(condition.at_most.iter().map(|name| ("at_most", name)));
        for (key, name) in named {
            if !levels.contains(name) {
                faults.push(format!(
                    "{field}'s {key} names {name}, which its levels do not hold: {}",
                    levels.join(", ")
                ));
            }
        }
    }
    if !faults.is_empty() {
        return faults;
    }
    if let (Some(min), Some(max)) = (condition.min, condition.max) {
        if min > max {
            faults.push(format!(
                "{field} can never be met: its min {} is above its max {}",
                crate::monitors::words::number(min),
                crate::monitors::words::number(max)
            ));
        }
    }
    if let (Some(within), Some(at_most), Some(levels)) =
        (&condition.within, &condition.at_most, levels)
    {
        let probe = |name: &String| meets(condition, &Value::Level(name.clone()), Some(levels));
        if !within.iter().any(probe) {
            faults.push(format!(
                "{field} can never be met: none of {} is at most {at_most}",
                within.join(", ")
            ));
        }
    }
    faults
}

/// Whether `warn` is laxer than `healthy` where that can be decided
/// (§FS-021-monitors.3.5): both have a numeric bound on the same side, and
/// the warn bound lets through a value the healthy bound refuses.
pub fn laxer(healthy: &Condition, warn: &Condition) -> Option<String> {
    let say = crate::monitors::words::number;
    if let (Some(bound), Some(lax)) = (healthy.max, warn.max) {
        if lax > bound {
            return Some(format!(
                "warn's max {} lets through what healthy's max {} refuses",
                say(lax),
                say(bound)
            ));
        }
    }
    if let (Some(bound), Some(lax)) = (healthy.min, warn.min) {
        if lax < bound {
            return Some(format!(
                "warn's min {} lets through what healthy's min {} refuses",
                say(lax),
                say(bound)
            ));
        }
    }
    None
}

#[cfg(test)]
#[path = "condition_tests.rs"]
mod tests;
