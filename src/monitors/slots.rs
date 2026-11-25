//! Where a monitor's slots fall (§FS-021-monitors.6.3).
//!
//! Without `at`, slots count from the anchor, one interval apart, as lengths
//! of time. With `at`, they are times on the local wall clock, so a daily
//! `06:00` stays at 06:00 across a change of clocks: a slot the change skips
//! is due at the first instant after the gap, and a slot it repeats at its
//! first occurrence. Every function here is generic over the zone, so the
//! rules run under test with a zone that changes its clocks.

use chrono::{DateTime, Datelike, Days, NaiveDate, NaiveDateTime, TimeDelta, TimeZone, Utc};

use crate::monitors::cadence::{At, Cadence};

/// How far past a skipped wall time the clock is searched for the first
/// instant that exists again. The widest gap a zone has had is a whole day.
const LONGEST_GAP_MINUTES: i64 = 2 * 24 * 60;

/// The first slot strictly after `after` (§FS-021-monitors.6.3), or none
/// where it would fall past the end of the calendar.
///
/// `anchor` is where slots without `at` count from: ephor's first attempt to
/// read the monitor. Slots with `at` count from the clock alone.
pub fn first_after<Tz: TimeZone>(
    cadence: &Cadence,
    anchor: DateTime<Utc>,
    after: DateTime<Utc>,
    zone: &Tz,
) -> Option<DateTime<Utc>> {
    match cadence.at {
        None => from_anchor(cadence.every, anchor, after),
        Some(at) => on_the_clock(cadence.every, at, after, zone),
    }
}

/// `anchor + k·every` for the least `k` that falls after `after`. `k` may be
/// negative, so a clock set back behind the anchor still finds its slot.
fn from_anchor(
    every: TimeDelta,
    anchor: DateTime<Utc>,
    after: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let step = every.num_seconds().max(1);
    let elapsed = after.signed_duration_since(anchor).num_seconds();
    let count = elapsed.div_euclid(step) + 1;
    let offset = TimeDelta::try_seconds(count.checked_mul(step)?)?;
    let slot = anchor.checked_add_signed(offset)?;
    // A sub-second remainder can leave `slot` equal to `after`.
    if slot > after {
        Some(slot)
    } else {
        slot.checked_add_signed(TimeDelta::try_seconds(step)?)
    }
}

/// The first wall-clock slot after `after`, on the dates the cadence reads.
fn on_the_clock<Tz: TimeZone>(
    every: TimeDelta,
    at: At,
    after: DateTime<Utc>,
    zone: &Tz,
) -> Option<DateTime<Utc>> {
    let minutes = every.num_minutes().max(1);
    let local = after.with_timezone(zone).naive_local().date();
    // A day early, because a slot on the day before can still lie after
    // `after` once a gap has moved it forward.
    let mut date = local.checked_sub_days(Days::new(1))?;
    let period = period_days(minutes);
    date = first_date_on_or_after(date, minutes, at)?;
    // Three periods always reach a slot past `after`: one may hold only
    // slots before it, the next may lose its slot to a gap.
    for _ in 0..4 {
        for time in times_of_day(minutes, at) {
            if let Some(slot) = instant(zone, date.and_time(time)) {
                if slot > after {
                    return Some(slot);
                }
            }
        }
        date = date.checked_add_days(Days::new(period))?;
    }
    None
}

/// How many days lie between the dates a cadence reads on.
fn period_days(minutes: i64) -> u64 {
    if minutes < 1440 {
        1
    } else {
        (minutes / 1440) as u64
    }
}

/// The first date on or after `from` that the cadence reads on: every date
/// for a cadence under a day, every n-th date counted from the calendar's
/// start for n whole days, and the named day of every n-th week for one that
/// names a day. Counted from the calendar, never from when ephor first saw
/// the monitor, so the slots are the clock's (§FS-021-monitors.6.3).
fn first_date_on_or_after(from: NaiveDate, minutes: i64, at: At) -> Option<NaiveDate> {
    let days = from.num_days_from_ce() as i64;
    let skip = match at.day {
        Some(day) => {
            let weeks = (minutes / (7 * 1440)).max(1);
            // 0001-01-01 is a Monday and day 1, so weeks start on Mondays.
            let monday = days - (days - 1).rem_euclid(7);
            let week = (monday - 1).div_euclid(7);
            let mut first = monday
                + (weeks - week.rem_euclid(weeks)) % weeks * 7
                + day.num_days_from_monday() as i64;
            if first < days {
                first += weeks * 7;
            }
            first - days
        }
        None if minutes < 1440 => 0,
        None => {
            let every = minutes / 1440;
            (every - days.rem_euclid(every)) % every
        }
    };
    from.checked_add_days(Days::new(u64::try_from(skip).ok()?))
}

/// The times of day a cadence reads at on each date it reads on: `at`'s time
/// alone, or, for a cadence that divides a day, every `every` from it round
/// the clock, in order.
fn times_of_day(minutes: i64, at: At) -> Vec<chrono::NaiveTime> {
    if minutes >= 1440 {
        return vec![at.time];
    }
    let start = (at.time.signed_duration_since(chrono::NaiveTime::MIN)).num_minutes();
    let mut times: Vec<i64> = (0..1440 / minutes)
        .map(|k| (start + k * minutes).rem_euclid(1440))
        .collect();
    times.sort_unstable();
    times
        .into_iter()
        .filter_map(|minute| {
            chrono::NaiveTime::from_hms_opt((minute / 60) as u32, (minute % 60) as u32, 0)
        })
        .collect()
}

/// The instant a wall-clock time names in `zone` (§FS-021-monitors.6.3): its
/// first occurrence where a change of clocks repeats it, and the first instant
/// after the gap where a change skips it.
pub fn instant<Tz: TimeZone>(zone: &Tz, wall: NaiveDateTime) -> Option<DateTime<Utc>> {
    if let Some(first) = zone.from_local_datetime(&wall).earliest() {
        return Some(first.with_timezone(&Utc));
    }
    (1..=LONGEST_GAP_MINUTES).find_map(|minute| {
        let later = wall.checked_add_signed(TimeDelta::minutes(minute))?;
        zone.from_local_datetime(&later)
            .earliest()
            .map(|first| first.with_timezone(&Utc))
    })
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod tests;
