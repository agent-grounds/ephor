//! What the store keeps of one monitor in `monitors.json`
//! (§FS-021-monitors.9.1). Keeping it is the store's; changing it is
//! [`crate::monitors::machine`]'s.

use chrono::{DateTime, FixedOffset, TimeDelta, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use crate::monitors::cadence::Cadence;
use crate::monitors::condition::Judge;
use crate::monitors::episode::{Ending, Sort};
use crate::monitors::freshness::{self, Attempt, Basis, Cause, Freshness};
use crate::monitors::model::{Band, Health, Kind, Value};
use crate::monitors::route::Owner;
use crate::monitors::schedule::Schedule;

/// One monitor's record (§FS-021-monitors.9.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub schedule: Schedule,
    #[serde(default)]
    pub hold: Hold,
    /// The last passing reading's value and when it was measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_ok: Option<Kept>,
    /// When the reading the value holds from was read
    /// (§FS-021-monitors.7.1); none before the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_from: Option<DateTime<Utc>>,
    /// The last reading's value and when it was measured, which a stale read
    /// is judged against (§FS-021-monitors.9.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_reading: Option<Kept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_attempt: Option<Attempt>,
    /// A sample's own `at` that lay after its read, noted on the row
    /// (§FS-021-monitors.7.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skew: Option<DateTime<FixedOffset>>,
    /// The cause of an unknown monitor, as last judged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<Cause>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop: Option<OpenDrop>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unknown: Option<OpenUnknown>,
    /// The episodes it has had (§FS-021-monitors.9.1).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub past: Vec<Past>,
    /// The name of its last episode of each sort, which the next must be
    /// later than (§FS-021-monitors.11.3).
    #[serde(default)]
    pub named: Named,
    /// The answer that owns it, where it is read from the answer that brings
    /// it (§FS-021-monitors.5.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<Owner>,
    /// When an answer last brought it, for one nothing declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_brought: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<Claim>,
    /// The last accepted sample, whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sample: Option<serde_json::Value>,
    /// The kind it was read as, so a change of it is seen
    /// (§FS-021-monitors.4.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
}

impl Record {
    /// What its freshness is judged from (§FS-021-monitors.7.1): the read
    /// its value holds from, or, with no reading yet, when ephor first saw it.
    pub fn basis(&self) -> Basis<'_> {
        Basis {
            held_from: self.held_from.unwrap_or(self.schedule.first_seen),
            has_reading: self.held_from.is_some(),
            origin: self.schedule.origin(),
            last_attempt: self.last_attempt.as_ref(),
            parked_since: self.schedule.parked_since,
        }
    }

    /// Where its value stands at `now` under `cadence`, judged without
    /// changing the record (§FS-021-monitors.7): what the page, `ephor
    /// monitors`, the doctor and Waiting's overview line read, since they
    /// write nothing (§FS-021-monitors.18.1, §FS-021-monitors.21.1).
    /// `slot_error` is the monitor slot's failure, every late monitor's cause
    /// while it lasts (§FS-021-monitors.7.2).
    pub fn freshness<Tz: TimeZone>(
        &self,
        cadence: &Cadence,
        now: DateTime<Utc>,
        zone: &Tz,
        slot_error: Option<&str>,
    ) -> Freshness {
        freshness::freshness(&self.basis(), cadence, now, zone, slot_error)
    }

    /// A monitor ephor sees for the first time at `now`.
    pub fn new(now: DateTime<Utc>) -> Record {
        Record {
            schedule: Schedule::new(now),
            hold: Hold::default(),
            last_ok: None,
            held_from: None,
            last_reading: None,
            last_attempt: None,
            skew: None,
            cause: None,
            drop: None,
            unknown: None,
            past: Vec::new(),
            named: Named::default(),
            owner: None,
            last_brought: None,
            claim: None,
            last_sample: None,
            kind: None,
        }
    }
}

/// A value and when it was measured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    pub t: DateTime<Utc>,
    pub v: Value,
}

/// The health after the hold, and the count that moves it
/// (§FS-021-monitors.10).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Hold {
    /// `down`, `warn` or `ok` after the hold; none before the first reading,
    /// and for a KPI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<Health>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<DateTime<Utc>>,
    /// The last reading's own band.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub band: Option<Band>,
    /// How many readings in a row have passed, or failed, as it did
    /// (§FS-021-monitors.10.2).
    #[serde(default)]
    pub run: u32,
}

/// A hold that is counting, as the page says it: `failing 1 of 2` or
/// `recovering 1 of 2` (§FS-021-monitors.18.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Counting {
    Failing { of: u32, run: u32 },
    Recovering { of: u32, run: u32 },
}

impl Counting {
    pub fn say(&self) -> String {
        match self {
            Counting::Failing { of, run } => format!("failing {run} of {of}"),
            Counting::Recovering { of, run } => format!("recovering {run} of {of}"),
        }
    }
}

impl Hold {
    /// Whether the hold is counting toward a drop or a recovery.
    pub fn counting(&self, down: bool, drop_after: u32, recover_after: u32) -> Option<Counting> {
        let passing = self.band?.passes();
        match (down, passing) {
            (true, true) if self.run < recover_after => Some(Counting::Recovering {
                of: recover_after,
                run: self.run,
            }),
            (false, false) if self.run < drop_after => Some(Counting::Failing {
                of: drop_after,
                run: self.run,
            }),
            _ => None,
        }
    }
}

/// An open drop episode (§FS-021-monitors.9.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenDrop {
    pub named: DateTime<Utc>,
    /// When its matter last moved (§FS-021-monitors.13.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved: Option<DateTime<Utc>>,
    /// The worst level it has reached, for a level monitor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worst: Option<String>,
    /// The condition it was last judged under, and that condition in words.
    pub judged_under: Judge,
    pub judged_words: String,
    /// The work laid on it it has seen end (§FS-021-monitors.15).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub works_ended: Vec<String>,
}

/// An open unknown episode (§FS-021-monitors.9.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenUnknown {
    pub named: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved: Option<DateTime<Utc>>,
    #[serde(flatten)]
    pub cause: Cause,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub works_ended: Vec<String>,
}

/// An episode that ended, kept as long as the history it falls in
/// (§FS-021-monitors.9.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Past {
    pub sort: Sort,
    pub named: DateTime<Utc>,
    pub ended: DateTime<Utc>,
    #[serde(flatten)]
    pub how: Ending,
}

/// The last name each sort of episode took.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Named {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unknown: Option<DateTime<Utc>>,
}

/// A read's claim on a due monitor (§FS-021-monitors.6.9): the process, the
/// time and the slot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Claim {
    pub pid: u32,
    pub at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<DateTime<Utc>>,
}

impl Claim {
    /// Whether it is void: older than the command's timeout plus a minute,
    /// so the process that wrote it is gone (§FS-021-monitors.6.9).
    pub fn is_void(&self, timeout_seconds: u64, now: DateTime<Utc>) -> bool {
        let timeout = i64::try_from(timeout_seconds).unwrap_or(i64::MAX / 2);
        let limit = TimeDelta::try_seconds(timeout)
            .and_then(|timeout| timeout.checked_add(&TimeDelta::minutes(1)))
            .unwrap_or(TimeDelta::MAX);
        now.signed_duration_since(self.at) > limit
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
