use chrono::{TimeDelta, Utc};
use serde_json::json;

use super::*;
use crate::monitors::condition;
use crate::monitors::model::Condition;
use crate::monitors::test_zone::utc;
use crate::monitors::words::{self, Terms};

fn healthy(json: serde_json::Value) -> Judge {
    Judge::Condition(serde_json::from_value::<Condition>(json).unwrap())
}

fn watch(judge: Judge, drop_after: u32, recover_after: u32) -> Watch<'static> {
    let terms = Terms {
        unit: Some("%"),
        ..Terms::default()
    };
    Watch {
        healthy_words: words::healthy(&judge, &terms),
        judge,
        kind: Some(Kind::Number),
        levels: None,
        drop_after,
        recover_after,
        cadence: Cadence::resolve(Some("10m"), None).unwrap(),
    }
}

fn coverage(drop_after: u32, recover_after: u32) -> Watch<'static> {
    watch(healthy(json!({"min": 80})), drop_after, recover_after)
}

/// A monitor first seen at 12:00, read every ten minutes from then on.
struct Run {
    record: Record,
    clock: DateTime<Utc>,
}

impl Run {
    fn new() -> Run {
        Run {
            record: Record::new(utc("2026-11-25T12:00:00Z")),
            clock: utc("2026-11-25T12:00:00Z"),
        }
    }

    fn found(
        &mut self,
        watch: &Watch,
        found: Found,
        in_cadence: bool,
        after_work: Vec<EndedWork>,
    ) -> Vec<Effect> {
        let read = Read {
            at: self.clock,
            in_cadence,
            found,
            after_work,
        };
        let effects = step(
            &mut self.record,
            watch,
            &Event::Read(read),
            self.clock,
            &Utc,
        );
        self.clock += TimeDelta::minutes(10);
        effects
    }

    fn value(&mut self, watch: &Watch, value: Value) -> Vec<Effect> {
        self.found(
            watch,
            Found::Reading(reading(watch, value, self.clock)),
            true,
            Vec::new(),
        )
    }

    fn number(&mut self, watch: &Watch, v: f64) -> Vec<Effect> {
        self.value(watch, Value::Number(v))
    }

    fn unreadable(&mut self, watch: &Watch) -> Vec<Effect> {
        self.found(
            watch,
            Found::Unreadable {
                why: "exit 2".into(),
            },
            true,
            Vec::new(),
        )
    }

    fn look(&mut self, watch: &Watch, at: DateTime<Utc>) -> Vec<Effect> {
        self.clock = at;
        step(
            &mut self.record,
            watch,
            &Event::Look { slot_error: None },
            at,
            &Utc,
        )
    }
}

fn reading(watch: &Watch, value: Value, at: DateTime<Utc>) -> Reading {
    Reading {
        said: words::value(&value, Some("%"), None),
        band: condition::band(&watch.judge, None, &value, None, watch.levels),
        value,
        measured: at,
        skew: None,
        sample: None,
    }
}

fn opened(effects: &[Effect], sort: Sort) -> Option<DateTime<Utc>> {
    effects.iter().find_map(|effect| match effect {
        Effect::Opened {
            sort: opened,
            named,
        } if *opened == sort => Some(*named),
        _ => None,
    })
}

fn ended(effects: &[Effect], sort: Sort) -> Option<Ending> {
    effects.iter().find_map(|effect| match effect {
        Effect::Ended {
            sort: ended,
            ending,
            ..
        } if *ended == sort => Some(ending.clone()),
        _ => None,
    })
}

fn moved(effects: &[Effect]) -> Vec<String> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Moved { why, .. } => Some(why.say()),
            _ => None,
        })
        .collect()
}

#[test]
fn drop_after_1_opens_at_the_first_failing_reading() {
    let watch = coverage(1, 2);
    let mut run = Run::new();
    assert!(opened(&run.number(&watch, 85.0), Sort::Drop).is_none());
    let effects = run.number(&watch, 79.2);
    assert_eq!(
        opened(&effects, Sort::Drop),
        Some(utc("2026-11-25T12:10:00Z"))
    );
    assert_eq!(run.record.hold.health, Some(Health::Down));
    assert_eq!(run.record.hold.since, Some(utc("2026-11-25T12:10:00Z")));
    assert!(effects.contains(&Effect::Record(Line::Reading {
        t: utc("2026-11-25T12:10:00Z"),
        v: Value::Number(79.2),
        health: Some(Health::Down)
    })));
}

