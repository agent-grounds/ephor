//! What of a monitor's history is kept (§FS-021-monitors.9.3).
//!
//! Every reading for 30 days, then one line an hour to 90 days, then one a
//! day to two years, and nothing older; a monitor read daily or less often is
//! never thinned, only cut; and the last 104 readings stay whatever their
//! age. Hours and days are counted in UTC, and an hour or a day is thinned
//! only once all of it is past its boundary, and cut only once all of it is,
//! so thinning again changes nothing.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use chrono::{DateTime, DurationRound, TimeDelta, Utc};

use crate::monitors::history::{worst, Line};
use crate::monitors::model::{Health, Kind, Value};
use crate::monitors::state::Past;

/// How many readings stay whatever their age: two years of a weekly monitor.
pub const KEPT_READINGS: usize = 104;

/// How long every reading is kept.
pub fn raw_for() -> TimeDelta {
    TimeDelta::days(30)
}

/// How long one line an hour is kept.
pub fn hourly_for() -> TimeDelta {
    TimeDelta::days(90)
}

/// How long anything is kept.
pub fn kept_for() -> TimeDelta {
    TimeDelta::days(730)
}

/// A monitor's history thinned and cut as of `now` (§FS-021-monitors.9.3),
/// in time order. `interval` is the monitor's, and `kind` and `levels` give
/// a boolean's and a level's order.
pub fn thin(
    lines: &[Line],
    interval: TimeDelta,
    kind: Kind,
    levels: Option<&[String]>,
    now: DateTime<Utc>,
) -> Vec<Line> {
    let readings: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].is_reading())
        .collect();
    let protected = &readings[readings.len().saturating_sub(KEPT_READINGS)..];
    let thinned = interval < TimeDelta::days(1);
    // A thinned monitor keeps one line a day near the cut, named by the day's
    // start, so the cut takes a day only once all of it is older than two
    // years: a reading inside the two years is never lost to its day's name.
    let cut = now - kept_for();
    let cut = match thinned {
        true => cut.duration_trunc(TimeDelta::days(1)).unwrap_or(cut),
        false => cut,
    };
    let raw_until = now - raw_for();
    let hourly_until = now - hourly_for();

    let mut kept: Vec<Line> = Vec::new();
    let mut buckets: BTreeMap<DateTime<Utc>, Vec<&Line>> = BTreeMap::new();
    for (index, line) in lines.iter().enumerate() {
        let t = line.t();
        if protected.binary_search(&index).is_ok() {
            kept.push(line.clone());
            continue;
        }
        if t < cut {
            continue;
        }
        match thinned
            .then(|| bucket(t, raw_until, hourly_until))
            .flatten()
        {
            Some(start) => buckets.entry(start).or_default().push(line),
            None => kept.push(line.clone()),
        }
    }
    kept.extend(
        buckets
            .into_iter()
            .map(|(start, lines)| fold(start, &lines, kind, levels)),
    );
    kept.sort_by_key(Line::t);
    kept
}

/// The episodes a monitor keeps, as long as the history they fall in
/// (§FS-021-monitors.9.1): one that ended within two years of `now`, or no
/// earlier than the oldest line its history still keeps, `kept` being that
/// history as [`thin`] left it.
pub fn past(past: &[Past], kept: &[Line], now: DateTime<Utc>) -> Vec<Past> {
    let oldest = kept.iter().map(Line::t).min();
    let cut = now - kept_for();
    past.iter()
        .filter(|episode| episode.ended >= cut || oldest.is_some_and(|t| t <= episode.ended))
        .cloned()
        .collect()
}

/// The day or the hour a line folds into, where all of it is past its
/// boundary.
fn bucket(
    t: DateTime<Utc>,
    raw_until: DateTime<Utc>,
    hourly_until: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let day = t.duration_trunc(TimeDelta::days(1)).ok()?;
    if day + TimeDelta::days(1) <= hourly_until {
        return Some(day);
    }
    let hour = t.duration_trunc(TimeDelta::hours(1)).ok()?;
    (hour + TimeDelta::hours(1) <= raw_until).then_some(hour)
}

/// One bucket's lines as one: the least, mean and greatest value, how many
/// readings, and the worst health. Markers count toward the health alone; a
/// bucket of markers only stays one marker.
fn fold(start: DateTime<Utc>, lines: &[&Line], kind: Kind, levels: Option<&[String]>) -> Line {
    let numeric = kind.is_numeric();
    let mut least: Option<Value> = None;
    let mut greatest: Option<Value> = None;
    let mut n = 0u32;
    let mut healths: Vec<Option<Health>> = Vec::new();
    // Each line's mean and how many readings it holds, weighed once the whole
    // count is known, so a sum of large values never overflows.
    let mut means: Vec<(f64, u32)> = Vec::new();
    for line in lines {
        healths.push(line.health());
        let (low, high, count, mean) = match line {
            Line::Marker { .. } => continue,
            Line::Reading { v, .. } => (v, v, 1, v.as_number()),
            Line::Thinned {
                min, max, n, mean, ..
            } => (min, max, *n, mean.or_else(|| midpoint(min, max))),
        };
        means.extend(mean.map(|mean| (mean, count)));
        if least
            .as_ref()
            .is_none_or(|least| order(levels, low, least) == Ordering::Less)
        {
            least = Some(low.clone());
        }
        if greatest
            .as_ref()
            .is_none_or(|greatest| order(levels, high, greatest) == Ordering::Greater)
        {
            greatest = Some(high.clone());
        }
        n = n.saturating_add(count);
    }
    match (least, greatest) {
        (Some(min), Some(max)) => Line::Thinned {
            t: start,
            mean: numeric.then(|| weighed(&means, &min, &max)).flatten(),
            min,
            max,
            n,
            health: worst(healths),
        },
        _ => Line::marker(start),
    }
}

/// The mean of some lines' means, each weighed by how many readings it holds,
/// and never outside the least and greatest value.
fn weighed(means: &[(f64, u32)], min: &Value, max: &Value) -> Option<f64> {
    let total: f64 = means.iter().map(|(_, count)| f64::from(*count)).sum();
    if total == 0.0 {
        return None;
    }
    let mean: f64 = means
        .iter()
        .map(|(mean, count)| mean * (f64::from(*count) / total))
        .sum();
    Some(mean.clamp(min.as_number()?, max.as_number()?))
}

/// The middle of a thinned line that lost its mean.
fn midpoint(min: &Value, max: &Value) -> Option<f64> {
    Some(min.as_number()? / 2.0 + max.as_number()? / 2.0)
}

/// Two values in their kind's order: `false` before `true`, levels as `levels`
/// lists them, numbers by size.
fn order(levels: Option<&[String]>, a: &Value, b: &Value) -> Ordering {
    let rank = |value: &Value| match value {
        Value::Boolean(value) => f64::from(u8::from(*value)),
        Value::Number(number) => *number,
        Value::Level(level) => levels
            .and_then(|levels| levels.iter().position(|name| name == level))
            .unwrap_or(usize::MAX) as f64,
    };
    rank(a).total_cmp(&rank(b))
}

#[cfg(test)]
#[path = "retention_tests.rs"]
mod tests;
