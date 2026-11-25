use chrono::{TimeDelta, Weekday};

use super::*;

fn cadence(every: Option<&str>, at: Option<&str>) -> Cadence {
    Cadence::resolve(every, at).unwrap()
}

#[test]
fn at_reads_a_time_or_a_day_and_a_time() {
    let at = At::parse("06:00").unwrap();
    assert_eq!(
        (at.day, at.time.to_string()),
        (None, "06:00:00".to_string())
    );
    let at = At::parse("Mon 06:30").unwrap();
    assert_eq!(at.day, Some(Weekday::Mon));
    assert_eq!(at.say(), "Mon 06:30");
    for bad in [
        "6:00",
        "06:0",
        "24:00",
        "12:60",
        "mon 06:00",
        "Mon  06:00",
        "Mon",
        "06-00",
        "+6:00",
    ] {
        assert_eq!(At::parse(bad), None, "{bad}");
    }
}

#[test]
fn the_defaults_follow_what_is_stated() {
    assert_eq!(cadence(None, None).say(), "every 10m");
    assert_eq!(cadence(None, Some("06:00")).say(), "every 1d at 06:00");
    assert_eq!(
        cadence(None, Some("Mon 06:00")).say(),
        "every 1w at Mon 06:00"
    );
    assert_eq!(cadence(Some("4h"), None).say(), "every 4h");
    assert_eq!(
        cadence(Some("2w"), Some("Fri 18:00")).say(),
        "every 2w at Fri 18:00"
    );
    assert_eq!(
        cadence(Some("6h"), Some("00:30")).say(),
        "every 6h at 00:30"
    );
    assert_eq!(
        cadence(Some("3d"), Some("00:30")).say(),
        "every 3d at 00:30"
    );
}

#[test]
fn at_and_every_must_pair() {
    assert_eq!(
        Cadence::resolve(Some("1d"), Some("Mon 06:00")).unwrap_err(),
        "at Mon 06:00 needs an every of whole weeks, and 1d is not one"
    );
    assert_eq!(
        Cadence::resolve(Some("7m"), Some("06:00")).unwrap_err(),
        "at 06:00 needs an every that divides a day evenly or is whole days, and 7m is neither"
    );
    assert_eq!(
        Cadence::resolve(Some("36h"), Some("06:00")).unwrap_err(),
        "at 06:00 needs an every that divides a day evenly or is whole days, and 36h is neither"
    );
    assert!(Cadence::resolve(Some("90m"), Some("06:00")).is_ok());
    assert!(Cadence::resolve(Some("14d"), Some("Mon 06:00")).is_ok());
}

#[test]
fn every_reads_minutes_hours_days_and_weeks() {
    assert_eq!(read_every("10m").unwrap(), TimeDelta::minutes(10));
    assert_eq!(read_every("4h").unwrap(), TimeDelta::hours(4));
    assert_eq!(read_every("1d").unwrap(), TimeDelta::days(1));
    assert_eq!(read_every("2w").unwrap(), TimeDelta::weeks(2));
    assert_eq!(
        read_every("0m").unwrap_err(),
        "every 0m is no time at all; a monitor is read at most every minute"
    );
    assert_eq!(
        read_every("10s").unwrap_err(),
        "every 10s is not a duration: a whole number and m, h, d or w, such as 10m or 1d"
    );
    assert_eq!(
        read_every("99999999999999w").unwrap_err(),
        "every 99999999999999w is longer than anything can wait"
    );
}

#[test]
fn a_length_is_spelt_in_its_largest_whole_unit() {
    assert_eq!(spell(TimeDelta::minutes(90)), "90m");
    assert_eq!(spell(TimeDelta::minutes(120)), "2h");
    assert_eq!(spell(TimeDelta::hours(48)), "2d");
    assert_eq!(spell(TimeDelta::days(14)), "2w");
    assert_eq!(spell(TimeDelta::zero()), "0m");
}

#[test]
fn the_grace_is_a_quarter_interval_and_at_least_an_hour() {
    assert_eq!(cadence(Some("10m"), None).grace(), TimeDelta::hours(1));
    assert_eq!(cadence(Some("4h"), None).grace(), TimeDelta::hours(1));
    assert_eq!(cadence(Some("8h"), None).grace(), TimeDelta::hours(2));
    assert_eq!(cadence(Some("1d"), None).grace(), TimeDelta::hours(6));
    assert_eq!(cadence(Some("1w"), None).grace(), TimeDelta::hours(42));
}

#[test]
fn recovery_needs_two_readings_under_an_hour_and_one_from_an_hour() {
    assert_eq!(cadence(Some("10m"), None).recover_after(), 2);
    assert_eq!(cadence(Some("59m"), None).recover_after(), 2);
    assert_eq!(cadence(Some("1h"), None).recover_after(), 1);
    assert_eq!(cadence(Some("1d"), None).recover_after(), 1);
}

#[test]
fn the_column_says_daily_and_weekly() {
    assert_eq!(cadence(Some("1d"), None).column(), "daily");
    assert_eq!(cadence(None, Some("Tue 09:00")).column(), "weekly");
    assert_eq!(cadence(Some("10m"), None).column(), "10m");
}

#[test]
fn a_command_waits_for_its_slowest_monitor_and_runs_at_its_fastest() {
    assert_eq!(run_timeout([]), DEFAULT_TIMEOUT_SECONDS);
    assert_eq!(run_timeout([30, 120, 60]), 120);
    let fastest = fastest([cadence(Some("1d"), None), cadence(Some("1h"), None)]).unwrap();
    assert_eq!(fastest.every, TimeDelta::hours(1));
    assert_eq!(super::fastest([]), None);
}

#[test]
fn a_cadence_keeps_its_spelling_in_the_store() {
    let pinned = cadence(Some("1w"), Some("Mon 06:00"));
    let json = serde_json::to_value(pinned).unwrap();
    assert_eq!(json, serde_json::json!({"every": "1w", "at": "Mon 06:00"}));
    let back: Cadence = serde_json::from_value(json).unwrap();
    assert_eq!(back, pinned);
    let plain = serde_json::to_value(cadence(Some("10m"), None)).unwrap();
    assert_eq!(plain, serde_json::json!({"every": "10m"}));
}
