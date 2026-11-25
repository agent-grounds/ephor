use serde_json::json;

use super::tests::{coverage, ended, healthy, moved, reading, watch, Run};
use super::*;
use crate::monitors::model::Sample;
use crate::monitors::test_zone::utc;

fn sample(json: serde_json::Value) -> Sample {
    serde_json::from_value(json).unwrap()
}

fn with_sample(watch: &Watch, run: &Run, v: f64, sample: Sample) -> Finding {
    Finding::Reading(Reading {
        sample: Some(sample),
        ..reading(watch, Value::Number(v), run.clock)
    })
}

/// A monitor read at every slot until an unknown episode is open on it.
fn unknown_open(watch: &Watch) -> Run {
    let mut run = Run::new();
    run.number(watch, 85.0);
    for _ in 0..7 {
        run.unreadable(watch);
    }
    assert!(run.record.unknown.is_some());
    run
}

fn work(on: Sort, id: &str) -> Vec<EndedWork> {
    vec![EndedWork {
        on,
        work: id.into(),
    }]
}

#[test]
fn a_stale_read_ends_no_unknown_episode() {
    let watch = coverage(1, 2);
    let mut run = unknown_open(&watch);
    let named = run.record.unknown.as_ref().unwrap().named;
    let stale = Finding::Stale {
        measured: Some(utc("2026-11-25T12:00:00Z")),
    };
    let effects = run.found(&watch, stale, true, Vec::new());
    assert_eq!(ended(&effects, Sort::Unknown), None);
    assert_eq!(run.record.unknown.as_ref().unwrap().named, named);
    assert!(run.record.past.is_empty());
}

#[test]
fn still_parked_after_its_work_ended_names_parked() {
    let watch = coverage(1, 2);
    let mut run = unknown_open(&watch);
    let first = run.found(&watch, Finding::Parked, false, work(Sort::Unknown, "w1"));
    assert_eq!(moved(&first), vec!["still parked after its work ended"]);
    let again = run.found(&watch, Finding::Parked, false, work(Sort::Unknown, "w1"));
    assert!(moved(&again).is_empty());
    let next = run.found(&watch, Finding::Parked, false, work(Sort::Unknown, "w2"));
    assert_eq!(moved(&next), vec!["still parked after its work ended"]);
}

#[test]
fn a_numbers_points_stay_those_of_the_last_sample_that_carried_any() {
    let watch = coverage(1, 2);
    let mut run = Run::new();
    let three = sample(json!({"id": "m", "value": 83, "points": [
        {"t": "2026-11-23T12:00:00Z", "v": 81},
        {"t": "2026-11-24T12:00:00Z", "v": 82},
        {"t": "2026-11-25T11:00:00Z", "v": 83}
    ]}));
    let found = with_sample(&watch, &run, 83.0, three.clone());
    run.found(&watch, found, true, Vec::new());
    let found = with_sample(&watch, &run, 84.0, sample(json!({"id": "m", "value": 84})));
    run.found(&watch, found, true, Vec::new());
    assert_eq!(run.record.points(), three.points.as_deref());
    // Its kind changes, and its history starts again.
    let level = Watch {
        kind: Some(Kind::Level),
        ..watch
    };
    run.found(
        &level,
        Finding::Unreadable { why: "x".into() },
        true,
        Vec::new(),
    );
    assert_eq!(run.record.earlier_points, None);
}

