//! When a monitor is next read (§FS-021-monitors.6.3, §FS-021-monitors.6.4).
//!
//! A monitor ephor has not read yet is due at once. After a read, its next
//! slot is the first one after the read, so a slot that passed while nothing
//! ran is read once, not once per missed slot. A change of cadence counts the
//! next slot again, and a next slot further off than one interval, which only
//! a clock set back can leave, is counted again from now.

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use crate::monitors::cadence::Cadence;
use crate::monitors::slots;

/// A monitor's schedule, as the store keeps it (§FS-021-monitors.9.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    /// When ephor first saw the monitor: what its value holds from before it
    /// has a reading (§FS-021-monitors.7.1).
    pub first_seen: DateTime<Utc>,
    /// Ephor's first attempt to read it, whatever that found: where slots
    /// without `at` count from (§FS-021-monitors.6.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<DateTime<Utc>>,
    /// The last read that moved the schedule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_read: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_slot: Option<DateTime<Utc>>,
    /// Since when every read has exited 75 (§FS-021-monitors.9.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parked_since: Option<DateTime<Utc>>,
    /// The cadence `next_slot` was counted under, so a change of it is seen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counted_under: Option<Cadence>,
}

impl Schedule {
    /// A monitor first seen at `now`: due at once (§FS-021-monitors.6.3).
    pub fn new(now: DateTime<Utc>) -> Schedule {
        Schedule {
            first_seen: now,
            anchor: None,
            last_read: None,
            next_slot: None,
            parked_since: None,
            counted_under: None,
        }
    }

    /// Where slots without `at` count from: the anchor, or, before any
    /// attempt, when ephor first saw the monitor.
    pub fn origin(&self) -> DateTime<Utc> {
        self.anchor.unwrap_or(self.first_seen)
    }

    /// The next slot under `cadence` as of `now` (§FS-021-monitors.6.3), or
    /// none where the monitor is due at once because it was never read.
    ///
    /// Counted again where the cadence changed since it was counted, from the
    /// last read, so a slot the new cadence has already passed is due at
    /// once; and counted again from `now` where it lies more than one
    /// interval ahead.
    pub fn next_slot<Tz: TimeZone>(
        &self,
        cadence: &Cadence,
        now: DateTime<Utc>,
        zone: &Tz,
    ) -> Option<DateTime<Utc>> {
        let last_read = self.last_read?;
        let next = match (&self.counted_under, self.next_slot) {
            (Some(counted), Some(next)) if counted == cadence => next,
            _ => slots::first_after(cadence, self.origin(), last_read, zone)?,
        };
        if next.signed_duration_since(now) > cadence.interval() {
            return slots::first_after(cadence, self.origin(), now, zone);
        }
        Some(next)
    }

    /// Whether its slot has come (§FS-021-monitors.6.4).
    pub fn is_due<Tz: TimeZone>(&self, cadence: &Cadence, now: DateTime<Utc>, zone: &Tz) -> bool {
        self.next_slot(cadence, now, zone)
            .is_none_or(|next| next <= now)
    }

    /// Writes down the next slot as [`Schedule::next_slot`] counts it, so a
    /// recount is kept.
    pub fn settle<Tz: TimeZone>(&mut self, cadence: &Cadence, now: DateTime<Utc>, zone: &Tz) {
        if self.last_read.is_some() {
            self.next_slot = self.next_slot(cadence, now, zone);
            self.counted_under = Some(*cadence);
        }
    }

    /// A read at a slot moved the schedule (§FS-021-monitors.9.2): the
    /// anchor is its first attempt, and the next slot is the first after the
    /// read. A parked read lets the slot pass and marks since when it has
    /// been parked; any other clears that.
    pub fn read<Tz: TimeZone>(
        &mut self,
        cadence: &Cadence,
        at: DateTime<Utc>,
        parked: bool,
        zone: &Tz,
    ) {
        let anchor = *self.anchor.get_or_insert(at);
        self.last_read = Some(at);
        self.next_slot = slots::first_after(cadence, anchor, at, zone);
        self.counted_under = Some(*cadence);
        self.parked_since = if parked {
            Some(self.parked_since.unwrap_or(at))
        } else {
            None
        };
    }
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;
