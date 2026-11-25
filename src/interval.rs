//! An interval written as a whole number and a unit, the one spelling every
//! rhythm in ephor's configuration shares: a recipe's sweep
//! (§FS-005-dispatch.32) and a monitor's `every` (§FS-021-monitors.6.1).
//!
//! Each caller says which units it reads and words its own refusal, because
//! what a refused interval costs differs between them: a sweep refuses the
//! whole recipe, a monitor costs that monitor alone. What is shared is the
//! reading itself, so `6h` means one thing wherever it is written.

use chrono::{DateTime, TimeDelta, Utc};

/// A unit an interval may be written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Minute,
    Hour,
    Day,
    Week,
}

impl Unit {
    /// The character that names the unit after the count.
    pub fn suffix(self) -> char {
        match self {
            Unit::Minute => 'm',
            Unit::Hour => 'h',
            Unit::Day => 'd',
            Unit::Week => 'w',
        }
    }

    /// How many minutes one of the unit is. A day is 1440 minutes, never a
    /// calendar day: an interval is a length of time, and the clock it may be
    /// pinned to is the caller's (§FS-021-monitors.6.3).
    pub fn minutes(self) -> i64 {
        match self {
            Unit::Minute => 1,
            Unit::Hour => 60,
            Unit::Day => 60 * 24,
            Unit::Week => 60 * 24 * 7,
        }
    }
}

/// Why a text did not read as an interval, for the caller to word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unread {
    /// It does not end in one of the units the caller reads.
    NoUnit,
    /// What stands before the unit is not a whole number.
    NotACount,
    /// The count is below zero.
    Negative,
    /// It is longer than the calendar can hold, so nothing written in it would
    /// ever come due.
    TooLong,
}

/// The longest wait that could still come due: the whole span the calendar
/// can represent. `TimeDelta` reaches some ten thousand times further than a
/// `DateTime` does, so an interval it accepts is not yet one a rhythm can be
/// written in — beyond this the rhythm would simply never come round.
pub fn longest() -> TimeDelta {
    DateTime::<Utc>::MAX_UTC.signed_duration_since(DateTime::<Utc>::MIN_UTC)
}

/// `<count><unit>` with the unit one of `units`, as a length of time.
///
/// The unit is stripped as a character rather than sliced at `len() - 1`:
/// the length is bytes, so a tail nobody meant (`6µ`, a pasted smart quote)
/// would split mid-character and panic where this is meant to refuse. The
/// text is read as given; trimming it is the caller's choice.
pub fn read(text: &str, units: &[Unit]) -> Result<TimeDelta, Unread> {
    let (count, unit) = units
        .iter()
        .find_map(|unit| text.strip_suffix(unit.suffix()).map(|count| (count, *unit)))
        .ok_or(Unread::NoUnit)?;
    let count: i64 = count.parse().map_err(|_| Unread::NotACount)?;
    if count < 0 {
        return Err(Unread::Negative);
    }
    let minutes = count.saturating_mul(unit.minutes());
    TimeDelta::try_minutes(minutes)
        .filter(|every| *every <= longest())
        .ok_or(Unread::TooLong)
}

#[cfg(test)]
#[path = "interval_tests.rs"]
mod tests;
