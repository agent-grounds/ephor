//! How a plain refresh reads, claims and keeps its monitors' runs: the
//! claims a run's timeout holds, the cadence each monitor is judged at, the
//! moment an answer is taken, and a store that stays whole when a write
//! fails.

use serde_json::json;

use super::*;
use crate::monitors::freshness::Found;
use crate::monitors::history::Line;
use crate::monitors::model::{Kind, Value};
use crate::monitors::test_site::*;

fn claim(site: &Site, ids: &[&str], at: DateTime<Utc>) {
    site.store
        .update(|document| {
            for id in ids {
                let record = document
                    .monitors
                    .entry((*id).to_string())
                    .or_insert_with(|| Record::new(at));
                record.claim = Some(Claim {
                    pid: 1,
                    at,
                    slot: None,
                    timeout: None,
                });
            }
            Ok(())
        })
        .unwrap();
}

/// §FS-021-monitors.6.9, §FS-021-monitors.5.5: a claim stands for as long as
/// the run that wrote it may take, the longest timeout among the monitors
/// its command reads, not the shortest one of them has on its own.
#[test]
fn a_claim_holds_for_its_runs_timeout() {
    let site = site(None);
    let store = site.store.path();
    // The script keeps the store as it stood while it ran: the claims.
    std::fs::write(
        site.root.join("kpis.sh"),
        format!(
            "echo \"$EPHOR_MONITORS\" >> runs.log\ncp '{}' claimed.json\nprintf '%s' '{BOTH}' > \"$EPHOR_ANSWER\"\n",
            store.display()
        ),
    )
    .unwrap();
    let declared = [
        json!({"id": "a", "healthy": {"min": 1}, "timeout": 30,
               "from": {"command": "sh kpis.sh"}}),
        json!({"id": "b", "healthy": {"min": 1}, "timeout": 300,
               "from": {"command": "sh kpis.sh"}}),
    ];
    refresh_at(&site, &declared, When::Due, 0);
    let claimed: Document =
        serde_json::from_str(&std::fs::read_to_string(site.root.join("claimed.json")).unwrap())
            .unwrap();
    for id in ["a", "b"] {
        assert_eq!(
            claimed.monitors[id].claim.as_ref().unwrap().timeout,
            Some(300)
        );
    }

    // A claim that keeps no timeout is judged by its run's all the same:
    // 100 seconds on, `a`'s own 30 have passed and the run's 300 have not.
    claim(&site, &["a", "b"], at(30));
    let later = at(30) + TimeDelta::seconds(100);
    refresh_then(&site, &declared, When::Due, later);
    assert_eq!(runs(&site.root).len(), 1, "{:?}", runs(&site.root));
}

/// §FS-021-monitors.6.6: a command that answers several monitors runs when
/// any is due, and only the due ones record a reading.
#[test]
fn a_shared_command_records_only_its_due_monitors() {
    let site = site(None);
    script(&site.root, BOTH, 0);
    let declared = [
        json!({"id": "a", "healthy": {"min": 1}, "every": "10m",
               "from": {"command": "sh kpis.sh"}}),
        json!({"id": "b", "healthy": {"min": 1}, "every": "1h",
               "from": {"command": "sh kpis.sh"}}),
    ];
    refresh_at(&site, &declared, When::Due, 0);
    refresh_at(&site, &declared, When::Due, 11);
    assert_eq!(runs(&site.root), vec!["a,b", "a"]);
    assert_eq!(site.store.history("a").unwrap().len(), 2);
    assert_eq!(site.store.history("b").unwrap().len(), 1);
    let document = site.store.load().unwrap();
    assert!(document.monitors["b"].last_attempt.as_ref().unwrap().at < at(1));
}

/// §FS-021-monitors.5.6: a declared monitor no answer has owned yet is read
/// from every answer a plain refresh takes, and finding it in none is an
/// unreadable read that says so.
#[test]
fn a_monitor_no_answer_carries_is_unreadable() {
    let site = site(None);
    let declared = [json!({"id": "k", "healthy": {"min": 1}})];
    assert_eq!(
        refresh_at(&site, &declared, When::Due, 0).slot,
        Some(Ok(()))
    );
    assert_eq!(unreadable(&site, "k"), "no answer carries k");
}

/// §FS-021-monitors.6.8: a site declaration that states a cadence for a
/// monitor a custom-status answer brings is the cadence its sample is
/// measured at: a sample two hours old is fresh at `every: 1d`.
#[test]
fn a_brought_monitor_is_measured_at_its_stated_cadence() {
    let site = site(None);
    let sample = json!({"id": "k", "value": 5,
                        "at": (at(0) - TimeDelta::hours(2)).to_rfc3339()});
    let declared = [json!({"id": "k", "healthy": {"min": 1}, "every": "1d"})];
    let report = refresh_bringing(
        &site,
        &declared,
        vec![Outcome::Answered(vec![sample])],
        When::Due,
        at(0),
    );
    assert_eq!(report.slot, Some(Ok(())));
    assert_eq!(found(&site, "k"), Some(Found::Reading));
}

