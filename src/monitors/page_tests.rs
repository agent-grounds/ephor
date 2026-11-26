use chrono::FixedOffset;
use serde_json::{json, Value as Json};

use super::*;
use crate::monitors::condition::Judge;
use crate::monitors::declared;
use crate::monitors::model::{Band, Point};
use crate::monitors::state::{Hold, OpenDrop, Past, Shown};
use crate::monitors::test_zone::utc;

fn zone() -> FixedOffset {
    FixedOffset::east_opt(0).unwrap()
}

fn t0() -> DateTime<Utc> {
    utc("2026-11-25T00:00:00Z")
}

fn minutes(minutes: i64) -> DateTime<Utc> {
    t0() + TimeDelta::minutes(minutes)
}

/// One monitor as the site declares it, resolved.
fn monitor(declaration: Json) -> Monitor {
    let id = declaration["id"].as_str().unwrap().to_string();
    declared::read(None, &[declaration]).resolve(&id, None)
}

fn coverage() -> Monitor {
    monitor(json!({
        "id": "coverage", "unit": "%", "healthy": {"min": 80}, "every": "1h",
        "drop_after": 2, "recover_after": 2, "from": {"command": "kpis.sh"}
    }))
}

/// A record read at `t0` and holding `health` since then.
fn read(health: Health, band: Band) -> Record {
    let mut record = Record::new(t0());
    record.held_from = Some(t0());
    record.hold = Hold {
        health: Some(health),
        since: Some(t0()),
        band: Some(band),
        run: 1,
    };
    record
}

fn open_drop() -> OpenDrop {
    OpenDrop {
        named: t0(),
        moved: None,
        worst: None,
        judged_under: Judge::SampleOk,
        judged_words: "healthy ≥ 80%".into(),
        works_ended: Vec::new(),
        renewed: None,
        shown: Shown::default(),
    }
}

fn at(monitor: &Monitor, record: Option<&Record>, now: DateTime<Utc>) -> Standing {
    let facts = Facts {
        monitor,
        cadence: &monitor.cadence,
        record,
        slot_error: None,
        recent: TimeDelta::days(7),
    };
    standing(&facts, now, &zone())
}

/// §FS-021-monitors.18.7: the health and how long it has held, while the
/// value holds; the cause and how long once a slot passed without a
/// reading, the last health still holding; and unknown once it no longer
/// does, counted a second time where it is overdue (§FS-021-monitors.18.2).
#[test]
fn a_late_monitor_names_its_cause_and_an_unknown_one_counts_overdue() {
    let monitor = coverage();
    let record = read(Health::Ok, Band::Ok);

    let holding = at(&monitor, Some(&record), minutes(30));
    assert_eq!(holding.rank, Rank::Ok);
    assert_eq!(holding.column.text(), "ok 30m");
    assert_eq!(holding.counts, Some(Rank::Ok));
    assert!(!holding.overdue);

    let late = at(&monitor, Some(&record), minutes(90));
    assert_eq!(late.rank, Rank::Ok, "the last health holds while late");
    assert_eq!(late.column.text(), "overdue 30m");
    assert!(!late.overdue);

    let unknown = at(&monitor, Some(&record), minutes(250));
    assert_eq!(unknown.rank, Rank::Unknown);
    assert_eq!(unknown.column.text(), "overdue 3h 10m");
    assert_eq!(unknown.health, Some(Health::Unknown));
    assert_eq!(unknown.since, Some(minutes(120)));
    assert_eq!(unknown.counts, Some(Rank::Unknown));
    assert!(unknown.overdue && unknown.status);
}

/// §FS-021-monitors.18.3: while a drop is open the row is down whatever its
/// freshness, and its health column names the cause.
#[test]
fn an_open_drop_is_down_while_it_is_also_unknown() {
    let monitor = coverage();
    let mut record = read(Health::Down, Band::Down);
    record.drop = Some(open_drop());
    let standing = at(&monitor, Some(&record), minutes(250));
    assert_eq!(standing.rank, Rank::Down);
    assert_eq!(standing.health, Some(Health::Down));
    assert_eq!(standing.column.state, "overdue");
    assert_eq!(standing.counts, Some(Rank::Down));
    assert!(standing.overdue);
    assert_eq!(Rank::Down.glyph(), "✖");
}

/// §FS-021-monitors.18.7: a hold counting toward a drop is failing, and one
/// counting toward a recovery is recovering, which the page ranks down.
#[test]
fn a_counting_hold_is_failing_or_recovering() {
    let monitor = coverage();
    let failing = at(&monitor, Some(&read(Health::Ok, Band::Down)), minutes(10));
    assert_eq!(failing.rank, Rank::Failing);
    assert_eq!(failing.column.text(), "failing 1 of 2");
    assert_eq!(failing.counts, Some(Rank::Failing));

    let mut record = read(Health::Down, Band::Ok);
    record.drop = Some(open_drop());
    let recovering = at(&monitor, Some(&record), minutes(10));
    assert_eq!(recovering.rank, Rank::Down);
    assert_eq!(recovering.column.text(), "recovering 1 of 2");
}

