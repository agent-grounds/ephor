//! How long what a monitor read holds, and what it is once it no longer does
//! (§FS-021-monitors.7).
//!
//! One rule for every way a value goes missing: a value holds until the first
//! slot after the reading that brought it, plus a grace. A slot that passes
//! without a new reading makes the monitor late, and the cause is what the
//! attempts since then found; once the value no longer holds, it is unknown.

use chrono::{DateTime, FixedOffset, TimeDelta, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use crate::monitors::cadence::Cadence;
use crate::monitors::slots;

/// What one attempt to read a monitor found (§FS-021-monitors.7.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "found", rename_all = "lowercase")]
pub enum Found {
    /// A reading (§FS-021-monitors.9.2).
    Reading,
    /// The read failed, and why: an exit other than `0` or `75`, a timeout,
    /// a missing file, an answer refused, or no sample for the monitor.
    Unreadable { why: String },
    /// The command exited `75` (§FS-021-monitors.5.5).
    Parked,
    /// The read worked and brought nothing measured since the reading held,
    /// or only a sample measured more than one interval before it.
    Stale {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        measured: Option<DateTime<Utc>>,
    },
}

/// One attempt, and when it was made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attempt {
    pub at: DateTime<Utc>,
    #[serde(flatten)]
    pub found: Found,
}

/// Why a monitor is late or unknown (§FS-021-monitors.7.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cause", rename_all = "lowercase")]
pub enum Cause {
    /// The project's: the read failed.
    Unreadable { why: String },
    /// The project's, once it outlasts the grace: every read since `since`
    /// exited 75.
    Parked { since: DateTime<Utc> },
    /// The project's: whatever writes the measurement did not run.
    Stale {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        measured: Option<DateTime<Utc>>,
    },
    /// The watch's own: nothing tried since the slot (§FS-021-monitors.7.5).
    Overdue,
}

impl Cause {
    /// The cause's word: `unreadable`, `parked`, `stale` or `overdue`.
    pub fn name(&self) -> &'static str {
        match self {
            Cause::Unreadable { .. } => "unreadable",
            Cause::Parked { .. } => "parked",
            Cause::Stale { .. } => "stale",
            Cause::Overdue => "overdue",
        }
    }
}

/// Where a monitor's value stands at a moment (§FS-021-monitors.7).
#[derive(Debug, Clone, PartialEq)]
pub enum Freshness {
    /// No reading yet, and its first grace has not ended
    /// (§FS-021-monitors.7.1).
    NotReadYet { holds_until: DateTime<Utc> },
    /// The value holds and no slot has passed without a reading.
    Holds { holds_until: DateTime<Utc> },
    /// A slot passed without a new reading; the last health holds and the
    /// cause is named (§FS-021-monitors.7.3).
    Late {
        cause: Cause,
        since: DateTime<Utc>,
        holds_until: DateTime<Utc>,
    },
    /// The value no longer holds (§FS-021-monitors.7.4).
    Unknown { cause: Cause, since: DateTime<Utc> },
}

impl Freshness {
    pub fn is_unknown(&self) -> bool {
        matches!(self, Freshness::Unknown { .. })
    }

    pub fn cause(&self) -> Option<&Cause> {
        match self {
            Freshness::Late { cause, .. } | Freshness::Unknown { cause, .. } => Some(cause),
            _ => None,
        }
    }
}

/// What freshness is judged from: the monitor's record as the store keeps
/// it.
#[derive(Debug, Clone, Copy)]
pub struct Basis<'a> {
    /// When the last reading was read, or, with none yet, when ephor first saw
    /// the monitor (§FS-021-monitors.7.1).
    pub held_from: DateTime<Utc>,
    pub has_reading: bool,
    /// Where slots without `at` count from (§FS-021-monitors.6.3).
    pub origin: DateTime<Utc>,
    pub last_attempt: Option<&'a Attempt>,
    pub parked_since: Option<DateTime<Utc>>,
}

/// The first slot after the reading, where a value starts to be late.
///
/// A monitor with no reading yet holds as if ephor had read it when it first
/// saw the monitor (§FS-021-monitors.7.1), so its slots without `at` count
/// from then too, not from an anchor its first attempt set later.
pub fn late_from<Tz: TimeZone>(
    basis: &Basis,
    cadence: &Cadence,
    zone: &Tz,
) -> Option<DateTime<Utc>> {
    let origin = if basis.has_reading {
        basis.origin
    } else {
        basis.held_from
    };
    slots::first_after(cadence, origin, basis.held_from, zone)
}

