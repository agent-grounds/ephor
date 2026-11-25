use chrono::{FixedOffset, NaiveDate, TimeDelta, Timelike};

use super::*;
use crate::monitors::test_zone::{utc, Shifting};

fn cadence(every: Option<&str>, at: Option<&str>) -> Cadence {
    Cadence::resolve(every, at).unwrap()
}

fn after<Tz: TimeZone>(cadence: &Cadence, after: &str, zone: &Tz) -> DateTime<Utc> {
    let never = utc("2000-01-01T00:00:00Z");
    first_after(cadence, never, utc(after), zone).unwrap()
}

#[test]
fn without_at_slots_count_from_the_anchor() {
    let every = cadence(Some("10m"), None);
    let anchor = utc("2026-11-25T12:03:00Z");
    let at = |after: &str| first_after(&every, anchor, utc(after), &Utc).unwrap();
    assert_eq!(at("2026-11-25T12:03:00Z"), utc("2026-11-25T12:13:00Z"));
    assert_eq!(at("2026-11-25T12:12:59Z"), utc("2026-11-25T12:13:00Z"));
    assert_eq!(at("2026-11-25T12:20:00Z"), utc("2026-11-25T12:23:00Z"));
    assert_eq!(at("2026-11-25T15:00:00Z"), utc("2026-11-25T15:03:00Z"));
    // A clock set back behind the anchor counts back from it.
    assert_eq!(at("2026-11-25T11:00:00Z"), utc("2026-11-25T11:03:00Z"));
    assert_eq!(at("2026-11-25T11:53:00Z"), anchor);
}

#[test]
fn without_at_a_day_is_24_hours_across_a_change_of_clocks() {
    let daily = cadence(Some("1d"), None);
    let anchor = utc("2026-03-28T05:00:00Z");
    let slot = first_after(&daily, anchor, anchor, &Shifting).unwrap();
    assert_eq!(slot, utc("2026-03-29T05:00:00Z"));
}

#[test]
fn a_daily_at_is_read_at_that_time_on_the_local_clock() {
    let daily = cadence(None, Some("06:00"));
    let paris = FixedOffset::east_opt(3600).unwrap();
    assert_eq!(
        after(&daily, "2026-11-25T04:00:00Z", &paris),
        utc("2026-11-25T05:00:00Z")
    );
    assert_eq!(
        after(&daily, "2026-11-25T05:00:00Z", &paris),
        utc("2026-11-26T05:00:00Z")
    );
    let west = FixedOffset::west_opt(5 * 3600).unwrap();
    let late = cadence(None, Some("23:30"));
    assert_eq!(
        after(&late, "2026-11-25T04:00:00Z", &west),
        utc("2026-11-25T04:30:00Z")
    );
    assert_eq!(
        after(&late, "2026-11-25T04:30:00Z", &west),
        utc("2026-11-26T04:30:00Z")
    );
}

#[test]
fn a_daily_at_keeps_its_local_time_across_a_change_of_clocks() {
    let daily = cadence(None, Some("06:00"));
    assert_eq!(
        after(&daily, "2026-03-28T06:00:00Z", &Shifting),
        utc("2026-03-29T04:00:00Z")
    );
    assert_eq!(
        after(&daily, "2026-10-24T05:00:00Z", &Shifting),
        utc("2026-10-25T05:00:00Z")
    );
}

#[test]
fn a_slot_the_clocks_skip_is_due_at_the_first_instant_after_the_gap() {
    let daily = cadence(None, Some("02:30"));
    assert_eq!(
        after(&daily, "2026-03-28T02:00:00Z", &Shifting),
        utc("2026-03-29T01:00:00Z")
    );
    assert_eq!(
        after(&daily, "2026-03-29T01:00:00Z", &Shifting),
        utc("2026-03-30T00:30:00Z")
    );
}

#[test]
fn a_slot_the_clocks_repeat_is_read_once_at_its_first_occurrence() {
    let daily = cadence(None, Some("02:30"));
    assert_eq!(
        after(&daily, "2026-10-24T02:00:00Z", &Shifting),
        utc("2026-10-25T00:30:00Z")
    );
    // The second 02:30 is not a slot of its own.
    assert_eq!(
        after(&daily, "2026-10-25T00:30:00Z", &Shifting),
        utc("2026-10-26T01:30:00Z")
    );
}

