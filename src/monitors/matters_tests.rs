use super::*;
use crate::monitors::condition::Judge;
use crate::monitors::model::Value;
use crate::monitors::state::{Kept, Record};
use crate::monitors::test_zone::utc;
use chrono::FixedOffset;

fn zone() -> FixedOffset {
    FixedOffset::east_opt(0).unwrap()
}

fn coverage() -> Shown {
    Shown {
        title: "Line coverage".into(),
        value: Some(Value::Number(79.2)),
        unit: Some("%".into()),
        condition: "healthy ≥ 80%".into(),
        since: Some(utc("2026-11-25T06:00:00Z")),
        last_ok: Some(Kept {
            t: utc("2026-11-24T06:00:00Z"),
            v: Value::Number(81.0),
            of: None,
        }),
        detail: Some("3 files lost tests".into()),
        url: Some("https://example.com/runs/7".into()),
        ..Shown::default()
    }
}

fn drop(moved: Option<Moved>) -> OpenDrop {
    OpenDrop {
        named: utc("2026-11-25T06:00:00Z"),
        moved,
        worst: None,
        judged_under: Judge::SampleOk,
        judged_words: "healthy ≥ 80%".into(),
        works_ended: Vec::new(),
        renewed: None,
        shown: coverage(),
    }
}

fn document(record: Record) -> Document {
    let mut document = Document::default();
    document.monitors.insert("coverage".into(), record);
    document
}

fn one(document: &Document, now: DateTime<Utc>) -> Item {
    let items = items("widget", document, &BTreeMap::new(), now, 7, &zone());
    assert_eq!(items.len(), 1, "{items:?}");
    items.into_iter().next().unwrap()
}

/// A drop is a status matter of the monitor source, keyed by its episode,
/// titled with its value, condition and detail, `down`, timed when it
/// opened, linked to its sample, and carrying its facts as meta
/// (§FS-021-monitors.11.3, §FS-021-monitors.12).
#[test]
fn an_open_drop_is_a_down_status_matter_with_its_facts() {
    let mut record = Record::new(utc("2026-11-20T06:00:00Z"));
    record.drop = Some(drop(None));
    let item = one(&document(record), utc("2026-11-25T07:00:00Z"));
    assert_eq!(item.id, "monitor:widget/coverage@20261125T060000Z");
    assert_eq!(item.source, "monitor");
    assert_eq!(item.kind, ItemKind::Status);
    assert_eq!(
        item.title,
        "Line coverage dropped: 79.2% (healthy ≥ 80%) · 3 files lost tests"
    );
    assert_eq!(item.state.as_deref(), Some("down"));
    assert_eq!(item.updated_at, utc("2026-11-25T06:00:00Z"));
    assert_eq!(item.url.as_deref(), Some("https://example.com/runs/7"));
    assert!(!item.is_finished());
    let meta = item.meta().unwrap();
    assert_eq!(meta["monitor"], "coverage");
    assert_eq!(meta["value"], "79.2");
    assert_eq!(meta["unit"], "%");
    assert_eq!(meta["condition"], "healthy ≥ 80%");
    assert_eq!(meta["last_ok_value"], "81");
    assert_eq!(meta["detail"], "3 files lost tests");
    assert!(!meta.contains_key("cause"));
}

/// A matter's time is when it last moved (§FS-021-monitors.12.2), and a
/// move that renews it says so (§FS-021-monitors.13.2).
#[test]
fn a_moved_drop_is_timed_at_its_move_and_states_why() {
    let mut record = Record::new(utc("2026-11-20T06:00:00Z"));
    let mut open = drop(Some(Moved {
        at: utc("2026-11-25T09:00:00Z"),
        why: Movement::StillDown,
    }));
    open.works_ended.push("w1".into());
    open.renewed = Some(utc("2026-11-25T09:00:00Z"));
    record.drop = Some(open);
    let item = one(&document(record), utc("2026-11-25T10:00:00Z"));
    assert_eq!(item.updated_at, utc("2026-11-25T09:00:00Z"));
    let stated = stated(&item.source, &item.raw).unwrap();
    assert_eq!(stated.moves.renewed, Some(utc("2026-11-25T09:00:00Z")));
    assert_eq!(
        stated.moves.since(&Moves {
            condition: "healthy ≥ 80%".into(),
            ..Moves::default()
        }),
        vec!["still down after its work ended"]
    );
}

/// A matter New again stays New through a later move of another kind: a
/// worse level after it was renewed is still a matter nobody has laid work
/// on since its work ended (§FS-021-monitors.13.2, §FS-019-waiting.1.2).
#[test]
fn a_renewed_drop_stays_renewed_through_a_later_move() {
    let mut record = Record::new(utc("2026-11-20T06:00:00Z"));
    let mut open = drop(Some(Moved {
        at: utc("2026-11-25T10:00:00Z"),
        why: Movement::LevelWorse {
            from: "degraded".into(),
            to: "failed".into(),
        },
    }));
    open.works_ended.push("w1".into());
    open.renewed = Some(utc("2026-11-25T09:00:00Z"));
    record.drop = Some(open);
    let item = one(&document(record), utc("2026-11-25T11:00:00Z"));
    assert_eq!(item.updated_at, utc("2026-11-25T10:00:00Z"));
    let laid = [utc("2026-11-25T08:00:00Z")];
    assert!(!still_processed(&item.source, &item.raw, laid));
}

