//! Where one monitor stands on the Monitors page (§FS-021-monitors.18): the
//! rank its row shows (§FS-021-monitors.18.3), its health column
//! (§FS-021-monitors.18.7), its health and since when, and the header count
//! it falls under (§FS-021-monitors.18.2).
//!
//! One judgement for every reader of it: the page and `ephor monitors` draw
//! it, `--check` reads its count (§FS-021-monitors.21.2), and Waiting's
//! overview line counts the same rows (§FS-021-monitors.20.1), so the three
//! cannot disagree about a monitor. It reads the record and writes nothing.

use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use serde::Serialize;

use crate::monitors::cadence::Cadence;
use crate::monitors::condition;
use crate::monitors::episode::{Ending, Sort};
use crate::monitors::freshness::{self, Cause, Freshness};
use crate::monitors::model::{Health, Value};
use crate::monitors::overlay::{Monitor, NOT_RUN};
use crate::monitors::state::{Counting, Record};

/// What a row shows, worst first (§FS-021-monitors.18.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Rank {
    /// Down, recovering included, and down while it is also unknown.
    Down,
    /// Unknown, a KPI's included; not run, not read, and not read yet.
    Unknown,
    /// Past its condition, short of `drop_after`.
    Failing,
    Warn,
    Ok,
    Kpi,
}

impl Rank {
    /// The glyph that carries the rank in shape as well as colour, so the
    /// page reads without colour (§FS-021-monitors.18.3).
    pub fn glyph(self) -> &'static str {
        match self {
            Rank::Down => "✖",
            Rank::Unknown => "?",
            Rank::Failing => "▼",
            Rank::Warn => "▲",
            Rank::Ok => "●",
            Rank::Kpi => "◆",
        }
    }

    /// The rank as the header counts it: `down`, `unknown`, `failing`,
    /// `warn`, `ok` or `KPIs` (§FS-021-monitors.18.2).
    pub fn counted(self) -> &'static str {
        match self {
            Rank::Down => "down",
            Rank::Unknown => "unknown",
            Rank::Failing => "failing",
            Rank::Warn => "warn",
            Rank::Ok => "ok",
            Rank::Kpi => "KPIs",
        }
    }
}

/// The health column (§FS-021-monitors.18.7): what it says, and for how long
/// where it says a duration. `ephor monitors` prints the two as its `health`
/// and `since` columns (§FS-021-monitors.21.1).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Column {
    /// `parked`, `not run`, `not read yet`, `failing 1 of 2`, `recovered`,
    /// `down`, `KPI`, …
    pub state: String,
    /// `3h 50m`, or `25 points` for a series drawn by `x` labels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lasted: Option<String>,
    /// The sentence a monitor ephor does not read carries
    /// (§FS-021-monitors.4.5, §FS-021-monitors.4.6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentence: Option<String>,
}

impl Column {
    fn said(state: impl Into<String>, lasted: Option<String>) -> Column {
        Column {
            state: state.into(),
            lasted,
            sentence: None,
        }
    }

    /// The whole column: `parked 3h 50m`, `failing 1 of 2`, `ok 12d`.
    pub fn text(&self) -> String {
        match &self.lasted {
            Some(lasted) => format!("{} {lasted}", self.state),
            None => self.state.clone(),
        }
    }
}

/// What one monitor's record says it stands at, at a moment.
#[derive(Debug, Clone, PartialEq)]
pub struct Standing {
    pub rank: Rank,
    /// Its health (§FS-021-monitors.1.3): none for a KPI that holds.
    pub health: Option<Health>,
    /// When that health began.
    pub since: Option<DateTime<Utc>>,
    pub column: Column,
    /// The header count it falls under; none where it is not yet a failure:
    /// not run because its row trusts only descriptions, or not read yet
    /// inside its first grace (§FS-021-monitors.18.2).
    pub counts: Option<Rank>,
    /// Unknown because it is overdue, which the header counts a second time
    /// (§FS-021-monitors.18.2).
    pub overdue: bool,
    /// Something judges it: a status, which `--check` and Waiting's overview
    /// line read, where a KPI never counts (§FS-021-monitors.21.2).
    pub status: bool,
    /// Where its value stands, none for a monitor ephor does not read or has
    /// never seen (§FS-021-monitors.7).
    pub freshness: Option<Freshness>,
}

/// What a monitor's standing is judged from.
#[derive(Debug, Clone, Copy)]
pub struct Facts<'a> {
    pub monitor: &'a Monitor,
    pub cadence: &'a Cadence,
    pub record: Option<&'a Record>,
    /// The monitor slot's failure, every late monitor's cause while it lasts
    /// (§FS-021-monitors.7.2).
    pub slot_error: Option<&'a str>,
    /// How long a finished matter is still reported (§FS-021-monitors.14.3).
    pub recent: TimeDelta,
}

