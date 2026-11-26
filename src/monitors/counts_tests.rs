use serde_json::json;

use super::*;
use crate::monitors::test_site::*;
use crate::monitors::watch::When;

/// §FS-021-monitors.20.1: statuses only, a down one under `down` whatever its
/// freshness (§FS-021-monitors.18.3), and overdue counted a second time
/// unless the slot's own failure is the cause.
#[test]
fn statuses_are_counted_down_unknown_and_overdue() {
    let site = site(None);
    script(
        &site.root,
        r#"{"v":1,"monitors":[{"id":"a","value":79},{"id":"k","value":5}]}"#,
        0,
    );
    let declared = [
        json!({"id": "a", "healthy": {"min": 80}, "from": {"command": "sh kpis.sh"}}),
        json!({"id": "k", "from": {"command": "sh kpis.sh"}}),
    ];
    refresh_at(&site, &declared, When::Due, 0);
    let count = |slot_error, minutes| {
        of(
            &site.store,
            Some(&site.placement),
            &declared,
            slot_error,
            at(minutes),
        )
    };
    assert_eq!(
        count(None, 1),
        Some(Counts {
            down: 1,
            unknown: 0,
            overdue: 0
        })
    );
    let days = 60 * 24 * 3;
    assert_eq!(
        count(None, days),
        Some(Counts {
            down: 1,
            unknown: 0,
            overdue: 1
        })
    );
    assert_eq!(count(Some("the store is locked"), days).unwrap().overdue, 0);
    assert_eq!(
        count(None, days).unwrap().say(),
        "monitors: 1 down · 0 unknown · 1 overdue"
    );
    assert_eq!(Counts::default().say(), "monitors: 0 down · 0 unknown");
}

/// A project whose monitors are all KPIs has no status to count.
#[test]
fn kpis_alone_count_nothing() {
    let site = site(None);
    script(
        &site.root,
        r#"{"v":1,"monitors":[{"id":"k","value":5}]}"#,
        0,
    );
    let declared = [json!({"id": "k", "from": {"command": "sh kpis.sh"}})];
    refresh_at(&site, &declared, When::Due, 0);
    assert_eq!(
        of(&site.store, Some(&site.placement), &declared, None, at(1)),
        None
    );
}

/// §FS-021-monitors.18.3: a status down and unknown at once counts down, and
/// one only unknown counts unknown.
#[test]
fn a_status_down_while_it_is_also_unknown_counts_down() {
    let site = site(None);
    script(
        &site.root,
        r#"{"v":1,"monitors":[{"id":"a","value":79},{"id":"b","value":81}]}"#,
        0,
    );
    let declared = [
        json!({"id": "a", "healthy": {"min": 80}, "from": {"command": "sh kpis.sh"}}),
        json!({"id": "b", "healthy": {"min": 80}, "from": {"command": "sh kpis.sh"}}),
    ];
    refresh_at(&site, &declared, When::Due, 0);
    script(&site.root, "not json", 0);
    let days = 60 * 24 * 3;
    refresh_at(&site, &declared, When::Due, days);
    let document = site.store.load().unwrap();
    assert!(document.monitors["a"].drop.is_some());
    assert!(document.monitors["a"].unknown.is_some());
    assert!(document.monitors["b"].unknown.is_some());
    assert_eq!(
        of(
            &site.store,
            Some(&site.placement),
            &declared,
            None,
            at(days + 1)
        ),
        Some(Counts {
            down: 1,
            unknown: 1,
            overdue: 0
        })
    );
}

