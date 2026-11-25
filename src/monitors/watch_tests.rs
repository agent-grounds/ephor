use super::*;

use serde_json::json;

use crate::monitors::freshness::Found;
use crate::monitors::test_site::*;

/// §FS-021-monitors.5.1, §FS-021-monitors.6.4: a plain refresh reads the
/// due monitors and runs their shared command once, naming both in
/// `EPHOR_MONITORS`; a refresh before the next slot reads none.
#[test]
fn a_plain_refresh_runs_a_shared_from_once_and_only_when_due() {
    let site = site(None);
    script(&site.root, BOTH, 0);
    let declared = [from_kpis("a"), from_kpis("b")];
    let report = refresh_at(&site, &declared, When::Due, 0);
    assert_eq!(report.slot, Some(Ok(())));
    assert_eq!(runs(&site.root), vec!["a,b"]);
    assert_eq!(found(&site, "a"), Some(Found::Reading));
    assert_eq!(found(&site, "b"), Some(Found::Reading));
    assert_eq!(site.store.history("a").unwrap().len(), 1);

    // Not due again for ten minutes: the slot still answers, nothing runs.
    let report = refresh_at(&site, &declared, When::Due, 1);
    assert_eq!(report.slot, Some(Ok(())));
    assert_eq!(runs(&site.root).len(), 1);

    refresh_at(&site, &declared, When::Due, 11);
    assert_eq!(runs(&site.root).len(), 2);
    // No live claim is left behind (§FS-021-monitors.6.9).
    assert!(site.store.load().unwrap().monitors["a"].claim.is_none());
}

/// §FS-021-monitors.6.7, §FS-021-monitors.21.4: `--monitors-now` reads every
/// monitor out of cadence, and its line says so.
#[test]
fn reading_now_reads_out_of_cadence() {
    let site = site(None);
    script(&site.root, BOTH, 0);
    let declared = [from_kpis("a"), from_kpis("b")];
    refresh_at(&site, &declared, When::Due, 0);
    refresh_at(&site, &declared, When::Now, 1);
    assert_eq!(runs(&site.root).len(), 2);
    let history = site.store.history("a").unwrap();
    assert!(
        matches!(
            history.last(),
            Some(crate::monitors::history::Line::Reading {
                out_of_cadence: true,
                ..
            })
        ),
        "{history:?}"
    );
}

/// §FS-021-monitors.5.5, §FS-021-monitors.17.2: a command that fails, one
/// that times out, and an answer that does not read leave every monitor it
/// reads unreadable, and never fail the slot.
#[test]
fn a_failing_command_leaves_its_monitors_unreadable_and_the_slot_answered() {
    let site = site(None);
    script(&site.root, BOTH, 1);
    let declared = [from_kpis("a")];
    assert_eq!(
        refresh_at(&site, &declared, When::Due, 0).slot,
        Some(Ok(()))
    );
    assert!(unreadable(&site, "a").contains("exited 1"));

    script(&site.root, "not json", 0);
    assert_eq!(
        refresh_at(&site, &declared, When::Now, 1).slot,
        Some(Ok(()))
    );
    assert!(!unreadable(&site, "a").is_empty());

    std::fs::write(site.root.join("kpis.sh"), "sleep 5\n").unwrap();
    let slow = [json!({"id": "a", "healthy": {"min": 1}, "timeout": 1,
                       "from": {"command": "sh kpis.sh"}})];
    assert_eq!(refresh_at(&site, &slow, When::Now, 2).slot, Some(Ok(())));
    assert!(
        unreadable(&site, "a").contains("timed out"),
        "{}",
        unreadable(&site, "a")
    );
}

/// §FS-021-monitors.5.6: an answer that carries no sample for a monitor it
/// reads leaves that monitor unreadable, and its sibling read.
#[test]
fn a_missing_sample_is_that_monitors_alone() {
    let site = site(None);
    script(
        &site.root,
        r#"{"v":1,"monitors":[{"id":"a","value":81}]}"#,
        0,
    );
    refresh_at(&site, &[from_kpis("a"), from_kpis("b")], When::Due, 0);
    assert_eq!(found(&site, "a"), Some(Found::Reading));
    assert_eq!(unreadable(&site, "b"), "no sample for b");
}