#[test]
fn an_episode_keeps_what_its_matter_says_and_ends_with_it() {
    let watch = coverage(1, 2);
    let mut run = Run::new();
    run.number(&watch, 85.0);
    let lost = sample(
        json!({"id": "m", "value": 79.2, "detail": "two files lost tests",
        "url": "https://example.com/coverage"}),
    );
    let found = with_sample(&watch, &run, 79.2, lost);
    let dropped_at = run.clock;
    run.found(&watch, found, true, Vec::new());
    let shown = &run.record.drop.as_ref().unwrap().shown;
    let ok = Kept {
        t: utc("2026-11-25T12:00:00Z"),
        v: Value::Number(85.0),
        of: None,
    };
    assert_eq!(
        *shown,
        Shown {
            title: "Line coverage".into(),
            value: Some(Value::Number(79.2)),
            of: None,
            unit: Some("%".into()),
            condition: watch.healthy_words.clone(),
            since: Some(dropped_at),
            last_ok: Some(ok.clone()),
            detail: Some("two files lost tests".into()),
            url: Some("https://example.com/coverage".into()),
        }
    );
    run.number(&watch, 90.0);
    let recovered_at = run.clock;
    let effects = run.number(&watch, 91.0);
    assert!(ended(&effects, Sort::Drop).is_some());
    let past = &run.record.past[0];
    assert_eq!(past.shown.value, Some(Value::Number(91.0)));
    assert_eq!(past.shown.since, Some(recovered_at));
    assert_eq!(past.shown.title, "Line coverage");
    assert_eq!(past.shown.detail.as_deref(), Some("two files lost tests"));
    assert_eq!(past.cause, None);
    // An unknown episode keeps its last cause.
    let mut run = unknown_open(&watch);
    run.number(&watch, 85.0);
    let past = run.record.past.last().unwrap();
    assert_eq!(past.sort, Sort::Unknown);
    assert_eq!(
        past.cause,
        Some(freshness::Cause::Unreadable {
            why: "exit 2".into()
        })
    );
    assert_eq!(past.shown.value, Some(Value::Number(85.0)));
}

#[test]
fn a_move_keeps_when_it_moved_and_why() {
    let levels: Vec<String> = ["clean", "degraded", "failed"].map(String::from).to_vec();
    let watch = Watch {
        levels: Some(&levels),
        kind: Some(Kind::Level),
        ..watch(healthy(json!({"in": ["clean"]})), 1, 1)
    };
    let mut run = Run::new();
    run.value(&watch, Value::Level("degraded".into()));
    assert_eq!(run.record.drop.as_ref().unwrap().moved, None);
    let worse_at = run.clock;
    run.value(&watch, Value::Level("failed".into()));
    let why = Movement::LevelWorse {
        from: "degraded".into(),
        to: "failed".into(),
    };
    let moved = Some(Moved { at: worse_at, why });
    assert_eq!(run.record.drop.as_ref().unwrap().moved, moved);
    run.value(&watch, Value::Level("clean".into()));
    assert_eq!(run.record.past[0].moved, moved);
}

#[test]
fn a_reading_keeps_the_file_it_was_read_from() {
    let watch = coverage(1, 2);
    let mut run = Run::new();
    let mark = FileMark {
        mtime: utc("2026-11-25T11:59:00Z"),
        digest: "abc".into(),
    };
    let found = Finding::Reading(Reading {
        mark: Some(mark.clone()),
        ..reading(&watch, Value::Number(85.0), run.clock)
    });
    run.found(&watch, found, true, Vec::new());
    assert!(run.record.file_unchanged(&mark));
    run.unreadable(&watch);
    run.found(&watch, Finding::Stale { measured: None }, true, Vec::new());
    assert_eq!(run.record.file, Some(mark));
}

#[test]
fn a_reading_out_of_cadence_says_so_in_its_history_line() {
    let watch = coverage(1, 2);
    let mut run = Run::new();
    let line = |effects: Vec<Effect>| {
        effects.into_iter().find_map(|effect| match effect {
            Effect::Record(line) => Some(serde_json::to_value(line).unwrap()),
            _ => None,
        })
    };
    let slot = line(run.number(&watch, 85.0)).unwrap();
    assert_eq!(slot.get("out_of_cadence"), None);
    let found = Finding::Reading(reading(&watch, Value::Number(86.0), run.clock));
    let out = line(run.found(&watch, found, false, Vec::new())).unwrap();
    assert_eq!(out["out_of_cadence"], true);
    let back: Line = serde_json::from_value(out).unwrap();
    assert!(matches!(
        back,
        Line::Reading {
            out_of_cadence: true,
            ..
        }
    ));
}

#[test]
fn a_reading_keeps_its_whole_with_its_value() {
    let watch = coverage(1, 2);
    let mut run = Run::new();
    let found = Finding::Reading(Reading {
        of: Some(15.0),
        ..reading(&watch, Value::Number(12.0), run.clock)
    });
    run.found(&watch, found, true, Vec::new());
    let kept = Kept {
        t: utc("2026-11-25T12:00:00Z"),
        v: Value::Number(12.0),
        of: Some(15.0),
    };
    assert_eq!(run.record.last_reading, Some(kept));
    assert_eq!(run.record.drop.as_ref().unwrap().shown.of, Some(15.0));
}
