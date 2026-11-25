//! A monitor's history: one line per reading, an unknown marker per
//! unreadable read, and the lines retention thins them to
//! (§FS-021-monitors.9.1, §FS-021-monitors.9.3), judged again for display
//! under today's condition (§FS-021-monitors.18.9).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::monitors::condition::{self, Judge};
use crate::monitors::model::{Condition, Health, Value};

/// One line of `<id>.jsonl` (§FS-021-monitors.9.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Line {
    /// An hour's or a day's readings, thinned (§FS-021-monitors.9.3):
    /// `{t, min, mean, max, n, health}`, with no `mean` for a boolean or a
    /// level, and `health` the worst among them, an unknown marker included.
    Thinned {
        t: DateTime<Utc>,
        min: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mean: Option<f64>,
        max: Value,
        n: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        health: Option<Health>,
    },
    /// One reading, `{t, v, health}`: `t` when it was measured, `health` its
    /// own before the hold, and none for a KPI. One read out of cadence says
    /// so, `out_of_cadence: true`, which the detail view notes
    /// (§FS-021-monitors.19).
    Reading {
        t: DateTime<Utc>,
        v: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        health: Option<Health>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        out_of_cadence: bool,
    },
    /// An unreadable read, `{t, health: "unknown"}`, `t` when it was read
    /// (§FS-021-monitors.9.2).
    Marker { t: DateTime<Utc>, health: Health },
}

impl Line {
    /// An unknown marker read at `t`.
    pub fn marker(t: DateTime<Utc>) -> Line {
        Line::Marker {
            t,
            health: Health::Unknown,
        }
    }

    pub fn t(&self) -> DateTime<Utc> {
        match self {
            Line::Thinned { t, .. } | Line::Reading { t, .. } | Line::Marker { t, .. } => *t,
        }
    }

    /// The health recorded when it happened, which is never recomputed
    /// (§FS-021-monitors.18.9).
    pub fn health(&self) -> Option<Health> {
        match self {
            Line::Thinned { health, .. } | Line::Reading { health, .. } => *health,
            Line::Marker { health, .. } => Some(*health),
        }
    }

    pub fn is_reading(&self) -> bool {
        matches!(self, Line::Reading { .. })
    }
}

/// A line's health as the page draws it, judged under today's condition
/// (§FS-021-monitors.18.9): a thinned line takes the worse of its least and
/// greatest value, a marker stays unknown, and a line judged by its sample's
/// own `ok`, which history does not keep, keeps the health it was recorded
/// with.
pub fn rejudged(
    line: &Line,
    judge: &Judge,
    warn: Option<&Condition>,
    levels: Option<&[String]>,
) -> Option<Health> {
    if *judge == Judge::SampleOk {
        return line.health();
    }
    let judged =
        |value: &Value| condition::band(judge, warn, value, None, levels).map(Health::from);
    match line {
        Line::Marker { .. } => Some(Health::Unknown),
        Line::Reading { v, .. } => judged(v),
        Line::Thinned { min, max, .. } => worst([judged(min), judged(max)]),
    }
}

/// The worst of some healths: down, then unknown, then warn, then ok
/// (§FS-021-monitors.9.3).
pub fn worst(healths: impl IntoIterator<Item = Option<Health>>) -> Option<Health> {
    healths
        .into_iter()
        .flatten()
        .max_by_key(|health| health.badness())
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