/// §FS-021-monitors.21.2, §FS-021-monitors.4.6: a status whose declaration
/// is in error counts as unknown, read or never read; a KPI in error counts
/// nowhere.
#[test]
fn a_status_whose_declaration_is_in_error_counts_unknown() {
    let site = site(None);
    script(
        &site.root,
        r#"{"v":1,"monitors":[{"id":"a","value":3},{"id":"k","value":5}]}"#,
        0,
    );
    let good = json!({"id": "a", "healthy": {"max": 5}, "from": {"command": "sh kpis.sh"}});
    refresh_at(&site, &[good], When::Due, 0);
    let declared = [
        json!({"id": "a", "healthy": {"max": 5}, "warn": {"max": 7},
               "from": {"command": "sh kpis.sh"}}),
        json!({"id": "never", "healthy": {"max": 5}, "warn": {"max": 7},
               "from": {"command": "sh kpis.sh"}}),
        json!({"id": "k", "every": "1d", "at": "06:00:30", "from": {"command": "sh kpis.sh"}}),
    ];
    let report = refresh_at(&site, &declared, When::Due, 1);
    assert!(
        report
            .notes
            .iter()
            .any(|note| note.contains("a is not read")),
        "{:?}",
        report.notes
    );
    assert_eq!(
        of(&site.store, Some(&site.placement), &declared, None, at(2)),
        Some(Counts {
            down: 0,
            unknown: 2,
            overdue: 0
        })
    );
}

/// §FS-021-monitors.4.6, §FS-021-monitors.21.2: a manifest declaration
/// refused for an id that is not one is shown by its place and counts as an
/// unknown status, and nothing is kept of it.
#[test]
fn a_declaration_with_no_id_to_name_it_counts_unknown() {
    let site = site(None);
    let manifest = json!({"monitors": [
        {"id": "Coverage", "healthy": {"min": 80}, "from": {"command": "sh kpis.sh"}}
    ]});
    std::fs::write(site.root.join("ephor.json"), manifest.to_string()).unwrap();
    let report = refresh_at(&site, &[], When::Due, 0);
    assert_eq!(report.slot, Some(Ok(())), "{:?}", report.notes);
    assert!(
        report
            .notes
            .iter()
            .any(|note| note.starts_with("manifest monitor monitors[0] is not read: ")),
        "{:?}",
        report.notes
    );
    assert_eq!(
        of(&site.store, Some(&site.placement), &[], None, at(1)),
        Some(Counts {
            unknown: 1,
            ..Counts::default()
        })
    );
    assert!(site.store.load().unwrap().monitors.is_empty());
}

/// §FS-021-monitors.18.2: a status not run because its row trusts only
/// descriptions counts in none of them.
#[test]
fn a_status_not_run_counts_nowhere() {
    let site = site(Some("descriptions"));
    std::fs::write(
        site.root.join("ephor.json"),
        serde_json::to_string(&json!({"monitors": [
            {"id": "a", "healthy": {"min": 80}, "from": {"command": "sh kpis.sh"}}
        ]}))
        .unwrap(),
    )
    .unwrap();
    refresh_at(&site, &[], When::Due, 0);
    let days = 60 * 24 * 3;
    assert_eq!(
        of(&site.store, Some(&site.placement), &[], None, at(days)),
        None
    );
}

/// §FS-021-monitors.21.2, §FS-021-monitors.20.1: a status whose manifest
/// declaration the shape refuses is still a status, so Waiting's overview
/// line counts it unknown; a KPI refused the same way counts nowhere.
#[test]
fn a_status_the_shape_refuses_counts_unknown_and_a_kpi_nowhere() {
    let site = site(None);
    std::fs::write(
        site.root.join("ephor.json"),
        serde_json::to_string(&json!({"monitors": [
            {"id": "a", "healthy": {"min": 80}, "every": "90s",
             "from": {"command": "sh kpis.sh"}},
            {"id": "k", "kind": "number", "every": "90s", "from": {"command": "sh kpis.sh"}}
        ]}))
        .unwrap(),
    )
    .unwrap();
    refresh_at(&site, &[], When::Due, 0);
    assert_eq!(
        of(&site.store, Some(&site.placement), &[], None, at(1)),
        Some(Counts {
            down: 0,
            unknown: 1,
            overdue: 0
        })
    );
}
