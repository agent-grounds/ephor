use super::*;
use crate::monitors::test_zone::{utc, Shifting};

fn every(text: &str) -> Cadence {
    Cadence::resolve(Some(text), None).unwrap()
}

#[test]
fn a_monitor_never_read_is_due_at_once() {
    let schedule = Schedule::new(utc("2026-11-25T12:00:00Z"));
    assert_eq!(
        schedule.next_slot(&every("10m"), utc("2026-11-25T12:00:00Z"), &Utc),
        None
    );
    assert!(schedule.is_due(&every("10m"), utc("2026-11-25T12:00:00Z"), &Utc));
    assert_eq!(schedule.origin(), utc("2026-11-25T12:00:00Z"));
}

#[test]
fn the_next_slot_is_the_first_after_the_read() {
    let ten = every("10m");
    let mut schedule = Schedule::new(utc("2026-11-25T11:58:00Z"));
    schedule.read(&ten, utc("2026-11-25T12:00:00Z"), false, &Utc);
    assert_eq!(schedule.anchor, Some(utc("2026-11-25T12:00:00Z")));
    assert_eq!(schedule.next_slot, Some(utc("2026-11-25T12:10:00Z")));
    assert!(!schedule.is_due(&ten, utc("2026-11-25T12:09:59Z"), &Utc));
    assert!(schedule.is_due(&ten, utc("2026-11-25T12:10:00Z"), &Utc));
}

#[test]
fn slots_missed_while_nothing_ran_are_read_once() {
    let ten = every("10m");
    let mut schedule = Schedule::new(utc("2026-11-25T12:00:00Z"));
    schedule.read(&ten, utc("2026-11-25T12:00:00Z"), false, &Utc);
    assert!(schedule.is_due(&ten, utc("2026-11-25T15:00:00Z"), &Utc));
    schedule.read(&ten, utc("2026-11-25T15:00:30Z"), false, &Utc);
    assert_eq!(schedule.anchor, Some(utc("2026-11-25T12:00:00Z")));
    assert_eq!(schedule.next_slot, Some(utc("2026-11-25T15:10:00Z")));
}

#[test]
fn a_change_of_cadence_counts_the_next_slot_again() {
    let mut schedule = Schedule::new(utc("2026-11-25T12:00:00Z"));
    schedule.read(&every("10m"), utc("2026-11-25T12:00:00Z"), false, &Utc);
    let now = utc("2026-11-25T12:05:00Z");
    assert_eq!(
        schedule.next_slot(&every("1h"), now, &Utc),
        Some(utc("2026-11-25T13:00:00Z"))
    );

    let mut slow = Schedule::new(utc("2026-11-25T12:00:00Z"));
    slow.read(&every("1h"), utc("2026-11-25T12:00:00Z"), false, &Utc);
    let now = utc("2026-11-25T12:30:00Z");
    assert_eq!(
        slow.next_slot(&every("10m"), now, &Utc),
        Some(utc("2026-11-25T12:10:00Z"))
    );
    assert!(slow.is_due(&every("10m"), now, &Utc));

    let pinned = Cadence::resolve(None, Some("06:00")).unwrap();
    slow.settle(&pinned, now, &Shifting);
    assert_eq!(slow.counted_under, Some(pinned));
    assert_eq!(slow.next_slot, Some(utc("2026-11-26T05:00:00Z")));
}

#[test]
fn a_next_slot_more_than_one_interval_ahead_is_counted_again_from_now() {
    let ten = every("10m");
    let mut schedule = Schedule::new(utc("2026-11-25T12:00:00Z"));
    schedule.read(&ten, utc("2026-11-25T12:00:00Z"), false, &Utc);
    // The clock was set back a day after the read.
    let now = utc("2026-11-24T12:05:00Z");
    schedule.next_slot = Some(utc("2026-11-25T12:10:00Z"));
    assert_eq!(
        schedule.next_slot(&ten, now, &Utc),
        Some(utc("2026-11-24T12:10:00Z"))
    );
    let mut back = schedule.clone();
    back.anchor = Some(utc("2026-11-20T00:03:00Z"));
    assert_eq!(
        back.next_slot(&ten, now, &Utc),
        Some(utc("2026-11-24T12:13:00Z"))
    );
    back.settle(&ten, now, &Utc);
    assert_eq!(back.next_slot, Some(utc("2026-11-24T12:13:00Z")));
    // Exactly one interval ahead is not recounted.
    let mut edge = Schedule::new(utc("2026-11-25T12:00:00Z"));
    edge.read(&ten, utc("2026-11-25T12:00:00Z"), false, &Utc);
    assert_eq!(
        edge.next_slot(&ten, utc("2026-11-25T12:00:00Z"), &Utc),
        Some(utc("2026-11-25T12:10:00Z"))
    );
}

#[test]
fn a_parked_read_lets_the_slot_pass_and_marks_since_when() {
    let ten = every("10m");
    let mut schedule = Schedule::new(utc("2026-11-25T12:00:00Z"));
    schedule.read(&ten, utc("2026-11-25T12:00:00Z"), true, &Utc);
    assert_eq!(schedule.parked_since, Some(utc("2026-11-25T12:00:00Z")));
    schedule.read(&ten, utc("2026-11-25T12:10:00Z"), true, &Utc);
    assert_eq!(schedule.parked_since, Some(utc("2026-11-25T12:00:00Z")));
    assert_eq!(schedule.next_slot, Some(utc("2026-11-25T12:20:00Z")));
    schedule.read(&ten, utc("2026-11-25T12:20:00Z"), false, &Utc);
    assert_eq!(schedule.parked_since, None);
}

#[test]
fn settle_before_any_read_leaves_the_monitor_due() {
    let mut schedule = Schedule::new(utc("2026-11-25T12:00:00Z"));
    schedule.settle(&every("10m"), utc("2026-11-25T12:30:00Z"), &Utc);
    assert_eq!(schedule.next_slot, None);
    assert_eq!(schedule.counted_under, None);
}

#[test]
fn a_schedule_keeps_its_shape_in_the_store() {
    let mut schedule = Schedule::new(utc("2026-11-25T12:00:00Z"));
    schedule.read(&every("1d"), utc("2026-11-25T12:00:00Z"), false, &Utc);
    let json = serde_json::to_value(&schedule).unwrap();
    assert_eq!(json["counted_under"], serde_json::json!({"every": "1d"}));
    assert!(json.get("parked_since").is_none());
    assert_eq!(serde_json::from_value::<Schedule>(json).unwrap(), schedule);
}