/// §FS-021-monitors.9.3, §FS-021-monitors.6.8: a monitor read from a daily
/// answer is never thinned, whatever cadence it would have on its own.
#[test]
fn a_monitor_brought_by_a_daily_answer_is_not_thinned() {
    let site = site(None);
    script(&site.root, BOTH, 0);
    let declared = [json!({"id": "a", "healthy": {"min": 1}, "every": "1d",
                           "from": {"command": "sh kpis.sh"}})];
    refresh_at(&site, &declared, When::Due, 0);
    let old: Vec<Line> = (0..300)
        .map(|n| Line::Reading {
            t: at(-60 * 24 * 60 + n * 10),
            v: Value::Number(3.0),
            health: None,
            out_of_cadence: false,
        })
        .collect();
    site.store.rewrite("b", &old).unwrap();
    site.store
        .update(|document| {
            document.thinned = None;
            document.monitors.get_mut("b").unwrap().kind = Some(Kind::Number);
            Ok(())
        })
        .unwrap();
    refresh_at(&site, &declared, When::Now, 24 * 60);
    let history = site.store.history("b").unwrap();
    assert!(
        history
            .iter()
            .all(|line| !matches!(line, Line::Thinned { .. })),
        "{history:?}"
    );
    assert_eq!(history.len(), 301);
}

/// §FS-021-monitors.3.5, §FS-021-monitors.4.6: a monitor whose homes break a
/// rule together is noted by every refresh, and not read.
#[test]
fn a_rule_the_homes_break_together_is_noted() {
    let site = site(None);
    script(&site.root, BOTH, 0);
    let declared = [
        json!({"id": "a", "healthy": {"max": 5}, "warn": {"max": 7},
               "from": {"command": "sh kpis.sh"}}),
        from_kpis("b"),
    ];
    let report = refresh_at(&site, &declared, When::Due, 0);
    assert_eq!(report.slot, Some(Ok(())));
    assert!(
        report
            .notes
            .iter()
            .any(|note| note.starts_with("monitor a is not read: ")),
        "{:?}",
        report.notes
    );
    assert_eq!(runs(&site.root), vec!["b"]);
}

/// §FS-021-monitors.7.1: a sample that says no time of its own was measured
/// when its answer was taken, after its command ran, so a slow command's
/// reading is neither stale nor skewed.
#[test]
fn a_reading_is_timed_when_its_answer_is_taken() {
    let site = site(None);
    std::fs::write(
        site.root.join("kpis.sh"),
        format!("sleep 1\nprintf '%s' '{BOTH}' > \"$EPHOR_ANSWER\"\n"),
    )
    .unwrap();
    let began = Utc::now();
    refresh_then(&site, &[from_kpis("a")], When::Due, began);
    let history = site.store.history("a").unwrap();
    let Some(Line::Reading { t, .. }) = history.last() else {
        panic!("{history:?}");
    };
    assert!(*t >= began + TimeDelta::seconds(1), "{t} for {began}");
    let record = &site.store.load().unwrap().monitors["a"];
    assert_eq!(record.skew, None);
    assert_eq!(record.last_attempt.as_ref().unwrap().at, *t);
}

/// §FS-021-monitors.9.1, §FS-021-monitors.17.3: a history that cannot be
/// written fails the slot and takes back what the refresh appended, so the
/// histories and the records still agree and the next read appends nothing
/// twice.
#[test]
fn a_history_that_cannot_be_written_leaves_the_store_as_it_was() {
    let site = site(None);
    script(&site.root, BOTH, 0);
    let declared = [from_kpis("a"), from_kpis("b")];
    refresh_at(&site, &declared, When::Due, 0);
    let before = site.store.load().unwrap();
    let a = std::fs::read(site.store.history_path("a")).unwrap();
    std::fs::remove_file(site.store.history_path("b")).unwrap();
    std::fs::create_dir(site.store.history_path("b")).unwrap();

    let report = refresh_at(&site, &declared, When::Due, 11);
    assert!(matches!(report.slot, Some(Err(_))), "{:?}", report.slot);
    assert_eq!(std::fs::read(site.store.history_path("a")).unwrap(), a);
    let after = site.store.load().unwrap();
    assert_eq!(
        after.monitors["a"].last_attempt,
        before.monitors["a"].last_attempt
    );

    std::fs::remove_dir(site.store.history_path("b")).unwrap();
    refresh_at(&site, &declared, When::Due, 12);
    assert_eq!(site.store.history("a").unwrap().len(), 2);
}

