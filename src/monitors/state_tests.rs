use super::*;
use crate::monitors::test_zone::utc;

#[test]
fn a_new_record_writes_only_what_it_knows() {
    let record = Record::new(utc("2026-11-25T06:00:00Z"));
    let json = serde_json::to_value(&record).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "schedule": {"first_seen": "2026-11-25T06:00:00Z"},
            "hold": {"run": 0},
            "named": {}
        })
    );
    assert_eq!(serde_json::from_value::<Record>(json).unwrap(), record);
}

#[test]
fn a_record_keeps_every_field_through_the_store() {
    let mut record = Record::new(utc("2026-11-25T06:00:00Z"));
    record.hold = Hold {
        health: Some(Health::Down),
        since: Some(utc("2026-11-25T06:10:00Z")),
        band: Some(Band::Down),
        run: 2,
    };
    record.drop = Some(OpenDrop {
        named: utc("2026-11-25T06:10:00Z"),
        moved: Some(utc("2026-11-25T07:00:00Z")),
        worst: Some("failed".into()),
        judged_under: Judge::SampleOk,
        judged_words: "healthy as its sample says".into(),
        works_ended: vec!["w1".into()],
    });
    record.unknown = Some(OpenUnknown {
        named: utc("2026-11-25T08:00:00Z"),
        moved: None,
        cause: Cause::Parked {
            since: utc("2026-11-25T06:20:00Z"),
        },
        works_ended: Vec::new(),
    });
    record.past.push(Past {
        sort: Sort::Drop,
        named: utc("2026-11-20T06:00:00Z"),
        ended: utc("2026-11-21T06:00:00Z"),
        how: Ending::Recovered {
            value: "clean".into(),
        },
    });
    record.owner = Some(Owner::CustomStatus);
    record.claim = Some(Claim {
        pid: 42,
        at: utc("2026-11-25T08:00:00Z"),
        slot: None,
    });
    record.kind = Some(Kind::Level);
    record.last_ok = Some(Kept {
        t: utc("2026-11-25T06:00:00Z"),
        v: Value::Level("clean".into()),
    });
    let json = serde_json::to_value(&record).unwrap();
    assert_eq!(json["past"][0]["how"], "recovered");
    assert_eq!(json["unknown"]["cause"], "parked");
    assert_eq!(serde_json::from_value::<Record>(json).unwrap(), record);
}

#[test]
fn a_counting_hold_says_how_far_it_has_come() {
    let failing = Hold {
        band: Some(Band::Down),
        run: 1,
        ..Hold::default()
    };
    assert_eq!(
        failing.counting(false, 2, 2).unwrap().say(),
        "failing 1 of 2"
    );
    assert_eq!(failing.counting(false, 1, 2), None);
    let recovering = Hold {
        band: Some(Band::Warn),
        run: 1,
        ..Hold::default()
    };
    assert_eq!(
        recovering.counting(true, 1, 2).unwrap().say(),
        "recovering 1 of 2"
    );
    assert_eq!(recovering.counting(false, 1, 2), None);
    assert_eq!(Hold::default().counting(true, 1, 2), None);
}

#[test]
fn a_claim_is_void_after_the_timeout_and_a_minute() {
    let claim = Claim {
        pid: 7,
        at: utc("2026-11-25T06:00:00Z"),
        slot: Some(utc("2026-11-25T06:00:00Z")),
    };
    assert!(!claim.is_void(30, utc("2026-11-25T06:01:30Z")));
    assert!(claim.is_void(30, utc("2026-11-25T06:01:31Z")));
    assert!(!claim.is_void(u64::MAX, utc("2030-01-01T00:00:00Z")));
}

#[test]
fn freshness_is_read_from_a_record_without_changing_it() {
    let ten = Cadence::resolve(Some("10m"), None).unwrap();
    let mut record = Record::new(utc("2026-11-25T12:00:00Z"));
    // Its first attempt came later than ephor first saw it.
    record.schedule.anchor = Some(utc("2026-11-25T12:05:00Z"));
    let before = record.clone();
    let not_read = Freshness::NotReadYet {
        holds_until: utc("2026-11-25T13:10:00Z"),
    };
    let at = |now: &str| record.freshness(&ten, utc(now), &chrono::Utc, None);
    assert_eq!(at("2026-11-25T13:09:59Z"), not_read);
    assert_eq!(
        at("2026-11-25T13:10:00Z"),
        Freshness::Unknown {
            cause: Cause::Overdue,
            since: utc("2026-11-25T13:10:00Z")
        }
    );
    assert_eq!(
        record.freshness(&ten, utc("2026-11-25T13:10:00Z"), &chrono::Utc, Some("x")),
        Freshness::Unknown {
            cause: Cause::Unreadable { why: "x".into() },
            since: utc("2026-11-25T13:10:00Z")
        }
    );
    assert_eq!(record, before);
}
