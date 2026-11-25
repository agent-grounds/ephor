use chrono::TimeDelta;

use super::*;
use crate::monitors::test_zone::{utc, Shifting};

fn every(text: &str) -> Cadence {
    Cadence::resolve(Some(text), None).unwrap()
}

fn read_at<'a>(time: &str, attempt: Option<&'a Attempt>) -> Basis<'a> {
    Basis {
        held_from: utc(time),
        has_reading: true,
        origin: utc(time),
        last_attempt: attempt,
        parked_since: None,
    }
}

fn attempt(time: &str, found: Found) -> Attempt {
    Attempt {
        at: utc(time),
        found,
    }
}

#[test]
fn a_value_holds_until_its_next_slot_and_a_grace() {
    let ten = every("10m");
    assert_eq!(
        holds_until(&read_at("2026-11-25T12:00:00Z", None), &ten, &Utc),
        Some(utc("2026-11-25T13:10:00Z"))
    );
    let daily = Cadence::resolve(None, Some("06:00")).unwrap();
    assert_eq!(
        holds_until(&read_at("2026-11-25T05:30:00Z", None), &daily, &Shifting),
        Some(utc("2026-11-26T11:00:00Z"))
    );
    let weekly = Cadence::resolve(None, Some("Mon 06:00")).unwrap();
    let read = read_at("2026-11-30T06:00:00Z", None);
    assert_eq!(
        holds_until(&read, &weekly, &Utc).unwrap() - utc("2026-11-30T06:00:00Z"),
        TimeDelta::days(8) + TimeDelta::hours(18)
    );
}

#[test]
fn a_fresh_value_holds_then_is_late_then_unknown() {
    let ten = every("10m");
    let basis = read_at("2026-11-25T12:00:00Z", None);
    let at = |time: &str| freshness(&basis, &ten, utc(time), &Utc, None);
    assert_eq!(
        at("2026-11-25T12:09:59Z"),
        Freshness::Holds {
            holds_until: utc("2026-11-25T13:10:00Z")
        }
    );
    assert_eq!(
        at("2026-11-25T12:10:00Z"),
        Freshness::Late {
            cause: Cause::Overdue,
            since: utc("2026-11-25T12:10:00Z"),
            holds_until: utc("2026-11-25T13:10:00Z")
        }
    );
    let unknown = at("2026-11-25T13:10:00Z");
    assert_eq!(
        unknown,
        Freshness::Unknown {
            cause: Cause::Overdue,
            since: utc("2026-11-25T13:10:00Z")
        }
    );
    assert!(unknown.is_unknown());
    assert_eq!(unknown.cause().unwrap().name(), "overdue");
}

#[test]
fn the_last_attempt_since_the_slot_decides_the_cause() {
    let ten = every("10m");
    let now = utc("2026-11-25T12:30:00Z");
    let cause = |found: Found, time: &str| {
        let attempt = attempt(time, found);
        freshness(
            &read_at("2026-11-25T12:00:00Z", Some(&attempt)),
            &ten,
            now,
            &Utc,
            None,
        )
        .cause()
        .cloned()
        .unwrap()
    };
    let refused = Found::Unreadable {
        why: "exit 2".into(),
    };
    assert_eq!(
        cause(refused.clone(), "2026-11-25T12:20:00Z"),
        Cause::Unreadable {
            why: "exit 2".into()
        }
    );
    // An attempt before the slot is not since it.
    assert_eq!(cause(refused, "2026-11-25T12:05:00Z"), Cause::Overdue);
    assert_eq!(
        cause(
            Found::Stale {
                measured: Some(utc("2026-11-25T11:00:00Z"))
            },
            "2026-11-25T12:20:00Z"
        ),
        Cause::Stale {
            measured: Some(utc("2026-11-25T11:00:00Z"))
        }
    );
    assert_eq!(
        cause(Found::Parked, "2026-11-25T12:20:00Z"),
        Cause::Parked {
            since: utc("2026-11-25T12:20:00Z")
        }
    );
    let parked = attempt("2026-11-25T12:20:00Z", Found::Parked);
    let basis = Basis {
        parked_since: Some(utc("2026-11-25T12:10:00Z")),
        ..read_at("2026-11-25T12:00:00Z", Some(&parked))
    };
    assert_eq!(
        freshness(&basis, &ten, now, &Utc, None).cause(),
        Some(&Cause::Parked {
            since: utc("2026-11-25T12:10:00Z")
        })
    );
}

