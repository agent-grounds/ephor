use serde_json::json;

use super::*;
use crate::monitors::test_site::*;
use crate::monitors::watch::When;

/// §FS-021-monitors.20.1: statuses only, a down one under `down`, one whose
/// value no longer holds under `unknown`, and overdue counted a second time
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
            down: 0,
            unknown: 1,
            overdue: 1
        })
    );
    assert_eq!(count(Some("the store is locked"), days).unwrap().overdue, 0);
    assert_eq!(
        count(None, days).unwrap().say(),
        "monitors: 0 down · 1 unknown · 1 overdue"
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
