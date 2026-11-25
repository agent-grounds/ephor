use serde_json::json;

use super::*;
use crate::monitors::test_site::*;
use crate::monitors::watch::When;

fn overdue_at(
    site: &Site,
    declared: &[Json],
    slot_error: Option<&str>,
    minutes: i64,
) -> Vec<String> {
    overdue(
        &site.store,
        Some(&site.placement),
        declared,
        slot_error,
        at(minutes),
    )
}

/// §FS-021-monitors.7.5, §FS-021-monitors.20.2: a monitor counts as overdue
/// once it is unknown for that cause, never while what it read still holds
/// and the page says it late.
#[test]
fn overdue_is_unknown_for_that_cause_and_never_late() {
    let site = site(None);
    script(&site.root, BOTH, 0);
    let declared = [from_kpis("a")];
    refresh_at(&site, &declared, When::Due, 0);
    assert!(overdue_at(&site, &declared, None, 5).is_empty());
    // A `10m` monitor's value holds for 1h 10m: at 30 minutes it is late.
    assert!(overdue_at(&site, &declared, None, 30).is_empty());
    // `b`, which the answer brought and nothing declares, is the project's
    // monitor too.
    assert_eq!(
        overdue_at(&site, &declared, None, 60 * 24),
        vec!["a".to_string(), "b".to_string()]
    );
}

/// §FS-021-monitors.7.2: while the monitor slot fails, that failure is every
/// monitor's cause, and none of them is overdue.
#[test]
fn a_failed_slot_is_every_monitors_cause_and_none_is_overdue() {
    let site = site(None);
    script(&site.root, BOTH, 0);
    let declared = [from_kpis("a")];
    refresh_at(&site, &declared, When::Due, 0);
    assert!(overdue_at(&site, &declared, Some("the store cannot be read"), 60 * 24).is_empty());
}

/// §FS-021-monitors.6.8: a monitor read from the answer that brings it is
/// judged at that answer's cadence, a day here, so two hours after the read
/// it still holds; and one nothing declares is judged too.
#[test]
fn a_brought_monitor_is_judged_at_its_answers_cadence() {
    let site = site(None);
    script(&site.root, BOTH, 0);
    let daily = json!({"id": "a", "healthy": {"min": 1}, "every": "1d",
                       "from": {"command": "sh kpis.sh"}});
    let declared = [daily.clone(), json!({"id": "b", "healthy": {"min": 1}})];
    refresh_at(&site, &declared, When::Due, 0);
    let document = site.store.load().unwrap();
    assert_eq!(
        document.monitors["b"].owner,
        Some(Owner::From(Source::Command {
            command: "sh kpis.sh".to_string(),
            cwd: None
        }))
    );
    assert!(overdue_at(&site, &declared, None, 120).is_empty());

    // Undeclared, `b` is still the project's, at the same cadence.
    let only_a = [daily];
    let known = all(&declared::read(Some(&site.placement), &only_a), &document);
    assert_eq!(known["b"].cadence, known["a"].cadence);
    assert!(overdue_at(&site, &only_a, None, 120).is_empty());
    assert_eq!(
        overdue_at(&site, &only_a, None, 60 * 24 * 3),
        vec!["a".to_string(), "b".to_string()]
    );
}

/// §FS-021-monitors.6.8: a site declaration of a brought monitor that states
/// a cadence is judged at it, and a custom-status answer's monitor at the
/// refresh's own otherwise.
#[test]
fn a_site_stated_cadence_is_the_one_judged() {
    let monitor = |declaration: Json| {
        let declarations = declared::read(None, &[declaration]);
        declarations.resolve("k", None)
    };
    let stated = monitor(json!({"id": "k", "every": "1d"}));
    let unstated = monitor(json!({"id": "k"}));
    assert!(stated.is_readable(), "{:?}", stated.faults);
    let custom = Some(&Owner::CustomStatus);
    assert_eq!(cadence(&stated, custom, &[]), stated.cadence);
    assert_eq!(cadence(&unstated, custom, &[]), Cadence::default_every());
    assert_ne!(stated.cadence, Cadence::default_every());
}
