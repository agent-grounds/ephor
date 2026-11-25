//! The shared interval reading (§FS-005-dispatch.32, §FS-021-monitors.6.1).

use super::*;

const RECIPE: &[Unit] = &[Unit::Minute, Unit::Hour, Unit::Day];
const MONITOR: &[Unit] = &[Unit::Minute, Unit::Hour, Unit::Day, Unit::Week];

#[test]
fn each_unit_reads_as_its_length() {
    assert_eq!(read("10m", MONITOR), Ok(TimeDelta::minutes(10)));
    assert_eq!(read("4h", MONITOR), Ok(TimeDelta::hours(4)));
    assert_eq!(read("1d", MONITOR), Ok(TimeDelta::days(1)));
    assert_eq!(read("1w", MONITOR), Ok(TimeDelta::weeks(1)));
    assert_eq!(read("0h", RECIPE), Ok(TimeDelta::zero()));
}

/// A recipe reads no weeks: the shared reader only reads the units its
/// caller names, so lifting it changed no recipe (§FS-005-dispatch.32).
#[test]
fn a_unit_the_caller_does_not_read_is_no_unit() {
    assert_eq!(read("1w", RECIPE), Err(Unread::NoUnit));
    assert_eq!(read("6y", MONITOR), Err(Unread::NoUnit));
    assert_eq!(read("30s", MONITOR), Err(Unread::NoUnit));
    assert_eq!(read("6µ", MONITOR), Err(Unread::NoUnit));
    assert_eq!(read("", MONITOR), Err(Unread::NoUnit));
}

#[test]
fn what_is_not_a_count_is_said_as_such() {
    assert_eq!(read("h", MONITOR), Err(Unread::NotACount));
    assert_eq!(read("6 h", MONITOR), Err(Unread::NotACount));
    assert_eq!(read("1.5h", MONITOR), Err(Unread::NotACount));
    assert_eq!(read("-1h", MONITOR), Err(Unread::Negative));
}

#[test]
fn a_length_past_the_calendar_is_too_long() {
    assert_eq!(read("100000000d", RECIPE).map(|d| d <= longest()), Ok(true));
    assert_eq!(read("9223372036854775807w", MONITOR), Err(Unread::TooLong));
}