#[test]
fn drop_after_2_waits_for_the_second_failing_reading_in_a_row() {
    let watch = coverage(2, 2);
    let mut run = Run::new();
    run.number(&watch, 85.0);
    assert!(opened(&run.number(&watch, 70.0), Sort::Drop).is_none());
    assert_eq!(
        run.record.hold.counting(false, 2, 2).unwrap().say(),
        "failing 1 of 2"
    );
    assert_eq!(run.record.hold.health, Some(Health::Ok));
    // A passing reading starts the count again.
    run.number(&watch, 90.0);
    assert!(opened(&run.number(&watch, 70.0), Sort::Drop).is_none());
    // An unreadable read neither adds to the count nor breaks it.
    run.unreadable(&watch);
    let effects = run.number(&watch, 70.0);
    assert_eq!(
        opened(&effects, Sort::Drop),
        Some(utc("2026-11-25T12:50:00Z"))
    );
}

#[test]
fn recover_after_1_ends_the_drop_at_the_first_passing_reading() {
    let watch = coverage(1, 1);
    let mut run = Run::new();
    run.number(&watch, 70.0);
    let effects = run.number(&watch, 81.0);
    assert_eq!(
        ended(&effects, Sort::Drop),
        Some(Ending::Recovered {
            value: "81.0%".into()
        })
    );
    assert_eq!(run.record.drop, None);
    assert_eq!(run.record.hold.health, Some(Health::Ok));
    assert_eq!(run.record.past.len(), 1);
    assert_eq!(run.record.past[0].ended, utc("2026-11-25T12:10:00Z"));
}

#[test]
fn recover_after_2_holds_the_drop_through_a_flap() {
    let watch = coverage(1, 2);
    let mut run = Run::new();
    let first = opened(&run.number(&watch, 70.0), Sort::Drop).unwrap();
    for v in [81.0, 70.0, 82.0] {
        let effects = run.number(&watch, v);
        assert!(ended(&effects, Sort::Drop).is_none(), "{v}");
        assert!(opened(&effects, Sort::Drop).is_none(), "{v}");
        assert_eq!(run.record.hold.health, Some(Health::Down));
    }
    assert_eq!(
        run.record.hold.counting(true, 1, 2).unwrap().say(),
        "recovering 1 of 2"
    );
    let effects = run.number(&watch, 83.0);
    assert_eq!(
        ended(&effects, Sort::Drop),
        Some(Ending::Recovered {
            value: "83.0%".into()
        })
    );
    let again = opened(&run.number(&watch, 10.0), Sort::Drop).unwrap();
    assert!(again > first);
    assert_eq!(run.record.past.len(), 1);
}

#[test]
fn warn_and_ok_follow_the_latest_reading_with_no_hold() {
    let judge = healthy(json!({"min": 80}));
    let mut watch = coverage(1, 2);
    watch.judge = judge.clone();
    let warn: Condition = serde_json::from_value(json!({"min": 90})).unwrap();
    let mut run = Run::new();
    let band = |v| condition::band(&judge, Some(&warn), &Value::Number(v), None, None);
    for (v, health) in [
        (85.0, Health::Warn),
        (95.0, Health::Ok),
        (85.0, Health::Warn),
    ] {
        let mut reading = reading(&watch, Value::Number(v), run.clock);
        reading.band = band(v);
        run.found(&watch, Found::Reading(reading), true, Vec::new());
        assert_eq!(run.record.hold.health, Some(health));
    }
}

#[test]
fn an_episode_name_never_goes_back_with_the_clock() {
    let watch = coverage(1, 1);
    let mut run = Run::new();
    run.number(&watch, 70.0);
    run.number(&watch, 90.0);
    run.clock = utc("2026-11-25T11:59:30Z");
    let named = opened(&run.number(&watch, 70.0), Sort::Drop).unwrap();
    assert_eq!(named, utc("2026-11-25T12:00:01Z"));
}

