//! A monitor's episodes: how one is named, the key of its matter, how it
//! ends and why it moves (§FS-021-monitors.11, §FS-021-monitors.13,
//! §FS-021-monitors.14).

use chrono::{DateTime, SubsecRound, TimeDelta, TimeZone, Utc};
use serde::{Deserialize, Serialize};

/// The two sorts of episode a status has (§FS-021-monitors.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    /// It turned `down` (§FS-021-monitors.11.1).
    Drop,
    /// It turned unknown for a cause other than `overdue`
    /// (§FS-021-monitors.11.2).
    Unknown,
}

/// The time a new episode is named by (§FS-021-monitors.11.3): `at`, to the
/// second, or the second after the previous episode of its sort where `at`
/// is not later, so a name never repeats and never goes back.
pub fn name(at: DateTime<Utc>, previous: Option<DateTime<Utc>>) -> DateTime<Utc> {
    let at = at.trunc_subsecs(0);
    match previous {
        Some(previous) if at <= previous => previous + TimeDelta::seconds(1),
        _ => at,
    }
}

/// An episode's name in UTC, `%Y%m%dT%H%M%SZ`, such as `20261125T060000Z`:
/// no `:`, so a short id that cuts at the last `:` keeps the whole subject
/// (§FS-021-monitors.11.3).
pub fn id(named: DateTime<Utc>) -> String {
    named.format("%Y%m%dT%H%M%SZ").to_string()
}

/// The key of an episode's matter (§FS-021-monitors.11.3):
/// `monitor:<project>/<id>@<episode>` for a drop and
/// `monitor:<project>/<id>.unknown@<episode>` for an unknown monitor.
pub fn key(project: &str, monitor: &str, sort: Sort, named: DateTime<Utc>) -> String {
    let suffix = match sort {
        Sort::Drop => "",
        Sort::Unknown => ".unknown",
    };
    format!("monitor:{project}/{monitor}{suffix}@{}", id(named))
}

/// Why a monitor stopped being watched as it was (§FS-021-monitors.14.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unwatched {
    /// No longer declared or brought.
    Gone,
    /// Nothing judges it any more: it is a KPI.
    Kpi,
    /// Its kind changed, and its history starts again.
    KindChanged,
    /// Its declaration is in error (§FS-021-monitors.4.6).
    InError,
    /// Its row runs no command for it (§FS-021-monitors.4.5).
    NotRun,
}

impl Unwatched {
    /// The reason a finished matter gives.
    pub fn reason(&self) -> &'static str {
        match self {
            Unwatched::Gone => "it is no longer declared or brought",
            Unwatched::Kpi => "nothing judges it any more",
            Unwatched::KindChanged => "its kind changed",
            Unwatched::InError => "its declaration is in error",
            Unwatched::NotRun => "its row runs no command for it",
        }
    }
}

/// How an episode ended (§FS-021-monitors.14).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "how", rename_all = "snake_case")]
pub enum Ending {
    /// A drop had `recover_after` passing readings in a row; `value` is the
    /// last one's, as the page says it.
    Recovered { value: String },
    /// An unknown episode's first reading.
    ReadAgain { value: String },
    /// It stopped being watched as it was.
    Unwatched { why: Unwatched },
}

impl Ending {
    /// What the finished matter says (§FS-021-monitors.14.1,
    /// §FS-021-monitors.14.2, §FS-021-monitors.14.4): `recovered at Nov 25
    /// 06:02 (81.0%)`, `read again at …`, `no longer watched at …: <reason>`,
    /// its time on the local clock.
    pub fn say<Tz: TimeZone>(&self, at: DateTime<Utc>, zone: &Tz) -> String
    where
        Tz::Offset: std::fmt::Display,
    {
        let at = at.with_timezone(zone).format("%b %-d %H:%M");
        match self {
            Ending::Recovered { value } => format!("recovered at {at} ({value})"),
            Ending::ReadAgain { value } => format!("read again at {at} ({value})"),
            Ending::Unwatched { why } => format!("no longer watched at {at}: {}", why.reason()),
        }
    }
}

/// Why an open episode's matter moved (§FS-021-monitors.13.1,
/// §FS-021-monitors.13.2, §FS-021-monitors.15).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "why", rename_all = "snake_case")]
pub enum Movement {
    /// A level reached a level worse than any its open drop had reached.
    LevelWorse { from: String, to: String },
    /// The `healthy` that judges it changed, in words.
    ConditionChanged { from: String, to: String },
    /// A drop still down after the work laid on it ended.
    StillDown,
    /// An unknown monitor still not read after the work laid on it ended,
    /// with the cause's word.
    StillUnknown { cause: String },
}

impl Movement {
    /// The reason the resurfaced matter names (§FS-021-monitors.13.2).
    pub fn say(&self) -> String {
        match self {
            Movement::LevelWorse { from, to } => format!("level worse: {from} → {to}"),
            Movement::ConditionChanged { from, to } => format!("condition changed: {from} → {to}"),
            Movement::StillDown => "still down after its work ended".to_string(),
            Movement::StillUnknown { cause } => format!("still {cause} after its work ended"),
        }
    }

    /// Whether the work laid on the matter is over, so it is New again and
    /// offered work again (§FS-021-monitors.13.2).
    pub fn renews(&self) -> bool {
        matches!(self, Movement::StillDown | Movement::StillUnknown { .. })
    }
}

#[cfg(test)]
#[path = "episode_tests.rs"]
mod tests;
