//! What the store keeps of one monitor in `monitors.json`
//! (§FS-021-monitors.9.1). Keeping it is the store's; changing it is
//! [`crate::monitors::machine`]'s.

use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset, TimeDelta, TimeZone, Utc};
use serde::{Deserialize, Deserializer, Serialize};

use crate::monitors::cadence::Cadence;
use crate::monitors::condition::Judge;
use crate::monitors::episode::{Ending, Movement, Sort};
use crate::monitors::freshness::{self, Attempt, Basis, Cause, Freshness};
use crate::monitors::model::{Band, Health, Kind, Point, Sample, Value};
use crate::monitors::route::Owner;
use crate::monitors::schedule::Schedule;

/// `monitors.json`: what the store keeps of a project's monitors
/// (§FS-021-monitors.9.1), each record by its monitor's id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    #[serde(default = "version")]
    pub version: u32,
    /// When retention last thinned the project's histories, which it does at
    /// most once a day (§FS-021-monitors.9.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinned: Option<DateTime<Utc>>,
    #[serde(default)]
    pub monitors: BTreeMap<String, Record>,
    /// What is left of each monitor the store forgot (§FS-021-monitors.5.6):
    /// its ended episodes, kept while the recency window still reports their
    /// matters (§FS-021-monitors.14.3, §FS-021-monitors.17.1).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub forgotten: BTreeMap<String, Vec<Past>>,
}

fn version() -> u32 {
    1
}

impl Default for Document {
    /// An empty store, as a project with no store yet reads.
    fn default() -> Document {
        Document {
            version: version(),
            thinned: None,
            monitors: BTreeMap::new(),
            forgotten: BTreeMap::new(),
        }
    }
}

impl Document {
    /// Whether retention is due to thin the project's histories at `now`: a
    /// day after it last did, or never yet (§FS-021-monitors.9.3).
    pub fn due_to_thin(&self, now: DateTime<Utc>) -> bool {
        self.thinned
            .is_none_or(|thinned| now.signed_duration_since(thinned) >= TimeDelta::days(1))
    }

    /// Whether the project has monitors by the store alone: it keeps a record
    /// of one, or a forgotten one's finished matters (§FS-021-monitors.1.4).
    pub fn keeps_any(&self) -> bool {
        !self.monitors.is_empty() || !self.forgotten.is_empty()
    }

    /// Lets a forgotten monitor's ended episodes go once the recency window
    /// no longer reports them (§FS-021-monitors.14.3).
    pub fn prune_forgotten(&mut self, recent: TimeDelta, now: DateTime<Utc>) {
        for past in self.forgotten.values_mut() {
            past.retain(|episode| now.signed_duration_since(episode.ended) <= recent);
        }
        self.forgotten.retain(|_, past| !past.is_empty());
    }
}

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
    /// The last accepted sample, whole (§FS-021-monitors.9.1). One the
    /// store holds that no longer reads as a sample is dropped rather than
    /// refusing the store.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_sample"
    )]
    pub last_sample: Option<Sample>,
    /// The points of the last sample that carried any, where the last
    /// accepted sample carries none (§FS-021-monitors.2.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earlier_points: Option<Vec<Point>>,
    /// The file it was last read from as that reading found it, which the
    /// next read of the file is stale against while neither its content
    /// nor its modification time changes (§FS-021-monitors.9.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<FileMark>,
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

    /// The points drawn before its first reading: those of the last sample
    /// that carried any (§FS-021-monitors.2.2).
    pub fn points(&self) -> Option<&[Point]> {
        self.last_sample
            .as_ref()
            .and_then(|sample| sample.points.as_deref())
            .or(self.earlier_points.as_deref())
    }

    /// Keeps an accepted sample whole (§FS-021-monitors.9.1), and the points
    /// of the one before it where this one carries none
    /// (§FS-021-monitors.2.2).
    pub fn keep_sample(&mut self, sample: Sample) {
        let earlier = self.last_sample.replace(sample);
        let carried = self
            .last_sample
            .as_ref()
            .is_some_and(|s| s.points.is_some());
        if carried {
            self.earlier_points = None;
        } else if let Some(points) = earlier.and_then(|earlier| earlier.points) {
            self.earlier_points = Some(points);
        }
    }

    /// Whether a read of the file it was last read from finds it as it was:
    /// the same content and the same modification time, so the read is
    /// stale (§FS-021-monitors.9.2).
    pub fn file_unchanged(&self, mark: &FileMark) -> bool {
        self.file.as_ref() == Some(mark)
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
            earlier_points: None,
            file: None,
            kind: None,
        }
    }
}

fn lenient_sample<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Sample>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| Sample::try_from(value).ok()))
}

/// A value, its whole, and when it was measured (§FS-021-monitors.2.2):
/// `of` is part of the measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    pub t: DateTime<Utc>,
    pub v: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<f64>,
}

/// A file as one read found it: its modification time and a digest of its
/// content (§FS-021-monitors.9.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMark {
    pub mtime: DateTime<Utc>,
    pub digest: String,
}

/// When an open episode's matter last moved and why, which its time and its
/// resurfacing reason are (§FS-021-monitors.12.2, §FS-021-monitors.13.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moved {
    pub at: DateTime<Utc>,
    #[serde(flatten)]
    pub why: Movement,
}

/// What an episode's matter says of its monitor (§FS-021-monitors.12.1,
/// §FS-021-monitors.12.3) as of its last step: kept with the episode, and
/// with it once it ends, so a finished matter is still said as it was while
/// it is reported (§FS-021-monitors.14.3).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Shown {
    pub title: String,
    /// The last reading's value and its whole; none before the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// What judges it, in words (§FS-021-monitors.3.4).
    pub condition: String,
    /// When its health began.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_ok: Option<Kept>,
    /// The last accepted sample's `detail` and `url`, its matter's link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
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
    /// When its matter last moved and why (§FS-021-monitors.13.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved: Option<Moved>,
    /// The worst level it has reached, for a level monitor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worst: Option<String>,
    /// The condition it was last judged under, and that condition in words.
    pub judged_under: Judge,
    pub judged_words: String,
    /// The work laid on it it has seen end (§FS-021-monitors.15).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub works_ended: Vec<String>,
    #[serde(default)]
    pub shown: Shown,
}

/// An open unknown episode (§FS-021-monitors.9.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenUnknown {
    pub named: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved: Option<Moved>,
    #[serde(flatten)]
    pub cause: Cause,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub works_ended: Vec<String>,
    #[serde(default)]
    pub shown: Shown,
}

/// An episode that ended, kept as long as the history it falls in
/// (§FS-021-monitors.9.1), with what its finished matter says
/// (§FS-021-monitors.14.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Past {
    pub sort: Sort,
    pub named: DateTime<Utc>,
    pub ended: DateTime<Utc>,
    #[serde(flatten)]
    pub how: Ending,
    /// When its matter last moved, before it ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved: Option<Moved>,
    /// An unknown episode's last cause (§FS-021-monitors.12.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<Cause>,
    #[serde(default)]
    pub shown: Shown,
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