#[test]
fn an_unreadable_status_opens_an_unknown_episode_once_its_value_stops_holding() {
    let watch = coverage(1, 2);
    let mut run = Run::new();
    run.number(&watch, 85.0);
    for _ in 0..6 {
        assert!(opened(&run.unreadable(&watch), Sort::Unknown).is_none());
    }
    // Read at 12:00; the slot at 12:10, plus an hour's grace.
    let effects = run.unreadable(&watch);
    assert_eq!(
        opened(&effects, Sort::Unknown),
        Some(utc("2026-11-25T13:10:00Z"))
    );
    assert_eq!(
        run.record.cause,
        Some(Cause::Unreadable {
            why: "exit 2".into()
        })
    );
    assert!(opened(&run.unreadable(&watch), Sort::Unknown).is_none());
    let effects = run.number(&watch, 75.0);
    assert_eq!(
        ended(&effects, Sort::Unknown),
        Some(Ending::ReadAgain {
            value: "75.0%".into()
        })
    );
    // Down while it was unknown: the reading that ended it opens a drop.
    assert!(opened(&effects, Sort::Drop).is_some());
    assert_eq!(run.record.unknown, None);
}

#[test]
fn an_overdue_status_opens_nothing() {
    let watch = coverage(1, 2);
    let mut run = Run::new();
    run.number(&watch, 85.0);
    let effects = run.look(&watch, utc("2026-11-25T15:00:00Z"));
    assert!(effects.is_empty());
    assert_eq!(run.record.cause, Some(Cause::Overdue));
    let failing = step(
        &mut run.record,
        &watch,
        &Event::Look {
            slot_error: Some("the slot exited 1".into()),
        },
        utc("2026-11-25T15:00:00Z"),
        &Utc,
    );
    assert_eq!(
        opened(&failing, Sort::Unknown),
        Some(utc("2026-11-25T13:10:00Z"))
    );
}

#[test]
fn unknown_while_down_keeps_the_drop_open_beside_it() {
    let watch = coverage(1, 2);
    let mut run = Run::new();
    let drop = opened(&run.number(&watch, 70.0), Sort::Drop).unwrap();
    for _ in 0..7 {
        run.unreadable(&watch);
    }
    assert!(run.record.unknown.is_some());
    assert_eq!(run.record.drop.as_ref().unwrap().named, drop);
    assert_eq!(run.record.hold.health, Some(Health::Down));
    // The first reading ends the unknown episode and counts toward recovery.
    let effects = run.number(&watch, 90.0);
    assert!(ended(&effects, Sort::Unknown).is_some());
    assert!(ended(&effects, Sort::Drop).is_none());
    assert!(ended(&run.number(&watch, 90.0), Sort::Drop).is_some());
}

#[test]
fn a_condition_edited_mid_episode_moves_the_matter_once_and_keeps_the_count() {
    let mut run = Run::new();
    let at_80 = coverage(1, 2);
    run.number(&at_80, 70.0);
    run.number(&at_80, 85.0);
    let at_85 = watch(healthy(json!({"min": 85})), 1, 2);
    let effects = step(
        &mut run.record,
        &at_85,
        &Event::Look { slot_error: None },
        run.clock,
        &Utc,
    );
    assert_eq!(
        moved(&effects),
        vec!["condition changed: healthy ≥ 80% → healthy ≥ 85%"]
    );
    assert_eq!(run.record.hold.run, 1);
    assert_eq!(run.record.hold.band, Some(Band::Ok));
    let again = step(
        &mut run.record,
        &at_85,
        &Event::Look { slot_error: None },
        run.clock,
        &Utc,
    );
    assert!(moved(&again).is_empty());
    assert!(ended(&run.number(&at_85, 86.0), Sort::Drop).is_some());
}

#[test]
fn a_level_worse_than_any_the_drop_reached_moves_it() {
    let levels: Vec<String> = ["clean", "degraded", "failed"].map(String::from).to_vec();
    let judge = healthy(json!({"in": ["clean"]}));
    let watch = Watch {
        levels: Some(&levels),
        kind: Some(Kind::Level),
        ..watch(judge, 1, 1)
    };
    let mut run = Run::new();
    let level = |name: &str| Value::Level(name.into());
    assert!(opened(&run.value(&watch, level("degraded")), Sort::Drop).is_some());
    assert_eq!(
        moved(&run.value(&watch, level("failed"))),
        vec!["level worse: degraded → failed"]
    );
    assert!(moved(&run.value(&watch, level("degraded"))).is_empty());
    assert!(moved(&run.value(&watch, level("failed"))).is_empty());
    assert_eq!(
        run.record.drop.as_ref().unwrap().worst.as_deref(),
        Some("failed")
    );
}