/// §FS-021-monitors.9.1, §FS-021-monitors.6.9: where `monitors.json` cannot
/// be written after the histories were appended to, the appends are taken
/// back too, so a reading the record never counted is never in a history,
/// and the read is taken again whole once its claim is void.
#[test]
fn a_store_that_cannot_be_written_takes_back_what_the_read_appended() {
    let site = site(None);
    let failing = r#"{"v":1,"monitors":[{"id":"a","value":0}]}"#;
    script(&site.root, failing, 0);
    let declared = [json!({"id": "a", "healthy": {"min": 1}, "drop_after": 2,
        "from": {"command": "sh kpis.sh"}})];
    refresh_at(&site, &declared, When::Due, 0);
    let lines = || site.store.history("a").unwrap().len();
    assert_eq!(lines(), 1);
    // The command blocks the write of `monitors.json` while it runs, after
    // the claims were written and before the readings are.
    let blocker = site.store.path().with_extension("json.tmp");
    std::fs::write(
        site.root.join("kpis.sh"),
        format!(
            "mkdir -p '{}'\nprintf '%s' '{failing}' > \"$EPHOR_ANSWER\"\n",
            blocker.display()
        ),
    )
    .unwrap();
    let report = refresh_at(&site, &declared, When::Due, 11);
    assert!(matches!(report.slot, Some(Err(_))), "{:?}", report.slot);
    assert_eq!(lines(), 1, "{:?}", site.store.history("a").unwrap());

    std::fs::remove_dir(&blocker).unwrap();
    script(&site.root, failing, 0);
    refresh_at(&site, &declared, When::Due, 13);
    assert_eq!(lines(), 2);
    let record = &site.store.load().unwrap().monitors["a"];
    assert_eq!(record.hold.run, 2);
    assert!(record.drop.is_some(), "{record:?}");
}

/// §FS-021-monitors.3.5, §FS-021-monitors.3.6: a declared `warn` whose
/// `healthy` its samples bring is read from the first refresh, and again
/// once the store is gone, rather than waiting on a sample it never reads.
#[test]
fn a_warn_its_samples_narrow_is_read_from_the_first_refresh() {
    let site = site(None);
    script(
        &site.root,
        r#"{"v":1,"monitors":[{"id":"a","value":4,"healthy":{"max":5}}]}"#,
        0,
    );
    let declared = [json!({"id": "a", "kind": "number", "warn": {"max": 3},
        "from": {"command": "sh kpis.sh"}})];
    let report = refresh_at(&site, &declared, When::Due, 0);
    assert_eq!(report.slot, Some(Ok(())), "{:?}", report.notes);
    assert_eq!(
        found(&site, "a"),
        Some(Found::Reading),
        "{:?}",
        report.notes
    );
    std::fs::remove_dir_all(site.store.path().parent().unwrap()).unwrap();
    refresh_at(&site, &declared, When::Due, 1);
    assert_eq!(found(&site, "a"), Some(Found::Reading));
}

/// §FS-021-monitors.1.1, §FS-021-monitors.8.2: a sample whose id is longer
/// than a monitor id may be is read for no monitor and noted, so no history
/// is named after it and the slot does not fail on a name the file system
/// refuses.
#[test]
fn a_sample_with_an_id_too_long_is_read_for_no_monitor() {
    let site = site(None);
    let long = "a".repeat(300);
    let brought = Outcome::Answered(vec![json!({"id": long, "value": 1})]);
    let report = refresh_bringing(&site, &[], vec![brought], When::Due, at(0));
    assert_eq!(report.slot, Some(Ok(())), "{:?}", report.notes);
    assert!(
        report
            .notes
            .iter()
            .any(|note| note.starts_with("a sample is not read: a sample's id")),
        "{:?}",
        report.notes
    );
    assert!(site.store.load().unwrap().monitors.is_empty());
}

/// §FS-021-monitors.5.6, §FS-021-monitors.14.4: a monitor nothing declares
/// that no answer brought for 30 days is forgotten: its open episodes end,
/// and what they were is kept for the finished matters to say, after its
/// record and its history go.
#[test]
fn a_forgotten_monitor_ends_its_episodes_and_keeps_what_they_were() {
    let site = site(None);
    let month = -31 * 24 * 60;
    let sample = json!({"id": "old", "value": 5, "healthy": {"min": 1}});
    let bring =
        |brought, minutes| refresh_bringing(&site, &[], vec![brought], When::Due, at(minutes)).slot;
    assert_eq!(bring(Outcome::Answered(vec![sample]), month), Some(Ok(())));
    assert_eq!(
        bring(Outcome::Failed("exited 1".to_string()), month + 180),
        Some(Ok(()))
    );
    assert!(site.store.load().unwrap().monitors["old"].unknown.is_some());

    assert_eq!(refresh_at(&site, &[], When::Due, 0).slot, Some(Ok(())));
    let document = site.store.load().unwrap();
    assert!(!document.monitors.contains_key("old"));
    assert!(!site.store.history_path("old").exists());
    let past = &document.forgotten["old"];
    assert_eq!(past.len(), 1, "{past:?}");
    assert_eq!(past[0].ended, at(0));
}