/// An unknown matter's title names its cause, and its meta the cause's
/// word (§FS-021-monitors.12.1, §FS-021-monitors.12.3).
#[test]
fn an_open_unknown_names_its_cause() {
    let mut record = Record::new(utc("2026-11-20T06:00:00Z"));
    record.unknown = Some(OpenUnknown {
        named: utc("2026-11-25T06:00:00Z"),
        moved: None,
        cause: Cause::Unreadable {
            why: "target/kpi/coverage.json is missing".into(),
        },
        works_ended: Vec::new(),
        renewed: None,
        shown: coverage(),
    });
    let item = one(&document(record), utc("2026-11-25T07:00:00Z"));
    assert_eq!(item.id, "monitor:widget/coverage.unknown@20261125T060000Z");
    assert_eq!(
        item.title,
        "Line coverage cannot be read: target/kpi/coverage.json is missing"
    );
    assert_eq!(item.state.as_deref(), Some("unknown"));
    assert_eq!(item.meta().unwrap()["cause"], "unreadable");
    assert_eq!(
        cannot(
            "Mutation score",
            &Cause::Stale {
                measured: Some(utc("2026-11-16T10:00:00Z"))
            },
            &zone()
        ),
        "Mutation score is stale: measured Nov 16"
    );
    assert_eq!(
        cannot(
            "Parse benchmark",
            &Cause::Parked {
                since: utc("2026-11-24T12:00:00Z")
            },
            &zone()
        ),
        "Parse benchmark is parked: every read since Nov 24 12:00 exited 75"
    );
}

/// A finished episode is terminal, says how it ended, and is reported
/// while the recency window keeps it, a forgotten monitor's included
/// (§FS-021-monitors.14).
#[test]
fn a_finished_episode_is_terminal_until_the_window_lets_it_go() {
    let past = |ended: &str, how: Ending| Past {
        sort: Sort::Drop,
        named: utc("2026-11-20T06:00:00Z"),
        ended: utc(ended),
        how,
        moved: None,
        cause: None,
        shown: coverage(),
    };
    let mut record = Record::new(utc("2026-11-01T06:00:00Z"));
    record.past.push(past(
        "2026-11-25T06:02:00Z",
        Ending::Recovered {
            value: "81.0%".into(),
        },
    ));
    record.past.push(past(
        "2026-11-01T06:02:00Z",
        Ending::Recovered {
            value: "81.0%".into(),
        },
    ));
    let mut document = document(record);
    document.forgotten.insert(
        "gone".into(),
        vec![past(
            "2026-11-24T06:00:00Z",
            Ending::Unwatched {
                why: episode::Unwatched::Gone,
            },
        )],
    );
    let items = items(
        "widget",
        &document,
        &BTreeMap::new(),
        utc("2026-11-25T07:00:00Z"),
        7,
        &zone(),
    );
    let said: Vec<(&str, Option<&str>, bool)> = items
        .iter()
        .map(|item| {
            (
                item.title.as_str(),
                item.state.as_deref(),
                item.is_finished(),
            )
        })
        .collect();
    assert_eq!(
        said,
        vec![
            (
                "Line coverage recovered at Nov 25 06:02 (81.0%)",
                Some("recovered"),
                true
            ),
            (
                "Line coverage no longer watched at Nov 24 06:00: it is no longer declared or brought",
                Some("unwatched"),
                true
            ),
        ]
    );
    assert_eq!(items[0].updated_at, utc("2026-11-25T06:02:00Z"));
}

/// Only the monitor slot states a monitor's finality: another source
/// carrying the same record has no authority, and a state that merely
/// contains a terminal word is not read as one (§FS-003-feed-categories.2).
#[test]
fn only_the_monitor_source_states_its_record() {
    let raw = serde_json::json!({RECORD: {"terminal": true}});
    assert!(stated("custom-status", &raw).is_none());
    assert!(stated(SOURCE, &raw).unwrap().terminal);
    let mut item = one(
        &{
            let mut record = Record::new(utc("2026-11-20T06:00:00Z"));
            record.drop = Some(drop(None));
            document(record)
        },
        utc("2026-11-25T07:00:00Z"),
    );
    item.state = Some("unresolved".into());
    assert!(!item.is_finished());
}

/// Movement is the worst level, the condition and being still down or
/// unknown after the work ended, said in the spec's sentences (§FS-021-monitors.13.1, §FS-021-monitors.13.2).
#[test]
fn moves_say_what_moved() {
    let was = Moves {
        worst: Some("degraded".into()),
        condition: "healthy ≥ 80%".into(),
        ..Moves::default()
    };
    let now = Moves {
        worst: Some("failed".into()),
        condition: "healthy ≥ 85%".into(),
        renewed: Some(utc("2026-11-25T09:00:00Z")),
        still: Some("still unreadable after its work ended".into()),
    };
    assert_eq!(
        now.since(&was),
        vec![
            "level worse: degraded → failed",
            "condition changed: healthy ≥ 80% → healthy ≥ 85%",
            "still unreadable after its work ended",
        ]
    );
    assert!(now.since(&now).is_empty());
}

/// Still down after its work ended, the matter is New again until work is
/// laid on it anew (§FS-021-monitors.13.2, §FS-019-waiting.1.2); any other
/// matter with work laid stays processed.
#[test]
fn a_renewed_matter_is_processed_only_by_work_laid_since() {
    let mut record = Record::new(utc("2026-11-20T06:00:00Z"));
    let mut open = drop(Some(Moved {
        at: utc("2026-11-25T09:00:00Z"),
        why: Movement::StillDown,
    }));
    open.renewed = Some(utc("2026-11-25T09:00:00Z"));
    record.drop = Some(open);
    let item = one(&document(record), utc("2026-11-25T10:00:00Z"));
    let before = utc("2026-11-25T07:00:00Z");
    let after = utc("2026-11-25T09:30:00Z");
    assert!(!still_processed(&item.source, &item.raw, [before]));
    assert!(still_processed(&item.source, &item.raw, [before, after]));
    assert!(still_processed("custom-status", &item.raw, [before]));
}