#[test]
fn a_removed_monitor_ends_its_episodes() {
    let watch = coverage(1, 2);
    let mut run = Run::new();
    run.number(&watch, 70.0);
    for _ in 0..7 {
        run.unreadable(&watch);
    }
    let effects = step(
        &mut run.record,
        &watch,
        &Event::Unwatched(Unwatched::Gone),
        run.clock,
        &Utc,
    );
    let gone = Ending::Unwatched {
        why: Unwatched::Gone,
    };
    assert_eq!(ended(&effects, Sort::Drop), Some(gone.clone()));
    assert_eq!(ended(&effects, Sort::Unknown), Some(gone));
    assert_eq!(
        (run.record.drop.is_none(), run.record.unknown.is_none()),
        (true, true)
    );
    assert_eq!(run.record.hold, Hold::default());
}

#[test]
fn a_monitor_that_turns_kpi_ends_its_drop_and_opens_nothing() {
    let mut run = Run::new();
    run.number(&coverage(1, 2), 70.0);
    let kpi = watch(Judge::Kpi, 1, 2);
    let effects = run.number(&kpi, 10.0);
    assert_eq!(
        ended(&effects, Sort::Drop),
        Some(Ending::Unwatched {
            why: Unwatched::Kpi
        })
    );
    assert!(effects.contains(&Effect::Record(Line::Reading {
        t: utc("2026-11-25T12:10:00Z"),
        v: Value::Number(10.0),
        health: None
    })));
    for _ in 0..8 {
        assert!(opened(&run.unreadable(&kpi), Sort::Unknown).is_none());
    }
    assert!(run.record.cause.is_some());
}

#[test]
fn a_change_of_kind_ends_the_episodes_and_restarts_history() {
    let mut run = Run::new();
    run.number(&coverage(1, 2), 70.0);
    assert_eq!(run.record.kind, Some(Kind::Number));
    let levels: Vec<String> = ["a", "b"].map(String::from).to_vec();
    let level = Watch {
        kind: Some(Kind::Level),
        levels: Some(&levels),
        ..watch(healthy(json!({"in": ["a"]})), 1, 2)
    };
    let effects = run.value(&level, Value::Level("a".into()));
    assert_eq!(
        ended(&effects, Sort::Drop),
        Some(Ending::Unwatched {
            why: Unwatched::KindChanged
        })
    );
    assert_eq!(effects[1], Effect::RestartHistory);
    assert_eq!(run.record.kind, Some(Kind::Level));
    assert_eq!(run.record.hold.health, Some(Health::Ok));
}

#[test]
fn still_down_after_its_work_ended_moves_once_per_work() {
    let watch = coverage(1, 2);
    let mut run = Run::new();
    run.number(&watch, 70.0);
    let work = |id: &str| {
        vec![EndedWork {
            on: Sort::Drop,
            work: id.into(),
        }]
    };
    let failing = |run: &mut Run, ended| {
        let reading = reading(&watch, Value::Number(71.0), run.clock);
        run.found(&watch, Found::Reading(reading), false, ended)
    };
    let next_slot = run.record.schedule.next_slot;
    assert_eq!(
        moved(&failing(&mut run, work("w1"))),
        vec!["still down after its work ended"]
    );
    assert_eq!(run.record.schedule.next_slot, next_slot);
    assert!(moved(&failing(&mut run, work("w1"))).is_empty());
    assert_eq!(
        moved(&failing(&mut run, work("w2"))),
        vec!["still down after its work ended"]
    );
    let passing = reading(&watch, Value::Number(90.0), run.clock);
    assert!(moved(&run.found(&watch, Found::Reading(passing), false, work("w3"))).is_empty());
    let unread = run.found(
        &watch,
        Found::Unreadable { why: "x".into() },
        false,
        work("w4"),
    );
    assert_eq!(moved(&unread), vec!["still down after its work ended"]);
    assert_eq!(
        run.record.drop.as_ref().unwrap().works_ended,
        vec!["w1", "w2", "w3", "w4"]
    );
}