/// §FS-021-monitors.5.5: exit 75 parks every monitor the run reads.
#[test]
fn a_parked_command_parks_its_monitors() {
    let site = site(None);
    script(&site.root, r#"{"v":1}"#, 75);
    refresh_at(&site, &[from_kpis("a")], When::Due, 0);
    assert_eq!(found(&site, "a"), Some(Found::Parked));
}

/// §FS-021-monitors.5.2, §FS-021-monitors.9.2: a file is read and never run;
/// one that is not there is unreadable, and one unchanged since the last read
/// is stale.
#[test]
fn a_file_is_read_and_an_unchanged_one_is_stale() {
    let site = site(None);
    let declared = [json!({"id": "f", "healthy": {"min": 1}, "from": {"file": "out/f.json"}})];
    refresh_at(&site, &declared, When::Due, 0);
    assert_eq!(unreadable(&site, "f"), "out/f.json is missing");

    std::fs::create_dir_all(site.root.join("out")).unwrap();
    std::fs::write(
        site.root.join("out/f.json"),
        r#"{"v":1,"monitors":[{"id":"f","value":4}]}"#,
    )
    .unwrap();
    refresh_at(&site, &declared, When::Now, 1);
    assert_eq!(found(&site, "f"), Some(Found::Reading));
    assert!(site.store.load().unwrap().monitors["f"].file.is_some());

    refresh_at(&site, &declared, When::Now, 2);
    assert!(
        matches!(found(&site, "f"), Some(Found::Stale { .. })),
        "{:?}",
        found(&site, "f")
    );
}

/// §FS-021-monitors.6.9: a monitor another read holds a live claim on is not
/// read, and its claim stands; a void claim is passed over.
#[test]
fn a_live_claim_elsewhere_is_respected_and_a_void_one_is_not() {
    let site = site(None);
    script(&site.root, BOTH, 0);
    let declared = [from_kpis("a")];
    let claim = |at: DateTime<Utc>| {
        site.store
            .update(|document| {
                let record = document
                    .monitors
                    .entry("a".to_string())
                    .or_insert_with(|| Record::new(at));
                record.claim = Some(Claim {
                    pid: 1,
                    at,
                    slot: None,
                    timeout: None,
                });
                Ok(())
            })
            .unwrap()
    };
    claim(at(0));
    assert_eq!(
        refresh_at(&site, &declared, When::Due, 0).slot,
        Some(Ok(()))
    );
    assert!(runs(&site.root).is_empty());
    assert_eq!(
        site.store.load().unwrap().monitors["a"]
            .claim
            .as_ref()
            .unwrap()
            .pid,
        1
    );

    claim(at(-120));
    refresh_at(&site, &declared, When::Due, 0);
    assert_eq!(runs(&site.root).len(), 1);
    assert!(site.store.load().unwrap().monitors["a"].claim.is_none());
}

/// §FS-021-monitors.6.9: two refreshes that overlap read a due monitor once.
#[test]
fn overlapping_refreshes_read_a_monitor_once() {
    let site = site(None);
    std::fs::write(
        site.root.join("kpis.sh"),
        format!("echo x >> runs.log\nsleep 1\nprintf '%s' '{BOTH}' > \"$EPHOR_ANSWER\"\n"),
    )
    .unwrap();
    let declared = [from_kpis("a")];
    let (site, declared) = (&site, &declared);
    std::thread::scope(|scope| {
        let first = scope.spawn(|| refresh_at(site, declared, When::Due, 0));
        let later = at(0) + TimeDelta::seconds(1);
        let second = scope.spawn(move || refresh_then(site, declared, When::Due, later));
        assert_eq!(first.join().unwrap().slot, Some(Ok(())));
        assert_eq!(second.join().unwrap().slot, Some(Ok(())));
    });
    assert_eq!(runs(&site.root).len(), 1);
    assert_eq!(site.store.history("a").unwrap().len(), 1);
}

/// §FS-021-monitors.9.3: retention thins at most once a day.
#[test]
fn histories_are_thinned_at_most_once_a_day() {
    let site = site(None);
    script(&site.root, BOTH, 0);
    let declared = [from_kpis("a")];
    refresh_at(&site, &declared, When::Due, 0);
    assert_eq!(site.store.load().unwrap().thinned, Some(at(0)));
    refresh_at(&site, &declared, When::Due, 11);
    assert_eq!(site.store.load().unwrap().thinned, Some(at(0)));
    refresh_at(&site, &declared, When::Due, 24 * 60 + 1);
    assert_eq!(site.store.load().unwrap().thinned, Some(at(24 * 60 + 1)));
}

/// §FS-021-monitors.5.6: a monitor nothing declares that no answer brought
/// for 30 days is forgotten, its history with it; one brought more recently
/// is kept.
#[test]
fn an_undeclared_monitor_is_forgotten_after_thirty_days() {
    let site = site(None);
    site.store
        .update(|document| {
            for (id, brought) in [("old", at(-31 * 24 * 60)), ("new", at(-60))] {
                let mut record = Record::new(at(-40 * 24 * 60));
                record.last_brought = Some(brought);
                document.monitors.insert(id.to_string(), record);
            }
            Ok(())
        })
        .unwrap();
    site.store
        .append(
            "old",
            &[crate::monitors::history::Line::marker(at(-40 * 24 * 60))],
        )
        .unwrap();
    assert_eq!(refresh_at(&site, &[], When::Due, 0).slot, Some(Ok(())));
    let document = site.store.load().unwrap();
    assert!(!document.monitors.contains_key("old"));
    assert!(document.monitors.contains_key("new"));
    assert!(!site.store.history_path("old").exists());
}

/// §FS-021-monitors.17.3: a manifest that cannot be read fails the slot only
/// where the project has monitors without it.
#[test]
fn an_unreadable_manifest_fails_the_slot_only_where_monitors_are_known() {
    let site = site(None);
    std::fs::write(site.root.join("ephor.json"), "{ not json").unwrap();
    let report = refresh_at(&site, &[], When::Due, 0);
    assert_eq!(report.slot, None);
    assert!(
        report.notes.iter().any(|note| note.contains("manifest")),
        "{:?}",
        report.notes
    );

    let report = refresh_at(&site, &[from_kpis("a")], When::Due, 0);
    assert!(matches!(report.slot, Some(Err(ref why)) if why.contains("manifest")));
}

/// §FS-021-monitors.4.5: a row that trusts only descriptions runs no command
/// its manifest names.
#[test]
fn a_descriptions_only_row_runs_no_manifest_command() {
    let site = site(Some("descriptions"));
    script(&site.root, BOTH, 0);
    std::fs::write(
        site.root.join("ephor.json"),
        serde_json::to_string(&json!({"monitors": [from_kpis("a")]})).unwrap(),
    )
    .unwrap();
    let report = refresh_at(&site, &[], When::Due, 0);
    assert_eq!(report.slot, Some(Ok(())));
    assert!(runs(&site.root).is_empty());
}

/// §FS-021-monitors.4.6: a declaration that breaks a rule is noted, and its
/// sibling is read as if it were not there.
#[test]
fn a_refused_declaration_is_noted_and_costs_that_monitor_alone() {
    let site = site(None);
    script(&site.root, BOTH, 0);
    let declared = [from_kpis("a"), from_kpis("b"), from_kpis("b")];
    let report = refresh_at(&site, &declared, When::Due, 0);
    assert_eq!(report.slot, Some(Ok(())));
    assert!(
        report.notes.iter().any(|note| note.contains("b")),
        "{:?}",
        report.notes
    );
    assert_eq!(runs(&site.root), vec!["a"]);
}

/// §FS-021-monitors.8.3: a custom-status answer's samples bring their
/// monitors, which the answer then owns.
#[test]
fn custom_status_samples_bring_their_monitors() {
    let site = site(None);
    let request = Request {
        project: "alpha",
        placement: Some(&site.placement),
        site: &[],
        brought: vec![Outcome::Answered(vec![json!({"id": "k", "value": 5})])],
        when: When::Due,
        recent_days: 7,
    };
    assert_eq!(refresh(&site.store, &request, at(0)).slot, Some(Ok(())));
    let document = site.store.load().unwrap();
    assert_eq!(document.monitors["k"].owner, Some(Owner::CustomStatus));
    assert_eq!(document.monitors["k"].last_brought, Some(at(0)));
}

/// §FS-021-monitors.1.4, §FS-021-monitors.17.1: a project with no monitors
/// has no slot, and its store is not made.
#[test]
fn a_project_with_no_monitors_has_no_slot() {
    let site = site(None);
    assert_eq!(refresh_at(&site, &[], When::Due, 0).slot, None);
    assert!(!site.store.path().exists());
}
