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
        moved: Some(Moved {
            at: utc("2026-11-25T07:00:00Z"),
            why: Movement::LevelWorse {
                from: "degraded".into(),
                to: "failed".into(),
            },
        }),
        worst: Some("failed".into()),
        judged_under: Judge::SampleOk,
        judged_words: "healthy as its sample says".into(),
        works_ended: vec!["w1".into()],
        renewed: Some(utc("2026-11-25T06:40:00Z")),
        shown: Shown {
            title: "Release".into(),
            value: Some(Value::Level("failed".into())),
            condition: "healthy as its sample says".into(),
            url: Some("https://example.com/runs/1".into()),
            ..Shown::default()
        },
    });
    record.unknown = Some(OpenUnknown {
        named: utc("2026-11-25T08:00:00Z"),
        moved: None,
        cause: Cause::Parked {
            since: utc("2026-11-25T06:20:00Z"),
        },
        works_ended: Vec::new(),
        renewed: None,
        shown: Shown::default(),
    });
    record.past.push(Past {
        sort: Sort::Drop,
        named: utc("2026-11-20T06:00:00Z"),
        ended: utc("2026-11-21T06:00:00Z"),
        how: Ending::Recovered {
            value: "clean".into(),
        },
        moved: None,
        cause: Some(Cause::Overdue),
        shown: Shown::default(),
    });
    record.owner = Some(Owner::CustomStatus);
    record.claim = Some(Claim {
        pid: 42,
        at: utc("2026-11-25T08:00:00Z"),
        slot: None,
        timeout: Some(120),
    });
    record.kind = Some(Kind::Level);
    record.last_ok = Some(Kept {
        t: utc("2026-11-25T06:00:00Z"),
        v: Value::Level("clean".into()),
        of: None,
    });
    record.earlier_points = Some(vec![Point {
        t: Some(utc("2026-11-24T06:00:00Z").fixed_offset()),
        x: None,
        v: 3.0,
    }]);
    record.file = Some(FileMark {
        mtime: utc("2026-11-25T05:59:00Z"),
        digest: "abc".into(),
    });
    record.last_sample =
        Some(serde_json::from_value(serde_json::json!({"id": "m", "value": "clean"})).unwrap());
    let json = serde_json::to_value(&record).unwrap();
    assert_eq!(json["past"][0]["how"], "recovered");
    assert_eq!(json["unknown"]["cause"], "parked");
    assert_eq!(json["drop"]["moved"]["why"], "level_worse");
    assert_eq!(json["drop"]["moved"]["to"], "failed");
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
        timeout: None,
    };
    assert!(!claim.is_void(30, utc("2026-11-25T06:01:30Z")));
    assert!(claim.is_void(30, utc("2026-11-25T06:01:31Z")));
    assert!(!claim.is_void(u64::MAX, utc("2030-01-01T00:00:00Z")));

    // The claiming run's timeout is the one it keeps, whatever the caller
    // would judge it by (§FS-021-monitors.5.5).
    let run = Claim {
        timeout: Some(300),
        ..claim
    };
    assert!(!run.is_void(30, utc("2026-11-25T06:05:59Z")));
    assert!(run.is_void(30, utc("2026-11-25T06:06:01Z")));
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

#[test]
fn the_store_document_keeps_its_version_when_it_last_thinned_and_each_record() {
    let mut document = Document::default();
    assert_eq!(
        serde_json::to_value(&document).unwrap(),
        serde_json::json!({"version": 1, "monitors": {}})
    );
    assert_eq!(serde_json::from_str::<Document>("{}").unwrap(), document);
    document.thinned = Some(utc("2026-11-25T03:00:00Z"));
    document
        .monitors
        .insert("coverage".into(), Record::new(utc("2026-11-25T06:00:00Z")));
    let json = serde_json::to_value(&document).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "version": 1,
            "thinned": "2026-11-25T03:00:00Z",
            "monitors": {"coverage": {
                "schedule": {"first_seen": "2026-11-25T06:00:00Z"},
                "hold": {"run": 0},
                "named": {}
            }}
        })
    );
    assert_eq!(serde_json::from_value::<Document>(json).unwrap(), document);
}

#[test]
fn retention_thins_a_project_at_most_once_a_day() {
    let mut document = Document::default();
    assert!(document.due_to_thin(utc("2026-11-25T03:00:00Z")));
    document.thinned = Some(utc("2026-11-25T03:00:00Z"));
    assert!(!document.due_to_thin(utc("2026-11-26T02:59:59Z")));
    assert!(document.due_to_thin(utc("2026-11-26T03:00:00Z")));
}

#[test]
fn a_kept_sample_that_no_longer_reads_as_one_is_dropped_not_the_store() {
    let json = serde_json::json!({
        "schedule": {"first_seen": "2026-11-25T06:00:00Z"},
        "last_sample": {"value": 1}
    });
    let record = serde_json::from_value::<Record>(json).unwrap();
    assert_eq!(record.last_sample, None);
    let json = serde_json::json!({
        "schedule": {"first_seen": "2026-11-25T06:00:00Z"},
        "last_sample": {"id": "m", "value": 1, "detail": "ok"}
    });
    let record = serde_json::from_value::<Record>(json).unwrap();
    assert_eq!(record.last_sample.unwrap().detail.as_deref(), Some("ok"));
}

#[test]
fn a_numbers_points_are_those_of_the_last_sample_that_carried_any() {
    let sample = |json: serde_json::Value| serde_json::from_value::<Sample>(json).unwrap();
    let three = sample(serde_json::json!({"id": "m", "value": 3, "points": [
        {"t": "2026-11-23T06:00:00Z", "v": 1},
        {"t": "2026-11-24T06:00:00Z", "v": 2},
        {"t": "2026-11-25T06:00:00Z", "v": 3}
    ]}));
    let mut record = Record::new(utc("2026-11-25T06:00:00Z"));
    assert_eq!(record.points(), None);
    record.keep_sample(three.clone());
    assert_eq!(record.points(), three.points.as_deref());
    record.keep_sample(sample(serde_json::json!({"id": "m", "value": 4})));
    assert_eq!(record.points(), three.points.as_deref());
    record.keep_sample(sample(serde_json::json!({"id": "m", "value": 5})));
    assert_eq!(record.points(), three.points.as_deref());
    let one = sample(serde_json::json!({"id": "m", "value": 6, "points": [
        {"t": "2026-11-25T07:00:00Z", "v": 6}
    ]}));
    record.keep_sample(one.clone());
    assert_eq!(record.points(), one.points.as_deref());
    assert_eq!(record.earlier_points, None);
}

#[test]
fn a_file_read_is_stale_while_its_content_and_its_time_are_unchanged() {
    let mark = FileMark {
        mtime: utc("2026-11-25T05:59:00Z"),
        digest: "abc".into(),
    };
    let mut record = Record::new(utc("2026-11-25T06:00:00Z"));
    assert!(!record.file_unchanged(&mark));
    record.file = Some(mark.clone());
    assert!(record.file_unchanged(&mark));
    let touched = FileMark {
        mtime: utc("2026-11-25T06:59:00Z"),
        ..mark.clone()
    };
    assert!(!record.file_unchanged(&touched));
    let rewritten = FileMark {
        digest: "abd".into(),
        ..mark
    };
    assert!(!record.file_unchanged(&rewritten));
}
