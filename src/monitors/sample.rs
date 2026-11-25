//! One sample checked against its monitor (§FS-021-monitors.8.2).
//!
//! A sample is checked on its own. One whose value does not fit its monitor's
//! kind, or whose description breaks a rule once its monitor's homes are laid
//! over it, is an unreadable read of that monitor and nothing else.

use chrono::{DateTime, FixedOffset};

use crate::monitors::model::{Kind, Point, Sample, Value};
use crate::monitors::overlay::{self, Homes, Monitor};
use crate::monitors::words;

/// A sample its monitor accepts: the monitor as this sample resolves it, and
/// the measurement (§FS-021-monitors.8.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Checked {
    /// The monitor with this sample's description laid under its
    /// declarations (§FS-021-monitors.4.4).
    pub monitor: Monitor,
    /// The kind it was read as: the monitor's, or the one the value states
    /// where nothing else does (§FS-021-monitors.4.3).
    pub kind: Kind,
    /// The value: a series' last point's `v`, in the order the points are
    /// listed (§FS-021-monitors.2.4).
    pub value: Value,
    /// This reading's whole: the sample's, or the declaration's where it
    /// states none (§FS-021-monitors.2.2).
    pub of: Option<f64>,
    pub ok: Option<bool>,
    pub at: Option<DateTime<FixedOffset>>,
    /// A series' last point, which says whether a read brings the
    /// measurement already held (§FS-021-monitors.2.4).
    pub last_point: Option<Point>,
}

/// Checks one sample against the monitor its id names (§FS-021-monitors.8.2),
/// with the sample's own description laid under the monitor's declarations
/// (§FS-021-monitors.4.3). The error is the sentence the unreadable read
/// shows.
pub fn check(sample: &Sample, homes: &Homes) -> Result<Checked, String> {
    let laid = Homes {
        sample: Some(&sample.description),
        ..*homes
    };
    let mut monitor = overlay::resolve(&sample.id, &laid);
    if monitor.kind.is_none() {
        let kind = stated_kind(sample, monitor.levels.is_some())?;
        monitor = overlay::resolve(
            &sample.id,
            &Homes {
                first_kind: Some(kind),
                ..laid
            },
        );
    }
    if let Some(fault) = monitor.faults.first() {
        return Err(fault.clone());
    }
    let kind = monitor.kind.expect("a kind was inferred above");
    let value = value_of(sample, kind, monitor.levels.as_deref())?;
    let last_point = match kind {
        Kind::Series => sample
            .points
            .as_ref()
            .and_then(|points| points.last().cloned()),
        _ => None,
    };
    Ok(Checked {
        of: sample.of.or(monitor.of),
        kind,
        value,
        ok: sample.ok,
        at: sample.at,
        last_point,
        monitor,
    })
}

/// The kind a sample's value states where nothing else does
/// (§FS-021-monitors.4.3): `true` or `false` a boolean, a number a number,
/// points with no value a series, and a string a level only where levels say
/// which.
fn stated_kind(sample: &Sample, has_levels: bool) -> Result<Kind, String> {
    match (&sample.value, &sample.points) {
        (None, Some(_)) => Ok(Kind::Series),
        (None, None) => Err("the sample has no value".to_string()),
        (Some(Value::Level(level)), _) if !has_levels => Err(format!(
            "the value \"{level}\" is a level's name, and no levels say which levels there are"
        )),
        (Some(value), _) => Ok(value.kind()),
    }
}

/// The value a sample gives a monitor of `kind` (§FS-021-monitors.2), or why
/// it gives none.
fn value_of(sample: &Sample, kind: Kind, levels: Option<&[String]>) -> Result<Value, String> {
    if kind == Kind::Series {
        let Some(points) = &sample.points else {
            return Err("a series' sample carries points, and this one has none".to_string());
        };
        let last = points
            .last()
            .expect("the shape refuses an empty list of points");
        return Ok(Value::Number(last.v));
    }
    let Some(value) = &sample.value else {
        return Err("the sample has no value".to_string());
    };
    if value.kind() != kind {
        return Err(format!(
            "the value {} does not fit a {}",
            spelled(value),
            kind.name()
        ));
    }
    if kind == Kind::Number
        && sample
            .points
            .iter()
            .flatten()
            .any(|point| point.x.is_some())
    {
        return Err(
            "the sample's points are labelled by x, and a number's are drawn against time"
                .to_string(),
        );
    }
    if let Value::Level(level) = value {
        let known = levels.is_some_and(|levels| levels.contains(level));
        if !known {
            return Err(format!(
                "the value \"{level}\" is not one of its levels: {}",
                levels.map(|levels| levels.join(", ")).unwrap_or_default()
            ));
        }
    }
    Ok(value.clone())
}

fn spelled(value: &Value) -> String {
    match value {
        Value::Boolean(value) => value.to_string(),
        Value::Number(number) => words::number(*number),
        Value::Level(level) => format!("\"{level}\""),
    }
}

#[cfg(test)]
#[path = "sample_tests.rs"]
mod tests;