/// Until when a value holds: the first slot after the reading that brought
/// it, plus the grace (§FS-021-monitors.7.1). So a `10m` monitor's holds for
/// 1h 10m, a daily one's for 30 hours, a weekly one's for 8 days and 18 hours.
pub fn holds_until<Tz: TimeZone>(
    basis: &Basis,
    cadence: &Cadence,
    zone: &Tz,
) -> Option<DateTime<Utc>> {
    late_from(basis, cadence, zone)?.checked_add_signed(cadence.grace())
}

/// Where a monitor's value stands at `now` (§FS-021-monitors.7).
///
/// The last attempt since the slot that passed decides the cause, so a
/// monitor overdue for a day and then read and refused is unreadable. While
/// the monitor slot itself fails, `slot_error` is every late monitor's
/// unreadable cause, never overdue (§FS-021-monitors.7.2).
pub fn freshness<Tz: TimeZone>(
    basis: &Basis,
    cadence: &Cadence,
    now: DateTime<Utc>,
    zone: &Tz,
    slot_error: Option<&str>,
) -> Freshness {
    let Some(late) = late_from(basis, cadence, zone) else {
        return Freshness::Holds {
            holds_until: DateTime::<Utc>::MAX_UTC,
        };
    };
    let holds_until = late
        .checked_add_signed(cadence.grace())
        .unwrap_or(DateTime::<Utc>::MAX_UTC);
    if now < holds_until && !basis.has_reading {
        return Freshness::NotReadYet { holds_until };
    }
    if now < late {
        return Freshness::Holds { holds_until };
    }
    let cause = match (slot_error, basis.last_attempt) {
        (Some(error), _) => Cause::Unreadable {
            why: error.to_string(),
        },
        (None, Some(attempt)) if attempt.at >= late => match &attempt.found {
            Found::Unreadable { why } => Cause::Unreadable { why: why.clone() },
            Found::Parked => Cause::Parked {
                since: basis.parked_since.unwrap_or(attempt.at),
            },
            Found::Stale { measured } => Cause::Stale {
                measured: *measured,
            },
            // A reading since the slot is the reading the value holds from.
            Found::Reading => Cause::Overdue,
        },
        _ => Cause::Overdue,
    };
    if now < holds_until {
        Freshness::Late {
            cause,
            since: late,
            holds_until,
        }
    } else {
        Freshness::Unknown {
            cause,
            since: holds_until,
        }
    }
}

/// When a sample was measured, and whether that makes its read stale
/// (§FS-021-monitors.7.1, §FS-021-monitors.9.2).
#[derive(Debug, Clone, PartialEq)]
pub enum Measured {
    /// A reading measured at `at`. `skew` is the sample's own `at` where it
    /// lay after the read, which then counts as measured when it was read,
    /// and the row notes it.
    Fresh {
        at: DateTime<Utc>,
        skew: Option<DateTime<FixedOffset>>,
    },
    /// Nothing measured since the reading held, or only a sample measured
    /// more than one interval before the read.
    Stale { at: DateTime<Utc> },
}

/// Judges when a sample was measured (§FS-021-monitors.7.1): at its `at`,
/// and when it was read where it states none or states a time after the
/// read. The read is stale where it brings the measurement already held
/// (`already_held`: a file unchanged, or a series whose last point is the one
/// held), one measured no later than the reading held, or one measured more
/// than one `interval` before the read.
pub fn measured(
    sample_at: Option<DateTime<FixedOffset>>,
    read_at: DateTime<Utc>,
    held: Option<DateTime<Utc>>,
    interval: TimeDelta,
    already_held: bool,
) -> Measured {
    let (at, skew) = match sample_at {
        Some(at) if at.with_timezone(&Utc) > read_at => (read_at, Some(at)),
        Some(at) => (at.with_timezone(&Utc), None),
        None => (read_at, None),
    };
    let old = read_at.signed_duration_since(at) > interval;
    if already_held || old || held.is_some_and(|held| at <= held) {
        Measured::Stale { at }
    } else {
        Measured::Fresh { at, skew }
    }
}

#[cfg(test)]
#[path = "freshness_tests.rs"]
mod tests;