#[test]
fn still_unknown_after_its_work_ended_names_the_cause() {
    let watch = coverage(1, 2);
    let mut run = Run::new();
    run.number(&watch, 85.0);
    for _ in 0..7 {
        run.unreadable(&watch);
    }
    let ended = vec![EndedWork {
        on: Sort::Unknown,
        work: "w1".into(),
    }];
    let effects = run.found(
        &watch,
        Found::Stale { measured: None },
        false,
        ended.clone(),
    );
    assert_eq!(moved(&effects), vec!["still stale after its work ended"]);
    assert!(moved(&run.found(&watch, Found::Parked, false, ended)).is_empty());
}

#[test]
fn what_a_read_records_and_whether_it_moves_the_schedule() {
    let watch = coverage(1, 2);
    let mut run = Run::new();
    run.number(&watch, 85.0);
    assert_eq!(
        run.record.schedule.next_slot,
        Some(utc("2026-11-25T12:10:00Z"))
    );
    // Out of cadence: counts, is appended, does not move the schedule.
    let out = run.found(
        &watch,
        Found::Reading(reading(&watch, Value::Number(86.0), run.clock)),
        false,
        vec![],
    );
    assert!(matches!(out[0], Effect::Record(Line::Reading { .. })));
    assert_eq!(
        run.record.schedule.next_slot,
        Some(utc("2026-11-25T12:10:00Z"))
    );
    assert_eq!(run.record.held_from, Some(utc("2026-11-25T12:10:00Z")));
    // Stale: moves the schedule only.
    let stale = run.found(&watch, Found::Stale { measured: None }, true, vec![]);
    assert!(stale.is_empty());
    assert_eq!(
        run.record.schedule.next_slot,
        Some(utc("2026-11-25T12:30:00Z"))
    );
    assert_eq!(run.record.held_from, Some(utc("2026-11-25T12:10:00Z")));
    // Parked: the slot passes and parked-since is set.
    assert!(run.found(&watch, Found::Parked, true, vec![]).is_empty());
    assert_eq!(
        run.record.schedule.parked_since,
        Some(utc("2026-11-25T12:30:00Z"))
    );
    // Unreadable: a marker, and the schedule moves.
    let unreadable = run.unreadable(&watch);
    assert_eq!(
        unreadable,
        vec![Effect::Record(Line::marker(utc("2026-11-25T12:40:00Z")))]
    );
    assert_eq!(run.record.schedule.parked_since, None);
    assert_eq!(run.record.last_ok.as_ref().unwrap().v, Value::Number(86.0));
}

#[test]
fn a_monitor_first_read_after_it_was_first_seen_holds_from_when_it_was_seen() {
    for (every, quiet, unknown) in [
        ("10m", "2026-11-25T13:09:59Z", "2026-11-25T13:10:00Z"),
        ("1w", "2026-12-04T05:59:59Z", "2026-12-04T06:00:00Z"),
    ] {
        let watch = Watch {
            cadence: Cadence::resolve(Some(every), None).unwrap(),
            ..coverage(1, 2)
        };
        let mut record = Record::new(utc("2026-11-25T12:00:00Z"));
        let mut unreadable = |at: &str| {
            let read = Read {
                at: utc(at),
                in_cadence: true,
                found: Found::Unreadable {
                    why: "exit 2".into(),
                },
                after_work: Vec::new(),
            };
            step(&mut record, &watch, &Event::Read(read), utc(at), &Utc)
        };
        // Its first read comes after ephor first saw it, and sets the anchor.
        let first = unreadable("2026-11-25T12:00:20Z");
        assert!(opened(&first, Sort::Unknown).is_none(), "{every}");
        assert!(
            opened(&unreadable(quiet), Sort::Unknown).is_none(),
            "{every}"
        );
        assert_eq!(
            opened(&unreadable(unknown), Sort::Unknown),
            Some(utc(unknown)),
            "{every}"
        );
    }
}