#[test]
fn a_failing_monitor_slot_makes_every_late_monitor_unreadable() {
    let ten = every("10m");
    let basis = read_at("2026-11-25T12:00:00Z", None);
    let late = freshness(
        &basis,
        &ten,
        utc("2026-11-25T12:30:00Z"),
        &Utc,
        Some("the slot exited 1"),
    );
    assert_eq!(
        late.cause(),
        Some(&Cause::Unreadable {
            why: "the slot exited 1".into()
        })
    );
    let fresh = freshness(
        &basis,
        &ten,
        utc("2026-11-25T12:05:00Z"),
        &Utc,
        Some("the slot exited 1"),
    );
    assert_eq!(fresh.cause(), None);
}

#[test]
fn a_monitor_not_read_yet_holds_from_when_it_was_first_seen() {
    let ten = every("10m");
    let basis = Basis {
        has_reading: false,
        ..read_at("2026-11-25T12:00:00Z", None)
    };
    assert_eq!(
        freshness(&basis, &ten, utc("2026-11-25T12:30:00Z"), &Utc, None),
        Freshness::NotReadYet {
            holds_until: utc("2026-11-25T13:10:00Z")
        }
    );
    let unknown = freshness(&basis, &ten, utc("2026-11-25T13:10:00Z"), &Utc, None);
    assert_eq!(
        unknown,
        Freshness::Unknown {
            cause: Cause::Overdue,
            since: utc("2026-11-25T13:10:00Z")
        }
    );
}

#[test]
fn a_cause_keeps_its_shape_in_the_store() {
    let cause = Cause::Unreadable {
        why: "exit 2".into(),
    };
    let json = serde_json::to_value(&cause).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"cause": "unreadable", "why": "exit 2"})
    );
    assert_eq!(serde_json::from_value::<Cause>(json).unwrap(), cause);
    assert_eq!(
        serde_json::to_value(Cause::Overdue).unwrap(),
        serde_json::json!({"cause": "overdue"})
    );
    let attempt = attempt("2026-11-25T12:00:00Z", Found::Parked);
    let json = serde_json::to_value(&attempt).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"at": "2026-11-25T12:00:00Z", "found": "parked"})
    );
    assert_eq!(serde_json::from_value::<Attempt>(json).unwrap(), attempt);
}

#[test]
fn a_sample_counts_as_measured_at_its_at() {
    let hour = TimeDelta::hours(1);
    let read = utc("2026-11-25T12:00:00Z");
    let at = |text: &str| Some(DateTime::parse_from_rfc3339(text).unwrap());
    assert_eq!(
        measured(None, read, None, hour, false),
        Measured::Fresh {
            at: read,
            skew: None
        }
    );
    assert_eq!(
        measured(at("2026-11-25T12:30:00+01:00"), read, None, hour, false),
        Measured::Fresh {
            at: utc("2026-11-25T11:30:00Z"),
            skew: None
        }
    );
}

#[test]
fn a_sample_after_its_read_counts_as_read_and_is_noted() {
    let read = utc("2026-11-25T12:00:00Z");
    let ahead = DateTime::parse_from_rfc3339("2026-11-25T12:05:00Z").unwrap();
    assert_eq!(
        measured(Some(ahead), read, None, TimeDelta::hours(1), false),
        Measured::Fresh {
            at: read,
            skew: Some(ahead)
        }
    );
}

#[test]
fn a_read_that_brings_nothing_new_is_stale() {
    let hour = TimeDelta::hours(1);
    let read = utc("2026-11-25T12:00:00Z");
    let at = |text: &str| Some(DateTime::parse_from_rfc3339(text).unwrap());
    let held = Some(utc("2026-11-25T11:30:00Z"));
    assert_eq!(
        measured(at("2026-11-25T11:30:00Z"), read, held, hour, false),
        Measured::Stale {
            at: utc("2026-11-25T11:30:00Z")
        }
    );
    assert_eq!(
        measured(at("2026-11-25T10:59:59Z"), read, None, hour, false),
        Measured::Stale {
            at: utc("2026-11-25T10:59:59Z")
        }
    );
    assert_eq!(
        measured(at("2026-11-25T11:00:00Z"), read, None, hour, false),
        Measured::Fresh {
            at: utc("2026-11-25T11:00:00Z"),
            skew: None
        }
    );
    assert_eq!(
        measured(None, read, held, hour, true),
        Measured::Stale { at: read }
    );
}
