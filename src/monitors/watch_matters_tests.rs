//! What the slot reports: its monitors' episodes as matters, and the first
//! refresh after the work laid on one ended.

use serde_json::json;

use super::*;
use crate::monitors::matters::{stated, still_processed, Moves};
use crate::monitors::model::Proposal;
use crate::monitors::test_site::*;

fn coverage() -> Json {
    json!({
        "id": "a", "title": "Line coverage", "unit": "%",
        "healthy": {"min": 80}, "from": {"command": "sh kpis.sh"},
        "workflow": "fix-coverage"
    })
}

fn answer(value: f64) -> String {
    format!(r#"{{"v":1,"monitors":[{{"id":"a","value":{value},"url":"https://example.com/7"}}]}}"#)
}

/// §FS-021-monitors.12, §FS-021-monitors.16.1: a status that drops is
/// reported as a matter of the slot, proposing its monitor's workflow.
#[test]
fn a_drop_is_reported_with_its_proposal() {
    let site = site(None);
    script(&site.root, &answer(79.2), 0);
    let report = refresh_at(&site, &[coverage()], When::Due, 0);
    assert_eq!(report.slot, Some(Ok(())), "{:?}", report.notes);
    let [item] = report.items.as_slice() else {
        panic!("{:?}", report.items);
    };
    assert!(item.id.starts_with("monitor:alpha/a@"), "{}", item.id);
    assert_eq!(item.title, "Line coverage dropped: 79.2% (healthy ≥ 80%)");
    assert_eq!(item.url.as_deref(), Some("https://example.com/7"));
    let stated = stated(&item.source, &item.raw).unwrap();
    assert_eq!(
        stated.proposes,
        Some(Proposal::Workflow("fix-coverage".into()))
    );
}

/// §FS-021-monitors.15: the first refresh after the work on a drop ended
/// reads it out of cadence; still down, the matter moves and is renewed,
/// once per work.
#[test]
fn still_down_after_its_work_ended_moves_the_matter_once() {
    let site = site(None);
    script(&site.root, &answer(79.2), 0);
    let first = refresh_at(&site, &[coverage()], When::Due, 0);
    let key = first.items[0].id.clone();
    let was = stated("monitor", &first.items[0].raw).unwrap().moves;

    script(&site.root, &answer(79.4), 0);
    let ended = BTreeMap::from([(key.clone(), "t1@x".to_string())]);
    let report = refresh_after(&site, &[coverage()], ended.clone(), at(1));
    assert_eq!(runs(&site.root).len(), 2, "read out of cadence");
    let item = &report.items[0];
    assert_eq!(item.id, key, "the same episode");
    assert!(item.updated_at >= at(1), "{}", item.updated_at);
    let now = stated("monitor", &item.raw).unwrap();
    assert_eq!(now.moves.renewed, Some(item.updated_at));
    assert_eq!(
        now.moves.since(&was),
        vec!["still down after its work ended"]
    );

    // Seen once: the same ended work neither reads nor moves it again.
    let again = refresh_after(&site, &[coverage()], ended, at(2));
    assert_eq!(runs(&site.root).len(), 2);
    assert_eq!(again.items[0].updated_at, item.updated_at);
}

/// §FS-021-monitors.15: a passing reading after the work ended counts toward
/// recovery and moves nothing, so the work is not stale and the matter not
/// New again; the work is seen, so it is not read again for it.
#[test]
fn a_passing_reading_after_its_work_ended_moves_nothing() {
    let site = site(None);
    script(&site.root, &answer(79.2), 0);
    let first = refresh_at(&site, &[coverage()], When::Due, 0);
    let (key, was) = (&first.items[0].id, &first.items[0]);

    script(&site.root, &answer(81.0), 0);
    let ended = BTreeMap::from([(key.clone(), "t1@x".to_string())]);
    let report = refresh_after(&site, &[coverage()], ended.clone(), at(1));
    assert_eq!(runs(&site.root).len(), 2, "read out of cadence");
    let item = &report.items[0];
    assert_eq!(&item.id, key, "1 of 2 recovery reads: still open");
    assert_eq!(item.state.as_deref(), Some("down"));
    assert_eq!(item.updated_at, was.updated_at);
    let (now, before) = (
        stated("monitor", &item.raw).unwrap().moves,
        stated("monitor", &was.raw).unwrap().moves,
    );
    assert_eq!(now.since(&before), Vec::<String>::new());
    assert_eq!(now, before);
    assert!(still_processed("monitor", &item.raw, [at(0)]));

    let again = refresh_after(&site, &[coverage()], ended, at(2));
    assert_eq!(runs(&site.root).len(), 2, "the work is seen");
    assert_eq!(again.items[0].updated_at, was.updated_at);
}

/// §FS-021-monitors.13.1: a value wobbling inside its band moves nothing
/// the feed compares.
#[test]
fn a_wobble_inside_the_band_does_not_move_the_matter() {
    let site = site(None);
    script(&site.root, &answer(79.2), 0);
    let first = refresh_at(&site, &[coverage()], When::Due, 0);
    script(&site.root, &answer(79.3), 0);
    let later = refresh_at(&site, &[coverage()], When::Now, 30);
    let (was, now) = (&first.items[0], &later.items[0]);
    assert_eq!(was.updated_at, now.updated_at);
    assert_eq!(was.state, now.state);
    let moves = |item: &crate::feed::model::Item| stated("monitor", &item.raw).unwrap().moves;
    assert_eq!(moves(now).since(&moves(was)), Vec::<String>::new());
    assert_eq!(moves(now), Moves { ..moves(was) });
}