#[test]
fn an_hourly_at_steps_over_the_gap_and_the_repeat() {
    let hourly = cadence(Some("1h"), Some("00:30"));
    let spring: Vec<_> = [
        "2026-03-28T23:45:00Z",
        "2026-03-29T00:30:00Z",
        "2026-03-29T01:00:00Z",
    ]
    .iter()
    .map(|time| after(&hourly, time, &Shifting))
    .collect();
    assert_eq!(
        spring,
        vec![
            utc("2026-03-29T00:30:00Z"), // 01:30 winter
            utc("2026-03-29T01:00:00Z"), // 02:30 skipped: 03:00 summer
            utc("2026-03-29T01:30:00Z"), // 03:30 summer
        ]
    );
    // 02:30 happens at 00:30Z and again at 01:30Z; only the first is a slot.
    assert_eq!(
        after(&hourly, "2026-10-25T00:00:00Z", &Shifting),
        utc("2026-10-25T00:30:00Z")
    );
    assert_eq!(
        after(&hourly, "2026-10-25T00:30:00Z", &Shifting),
        utc("2026-10-25T02:30:00Z")
    );
}

#[test]
fn a_sub_daily_at_fills_the_day_from_its_time() {
    let six = cadence(Some("6h"), Some("01:00"));
    let slots: Vec<_> = (0..5)
        .scan(utc("2026-11-25T00:00:00Z"), |time, _| {
            *time = first_after(&six, *time, *time, &Utc).unwrap();
            Some(time.hour())
        })
        .collect();
    assert_eq!(slots, vec![1, 7, 13, 19, 1]);
}

#[test]
fn a_weekly_at_is_read_on_its_day() {
    let weekly = cadence(None, Some("Mon 06:00"));
    // 2026-11-25 is a Wednesday.
    assert_eq!(
        after(&weekly, "2026-11-25T00:00:00Z", &Utc),
        utc("2026-11-30T06:00:00Z")
    );
    assert_eq!(
        after(&weekly, "2026-11-30T05:59:00Z", &Utc),
        utc("2026-11-30T06:00:00Z")
    );
    assert_eq!(
        after(&weekly, "2026-11-30T06:00:00Z", &Utc),
        utc("2026-12-07T06:00:00Z")
    );
}

#[test]
fn several_days_or_weeks_count_from_the_calendar_not_from_the_anchor() {
    let fortnightly = cadence(Some("2w"), Some("Mon 06:00"));
    assert_eq!(
        after(&fortnightly, "2026-11-25T00:00:00Z", &Utc),
        utc("2026-12-07T06:00:00Z")
    );
    assert_eq!(
        after(&fortnightly, "2026-12-01T00:00:00Z", &Utc),
        utc("2026-12-07T06:00:00Z")
    );
    assert_eq!(
        after(&fortnightly, "2026-12-07T06:00:00Z", &Utc),
        utc("2026-12-21T06:00:00Z")
    );
    let three = cadence(Some("3d"), Some("06:00"));
    assert_eq!(
        after(&three, "2026-11-25T00:00:00Z", &Utc),
        utc("2026-11-27T06:00:00Z")
    );
    assert_eq!(
        after(&three, "2026-11-26T00:00:00Z", &Utc),
        utc("2026-11-27T06:00:00Z")
    );
    assert_eq!(
        after(&three, "2026-11-27T06:00:00Z", &Utc),
        utc("2026-11-30T06:00:00Z")
    );
}

#[test]
fn a_wall_time_names_one_instant() {
    let wall = |text: &str| {
        NaiveDate::parse_from_str(&text[..10], "%Y-%m-%d")
            .unwrap()
            .and_time(chrono::NaiveTime::parse_from_str(&text[11..], "%H:%M").unwrap())
    };
    assert_eq!(
        instant(&Shifting, wall("2026-03-29 02:00")),
        Some(utc("2026-03-29T01:00:00Z"))
    );
    assert_eq!(
        instant(&Shifting, wall("2026-03-29 02:59")),
        Some(utc("2026-03-29T01:00:00Z"))
    );
    assert_eq!(
        instant(&Shifting, wall("2026-03-29 03:00")),
        Some(utc("2026-03-29T01:00:00Z"))
    );
    assert_eq!(
        instant(&Shifting, wall("2026-10-25 02:15")),
        Some(utc("2026-10-25T00:15:00Z"))
    );
    assert_eq!(
        instant(&Utc, wall("2026-10-25 02:15")),
        Some(utc("2026-10-25T02:15:00Z"))
    );
}

#[test]
fn the_end_of_the_calendar_has_no_slot() {
    let every = cadence(Some("1w"), None);
    let end = DateTime::<Utc>::MAX_UTC - TimeDelta::days(1);
    assert_eq!(first_after(&every, end, end, &Utc), None);
    let weekly = cadence(None, Some("Mon 06:00"));
    let last = DateTime::<Utc>::MAX_UTC;
    assert_eq!(first_after(&weekly, last, last, &Utc), None);
}