/// Where one monitor stands at `now` (§FS-021-monitors.18.3,
/// §FS-021-monitors.18.7). The health column says the first of these that
/// applies: the cause and how long for a late or unknown monitor; `not run`
/// or `not read` with its sentence, or `not read yet`; a hold counting; a
/// recovery whose finished matter is still reported; the health and how
/// long; `KPI`. While a drop is open the row is down, whatever its freshness.
pub fn standing<Tz: TimeZone>(facts: &Facts, now: DateTime<Utc>, zone: &Tz) -> Standing {
    let monitor = facts.monitor;
    let sample_ok = facts
        .record
        .and_then(|record| record.last_sample.as_ref())
        .and_then(|sample| sample.ok);
    let status = monitor.is_status(sample_ok);
    let unread = |state: &str, sentence: Option<String>, counts: Option<Rank>| Standing {
        rank: Rank::Unknown,
        health: Some(Health::Unknown),
        since: None,
        column: Column {
            sentence,
            ..Column::said(state, None)
        },
        counts,
        overdue: false,
        status,
        freshness: None,
    };
    // A declaration in error is shown and not read, and counts as unknown
    // (§FS-021-monitors.4.6, §FS-021-monitors.21.2).
    if !monitor.faults.is_empty() {
        let sentence = format!("not read: {}", monitor.faults.join("; "));
        return unread("not read", Some(sentence), Some(Rank::Unknown));
    }
    if monitor.not_run {
        return unread("not run", Some(NOT_RUN.to_string()), None);
    }
    let Some(record) = facts.record else {
        return unread("not read yet", None, None);
    };
    let fresh = record.freshness(facts.cadence, now, zone, facts.slot_error);
    if let Freshness::NotReadYet { .. } = fresh {
        return Standing {
            freshness: Some(fresh),
            ..unread("not read yet", None, None)
        };
    }
    let down = record.drop.is_some() || record.hold.health == Some(Health::Down);
    let unknown = fresh.is_unknown();
    let counting = record.hold.counting(
        down,
        monitor.drop_after,
        monitor.recover_after(facts.cadence),
    );
    let rank = if down {
        Rank::Down
    } else if unknown {
        Rank::Unknown
    } else if !status {
        Rank::Kpi
    } else if matches!(counting, Some(Counting::Failing { .. })) {
        Rank::Failing
    } else {
        match record.hold.health {
            Some(Health::Warn) => Rank::Warn,
            Some(Health::Ok) => Rank::Ok,
            _ => Rank::Unknown,
        }
    };
    let (health, since) = match &fresh {
        Freshness::Unknown { since, .. } if !down => (Some(Health::Unknown), Some(*since)),
        _ => (record.hold.health, record.hold.since),
    };
    let column = column(facts, record, (&fresh, counting, status), now, zone);
    Standing {
        rank,
        health,
        since,
        column,
        counts: Some(rank),
        overdue: matches!(
            fresh,
            Freshness::Unknown {
                cause: Cause::Overdue,
                ..
            }
        ),
        status,
        freshness: Some(fresh),
    }
}

/// The health column of a monitor ephor reads (§FS-021-monitors.18.7).
fn column<Tz: TimeZone>(
    facts: &Facts,
    record: &Record,
    (fresh, counting, status): (&Freshness, Option<Counting>, bool),
    now: DateTime<Utc>,
    zone: &Tz,
) -> Column {
    if let Some(cause) = fresh.cause() {
        // How long since the slot that passed without a reading, which is
        // when the cause began to apply (§FS-021-monitors.7.3).
        let began = freshness::late_from(&record.basis(), facts.cadence, zone).unwrap_or(now);
        return Column::said(cause.name(), Some(lasted(now - began)));
    }
    if let Some(counting) = counting {
        return Column::said(counting.say(), None);
    }
    if record.drop.is_none() {
        let recovered = record
            .past
            .iter()
            .filter(|past| past.sort == Sort::Drop && past.ended >= now - facts.recent)
            .filter(|past| matches!(past.how, Ending::Recovered { .. }))
            .map(|past| past.ended)
            .max();
        if let Some(ended) = recovered {
            return Column::said("recovered", Some(lasted(now - ended)));
        }
    }
    let Some(health) = record.hold.health else {
        return Column::said(if status { "unknown" } else { "KPI" }, None);
    };
    if let Some(points) = labelled(facts.monitor, record, health) {
        return Column::said(health.name(), Some(points));
    }
    let held = record.hold.since.map(|since| lasted(now - since));
    Column::said(health.name(), held)
}

/// How many of a series' last points, drawn by `x` labels, its health has
/// held for (§FS-021-monitors.18.7): `25 points`. None for a monitor whose
/// points carry times.
fn labelled(monitor: &Monitor, record: &Record, health: Health) -> Option<String> {
    let points = record.points()?;
    if points.is_empty() || points.iter().any(|point| point.t.is_some()) {
        return None;
    }
    let judge = monitor.judge(None);
    let levels = monitor.levels.as_deref();
    let held = points
        .iter()
        .rev()
        .take_while(|point| {
            condition::band(
                &judge,
                monitor.warn.as_ref(),
                &Value::Number(point.v),
                None,
                levels,
            )
            .map(Health::from)
                == Some(health)
        })
        .count();
    Some(match held {
        1 => "1 point".to_string(),
        held => format!("{held} points"),
    })
}

/// How long something has held, as the health column says it: `50m`,
/// `3h 50m`, `2d 4h`, `12d`, and in weeks from eight weeks on, `9w`.
pub fn lasted(length: TimeDelta) -> String {
    let minutes = length.num_minutes().max(0);
    let days = minutes / 1440;
    let hours = minutes % 1440 / 60;
    let minutes = minutes % 60;
    if days >= 56 {
        return format!("{}w", (days + 3) / 7);
    }
    match (days, hours, minutes) {
        (0, 0, minutes) => format!("{minutes}m"),
        (0, hours, 0) => format!("{hours}h"),
        (0, hours, minutes) => format!("{hours}h {minutes}m"),
        (days, 0, _) => format!("{days}d"),
        (days, hours, _) => format!("{days}d {hours}h"),
    }
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