/// §FS-021-monitors.18.7: `recovered` and how long, while its finished
/// matter is still reported, and the health once it is not.
#[test]
fn a_recovery_is_said_while_its_matter_is_reported() {
    let monitor = coverage();
    let mut record = read(Health::Ok, Band::Ok);
    record.hold.run = 2;
    record.past.push(Past {
        sort: Sort::Drop,
        named: t0() - TimeDelta::days(1),
        ended: t0(),
        how: Ending::Recovered {
            value: "81.0%".into(),
        },
        moved: None,
        cause: None,
        shown: Shown::default(),
    });
    assert_eq!(
        at(&monitor, Some(&record), minutes(50)).column.text(),
        "recovered 50m"
    );
    let facts = Facts {
        monitor: &monitor,
        cadence: &monitor.cadence,
        record: Some(&record),
        slot_error: None,
        recent: TimeDelta::minutes(10),
    };
    assert_eq!(
        standing(&facts, minutes(50), &zone()).column.text(),
        "ok 50m"
    );
}

/// §FS-021-monitors.18.2, §FS-021-monitors.21.2: a monitor not run, and one
/// with no reading yet, is shown unknown and counts nowhere; one whose
/// declaration is in error counts as unknown; a KPI is a KPI and no status.
#[test]
fn what_ephor_does_not_read_is_shown_and_counted_by_its_rule() {
    let mut not_run = coverage();
    not_run.not_run = true;
    let standing = at(&not_run, None, minutes(10));
    assert_eq!(standing.column.state, "not run");
    assert_eq!(standing.column.sentence.as_deref(), Some(NOT_RUN));
    assert_eq!((standing.rank, standing.counts), (Rank::Unknown, None));

    let mut faulted = coverage();
    faulted.faults.push("healthy and warn disagree".into());
    let standing = at(&faulted, None, minutes(10));
    assert_eq!(standing.column.state, "not read");
    assert_eq!(standing.counts, Some(Rank::Unknown));

    let standing = at(&coverage(), None, minutes(10));
    assert_eq!(standing.column.text(), "not read yet");
    assert_eq!(standing.counts, None);
    let unread = Record::new(t0());
    let standing = at(&coverage(), Some(&unread), minutes(10));
    assert_eq!(standing.column.text(), "not read yet");
    assert_eq!(standing.counts, None);

    let kpi = monitor(json!({"id": "lines", "healthy": null, "every": "1h"}));
    let mut record = Record::new(t0());
    record.held_from = Some(t0());
    let standing = at(&kpi, Some(&record), minutes(10));
    assert_eq!(standing.rank, Rank::Kpi);
    assert_eq!(standing.column.text(), "KPI");
    assert!(!standing.status);
    let standing = at(&kpi, Some(&record), minutes(250));
    assert_eq!(standing.rank, Rank::Unknown, "an unknown KPI shows unknown");
    assert!(!standing.status);
}

/// §FS-021-monitors.18.7: a series drawn by `x` labels counts its health in
/// points.
#[test]
fn a_labelled_series_counts_its_health_in_points() {
    let monitor = monitor(json!({
        "id": "bench", "kind": "series", "healthy": {"max": 10}, "every": "1h"
    }));
    let mut record = read(Health::Ok, Band::Ok);
    let point = |x: &str, v: f64| Point {
        t: None,
        x: Some(x.into()),
        v,
    };
    record.last_sample = Some(crate::monitors::model::Sample {
        id: "bench".into(),
        points: Some(vec![point("a", 12.0), point("b", 4.0), point("c", 5.0)]),
        ..Default::default()
    });
    assert_eq!(
        at(&monitor, Some(&record), minutes(10)).column.text(),
        "ok 2 points"
    );
}

/// §FS-021-monitors.18.7: how long, in the two largest units.
#[test]
fn a_duration_is_said_in_its_two_largest_units() {
    let said = |minutes: i64| lasted(TimeDelta::minutes(minutes));
    assert_eq!(said(50), "50m");
    assert_eq!(said(60), "1h");
    assert_eq!(said(230), "3h 50m");
    assert_eq!(said(60 * 52), "2d 4h");
    assert_eq!(said(60 * 24 * 12), "12d");
    assert_eq!(said(60 * 24 * 63), "9w");
    assert_eq!(said(-5), "0m");
}
